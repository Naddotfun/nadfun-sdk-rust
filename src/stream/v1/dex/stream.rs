//! DEX Swap event streaming support
//!
//! This module provides real-time streaming for DEX (Capricorn CL) Swap events.
//! All types are defined in the types::dex module.

use crate::constants::Network;
use crate::types::SwapEvent;
use alloy::{
    primitives::Address,
    providers::{DynProvider, Provider, ProviderBuilder, WsConnect},
    sol_types::SolEvent,
};
use anyhow::Result;
use futures_util::Stream;
use std::{pin::Pin, sync::Arc};

/// Specialized stream for DEX Swap events across multiple pools.
///
/// Bound to a `Network` so pool discovery and any future v1-helpers resolve
/// the correct deployment without consulting global state.
pub struct DexStream {
    #[allow(dead_code)] // Will be used when real streaming is implemented
    provider: Arc<DynProvider>,
    #[allow(dead_code)] // Will be used when real streaming is implemented
    pool_addresses: Vec<Address>,
    #[allow(dead_code)]
    network: Network,
}

impl DexStream {
    /// Create a WebSocket-based DEX swap stream with explicit pool addresses.
    pub async fn new(
        rpc_url: String,
        pool_addresses: Vec<Address>,
        network: Network,
    ) -> Result<DexStream> {
        let ws = WsConnect::new(rpc_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        let dyn_provider = Arc::new(DynProvider::new(provider));

        Ok(DexStream {
            provider: dyn_provider,
            pool_addresses,
            network,
        })
    }

    /// Create stream by discovering pools for token addresses on `network`.
    /// Uses Nad.fun standard 10_000 fee tier (1%).
    pub async fn discover_pools_for_tokens(
        rpc_url: String,
        token_addresses: Vec<Address>,
        network: Network,
    ) -> Result<Self> {
        use crate::contracts::get_pool_addresses_for_tokens;

        let ws = WsConnect::new(rpc_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        let dyn_provider = Arc::new(DynProvider::new(provider));

        let pool_addresses =
            get_pool_addresses_for_tokens(dyn_provider.clone(), token_addresses, network).await?;

        Ok(DexStream {
            provider: dyn_provider,
            pool_addresses,
            network,
        })
    }

    /// Create stream by discovering pool for a single token on `network`.
    pub async fn discover_pool_for_token(
        rpc_url: String,
        token_address: Address,
        network: Network,
    ) -> Result<Self> {
        Self::discover_pools_for_tokens(rpc_url, vec![token_address], network).await
    }

    /// Network this stream is bound to.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Subscribe to swap events - provides raw swap events
    pub async fn subscribe(&self) -> Result<Pin<Box<dyn Stream<Item = Result<SwapEvent>> + Send>>> {
        use crate::types::{decode_swap_event, ICapricornCLPool};
        use alloy::rpc::types::Filter;
        use futures_util::StreamExt;

        let swap_signature = ICapricornCLPool::Swap::SIGNATURE_HASH;

        // Create filter for all monitored pools
        let filter = Filter::new()
            .address(self.pool_addresses.clone())
            .event_signature(swap_signature);

        let sub = self.provider.subscribe_logs(&filter).await?;

        let stream = sub
            .into_stream()
            .map(decode_swap_event)
            .filter_map(|result| async move {
                match result {
                    Ok(event) => Some(Ok(event)),
                    Err(_) => None, // Silently skip invalid events
                }
            });

        Ok(Box::pin(stream))
    }
}
