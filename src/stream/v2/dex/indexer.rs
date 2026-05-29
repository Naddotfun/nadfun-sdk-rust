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
        // An empty address filter is a no-op on many RPC providers, so it
        // would scan every `Swap` log in the range and return unrelated
        // pools' events. No pairs means nothing to index — short-circuit.
        if self.pairs.is_empty() {
            return Ok(Vec::new());
        }

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
        events.sort_by(|a, b| (a.block_number, a.log_index).cmp(&(b.block_number, b.log_index)));
        Ok(events)
    }

    /// Fetch all swap events from `start_block` to chain head in `batch_size`
    /// chunks.
    pub async fn fetch_all_events(
        &self,
        start_block: u64,
        batch_size: u64,
    ) -> Result<Vec<NadFunSwapEvent>> {
        // Nothing to index, and avoid a needless `get_block_number` round-trip.
        if self.pairs.is_empty() {
            return Ok(Vec::new());
        }

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
    use alloy::providers::ProviderBuilder;

    /// An empty `pairs` list must NOT produce a broad on-chain query.
    ///
    /// `Filter::address(vec![])` is a no-op address filter on many RPC
    /// providers, so `get_logs` would scan every `Swap` log in the range and
    /// return unrelated pools' swaps. The indexer must short-circuit to an
    /// empty result before touching the network. The provider here points at
    /// an unreachable port: if the guard regressed, `get_logs` would attempt
    /// the connection and the call would fail instead of returning `Ok([])`.
    #[tokio::test]
    async fn fetch_events_empty_pairs_returns_empty_without_querying() {
        let provider = Arc::new(
            ProviderBuilder::new().connect_http("http://127.0.0.1:1".parse().expect("valid url")),
        );
        let indexer = NadFunSwapIndexer::new(provider, Vec::new(), Network::Testnet);

        let events = indexer
            .fetch_events(0, 100)
            .await
            .expect("empty pairs must short-circuit, not query the chain");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn fetch_all_events_empty_pairs_returns_empty_without_querying() {
        let provider = Arc::new(
            ProviderBuilder::new().connect_http("http://127.0.0.1:1".parse().expect("valid url")),
        );
        let indexer = NadFunSwapIndexer::new(provider, Vec::new(), Network::Testnet);

        let events = indexer
            .fetch_all_events(0, 1000)
            .await
            .expect("empty pairs must short-circuit, not query the chain");
        assert!(events.is_empty());
    }
}
