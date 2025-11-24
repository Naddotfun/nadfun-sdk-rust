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
//! - **DEX Integration**: Capricorn CL pools for established tokens
//! - **Routers**: Smart routing between bonding curves and DEX pools
//! - **Lens**: Utility contract for batched operations
//!
//! ## Usage
//!
//! ```rust,ignore
//! use nadfun_sdk::constants::{Network, set_network, get_bonding_curve, get_wmon, DEFAULT_FEE_TIER};
//!
//! // Set network once at the start
//! set_network(Network::Mainnet);
//!
//! // Now all get_* functions return mainnet addresses
//! let bonding_curve_addr = get_bonding_curve().parse::<Address>()?;
//! let wmon_addr = get_wmon().parse::<Address>()?;
//!
//! // Change to testnet
//! set_network(Network::Testnet);
//!
//! // Now all get_* functions return testnet addresses
//! let bonding_curve_addr = get_bonding_curve().parse::<Address>()?;
//! ```

use std::sync::RwLock;

/// Network type for selecting contract addresses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Network {
    /// Mainnet network
    #[default]
    Mainnet,
    /// Testnet network
    Testnet,
}

/// Global network configuration
static CURRENT_NETWORK: RwLock<Network> = RwLock::new(Network::Mainnet);

/// Set the current network for the SDK
///
/// This affects all subsequent calls to get_* functions.
/// Call this once at the start of your application.
///
/// # Example
/// ```rust,ignore
/// use nadfun_sdk::constants::{Network, set_network};
///
/// set_network(Network::Mainnet);
/// ```
pub fn set_network(network: Network) {
    if let Ok(mut current) = CURRENT_NETWORK.write() {
        *current = network;
    }
}

/// Get the current network setting
pub fn get_current_network() -> Network {
    CURRENT_NETWORK.read().map(|n| *n).unwrap_or_default()
}

/// Core contract addresses in the Nad.fun ecosystem
///
/// These addresses are for the production deployment and are used automatically
/// by all SDK operations. They represent the authoritative contract instances.
pub mod addresses {
    /// Mainnet contract addresses
    pub mod mainnet {
        /// DEX Factory contract for pool creation and discovery
        pub const DEX_FACTORY: &str = "0x6B5F564339DbAD6b780249827f2198a841FEB7F3";

        /// Wrapped MON (WMON) token - the base trading pair for all tokens
        pub const WMON: &str = "0x3bd359C1119dA7Da1D913D1C4D2B7c461115433A";

        /// Main bonding curve contract where new tokens are created and initially traded
        pub const BONDING_CURVE: &str = "0x6F6B8F1a20703309951a5127c45B49b1CD981A22";

        /// Bonding curve router for optimized trading operations
        pub const BONDING_CURVE_ROUTER: &str = "0x6B5F564339DbAD6b780249827f2198a841FEB7F3";

        /// DEX router for Capricorn CL operations
        pub const DEX_ROUTER: &str = "0x0B79d71AE99528D1dB24A4148b5f4F865cc2b137";

        /// Utility LENS contract for batched operations
        pub const LENS_ADDRESS: &str = "0x7e78A8DE94f21804F7a17F4E8BF9EC2c872187ea";
    }

    /// Testnet contract addresses
    pub mod testnet {
        /// DEX Factory contract for pool creation and discovery
        pub const DEX_FACTORY: &str = "0x99f4Aa293dcEfFA11aB0c03C359db45d05c7C863";

        /// Wrapped MON (WMON) token - the base trading pair for all tokens
        pub const WMON: &str = "0x760AfE86e5de5fa0Ee542fc7B7B713e1c5425701";

        /// Main bonding curve contract where new tokens are created and initially traded
        pub const BONDING_CURVE: &str = "0xaD720f94689edB929D9be7613223320a0b2f260F";

        /// Bonding curve router for optimized trading operations
        pub const BONDING_CURVE_ROUTER: &str = "0xF57F14335e9670ed2C0CeF4A59fB707Cf2eB3FAC";

        /// DEX router for Capricorn CL operations
        pub const DEX_ROUTER: &str = "0x34469738bbD2948F43E0e4B588BC5646BCf4a6bB";

        /// Utility LENS contract for batched operations
        pub const LENS_ADDRESS: &str = "0x1b2b500a6f6C8a25Ca0436d8183Ba25C9415e28E";
    }

    // Legacy exports for backward compatibility (defaults to mainnet)
    pub use mainnet::*;
}

/// Trading constants and fee configurations
///
/// These values define the economic parameters of the Nad.fun ecosystem.
pub mod fees {
    /// Standard Nad.fun fee tier for Capricorn CL pools (1.00% = 10,000 basis points)
    ///
    /// This is the default fee tier used for all WMON pairs in the ecosystem.
    /// Higher than typical DEX fees to account for the experimental nature of
    /// tokens and provide sustainable liquidity incentives.
    pub const DEFAULT_FEE_TIER: u32 = 10000;
}

// Helper functions to get addresses based on current network setting
/// Get DEX Factory address for the current network
pub fn get_dex_factory() -> &'static str {
    match get_current_network() {
        Network::Mainnet => addresses::mainnet::DEX_FACTORY,
        Network::Testnet => addresses::testnet::DEX_FACTORY,
    }
}

/// Get WMON address for the current network
pub fn get_wmon() -> &'static str {
    match get_current_network() {
        Network::Mainnet => addresses::mainnet::WMON,
        Network::Testnet => addresses::testnet::WMON,
    }
}

/// Get bonding curve address for the current network
pub fn get_bonding_curve() -> &'static str {
    match get_current_network() {
        Network::Mainnet => addresses::mainnet::BONDING_CURVE,
        Network::Testnet => addresses::testnet::BONDING_CURVE,
    }
}

/// Get bonding curve router address for the current network
pub fn get_bonding_curve_router() -> &'static str {
    match get_current_network() {
        Network::Mainnet => addresses::mainnet::BONDING_CURVE_ROUTER,
        Network::Testnet => addresses::testnet::BONDING_CURVE_ROUTER,
    }
}

/// Get DEX router address for the current network
pub fn get_dex_router() -> &'static str {
    match get_current_network() {
        Network::Mainnet => addresses::mainnet::DEX_ROUTER,
        Network::Testnet => addresses::testnet::DEX_ROUTER,
    }
}

/// Get LENS address for the current network
pub fn get_lens_address() -> &'static str {
    match get_current_network() {
        Network::Mainnet => addresses::mainnet::LENS_ADDRESS,
        Network::Testnet => addresses::testnet::LENS_ADDRESS,
    }
}

// Re-export commonly used constants for convenience
pub use addresses::*;
pub use fees::DEFAULT_FEE_TIER;
