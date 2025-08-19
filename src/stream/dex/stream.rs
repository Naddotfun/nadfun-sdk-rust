//! Uniswap V3 Swap event streaming support
//!
//! This module provides real-time streaming for Uniswap V3 Swap events.
//! All types are defined in the types::uniswap module.

use crate::types::SwapEvent;
use alloy::{
    primitives::Address,
    providers::{DynProvider, Provider, ProviderBuilder, WsConnect},
    sol_types::SolEvent,
};
use anyhow::Result;
use futures_util::Stream;
use std::{pin::Pin, sync::Arc};

/// Specialized stream for Uniswap V3 Swap events across multiple pools
/// Provides raw swap data - users handle their own filtering logic
pub struct UniswapSwapStream {
    #[allow(dead_code)] // Will be used when real streaming is implemented
    provider: Arc<DynProvider>,
    #[allow(dead_code)] // Will be used when real streaming is implemented
    pool_addresses: Vec<Address>,
}

impl UniswapSwapStream {
    /// Create a WebSocket-based Uniswap swap stream with pool addresses
    pub async fn new(rpc_url: String, pool_addresses: Vec<Address>) -> Result<UniswapSwapStream> {
        let ws = WsConnect::new(rpc_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        let dyn_provider = Arc::new(DynProvider::new(provider));

        Ok(UniswapSwapStream {
            provider: dyn_provider,
            pool_addresses,
        })
    }

    /// Create stream by discovering pools for token addresses
    /// Uses Nad.fun standard 10_000 fee tier (1%)
    pub async fn discover_pools_for_tokens(
        rpc_url: String,
        token_addresses: Vec<Address>,
    ) -> Result<Self> {
        use crate::contracts::get_pool_addresses_for_tokens;

        let ws = WsConnect::new(rpc_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        let dyn_provider = Arc::new(DynProvider::new(provider));

        let token_count = token_addresses.len();
        let pool_addresses =
            get_pool_addresses_for_tokens(dyn_provider.clone(), token_addresses).await?;

        println!(
            "🔍 Discovered {} pools for {} tokens",
            pool_addresses.len(),
            token_count
        );

        Ok(UniswapSwapStream {
            provider: dyn_provider,
            pool_addresses,
        })
    }

    /// Create stream by discovering pool for a single token
    pub async fn discover_pool_for_token(rpc_url: String, token_address: Address) -> Result<Self> {
        Self::discover_pools_for_tokens(rpc_url, vec![token_address]).await
    }

    /// Subscribe to swap events - provides raw swap events
    pub async fn subscribe(&self) -> Result<Pin<Box<dyn Stream<Item = Result<SwapEvent>> + Send>>> {
        use crate::types::{UniswapV3Pool, decode_swap_event};
        use alloy::rpc::types::Filter;
        use futures_util::StreamExt;

        let swap_signature = UniswapV3Pool::Swap::SIGNATURE_HASH;

        // Create filter for all monitored pools
        let filter = Filter::new()
            .address(self.pool_addresses.clone())
            .event_signature(swap_signature);

        let sub = self.provider.subscribe_logs(&filter).await?;

        let stream = sub
            .into_stream()
            .map(move |log| decode_swap_event(log))
            .filter_map(|result| async move {
                match result {
                    Ok(event) => Some(Ok(event)),
                    Err(e) => {
                        eprintln!("Error decoding swap event: {}", e);
                        None
                    }
                }
            });

        Ok(Box::pin(stream))
    }
}
