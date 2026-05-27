//! v2 streaming + indexing surface (NadFunRouter / BondingCurveV2 /
//! NadFunPair).
//!
//! Independent from `crate::stream::v1` — v1 callers' code is untouched.

pub mod curve;
pub mod dex;

pub use curve::{CurveIndexerV2, CurveStreamV2};
pub use dex::{decode_nadfun_swap_event, NadFunSwapEvent, NadFunSwapIndexer, NadFunSwapStream};

use crate::contracts::{NadFunFactory, PoolDiscovery as CapricornPoolDiscovery};
use alloy::{
    primitives::Address,
    providers::{DynProvider, Provider},
};
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
/// (against the configured `DEX_FACTORY` / WMON pair, fee tier 1%) AND a v2
/// NadFunFactory pair lookup. Any address that returns `Address::ZERO` is
/// treated as "no pool on that surface" and skipped.
///
/// Returns the flat list of discovered pools. Order is not guaranteed.
pub async fn discover_pools_unified(
    provider: Arc<DynProvider>,
    tokens: Vec<Address>,
    factory_v2_address: Option<Address>,
) -> Result<Vec<PoolLocation>> {
    let mut out: Vec<PoolLocation> = Vec::new();

    // v1 (Capricorn CL) pool discovery — per-token lookup so we preserve
    // the (token, pool) correspondence and skip tokens without a v1 pool.
    let cap = CapricornPoolDiscovery::new(provider.clone())?;
    for token in &tokens {
        if let Some(pool) = cap.get_pool_for_token(*token).await? {
            out.push(PoolLocation {
                token: *token,
                pool,
                surface: PoolSurface::Capricorn,
            });
        }
    }

    // v2 (NadFun) pool discovery — query factory.getPair per token.
    if let Some(addr) = factory_v2_address {
        let factory = NadFunFactory::new(addr, provider.clone());
        let wmon = crate::constants::get_wmon().parse::<Address>()?;
        for token in tokens {
            // NadFunFactory.getPair takes (tokenA, tokenB); the v2 default
            // quote is WMON. ERC-20-quote tokens have multiple pairs — only
            // the WMON-quoted one is surfaced here. Power users can query
            // TokenRegistryV2::get_pair(token) for the canonical pair.
            let pool = factory.get_pair(token, wmon).await.unwrap_or(Address::ZERO);
            if pool != Address::ZERO {
                out.push(PoolLocation {
                    token,
                    pool,
                    surface: PoolSurface::NadFun,
                });
            }
        }
    }

    Ok(out)
}
