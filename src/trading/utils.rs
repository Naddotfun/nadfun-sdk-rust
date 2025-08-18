use alloy::primitives::U256;

/// Utility functions for calculating amounts with slippage protection
pub struct SlippageUtils;

impl SlippageUtils {
    /// Calculate minimum amount out with slippage protection
    ///
    /// # Arguments
    /// * `amount_out` - Expected amount out without slippage
    /// * `slippage_percent` - Slippage in percentage (1.0 = 1%, 0.5 = 0.5%)
    ///
    /// # Returns
    /// Minimum amount out considering slippage
    pub fn calculate_amount_out_min(amount_out: U256, slippage_percent: f64) -> U256 {
        if slippage_percent < 0.0 || slippage_percent >= 100.0 {
            return U256::ZERO; // Invalid slippage
        }

        // Convert to basis points to avoid floating point errors
        let slippage_bp = (slippage_percent * 100.0) as u64;
        let remaining_bp = 10000 - slippage_bp;
        amount_out * U256::from(remaining_bp) / U256::from(10000)
    }

    /// Calculate maximum amount in with slippage protection
    ///
    /// # Arguments
    /// * `amount_in` - Expected amount in without slippage
    /// * `slippage_percent` - Slippage in percentage (1.0 = 1%, 0.5 = 0.5%)
    ///
    /// # Returns
    /// Maximum amount in considering slippage
    pub fn calculate_amount_in_max(amount_in: U256, slippage_percent: f64) -> U256 {
        if slippage_percent < 0.0 || slippage_percent >= 100.0 {
            return U256::MAX; // Invalid slippage
        }

        // Convert to basis points to avoid floating point errors
        let slippage_bp = (slippage_percent * 100.0) as u64;
        let total_bp = 10000 + slippage_bp;
        amount_in * U256::from(total_bp) / U256::from(10000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;

    #[test]
    fn test_calculate_amount_out_min() {
        let amount_out = U256::from(1000000000000000000u64); // 1 token

        // 1% slippage
        let min_out = SlippageUtils::calculate_amount_out_min(amount_out, 1.0);
        assert_eq!(min_out, U256::from(990000000000000000u64)); // 0.99 tokens

        // 5% slippage
        let min_out = SlippageUtils::calculate_amount_out_min(amount_out, 5.0);
        assert_eq!(min_out, U256::from(950000000000000000u64)); // 0.95 tokens

        // 0.5% slippage
        let min_out = SlippageUtils::calculate_amount_out_min(amount_out, 0.5);
        assert_eq!(min_out, U256::from(995000000000000000u64)); // 0.995 tokens

        // 30% slippage
        let min_out = SlippageUtils::calculate_amount_out_min(amount_out, 30.0);
        assert_eq!(min_out, U256::from(700000000000000000u64)); // 0.7 tokens (30% 뺀 70%)
    }

    #[test]
    fn test_calculate_amount_in_max() {
        let amount_in = U256::from(1000000000000000000u64); // 1 ETH

        // 1% slippage
        let max_in = SlippageUtils::calculate_amount_in_max(amount_in, 1.0);
        assert_eq!(max_in, U256::from(1010000000000000000u64)); // 1.01 ETH

        // 5% slippage
        let max_in = SlippageUtils::calculate_amount_in_max(amount_in, 5.0);
        assert_eq!(max_in, U256::from(1050000000000000000u64)); // 1.05 ETH

        // 0.5% slippage
        let max_in = SlippageUtils::calculate_amount_in_max(amount_in, 0.5);
        assert_eq!(max_in, U256::from(1005000000000000000u64)); // 1.005 ETH
    }
}
