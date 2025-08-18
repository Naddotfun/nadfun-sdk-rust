//! # NADS Pump SDK
//!
//! A simple and efficient Rust SDK for NADS Pump trading and event monitoring.
//!
//! ## Core Features
//!
//! - **Trading**: Buy/sell tokens with automatic routing (bonding curve ↔ DEX)
//! - **Event Streaming**: Real-time streaming and historical indexing
//! - **Simple API**: Two main modules - `Trade` for trading, `stream` for events
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use nadfun_sdk::{Trade, Router, Operation, get_default_gas_limit};
//! use alloy::primitives::{Address, U256};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Trading
//!     let trade = Trade::new("https://your-rpc-url".to_string(), "your-private-key".to_string()).await?;
//!     let (router, amount_out) = trade.get_amount_out(token, mon_amount, true).await?;
//!     let result = trade.buy(buy_params, router).await?;
//!     
//!     // Event Streaming
//!     let stream = EventStream::new("wss://your-ws-url".to_string()).await?;
//!     // Real-time event subscription available
//!     
//!     Ok(())
//! }
//! ```

/// Constants and contract addresses for the NADS ecosystem
///
/// Contains all contract addresses, fee tiers, and other system constants.
/// These are automatically used by the SDK but can be accessed directly if needed.
pub mod constants;

/// Trading functionality including buy/sell operations and slippage calculations
///
/// Provides the main trading interface (`Trade`) for buying/selling tokens with
/// automatic routing between bonding curves and DEX pools. Also includes slippage
/// calculation utilities (`SlippageUtils`) for precise trade protection.
pub mod trading;

/// Token interaction utilities for ERC-20 operations
///
/// Contains comprehensive token helper utilities (`TokenHelper`) for metadata
/// retrieval, balance checking, and approval management including EIP-2612 permit
/// signatures for gasless approvals.
pub mod token;

/// Real-time event streaming and historical data indexing
///
/// Provides both WebSocket-based real-time event streaming and HTTP-based historical
/// event indexing for bonding curve and DEX events. Supports advanced filtering by
/// event types, tokens, and custom criteria with optimized batch processing.
pub mod stream;

/// Type definitions for events, trading parameters, and API responses
///
/// Contains all structured data types used throughout the SDK including event
/// definitions (bonding curve events, swap events), trading parameters (buy/sell),
/// and response types (transaction results, token metadata).
pub mod types;

/// Internal contract interface definitions (not directly exposed to users)
///
/// Contains low-level contract bindings and pool discovery logic. These are used
/// internally by the public API but hidden from end users for simplicity.
pub(crate) mod contracts;

// Core API exports - only what users need
pub use contracts::{PoolDiscovery, get_pool_addresses_for_tokens};
// Export contract interfaces for gas estimation in examples
pub use contracts::bonding_curve::{IBondingCurveRouter};
pub use contracts::dex::{IDexRouter};
pub use stream::{
    BondingCurveEvent, CurveIndexer, CurveStream, EventType, PoolMetadata, SwapEvent,
    UniswapSwapIndexer, UniswapSwapStream,
};
pub use token::TokenHelper;
pub use trading::{SlippageUtils, Trade, Router, Operation, get_default_gas_limit, BondingCurveGas, DexRouterGas};
pub use types::*;

/// Convenient prelude module for importing commonly used types and functions
///
/// Import this module to get quick access to all the most frequently used SDK components:
///
/// ```rust
/// use nadfun_sdk::prelude::*;
///
/// // Now you have access to Trade, CurveStream, EventType, Address, U256, etc.
/// ```
///
/// This saves you from having to import each type individually and provides
/// a standardized way to get started with the SDK quickly.
pub mod prelude {
    // Trading functionality
    pub use crate::trading::{SlippageUtils, Trade, Router, Operation, get_default_gas_limit, BondingCurveGas, DexRouterGas};

    // Token operations
    pub use crate::token::TokenHelper;

    // Event streaming and indexing
    pub use crate::stream::{BondingCurveEvent, CurveIndexer, CurveStream, EventType};
    pub use crate::stream::{PoolMetadata, SwapEvent, UniswapSwapIndexer, UniswapSwapStream};

    // Pool discovery utilities
    pub use crate::contracts::{PoolDiscovery, get_pool_addresses_for_tokens};

    // Constants and types
    pub use crate::constants::*;
    pub use crate::types::*;

    // Common Alloy primitives
    pub use alloy::primitives::{Address, B256, U256};
}
