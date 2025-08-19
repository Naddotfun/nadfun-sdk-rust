//! Token interaction utilities and ERC-20 operations
//!
//! This module provides comprehensive utilities for interacting with ERC-20 tokens
//! in the Nad.fun ecosystem, including metadata retrieval, balance checking, and
//! advanced approval mechanisms.
//!
//! ## Main Components
//!
//! - **[`TokenHelper`]**: Complete ERC-20 interaction interface
//!   - Token metadata retrieval (name, symbol, decimals, total supply)
//!   - Balance and allowance checking for any address
//!   - Standard approval transactions with gas optimization
//!   - EIP-2612 permit signature support for gasless approvals
//!   - Batch operations for multiple tokens
//!
//! ## Token Metadata
//!
//! The `TokenHelper` can retrieve comprehensive metadata for any ERC-20 token:
//! - **Name**: Human-readable token name
//! - **Symbol**: Token ticker symbol
//! - **Decimals**: Number of decimal places for display
//! - **Total Supply**: Current circulating supply
//!
//! ## Permission Management
//!
//! Supports both traditional and modern approval mechanisms:
//! - **Standard Approvals**: Classic ERC-20 approve() transactions
//! - **Permit Signatures**: EIP-2612 gasless approvals using off-chain signatures
//! - **Allowance Checking**: Query existing permissions between addresses
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use nadfun_sdk::TokenHelper;
//! use alloy::primitives::{Address, utils::parse_ether};
//!
//! // Initialize token helper
//! let token_helper = TokenHelper::new(rpc_url, private_key).await?;
//! let token: Address = "0x...".parse()?;
//!
//! // Get comprehensive token information
//! let metadata = token_helper.get_token_metadata(token).await?;
//! println!("Token: {} ({}) - {} decimals", 
//!     metadata.name, metadata.symbol, metadata.decimals);
//!
//! // Check balances and allowances
//! let wallet = "0x...".parse()?;
//! let spender = "0x...".parse()?;
//! let balance = token_helper.balance_of(token, wallet).await?;
//! let allowance = token_helper.allowance(token, wallet, spender).await?;
//!
//! // Approve tokens for spending
//! let amount = parse_ether("100")?;
//! let tx_result = token_helper.approve(token, spender, amount).await?;
//!
//! // Use permit signatures for gasless approvals (if supported)
//! let permit_sig = token_helper.create_permit_signature(
//!     token, spender, amount, deadline
//! ).await?;
//! ```
//!
//! ## Error Handling
//!
//! The module provides detailed error handling for common scenarios:
//! - **Network Errors**: RPC connection and timeout issues
//! - **Contract Errors**: Invalid token addresses or failed calls
//! - **Insufficient Balance**: When attempting operations beyond available balance
//! - **Permission Errors**: When allowances are insufficient for operations
//!
//! ## Performance Optimization
//!
//! - **Batch Calls**: Multiple token operations in single RPC call where possible
//! - **Caching**: Metadata caching to reduce redundant network calls
//! - **Gas Estimation**: Automatic gas estimation with safety margins

/// ERC-20 token interaction utilities and helpers
pub mod token;

// Re-export main types for convenience
pub use token::TokenHelper;