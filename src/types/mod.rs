//! All types for the Nad.fun SDK.
//!
//! Organized by version: `v1` (legacy bonding curve + Capricorn CL types) and `v2` (NadFun unified router types).

pub mod v1;
pub mod v2;

// Re-export everything for easy access — preserves the existing `crate::types::*` flat access pattern.
pub use v1::*;
pub use v2::*;
