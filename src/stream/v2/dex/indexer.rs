use super::events::{decode_nadfun_swap_event, nadfun_swap_signature, NadFunSwapEvent};
use crate::constants::Network;
use alloy::{primitives::Address, providers::Provider, rpc::types::Filter};
use anyhow::Result;
use std::sync::Arc;

/// Historical NadFunPair swap indexer.
///
/// Bound to a `Network` for downstream callers that need the context.
/// Pairs with [`crate::stream::v2::dex::NadFunSwapStream`] for backfilling
/// pool history before tailing real-time events.
///
/// # Empty `pairs` = no address filter
///
/// An empty `pairs` list is **not** rejected: like v1 `DexIndexer`, it drops
/// the address filter, so every log matching the `Swap` topic in the range is
/// returned. `NadFunPair::Swap` is the standard Uniswap-V2 signature, so the
/// result can include swaps from **unrelated, non-NadFun** V2-fork contracts;
/// `pair_address` is whatever emitted the log, not a verified NadFun pair. Over
/// a wide block range this can also hit provider result-size caps or large
/// payloads. Bounding the range and result size is the **caller's**
/// responsibility. Pass explicit `pairs` to scope and trust the results.
pub struct NadFunSwapIndexer<P> {
    provider: Arc<P>,
    pairs: Vec<Address>,
    network: Network,
}

impl<P: Provider + Clone> NadFunSwapIndexer<P> {
    pub fn new(provider: Arc<P>, pairs: Vec<Address>, network: Network) -> Self {
        Self {
            provider,
            pairs,
            network,
        }
    }

    /// Network this indexer is bound to.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Fetch swap events for `from_block..=to_block`. Returned in
    /// chronological order (block / log-index).
    pub async fn fetch_events(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<NadFunSwapEvent>> {
        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(self.pairs.clone())
            .event_signature(nadfun_swap_signature());

        let logs = self.provider.get_logs(&filter).await?;
        let mut events: Vec<NadFunSwapEvent> = logs
            .into_iter()
            .filter_map(|log| decode_nadfun_swap_event(log).ok())
            .collect();
        // Sort by the full (block, tx_index, log_index) triple to match v1
        // DexIndexer — log_index alone can tie when metadata is missing.
        events.sort_by(|a, b| {
            (a.block_number, a.transaction_index, a.log_index).cmp(&(
                b.block_number,
                b.transaction_index,
                b.log_index,
            ))
        });
        Ok(events)
    }

    /// Fetch all swap events from `start_block` to chain head in `batch_size`
    /// chunks.
    pub async fn fetch_all_events(
        &self,
        start_block: u64,
        batch_size: u64,
    ) -> Result<Vec<NadFunSwapEvent>> {
        let mut all_events = Vec::new();
        let mut current_block = start_block;
        let target_block = self.provider.get_block_number().await?;

        while current_block <= target_block {
            let to_block = std::cmp::min(current_block + batch_size, target_block);
            let events = self.fetch_events(current_block, to_block).await?;
            all_events.extend(events);
            if to_block >= target_block {
                break;
            }
            current_block = to_block + 1;
        }
        Ok(all_events)
    }

    pub fn pairs(&self) -> &[Address] {
        &self.pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, Bytes, LogData, B256, U256, U64};
    use alloy::providers::ProviderBuilder;
    use alloy::sol_types::SolValue;
    use alloy::transports::mock::Asserter;

    /// Pad a 20-byte address into a 32-byte indexed-topic value.
    fn addr_topic(addr: Address) -> B256 {
        let mut t = [0u8; 32];
        t[12..].copy_from_slice(addr.as_slice());
        B256::from(t)
    }

    /// Build a synthetic NadFunPair `Swap` RPC log at `pair`/`block`/`log_index`.
    fn swap_log(pair: Address, block: u64, tx_index: u64, log_index: u64) -> alloy::rpc::types::Log {
        // Non-indexed data: amount0In, amount1In, amount0Out, amount1Out.
        let data: Bytes = (
            U256::from(1u64),
            U256::from(0u64),
            U256::from(0u64),
            U256::from(2u64),
        )
            .abi_encode_params()
            .into();
        // Indexed topics: [sig, sender, to].
        let topics = vec![
            nadfun_swap_signature(),
            addr_topic(Address::ZERO),
            addr_topic(Address::ZERO),
        ];
        let inner = alloy::primitives::Log {
            address: pair,
            data: LogData::new_unchecked(topics, data),
        };
        alloy::rpc::types::Log {
            inner,
            block_hash: None,
            block_number: Some(block),
            block_timestamp: None,
            transaction_hash: Some(B256::ZERO),
            transaction_index: Some(tx_index),
            log_index: Some(log_index),
            removed: false,
        }
    }

    /// Empty `pairs` means "no address filter" — matching v1 `DexIndexer`, the
    /// indexer must fetch EVERY NadFunPair `Swap` in the range rather than
    /// short-circuit. Owner decision (2026-05-30): v1/v2 empty-filter behavior
    /// is unified — empty input = receive all swaps. The mock has two swaps
    /// queued from different pairs; both must come back.
    #[tokio::test]
    async fn fetch_events_empty_pairs_fetches_all_swaps() {
        let asserter = Asserter::new();
        let logs = vec![
            swap_log(address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), 10, 0, 0),
            swap_log(address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"), 11, 1, 1),
        ];
        asserter.push_success(&logs);
        let provider = Arc::new(ProviderBuilder::new().connect_mocked_client(asserter));
        let indexer = NadFunSwapIndexer::new(provider, Vec::new(), Network::Testnet);

        let events = indexer
            .fetch_events(0, 100)
            .await
            .expect("empty pairs must query the chain, not short-circuit");
        assert_eq!(events.len(), 2, "empty pairs must return ALL swaps in range");
    }

    #[tokio::test]
    async fn fetch_all_events_empty_pairs_fetches_all_swaps() {
        let asserter = Asserter::new();
        asserter.push_success(&U64::from(100u64)); // eth_blockNumber
        asserter.push_success(&vec![swap_log(
            address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            10,
            0,
            0,
        )]); // eth_getLogs(0..=100)
        let provider = Arc::new(ProviderBuilder::new().connect_mocked_client(asserter));
        let indexer = NadFunSwapIndexer::new(provider, Vec::new(), Network::Testnet);

        let events = indexer
            .fetch_all_events(0, 1000)
            .await
            .expect("empty pairs must walk the range, not short-circuit");
        assert_eq!(events.len(), 1, "empty pairs must return all swaps");
    }

    /// Same block + same log_index but different transaction_index must sort by
    /// transaction_index (v1 `DexIndexer` parity; Codex P3). A stable sort on
    /// (block, log_index) alone would leave the input order, so tx 1 (queued
    /// first) would wrongly precede tx 0.
    #[tokio::test]
    async fn fetch_events_sorts_by_block_txindex_logindex() {
        let asserter = Asserter::new();
        // Queue tx 1 BEFORE tx 0, both at the same (block, log_index).
        asserter.push_success(&vec![
            swap_log(address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), 10, 1, 0),
            swap_log(address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"), 10, 0, 0),
        ]);
        let provider = Arc::new(ProviderBuilder::new().connect_mocked_client(asserter));
        let indexer = NadFunSwapIndexer::new(provider, Vec::new(), Network::Testnet);

        let events = indexer.fetch_events(0, 100).await.expect("fetch");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].transaction_index, 0, "tx 0 must sort first");
        assert_eq!(events[1].transaction_index, 1, "tx 1 must sort second");
    }
}
