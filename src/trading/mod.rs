//! Trading functionality for the NADS ecosystem
//!
//! This module provides comprehensive trading capabilities including:
//!
//! ## Main Components
//!
//! - **[`Trade`]**: High-level trading interface for buying and selling tokens
//!   - Automatic routing between bonding curves and DEX pools
//!   - Built-in slippage protection and deadline management
//!   - Support for both market and limit-style operations
//!   - Gas optimization through smart contract routing
//!
//! - **[`SlippageUtils`]**: Mathematical utilities for slippage calculations
//!   - Precise calculations using basis points to avoid floating-point errors
//!   - Support for both minimum output and maximum input calculations
//!   - Configurable slippage percentages with validation
//!
//! ## Trading Flow
//!
//! 1. **Quote Generation**: Get expected output amounts for a given input
//! 2. **Slippage Protection**: Calculate minimum acceptable outputs
//! 3. **Route Selection**: Automatically choose bonding curve or DEX routing
//! 4. **Transaction Execution**: Submit optimized transactions with proper gas limits
//! 5. **Result Verification**: Confirm successful execution and extract results
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use nadfun_sdk::{Trade, SlippageUtils, Router, Operation, get_default_gas_limit};
//! use alloy::primitives::{Address, utils::parse_ether};
//!
//! // Initialize trading interface
//! let trade = Trade::new(rpc_url, private_key).await?;
//!
//! // Get quote for buying tokens
//! let token: Address = "0x...".parse()?;
//! let mon_amount = parse_ether("0.1")?; // 0.1 MON
//! let (router, expected_tokens) = trade.get_amount_out(token, mon_amount, true).await?;
//!
//! // Apply slippage protection (5%)
//! let min_tokens = SlippageUtils::calculate_amount_out_min(expected_tokens, 5.0);
//!
//! // Execute trade with parameters
//! let buy_params = BuyParams {
//!     token,
//!     amount_in: mon_amount,
//!     amount_out_min: min_tokens,
//!     to: wallet_address,
//!     deadline: U256::from(deadline),
//!     gas_limit: Some(get_default_gas_limit(&router, Operation::Buy)),
//!     gas_price: None,
//!     nonce: None,
//! };
//!
//! let result = trade.buy(buy_params, router).await?;
//! ```
//!
//! ## Advanced Features
//!
//! - **Multi-router Support**: Automatic selection between bonding curve and DEX routers
//! - **Gas Estimation**: Built-in gas estimation with safety margins
//! - **Deadline Management**: Automatic deadline calculation for time-sensitive trades
//! - **Error Handling**: Comprehensive error types for different failure scenarios

/// Core trading interface and execution logic
pub mod trade;

/// Mathematical utilities for slippage calculations and amount conversions
pub mod utils;

/// Default gas limits for trading operations based on contract testing
pub mod gas;

// Re-export main types for convenience
pub use trade::Trade;
pub use crate::types::Router;
pub use utils::SlippageUtils;
pub use gas::{BondingCurveGas, DexRouterGas, Operation, get_default_gas_limit};
