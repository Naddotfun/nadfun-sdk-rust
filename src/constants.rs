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
        pub const DEX_FACTORY: &str = "0x99f4Aa293dcEfFA11aB0c03C359db45d05c7C863";

        /// Wrapped MON (WMON) token - the base trading pair for all tokens
        pub const WMON: &str = "0x760AfE86e5de5fa0Ee542fc7B7B713e1c5425701";

        /// Main bonding curve contract where new tokens are created and initially traded
        pub const BONDING_CURVE: &str = "0x175ed6583EdA113Bd0C0Bb7B473f760006651a99";

        /// Bonding curve router for optimized trading operations
        pub const BONDING_CURVE_ROUTER: &str = "0x92f96f59137f41ECF8cD9a3C7E70E9e7db9deadE";

        /// DEX router for Capricorn CL operations
        pub const DEX_ROUTER: &str = "0x006d317A4176b356aF3764db4d811bc953E33be9";

        /// Utility LENS contract for batched operations
        pub const LENS_ADDRESS: &str = "0xD1cd9821dA319ec214375f6cd155A940e28e758d";
    }

    /// Testnet contract addresses
    pub mod testnet {
        /// DEX Factory contract for pool creation and discovery
        pub const DEX_FACTORY: &str = "0x99f4Aa293dcEfFA11aB0c03C359db45d05c7C863";

        /// Wrapped MON (WMON) token - the base trading pair for all tokens
        pub const WMON: &str = "0x760AfE86e5de5fa0Ee542fc7B7B713e1c5425701";

        /// Quoter V3 contract for price quotes
        pub const QUOTER_V3: &str = "0xb9841fdAc1F0fD01C79095C3c97105D01273A192";

        /// Creator manager contract
        pub const CREATOR_MANAGER: &str = "0x3fA1DfA26f7f917433D6bcd07eE37d9dE4cB9F16";

        /// Token registry contract
        pub const TOKEN_REGISTRY: &str = "0x7179B008BDF664F6ADCE48aAC9e61d397F44533A";

        /// Foundation treasury address
        pub const FOUNDATION_TREASURY: &str = "0xcbd1105758D5F25f2aE4F29A819701406CEaFBD8";

        /// Community treasury address
        pub const COMMUNITY_TREASURY: &str = "0xC862B8f2dBff250E8ac1B9B5b0B12deD94699BA4";

        /// Creator treasury address
        pub const CREATOR_TREASURY: &str = "0x357dbf955317A372CcC2558651Ae6d0599ce9383";

        /// Token treasury address
        pub const TOKEN_TREASURY: &str = "0xE598e86C883BA1B7016726904F619B087559f08F";

        /// Bonding curve router for optimized trading operations
        pub const BONDING_CURVE_ROUTER: &str = "0x3a075D7dC83Fedc113954447AF47065090E4c47E";

        /// Main bonding curve contract where new tokens are created and initially traded
        pub const BONDING_CURVE: &str = "0xE238d6E9e19BF52D2f16847A2A310e9E8BA74e84";

        /// LP Manager contract
        pub const LP_MANAGER: &str = "0xe3911C155700961a1fecA99f6Cc3eeE0567204BC";

        /// Uniswap actor contract
        pub const UNISWAP_ACTOR: &str = "0x8bB92025D506101B0725d437075C1DCDdcD99425";

        /// DEX deployer contract
        pub const DEX_DEPLOYER: &str = "0xdc1A8249112f0D57235d2fe1a910fE28b7Aad283";

        /// DEX router for Capricorn CL operations
        pub const DEX_ROUTER: &str = "0xA3498357FB8105FCe397d8B01D46163b493b4057";

        /// Reward pool contract
        pub const REWARD_POOL: &str = "0x8cFbaE04e8D64cd0e3779c0969EB674a7f5216Ba";

        /// Utility LENS contract for batched operations
        pub const LENS_ADDRESS: &str = "0x827859075280A46CB201fB80F84E977061452D3B";

        /// Token implementation contract
        pub const TOKEN_IMPLEMENT: &str = "0x8608490F6c86A743F5104E5a00456350Fdb3df96";
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

/// Get Quoter V3 address for the current network (testnet only)
pub fn get_quoter_v3() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::QUOTER_V3),
    }
}

/// Get Creator Manager address for the current network (testnet only)
pub fn get_creator_manager() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::CREATOR_MANAGER),
    }
}

/// Get Token Registry address for the current network (testnet only)
pub fn get_token_registry() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::TOKEN_REGISTRY),
    }
}

/// Get Foundation Treasury address for the current network (testnet only)
pub fn get_foundation_treasury() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::FOUNDATION_TREASURY),
    }
}

/// Get Community Treasury address for the current network (testnet only)
pub fn get_community_treasury() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::COMMUNITY_TREASURY),
    }
}

/// Get Creator Treasury address for the current network (testnet only)
pub fn get_creator_treasury() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::CREATOR_TREASURY),
    }
}

/// Get Token Treasury address for the current network (testnet only)
pub fn get_token_treasury() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::TOKEN_TREASURY),
    }
}

/// Get LP Manager address for the current network (testnet only)
pub fn get_lp_manager() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::LP_MANAGER),
    }
}

/// Get Uniswap Actor address for the current network (testnet only)
pub fn get_uniswap_actor() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::UNISWAP_ACTOR),
    }
}

/// Get DEX Deployer address for the current network (testnet only)
pub fn get_dex_deployer() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::DEX_DEPLOYER),
    }
}

/// Get Reward Pool address for the current network (testnet only)
pub fn get_reward_pool() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::REWARD_POOL),
    }
}

/// Get Token Implementation address for the current network (testnet only)
pub fn get_token_implement() -> Option<&'static str> {
    match get_current_network() {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::TOKEN_IMPLEMENT),
    }
}

// Re-export commonly used constants for convenience
pub use addresses::*;
pub use fees::DEFAULT_FEE_TIER;
