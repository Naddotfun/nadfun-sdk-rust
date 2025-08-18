//! Token operation examples module
//!
//! This module contains comprehensive examples for ERC20 token operations
//! using the NADS SDK TokenHelper.
//!
//! ## Examples
//!
//! - `basic_operations.rs` - Core ERC20 operations (balance, transfer, approve)
//! - `permit_signature.rs` - EIP-2612 permit signature generation and usage
//! - `token_analysis.rs` - Multi-token portfolio analysis and monitoring
//!
//! ## Core TokenHelper Features
//!
//! ### Basic ERC20 Operations
//! - `name()`, `symbol()`, `decimals()`, `total_supply()` - Token metadata
//! - `balance_of()` - Check token balances
//! - `allowance()` - Check approval amounts
//! - `transfer()` - Send tokens to another address
//! - `transfer_from()` - Transfer tokens using allowance
//! - `approve()` - Approve token spending
//!
//! ### EIP-2612 Permit Support
//! - `get_nonce()` - Get current permit nonce
//! - `get_domain_separator()` - Get EIP-712 domain separator
//! - `generate_permit_signature()` - Create gasless approval signatures
//! - `build_domain_separator()` - Manual domain separator calculation
//!
//! ### Advanced Features
//! - `get_token_metadata()` - Batch metadata retrieval
//! - Parallel network calls for optimization
//! - Comprehensive error handling
//! - Gas optimization patterns
//!
//! ## Usage
//!
//! Before running examples, make sure to:
//! 1. Replace `"your_private_key_here"` with your actual private key
//! 2. Replace example token addresses with real token addresses
//! 3. Ensure you have sufficient ETH for gas fees
//! 4. Test with small amounts first
//!
//! ## Run Examples
//!
//! ```bash
//! # Basic ERC20 operations
//! cargo run --example basic_operations
//!
//! # EIP-2612 permit signatures
//! cargo run --example permit_signature
//!
//! # Multi-token analysis
//! cargo run --example token_analysis
//! ```
//!
//! ## Security Notes
//!
//! - Never share or commit private keys
//! - Always verify contract addresses before interacting
//! - Use small amounts for testing
//! - Review allowances regularly and revoke unused ones
//! - Understand permit signatures before using them

pub mod basic_operations;
pub mod permit_signature;
pub mod token_analysis;
