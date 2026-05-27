use crate::constants::get_bonding_curve_v2;
use crate::types::{decode_v2_bonding_curve_event, V2BondingCurveEvent, V2EventType};
use alloy::{
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::Filter,
};
use anyhow::Result;
use std::{collections::HashSet, sync::Arc};

/// Historical event indexer for the v2 BondingCurve contract.
///
/// Pairs with [`crate::stream::v2::curve::CurveStreamV2`] for backfilling
/// state before tailing real-time events.
pub struct CurveIndexerV2<P> {
    provider: Arc<P>,
}

impl<P: Provider + Clone> CurveIndexerV2<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }

    fn bonding_curve_address(&self) -> Result<Address> {
        Ok(get_bonding_curve_v2()
            .ok_or_else(|| {
                anyhow::anyhow!("BondingCurveV2 is not configured for the current network")
            })?
            .parse()?)
    }

    /// Fetch events for a specific block range. Events are returned in
    /// chronological order (block / tx-index / log-index).
    pub async fn fetch_events(
        &self,
        from_block: u64,
        to_block: u64,
        event_types: Vec<V2EventType>,
        token_filter: Option<Vec<Address>>,
    ) -> Result<Vec<V2BondingCurveEvent>> {
        let signatures: Vec<B256> = event_types.iter().map(|et| et.signature()).collect();

        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(self.bonding_curve_address()?)
            .event_signature(signatures);

        let logs = self.provider.get_logs(&filter).await?;
        let token_set = token_filter.map(|t| t.into_iter().collect::<HashSet<_>>());

        let mut events: Vec<V2BondingCurveEvent> = logs
            .into_iter()
            .filter_map(|log| {
                decode_v2_bonding_curve_event(log).ok().and_then(|event| {
                    if let Some(ref allowed_tokens) = token_set {
                        if !allowed_tokens.contains(&event.token()) {
                            return None;
                        }
                    }
                    Some(event)
                })
            })
            .collect();

        events.sort_by(|a, b| {
            (a.block_number(), a.log_index()).cmp(&(b.block_number(), b.log_index()))
        });

        Ok(events)
    }

    /// Fetch all events from `start_block` to the current chain head, in
    /// `batch_size`-block chunks (some RPC endpoints cap the range per call).
    pub async fn fetch_all_events(
        &self,
        start_block: u64,
        batch_size: u64,
        event_types: Vec<V2EventType>,
        token_filter: Option<Vec<Address>>,
    ) -> Result<Vec<V2BondingCurveEvent>> {
        let mut all_events = Vec::new();
        let mut current_block = start_block;
        let target_block = self.provider.get_block_number().await?;

        while current_block <= target_block {
            let to_block = std::cmp::min(current_block + batch_size, target_block);
            let events = self
                .fetch_events(
                    current_block,
                    to_block,
                    event_types.clone(),
                    token_filter.clone(),
                )
                .await?;
            all_events.extend(events);
            if to_block >= target_block {
                break;
            }
            current_block = to_block + 1;
        }

        Ok(all_events)
    }
}
