//! Smart contract interfaces and implementations.
//!
//! Organized by version: `v1` (bonding curve + Capricorn CL DEX) and `v2` (NadFun unified router).

pub mod v1;
pub mod v2;

// Re-export v1 contract types at the legacy `crate::contracts::*` paths so internal
// callers (and the public re-exports in `lib.rs`) keep working unchanged.
pub use v1::{
    get_pool_addresses_for_tokens, BondingCurveRouter, CreatorClient, DexRouter, Lens,
    PoolDiscovery,
};
pub use v2::{
    BondingCurveV2, NadFunFactory, NadFunPair, NadFunRouter, PairReserves, TokenRegistryInfo,
    TokenRegistryV2,
};
