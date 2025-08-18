use crate::types::Router;

/// Trading operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Buy,
    Sell,
    SellPermit,
}

/// Default gas limits for trading operations based on forge test gas reports
///
/// These values are based on actual gas usage from contract tests and include
/// a reasonable buffer for network variations. Users can override these values
/// or add their own buffer as needed.

/// Bonding Curve Router gas limits
pub struct BondingCurveGas;

impl BondingCurveGas {
    /// Gas limit for buy operations
    /// Based on forge test: mean 225,873, max 263,913
    /// Using max + 20% buffer = ~316,000
    pub const BUY: u64 = 320_000;

    /// Gas limit for sell operations  
    /// Based on forge test: mean 64,628, max 140,042
    /// Using max + 20% buffer = ~168,000
    pub const SELL: u64 = 170_000;

    /// Gas limit for sell permit operations
    /// Based on forge test: mean 116,058, max 174,789
    /// Using max + 20% buffer = ~210,000
    pub const SELL_PERMIT: u64 = 210_000;
}

/// DEX Router gas limits
pub struct DexRouterGas;

impl DexRouterGas {
    /// Gas limit for buy operations
    /// Estimated higher than bonding curve due to DEX complexity
    pub const BUY: u64 = 350_000;

    /// Gas limit for sell operations
    /// Estimated higher than bonding curve due to DEX complexity  
    pub const SELL: u64 = 200_000;

    /// Gas limit for sell permit operations
    /// Estimated higher than bonding curve due to DEX complexity
    pub const SELL_PERMIT: u64 = 250_000;
}

/// Helper function to get default gas limit for a trading operation using enums
pub fn get_default_gas_limit(router: &Router, operation: Operation) -> u64 {
    match (router, operation) {
        (Router::BondingCurve(_), Operation::Buy) => BondingCurveGas::BUY,
        (Router::BondingCurve(_), Operation::Sell) => BondingCurveGas::SELL,
        (Router::BondingCurve(_), Operation::SellPermit) => BondingCurveGas::SELL_PERMIT,
        (Router::Dex(_), Operation::Buy) => DexRouterGas::BUY,
        (Router::Dex(_), Operation::Sell) => DexRouterGas::SELL,
        (Router::Dex(_), Operation::SellPermit) => DexRouterGas::SELL_PERMIT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gas_limits() {
        assert_eq!(BondingCurveGas::BUY, 320_000);
        assert_eq!(BondingCurveGas::SELL, 170_000);
        assert_eq!(BondingCurveGas::SELL_PERMIT, 210_000);

        assert_eq!(DexRouterGas::BUY, 350_000);
        assert_eq!(DexRouterGas::SELL, 200_000);
        assert_eq!(DexRouterGas::SELL_PERMIT, 250_000);
    }

    #[test]
    fn test_get_default_gas_limit() {
        use alloy::primitives::Address;

        let bc_router = Router::BondingCurve(Address::ZERO);
        let dex_router = Router::Dex(Address::ZERO);

        assert_eq!(get_default_gas_limit(&bc_router, Operation::Buy), 320_000);
        assert_eq!(get_default_gas_limit(&dex_router, Operation::Sell), 200_000);
        assert_eq!(
            get_default_gas_limit(&bc_router, Operation::SellPermit),
            210_000
        );
        assert_eq!(
            get_default_gas_limit(&dex_router, Operation::SellPermit),
            250_000
        );
    }

    #[test]
    fn test_operation_enum() {
        assert_eq!(Operation::Buy, Operation::Buy);
        assert_ne!(Operation::Buy, Operation::Sell);
    }
}
