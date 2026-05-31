//! # Nad.fun SDK
//!
//! A simple and efficient Rust SDK for Nad.fun trading and event monitoring.
//!
//! ## Core Features
//!
//! - **Trading**: Buy/sell tokens with automatic routing (bonding curve ↔ DEX)
//! - **Event Streaming**: Real-time streaming and historical indexing
//! - **API Client**: Authenticated API access with session management
//! - **Simple API**: Two main modules - `Core` for trading, `stream` for events
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use nadfun_sdk::{Core, Network};
//! use alloy::primitives::{Address, U256};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize Core - binds to a Network at construction
//!     let core = Core::new(
//!         "https://your-rpc-url".to_string(),
//!         "your-private-key".to_string(),
//!         Network::Mainnet
//!     ).await?;
//!
//!     // v1 trading: access through core.v1() handle
//!     let (router, amount_out) = core.v1().get_amount_out(token, mon_amount, true).await?;
//!
//!     // Execute buy - returns tx_hash immediately (fast!)
//!     let tx_hash = core.v1().buy(buy_params, router).await?;
//!     println!("Transaction submitted: {}", tx_hash);
//!
//!     // Cross-cutting: get_receipt stays on Core
//!     let receipt = core.get_receipt(tx_hash).await?;
//!     println!("Confirmed: {}", receipt.status);
//!
//!     Ok(())
//! }
//! ```

/// Authenticated API client with session management
///
/// Provides automatic authentication flow (nonce → sign → session) and
/// handles cookie-based authentication for API requests.
pub mod api;

/// Constants and contract addresses for the Nad.fun ecosystem
///
/// Contains all contract addresses, fee tiers, and other system constants.
/// These are automatically used by the SDK but can be accessed directly if needed.
pub mod constants;

/// Core trading functionality including buy/sell operations and slippage calculations
///
/// Provides the main trading interface (`Core`) for buying/selling tokens with
/// automatic routing between bonding curves and DEX pools. Also includes slippage
/// calculation utilities (`SlippageUtils`) for precise trade protection.
pub mod core;

// Token creation functionality moved to api module
// Keeping create module for backward compatibility (deprecated)
#[deprecated(note = "Use ApiClient directly for token creation")]
pub mod create;

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

/// Token version discriminator (`SdkVersion`) and on-chain classification
/// (`TokenInfo` = version + quote token) used for user-side v1/v2 dispatch.
pub mod version;

// Pool discovery is still useful for advanced users
pub use api::{ApiClient, ALLOWED_IMAGE_TYPES};
pub use constants::{get_creator_manager, get_creator_treasury, get_nadfun_router_v2, Network};
pub use contracts::{get_pool_addresses_for_tokens, CreatorClient, PairReserves, PoolDiscovery};
pub use core::{estimate_gas, Core, CoreV1, CoreV2, GasEstimationParams, Router, SlippageUtils};
pub use stream::{
    BondingCurveEvent, CurveIndexer, CurveStream, DexIndexer, DexStream, EventType, PoolMetadata,
    SwapEvent,
};
pub use token::TokenHelper;
pub use types::*;
pub use version::{SdkVersion, TokenInfo};

/// Convenient prelude module for importing commonly used types and functions
///
/// Import this module to get quick access to all the most frequently used SDK components:
///
/// ```rust
/// use nadfun_sdk::prelude::*;
///
/// // Now you have access to Core, CurveStream, EventType, Address, U256, etc.
/// ```
///
/// This saves you from having to import each type individually and provides
/// a standardized way to get started with the SDK quickly.
pub mod prelude {
    // API client (handles all API operations including token creation)
    pub use crate::api::{ApiClient, ALLOWED_IMAGE_TYPES};

    // Core trading functionality
    pub use crate::core::{
        estimate_gas, Core, CoreV1, CoreV2, GasEstimationParams, Router, SlippageUtils,
    };

    // Token operations
    pub use crate::token::TokenHelper;

    // Event streaming and indexing
    pub use crate::stream::{BondingCurveEvent, CurveIndexer, CurveStream, EventType};
    pub use crate::stream::{DexIndexer, DexStream, PoolMetadata, SwapEvent};

    // Pool discovery utilities
    pub use crate::contracts::{get_pool_addresses_for_tokens, PairReserves, PoolDiscovery};

    // Constants and types
    pub use crate::constants::{get_nadfun_router_v2, Network};
    pub use crate::types::*;
    pub use crate::version::{SdkVersion, TokenInfo};

    // Creator reward claiming
    pub use crate::contracts::CreatorClient;

    // Common Alloy primitives
    pub use alloy::primitives::{Address, B256, U256};
}
