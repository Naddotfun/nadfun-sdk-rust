use crate::constants::BONDING_CURVE;
use crate::types::{BondingCurveEvent, EventType, decode_bonding_curve_event};
use alloy::{
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::Filter,
};
use anyhow::Result;
use std::{collections::HashSet, sync::Arc};

/// Event indexer for fetching historical events in batches
pub struct CurveIndexer<P> {
    provider: Arc<P>,
}

impl<P: Provider + Clone> CurveIndexer<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }

    fn bonding_curve_address(&self) -> Address {
        BONDING_CURVE
            .parse()
            .expect("Invalid bonding curve address")
    }

    /// Fetch events for a specific block range
    /// Returns events sorted chronologically
    pub async fn fetch_events(
        &self,
        from_block: u64,
        to_block: u64,
        event_types: Vec<EventType>,
        token_filter: Option<Vec<Address>>,
    ) -> Result<Vec<BondingCurveEvent>> {
        let signatures: Vec<B256> = event_types.iter().map(|et| et.signature()).collect();

        let filter = Filter::new()
            .from_block(from_block)
            .to_block(to_block)
            .address(self.bonding_curve_address())
            .event_signature(signatures);

        let logs = self.provider.get_logs(&filter).await?;

        // Process logs
        self.process_logs_with_method(logs, token_filter).await
    }

    /// Common log processing method
    async fn process_logs_with_method(
        &self,
        logs: Vec<alloy::rpc::types::Log>,
        token_filter: Option<Vec<Address>>,
    ) -> Result<Vec<BondingCurveEvent>> {
        let token_set = token_filter.map(|tokens| tokens.into_iter().collect::<HashSet<_>>());

        // Log processing - 벤치마크 결과 Sequential이 4-10배 빠름
        let events: Vec<BondingCurveEvent> = logs
            .into_iter()
            .filter_map(|log| {
                decode_bonding_curve_event(log).ok().and_then(|event| {
                    if let Some(ref allowed_tokens) = token_set {
                        if !allowed_tokens.contains(&event.token()) {
                            return None;
                        }
                    }
                    Some(event)
                })
            })
            .collect();

        // 이벤트 정렬 로직 개선: block_number -> transaction_index -> log_index 순서로 정렬
        // 벤치마크 결과: 복잡성 대비 병렬 처리 이득이 미미하여 순차 정렬로 통일
        let mut events = events;
        events.sort_by(|a, b| {
            (a.block_number(), a.transaction_index(), a.log_index()).cmp(&(
                b.block_number(),
                b.transaction_index(),
                b.log_index(),
            ))
        });

        Ok(events)
    }

    /// Fetch all historical events from start_block to current block
    /// This will automatically handle batching
    pub async fn fetch_all_events(
        &self,
        start_block: u64,
        batch_size: u64,
        event_types: Vec<EventType>,
        token_filter: Option<Vec<Address>>,
    ) -> Result<Vec<BondingCurveEvent>> {
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
