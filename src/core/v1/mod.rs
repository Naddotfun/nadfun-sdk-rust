//! v1 core trading: BondingCurveRouter + DexRouter + Lens auto-routing.

#[allow(clippy::module_inception)]
pub mod core;
pub mod gas;
pub mod utils;

pub use core::Core;
pub use gas::{
    estimate_buy_gas, estimate_gas, estimate_sell_gas, estimate_sell_permit_gas,
    GasEstimationParams,
};
pub use utils::SlippageUtils;
