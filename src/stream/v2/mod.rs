//! v2 streaming + indexing surface (NadFunRouter / BondingCurveV2 /
//! NadFunPair).
//!
//! Independent from `crate::stream::v1` — v1 callers' code is untouched.

pub mod curve;
pub mod dex;

pub use curve::{CurveIndexerV2, CurveStreamV2};
pub use dex::{
    decode_nadfun_swap_event, decode_nadfun_sync_event, NadFunSwapEvent, NadFunSwapIndexer,
    NadFunSwapStream, NadFunSyncEvent, NadFunSyncStream,
};

use crate::constants::{get_token_registry_v2, Network};
use crate::contracts::{PoolDiscovery as CapricornPoolDiscovery, TokenRegistryV2};
use alloy::{primitives::Address, providers::DynProvider};
use anyhow::Result;
use std::sync::Arc;

/// Surface a pool belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolSurface {
    /// Capricorn CL pool (v1).
    Capricorn,
    /// NadFunPair (v2).
    NadFun,
}

/// One pool resolved for a token, with its surface.
#[derive(Debug, Clone, Copy)]
pub struct PoolLocation {
    pub token: Address,
    pub pool: Address,
    pub surface: PoolSurface,
}

/// Discover pools for a list of tokens across both v1 (Capricorn CL) and v2
/// (NadFun) surfaces.
///
/// For each token the function attempts a v1 Capricorn CL pool lookup
/// (against the configured `DEX_FACTORY` / WMON pair, fee tier 1%) AND a
/// v2 `TokenRegistryV2::getPair(token)` lookup which returns the canonical
/// pair regardless of the token's quote currency (WMON, USDT, …). Any
/// address that returns `Address::ZERO` is treated as "no pool on that
/// surface" and skipped (Codex P2 #11 — previously this restricted v2 to
/// WMON-quoted pairs only).
///
/// Returns the flat list of discovered pools. Order is not guaranteed.
pub async fn discover_pools_unified(
    provider: Arc<DynProvider>,
    tokens: Vec<Address>,
    network: Network,
) -> Result<Vec<PoolLocation>> {
    let mut out: Vec<PoolLocation> = Vec::new();

    // v1 (Capricorn CL) pool discovery — per-token lookup so we preserve
    // the (token, pool) correspondence and skip tokens without a v1 pool.
    let cap = CapricornPoolDiscovery::new(provider.clone(), network)?;
    for token in &tokens {
        if let Some(pool) = cap.get_pool_for_token(*token).await? {
            out.push(PoolLocation {
                token: *token,
                pool,
                surface: PoolSurface::Capricorn,
            });
        }
    }

    // v2 (NadFun) pool discovery — query TokenRegistryV2::getPair, which
    // returns the canonical pair for a v2 token regardless of quote token.
    // Propagate RPC / contract errors (`?`) so callers see partial failures
    // — only treat an actual `Address::ZERO` as "no pool". Codex round 2 P2.
    let reg_addr: Address = get_token_registry_v2(network).parse()?;
    let registry = TokenRegistryV2::new(reg_addr, provider.clone());
    for token in tokens {
        let pool = registry.get_pair(token).await?;
        if pool != Address::ZERO {
            out.push(PoolLocation {
                token,
                pool,
                surface: PoolSurface::NadFun,
            });
        }
    }

    Ok(out)
}
