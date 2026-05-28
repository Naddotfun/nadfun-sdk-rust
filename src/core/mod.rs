//! Unified `Core` trading interface for the Nad.fun ecosystem.
//!
//! ## Main Components
//!
//! - **[`Core`]**: Single high-level trading client. Handles v1 (bonding
//!   curve + Capricorn CL DEX) and v2 (NadFunRouter + per-token registry +
//!   vaults) from one instance.
//!   - Auto-routing between bonding curve and DEX on v1 via `Lens`.
//!   - Explicit `*_v2` methods for v2-only operations (different params
//!     shape, can't auto-dispatch).
//!   - `Core::detect_version(token)` for picking v1 vs v2 at call sites.
//! - **[`SlippageUtils`]**: Slippage math (basis-points based, no floating
//!   point).
//! - **[`GasEstimationParams`]** / **[`estimate_gas`]**: v1 gas estimation
//!   surface, exposed for advanced callers that need fine-grained control.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use nadfun_sdk::{Core, Network, SdkVersion};
//!
//! let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
//!
//! match core.detect_version(token).await? {
//!     SdkVersion::V1 => {
//!         let (router, expected) = core.get_amount_out(token, amount_in, true).await?;
//!         core.buy(buy_params, router).await?;
//!     }
//!     SdkVersion::V2 => {
//!         let expected = core.quote_v2(token, amount_in, true).await?;
//!         core.buy_v2(v2_buy_params).await?;
//!     }
//! }
//! ```

#[allow(clippy::module_inception)]
pub mod core;
pub mod v1;

pub use crate::types::Router;
pub use core::Core;
pub use v1::{
    estimate_buy_gas, estimate_gas, estimate_sell_gas, estimate_sell_permit_gas,
    GasEstimationParams, SlippageUtils,
};
