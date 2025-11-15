//! Trading examples module
//! 
//! This module contains practical examples for trading tokens using the Nad.fun SDK.
//! All examples include proper slippage protection and real transaction execution.
//!
//! ## Examples
//!
//! - `buy.rs` - Buy tokens with ETH (slippage protected)
//! - `sell.rs` - Sell tokens for ETH (with approval + slippage protected)
//! - `sell_permit.rs` - Gasless sell using EIP-2612 permit signatures
//!
//! ## Usage
//!
//! Before running examples, make sure to:
//! 1. Replace `"your_private_key_here"` with your actual private key
//! 2. Replace example token addresses with real token addresses
//! 3. Adjust amounts and slippage tolerance as needed
//! 4. Ensure you have sufficient ETH for gas fees
//!
//! ## Run Examples
//!
//! ```bash
//! # Buy tokens
//! cargo run --example buy
//!
//! # Sell tokens (traditional method)
//! cargo run --example sell
//!
//! # Gasless sell (permit method)
//! cargo run --example sell_permit
//! ```

pub mod buy;
pub mod sell;
pub mod sell_permit;