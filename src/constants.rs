//! Nad.fun ecosystem constants and contract addresses
//!
//! This module centralizes all contract addresses, fee tiers, and system constants
//! used throughout the Nad.fun ecosystem. These constants are automatically used by
//! the SDK's internal operations but can be accessed directly when needed.
//!
//! ## Contract Architecture
//!
//! The Nad.fun ecosystem consists of several key contracts:
//! - **Bonding Curve**: Where new tokens are initially created and traded
//! - **DEX Integration**: Uniswap V3 pools for established tokens
//! - **Routers**: Smart routing between bonding curves and DEX pools
//! - **Lens**: Utility contract for batched operations
//!
//! ## Usage
//!
//! ```rust,ignore
//! use nadfun_sdk::constants::{BONDING_CURVE, WMON, DEFAULT_FEE_TIER};
//!
//! // Access contract addresses
//! let bonding_curve_addr = BONDING_CURVE.parse::<Address>()?;
//!
//! // Use standard fee tier
//! let fee = DEFAULT_FEE_TIER; // 1% = 10,000 basis points
//! ```

/// Core contract addresses in the Nad.fun ecosystem
///
/// These addresses are for the production deployment and are used automatically
/// by all SDK operations. They represent the authoritative contract instances.
pub mod addresses {
    /// Uniswap V3 Factory contract for pool creation and discovery
    ///
    /// Used internally for finding existing pools and creating new ones when
    /// tokens graduate from bonding curves to DEX trading.
    pub const UNISWAP_V3_FACTORY: &str = "0x961235a9020B05C44DF1026D956D1F4D78014276";

    /// Wrapped MON (WMON) token - the base trading pair for all tokens
    ///
    /// All tokens in the Nad.fun ecosystem are paired with WMON for trading.
    /// This is equivalent to WETH in Ethereum-based DEXes.
    pub const WMON: &str = "0x760AfE86e5de5fa0Ee542fc7B7B713e1c5425701";

    /// Main bonding curve contract where new tokens are created and initially traded
    ///
    /// This contract handles the mathematical bonding curve logic for price discovery
    /// during the initial token launch phase.
    pub const BONDING_CURVE: &str = "0x52D34d8536350Cd997bCBD0b9E9d722452f341F5";

    /// Bonding curve router for optimized trading operations
    ///
    /// Provides gas-efficient routing and batching for bonding curve trades.
    /// Used automatically by the Trade interface.
    pub const BONDING_CURVE_ROUTER: &str = "0x4F5A3518F082275edf59026f72B66AC2838c0414";

    /// DEX router for Uniswap V3 operations
    ///
    /// Handles routing and trade execution for tokens that have graduated
    /// from bonding curves to full DEX trading.
    pub const DEX_ROUTER: &str = "0x4FBDC27FAE5f99E7B09590bEc8Bf20481FCf9551";

    /// Utility LENS contract for batched operations
    ///
    /// Enables efficient batch operations and complex multi-step transactions.
    pub const LENS_ADDRESS: &str = "0xD47Dd1a82dd239688ECE1BA94D86f3D32960C339";
}

/// Trading constants and fee configurations
///
/// These values define the economic parameters of the Nad.fun ecosystem.
pub mod fees {
    /// Standard Nad.fun fee tier for Uniswap V3 pools (1.00% = 10,000 basis points)
    ///
    /// This is the default fee tier used for all WMON pairs in the ecosystem.
    /// Higher than typical DEX fees to account for the experimental nature of
    /// tokens and provide sustainable liquidity incentives.
    pub const DEFAULT_FEE_TIER: u32 = 10000;
}

// Re-export commonly used constants for convenience
pub use addresses::*;
pub use fees::DEFAULT_FEE_TIER;
