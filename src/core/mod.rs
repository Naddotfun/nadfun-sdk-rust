//! Unified `Core` trading interface for the Nad.fun ecosystem.
//!
//! ## Main Components
//!
//! - **[`Core`]**: Single high-level trading client. Handles v1 (bonding
//!   curve + Capricorn CL DEX) and v2 (NadFunRouter + per-token registry +
//!   vaults) from one instance.
//!   - v1 trading surface exposed via `core.v1()` → [`CoreV1`] handle.
//!   - v2 trading surface exposed via `core.v2()` → [`CoreV2`] handle
//!     (the `_v2` suffix is dropped on the handle methods).
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
//!         let (router, expected) = core.v1().get_amount_out(token, amount_in, true).await?;
//!         core.v1().buy(buy_params, router).await?;
//!     }
//!     SdkVersion::V2 => {
//!         let expected = core.v2().get_amount_out(token, amount_in, true).await?;
//!         core.v2().buy(v2_buy_params).await?;
//!     }
//! }
//! ```

#[allow(clippy::module_inception)]
pub mod core;
pub mod v1;
pub mod v2;

pub use crate::types::Router;
pub use core::Core;
pub use v1::{
    estimate_buy_gas, estimate_gas, estimate_sell_gas, estimate_sell_permit_gas, CoreV1,
    GasEstimationParams, SlippageUtils,
};
pub use v2::CoreV2;
