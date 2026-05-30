//! v1 trading internals — gas estimation + slippage utilities.
//!
//! The unified `Core` (see [`crate::core::Core`]) absorbs the v1 trading
//! surface. This sub-module is kept for the standalone helpers that
//! advanced callers still want direct access to.

pub mod gas;
pub mod handle;
pub mod utils;

pub use gas::{
    estimate_buy_gas, estimate_gas, estimate_sell_gas, estimate_sell_permit_gas,
    GasEstimationParams,
};
pub use handle::CoreV1;
pub use utils::SlippageUtils;
