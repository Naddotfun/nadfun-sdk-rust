//! v1 trading internals — gas estimation + slippage utilities.
//!
//! The v1 trading surface is exposed via `core.v1()` which returns a
//! borrowed [`CoreV1`] handle (see [`crate::core::Core`]). This sub-module
//! additionally exposes standalone gas/slippage helpers that advanced
//! callers may use directly.

pub mod gas;
pub mod handle;
pub mod utils;

pub use gas::{
    estimate_buy_gas, estimate_gas, estimate_sell_gas, estimate_sell_permit_gas,
    GasEstimationParams,
};
pub use handle::CoreV1;
pub use utils::SlippageUtils;
