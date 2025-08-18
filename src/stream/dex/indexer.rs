//! Uniswap V3 Swap event indexing support
//!
//! This module provides historical indexing for Uniswap V3 Swap events.
//! All types are defined in the types::uniswap module.

use crate::types::{SwapEvent, UniswapV3Pool, decode_swap_event};
use alloy::{
    primitives::Address,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::{BlockNumberOrTag, Filter},
    sol_types::SolEvent,
};
use anyhow::Result;
use std::sync::Arc;

/// Historical indexer for Uniswap V3 Swap events
/// Efficiently processes past swap events for analysis
pub struct UniswapSwapIndexer {
    provider: Arc<DynProvider>,
    pool_addresses: Vec<Address>,
}

impl UniswapSwapIndexer {
    /// Create a new Uniswap swap indexer for specific pool addresses using HTTP provider
    pub fn new(rpc_url: String, pool_addresses: Vec<Address>) -> Result<Self> {
        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let dyn_provider = Arc::new(DynProvider::new(provider));
        
        Ok(Self {
            provider: dyn_provider,
            pool_addresses,
        })
    }

    /// Create indexer by discovering pools for token addresses
    /// Uses NADS standard 10_000 fee tier (1%)
    pub async fn discover_pools_for_tokens(
        rpc_url: String,
        token_addresses: Vec<Address>,
    ) -> Result<Self> {
        use crate::contracts::get_pool_addresses_for_tokens;

        let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
        let dyn_provider = Arc::new(DynProvider::new(provider));

        let token_count = token_addresses.len();
        let pool_addresses =
            get_pool_addresses_for_tokens(dyn_provider.clone(), token_addresses).await?;

        println!(
            "🔍 Discovered {} pools for {} tokens",
            pool_addresses.len(),
            token_count
        );

        Ok(Self {
            provider: dyn_provider,
            pool_addresses,
        })
    }

    /// Create indexer by discovering pool for a single token
    pub async fn discover_pool_for_token(rpc_url: String, token_address: Address) -> Result<Self> {
        Self::discover_pools_for_tokens(rpc_url, vec![token_address]).await
    }

    /// Fetch swap events for a specific block range
    /// Returns events sorted chronologically
    pub async fn fetch_events(&self, from_block: u64, to_block: u64) -> Result<Vec<SwapEvent>> {
        let swap_signature = UniswapV3Pool::Swap::SIGNATURE_HASH;

        let filter = Filter::new()
            .from_block(BlockNumberOrTag::Number(from_block))
            .to_block(BlockNumberOrTag::Number(to_block))
            .address(self.pool_addresses.clone())
            .event_signature(swap_signature);

        let logs = self.provider.get_logs(&filter).await?;

        let mut events: Vec<SwapEvent> = logs
            .into_iter()
            .filter_map(|log| decode_swap_event(log).ok())
            .collect();

        // Sort events chronologically
        events.sort_by(|a, b| {
            a.block_number
                .cmp(&b.block_number)
                .then_with(|| a.transaction_index.cmp(&b.transaction_index))
                .then_with(|| a.log_index.cmp(&b.log_index))
        });

        Ok(events)
    }

    /// Fetch all historical events from start_block to current block
    /// This will automatically handle batching
    pub async fn fetch_all_events(
        &self,
        start_block: u64,
        batch_size: u64,
    ) -> Result<Vec<SwapEvent>> {
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

    /// Get all pool addresses being monitored
    pub fn pool_addresses(&self) -> &[Address] {
        &self.pool_addresses
    }
}
