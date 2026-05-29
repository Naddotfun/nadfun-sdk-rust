//! Nad.fun ecosystem constants and contract addresses
//!
//! This module centralizes all contract addresses, fee tiers, and system constants
//! used throughout the Nad.fun ecosystem. These constants are automatically used by
//! the SDK's internal operations but can be accessed directly when needed.
//!
//! ## Contract Architecture
//!
//! The Nad.fun ecosystem has two generations of contracts:
//!
//! - **v1** (`addresses::*::v1`): the original bonding curve + Capricorn CL DEX surface
//!   (BondingCurve, BondingCurveRouter, DexRouter, Lens, CreatorTreasury, ...).
//! - **v2** (`addresses::*::v2`): the unified `NadFunRouter` surface with a v2 bonding
//!   curve, NadFun-native pair/factory, multi-quote support, and 4 vault contracts.
//!
//! Both deployments coexist on mainnet and testnet — `Core` wraps v1 and `CoreV2`
//! wraps v2.
//!
//! ## Usage
//!
//! Pass a `Network` to each helper — no process-global state. Each SDK entry
//! point (`Core`, `ApiClient`, `CurveStream`, ...) stores its own `Network`
//! and threads it into these helpers internally.
//!
//! ```rust,ignore
//! use nadfun_sdk::constants::{Network, get_bonding_curve, get_wmon, get_nadfun_router_v2, DEFAULT_FEE_TIER};
//!
//! // v1 helpers
//! let bonding_curve_addr = get_bonding_curve(Network::Mainnet).parse::<Address>()?;
//! let wmon_addr = get_wmon(Network::Mainnet).parse::<Address>()?;
//!
//! // v2 helpers
//! let nadfun_router_v2 = get_nadfun_router_v2(Network::Mainnet)
//!     .expect("v2 deployed")
//!     .parse::<Address>()?;
//! ```

/// Network type for selecting contract addresses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Network {
    /// Mainnet network
    #[default]
    Mainnet,
    /// Testnet network
    Testnet,
}

/// Core contract addresses in the Nad.fun ecosystem
///
/// Organized by network (`mainnet`, `testnet`) and version (`v1`, `v2`).
/// Each network re-exports its `v1::*` flat at the network level to preserve
/// the legacy `addresses::mainnet::BONDING_CURVE` access pattern.
pub mod addresses {
    /// Mainnet contract addresses
    pub mod mainnet {
        /// v1 (legacy) contracts: bonding curve + Capricorn CL DEX surface.
        pub mod v1 {
            /// DEX Factory contract for pool creation and discovery
            pub const DEX_FACTORY: &str = "0x6B5F564339DbAD6b780249827f2198a841FEB7F3";

            /// Wrapped MON (WMON) token - the base trading pair for all tokens
            pub const WMON: &str = "0x3bd359C1119dA7Da1D913D1C4D2B7c461115433A";

            /// Main bonding curve contract where new tokens are created and initially traded
            pub const BONDING_CURVE: &str = "0xA7283d07812a02AFB7C09B60f8896bCEA3F90aCE";

            /// Bonding curve router for optimized trading operations
            pub const BONDING_CURVE_ROUTER: &str = "0x6F6B8F1a20703309951a5127c45B49b1CD981A22";

            /// DEX router for Capricorn CL operations
            pub const DEX_ROUTER: &str = "0x0B79d71AE99528D1dB24A4148b5f4F865cc2b137";

            /// Utility LENS contract for batched operations
            pub const LENS_ADDRESS: &str = "0x7e78A8DE94f21804F7a17F4E8BF9EC2c872187ea";

            /// API server URL for metadata and token creation
            pub const API_SERVER_URL: &str = "https://api.nadapp.net";

            /// CreatorTreasury contract for creator reward claims
            pub const CREATOR_TREASURY: &str = "0x24dFf9B68fA36f8400302e2babC3e049eA19459E";

            /// CreatorManager contract for creator verification
            pub const CREATOR_MANAGER: &str = "0x8796a581801533fA5c16D1C6ac4f7F57923870C9";
        }

        /// v2 contracts: NadFun unified router + per-token registry + vaults.
        pub mod v2 {
            /// NadFun unified router proxy — single entry point for trading and creation.
            pub const NAD_FUN_ROUTER: &str = "0x8986C8fD44eb85294A725a7e61AF35E76bA26F91";

            /// NadFun pair factory (CREATE2 deterministic pair deployment).
            pub const NAD_FUN_FACTORY: &str = "0xA25b13127e63ddae6d0b35570FF3D39dBD621001";

            /// NadFun pair implementation (Uniswap V2 fork with pair-level fee).
            pub const NAD_FUN_PAIR_IMPL: &str = "0x8115c15CBa6409187bC61B293881f8681394B4cF";

            /// IDexAdapter implementation wrapping NadFunPair.
            pub const NAD_SWAP_ADAPTER: &str = "0x251886adA13Aa3Fb2b02bB101FD9a263CCa7eDE4";

            /// TokenRegistry — per-token metadata (pair, quoteToken, dexType).
            pub const TOKEN_REGISTRY: &str = "0x3CBF1E9F8847A4c968Bb2636696723CC82b91565";

            /// Token implementation contract used by the router for new deployments.
            pub const TOKEN_IMPL: &str = "0x4f44eAFa383FE5f97a0d6CfF97fC5d605D026Fbd";

            /// ProtocolManager — global config (fees, quote tokens, sniping penalty table).
            pub const PROTOCOL_MANAGER: &str = "0x71F846A560a4d68F53e5bd34ED084E7992f171C7";

            /// v2 BondingCurve — state machine for token creation, curve trading, graduation.
            pub const BONDING_CURVE: &str = "0x9f3832732923252A21044F21eE6bd87F09514ae4";

            /// FeeCollector — central per-pair fee storage and settlement.
            pub const FEE_COLLECTOR: &str = "0xE1C8b73343f5A83EBe165BE90470d84B00e33022";

            /// CreatorFeeProcessor — distributes creator fees across vault slots.
            pub const CREATOR_FEE_PROCESSOR: &str = "0x46Bc5ce6a84B4F5595e7E78810b9365edd60fDe9";

            /// LPManager — manages liquidity during graduation.
            pub const LP_MANAGER: &str = "0x5992485CdcD35ccc164A8D893C92ef398C78Eee3";

            /// VaultRegistry — active vault registry.
            pub const VAULT_REGISTRY: &str = "0x64643b714823F0Ce2DfE6B35EF6D63781628f288";

            /// BurnVault — burns received quote tokens (deflationary mechanism).
            pub const BURN_VAULT: &str = "0x94CFAA4d41AE2336E2a4D8B307c7faf906384C27";

            /// LPVault — swaps received quote to LP via IDexAdapter.
            pub const LP_VAULT: &str = "0xA1A5ea7c9490A25E715351Ddc66A7771e1817e66";

            /// CreatorFeeVault — holds creator fees; claims unwrap native when quote = WMON.
            pub const CREATOR_FEE_VAULT: &str = "0x687f9172D5F4798694811333C5C5696afCF4F6f4";

            /// GiftVault — time-locked gift distribution with auto-expiry buyback.
            pub const GIFT_VAULT: &str = "0xa46A28558D77B1bF9dd98A451f78c43bE2545605";

            /// `TokenInfoLens` — stateless view contract that classifies a
            /// token as v1 / v2 / none AND returns its on-chain `quoteToken`
            /// by simultaneously probing the v1 and v2 token registries in one
            /// call. V1 tokens report WMON as their quote.
            pub const TOKEN_INFO_LENS: &str = "0x40c126f92DAD5C26D3b36aA7F2A949265FA534cB";
        }

        // Legacy flat access (`addresses::mainnet::BONDING_CURVE`) — v1 names only.
        pub use v1::*;
    }

    /// Testnet contract addresses
    pub mod testnet {
        /// v1 (legacy) contracts.
        pub mod v1 {
            /// DEX Factory contract for pool creation and discovery
            pub const DEX_FACTORY: &str = "0xd0a37cf728CE2902eB8d4F6f2afc76854048253b";

            /// Wrapped MON (WMON) token - the base trading pair for all tokens
            pub const WMON: &str = "0x5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd";

            /// Main bonding curve contract where new tokens are created and initially traded
            pub const BONDING_CURVE: &str = "0x1228b0dc9481C11D3071E7A924B794CfB038994e";

            /// Bonding curve router for optimized trading operations
            pub const BONDING_CURVE_ROUTER: &str = "0x865054F0F6A288adaAc30261731361EA7E908003";

            /// DEX router for Capricorn CL operations
            pub const DEX_ROUTER: &str = "0x5D4a4f430cA3B1b2dB86B9cFE48a5316800F5fb2";

            /// Utility LENS contract for batched operations
            pub const LENS_ADDRESS: &str = "0xB056d79CA5257589692699a46623F901a3BB76f1";

            /// API server URL for metadata and token creation
            pub const API_SERVER_URL: &str = "https://dev-api.nadapp.net";

            /// CreatorTreasury contract for creator reward claims
            pub const CREATOR_TREASURY: &str = "0x24dFf9B68fA36f8400302e2babC3e049eA19459E";

            /// CreatorManager contract for creator verification
            pub const CREATOR_MANAGER: &str = "0x8796a581801533fA5c16D1C6ac4f7F57923870C9";
        }

        /// v2 contracts.
        pub mod v2 {
            /// NadFun unified router proxy.
            pub const NAD_FUN_ROUTER: &str = "0x75588668999cA0557b78046b8a5E86b47b9234ec";

            /// NadFun pair factory.
            pub const NAD_FUN_FACTORY: &str = "0x59C51c66B79c68F63d5446940CD13b6968788e36";

            /// NadFun pair implementation.
            pub const NAD_FUN_PAIR_IMPL: &str = "0x3cAd42e28BC0D373F4B9419d0835b159eE4CbF1F";

            /// IDexAdapter implementation wrapping NadFunPair.
            pub const NAD_SWAP_ADAPTER: &str = "0x3D7B853D59c5ED21072bF00C9509c5324e9E43c9";

            /// TokenRegistry.
            pub const TOKEN_REGISTRY: &str = "0x2Bc127be900aD290E703Cd2C71eB0EDCa162C898";

            /// Token implementation contract.
            pub const TOKEN_IMPL: &str = "0xFD870fEbeeA5C1Cd5b10cd950eFF8f4d63dc81f1";

            /// ProtocolManager.
            pub const PROTOCOL_MANAGER: &str = "0x2F98030aBD7c59e3E5Dc6b4b66b6008821d0fB41";

            /// v2 BondingCurve.
            pub const BONDING_CURVE: &str = "0x27063a38eC0D3281D354090EB92e669Ed1eB956C";

            /// FeeCollector.
            pub const FEE_COLLECTOR: &str = "0x653cf4297fB3f7804173b8449950E20812DD6dC3";

            /// CreatorFeeProcessor.
            pub const CREATOR_FEE_PROCESSOR: &str = "0xad208200b138F98F6223837464662DaF3a852F02";

            /// LPManager.
            pub const LP_MANAGER: &str = "0x35B8A48f32913d50253a63614206588Cb1D9C402";

            /// VaultRegistry.
            pub const VAULT_REGISTRY: &str = "0x4f7315F8Acde8C521615BA4312d5fC61e632c99D";

            /// BurnVault.
            pub const BURN_VAULT: &str = "0xFA707fe7d2c2894bf0436c7B73947cBA9E888017";

            /// LPVault.
            pub const LP_VAULT: &str = "0x2acD9C75fe16c909237D9e6f080210D26c8c956D";

            /// CreatorFeeVault.
            pub const CREATOR_FEE_VAULT: &str = "0xfEB12B7698e296C57BBF9f0c9b38B3e908285A99";

            /// GiftVault.
            pub const GIFT_VAULT: &str = "0xC112EB5C40FC9A22425300D232A31d00FF840ad0";

            /// Protocol fee recipient (where protocol fees on graduate/dex
            /// trades land; surfaced for read-only inspection / auditing).
            pub const FEE_TO: &str = "0x2248217222bBfd42Ad9edf0689c4c096dCd4FFeE";

            /// Liquid-staked MON used by `ILvMonMinter` flows on v2 (set
            /// alongside the deployment that ships LvMON support).
            pub const LV_MON: &str = "0xBe3fa50514D9617ce645a02B34F595541AF02b6b";

            /// `TokenInfoLens` — stateless view contract that classifies a
            /// token as v1 / v2 / none AND returns its on-chain `quoteToken`
            /// by simultaneously probing the v1 and v2 token registries in one
            /// call. V1 tokens report WMON as their quote.
            pub const TOKEN_INFO_LENS: &str = "0xFC635B7A09cac1A643F5148F8e05Bcd979A8bcC4";
        }

        // Legacy flat access (`addresses::testnet::BONDING_CURVE`) — v1 names only.
        pub use v1::*;
    }
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

// ============================================================================
// v1 helpers — every helper takes the target `Network` explicitly. Each SDK
// entry point stores its own `Network` and threads it through.
// ============================================================================

/// Get DEX Factory address for the given network.
pub fn get_dex_factory(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::DEX_FACTORY,
        Network::Testnet => addresses::testnet::v1::DEX_FACTORY,
    }
}

/// Get WMON address for the given network.
pub fn get_wmon(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::WMON,
        Network::Testnet => addresses::testnet::v1::WMON,
    }
}

/// Get bonding curve address for the given network (v1).
pub fn get_bonding_curve(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::BONDING_CURVE,
        Network::Testnet => addresses::testnet::v1::BONDING_CURVE,
    }
}

/// Get bonding curve router address for the given network.
pub fn get_bonding_curve_router(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::BONDING_CURVE_ROUTER,
        Network::Testnet => addresses::testnet::v1::BONDING_CURVE_ROUTER,
    }
}

/// Get DEX router address for the given network.
pub fn get_dex_router(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::DEX_ROUTER,
        Network::Testnet => addresses::testnet::v1::DEX_ROUTER,
    }
}

/// Get LENS address for the given network.
pub fn get_lens_address(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::LENS_ADDRESS,
        Network::Testnet => addresses::testnet::v1::LENS_ADDRESS,
    }
}

/// Get API server URL for the given network.
pub fn get_api_server_url(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::API_SERVER_URL,
        Network::Testnet => addresses::testnet::v1::API_SERVER_URL,
    }
}

/// Get CreatorTreasury address for the given network.
pub fn get_creator_treasury(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::CREATOR_TREASURY,
        Network::Testnet => addresses::testnet::v1::CREATOR_TREASURY,
    }
}

/// Get CreatorManager address for the given network.
pub fn get_creator_manager(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::CREATOR_MANAGER,
        Network::Testnet => addresses::testnet::v1::CREATOR_MANAGER,
    }
}

// ============================================================================
// v2 helpers — return `Some(&'static str)` when v2 is configured for the
// given network, `None` otherwise. v2 is now deployed on both mainnet and
// testnet, so all helpers (except those gated by deployment, e.g. LvMON,
// FeeTo) return `Some` for either network.
// ============================================================================

/// Get NadFun v2 unified router address for the given network.
pub fn get_nadfun_router_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::NAD_FUN_ROUTER),
        Network::Testnet => Some(addresses::testnet::v2::NAD_FUN_ROUTER),
    }
}

/// Get NadFun v2 pair factory address for the given network.
pub fn get_nadfun_factory_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::NAD_FUN_FACTORY),
        Network::Testnet => Some(addresses::testnet::v2::NAD_FUN_FACTORY),
    }
}

/// Get NadFun v2 pair implementation address for the given network.
pub fn get_nadfun_pair_impl_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::NAD_FUN_PAIR_IMPL),
        Network::Testnet => Some(addresses::testnet::v2::NAD_FUN_PAIR_IMPL),
    }
}

/// Get NadFun v2 swap adapter (IDexAdapter) address for the given network.
pub fn get_nad_swap_adapter_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::NAD_SWAP_ADAPTER),
        Network::Testnet => Some(addresses::testnet::v2::NAD_SWAP_ADAPTER),
    }
}

/// Get NadFun v2 token registry address for the given network.
pub fn get_token_registry_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::TOKEN_REGISTRY),
        Network::Testnet => Some(addresses::testnet::v2::TOKEN_REGISTRY),
    }
}

/// Get NadFun v2 token implementation address for the given network.
pub fn get_token_impl_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::TOKEN_IMPL),
        Network::Testnet => Some(addresses::testnet::v2::TOKEN_IMPL),
    }
}

/// Get NadFun v2 protocol manager address for the given network.
pub fn get_protocol_manager_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::PROTOCOL_MANAGER),
        Network::Testnet => Some(addresses::testnet::v2::PROTOCOL_MANAGER),
    }
}

/// Get NadFun v2 bonding curve address for the given network.
pub fn get_bonding_curve_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::BONDING_CURVE),
        Network::Testnet => Some(addresses::testnet::v2::BONDING_CURVE),
    }
}

/// Get NadFun v2 fee collector address for the given network.
pub fn get_fee_collector_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::FEE_COLLECTOR),
        Network::Testnet => Some(addresses::testnet::v2::FEE_COLLECTOR),
    }
}

/// Get NadFun v2 creator fee processor address for the given network.
pub fn get_creator_fee_processor_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::CREATOR_FEE_PROCESSOR),
        Network::Testnet => Some(addresses::testnet::v2::CREATOR_FEE_PROCESSOR),
    }
}

/// Get NadFun v2 LP manager address for the given network.
pub fn get_lp_manager_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::LP_MANAGER),
        Network::Testnet => Some(addresses::testnet::v2::LP_MANAGER),
    }
}

/// Get NadFun v2 vault registry address for the given network.
pub fn get_vault_registry_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::VAULT_REGISTRY),
        Network::Testnet => Some(addresses::testnet::v2::VAULT_REGISTRY),
    }
}

/// Get NadFun v2 burn vault address for the given network.
pub fn get_burn_vault_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::BURN_VAULT),
        Network::Testnet => Some(addresses::testnet::v2::BURN_VAULT),
    }
}

/// Get NadFun v2 LP vault address for the given network.
pub fn get_lp_vault_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::LP_VAULT),
        Network::Testnet => Some(addresses::testnet::v2::LP_VAULT),
    }
}

/// Get NadFun v2 creator fee vault address for the given network.
pub fn get_creator_fee_vault_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::CREATOR_FEE_VAULT),
        Network::Testnet => Some(addresses::testnet::v2::CREATOR_FEE_VAULT),
    }
}

/// Get NadFun v2 gift vault address for the given network.
pub fn get_gift_vault_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::GIFT_VAULT),
        Network::Testnet => Some(addresses::testnet::v2::GIFT_VAULT),
    }
}

/// Get the v2 liquid-staked MON (LvMON) address for the given network.
///
/// Used by `ILvMonMinter` flows on v2 when the router needs to wrap/unwrap
/// against the liquid-staking version of MON instead of plain WMON. Returns
/// `None` on networks where LvMON isn't deployed yet (currently Mainnet).
pub fn get_lv_mon_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::v2::LV_MON),
    }
}

/// Get the v2 protocol fee recipient (`feeTo`) address for the given network.
pub fn get_fee_to_v2(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => None,
        Network::Testnet => Some(addresses::testnet::v2::FEE_TO),
    }
}

/// Get the `TokenInfoLens` view-contract address for the given network.
///
/// The Lens reads both v1 and v2 token registries in a single on-chain call
/// to classify a token as `V1` / `V2` / `None` and return its `quoteToken`.
/// `Core::detect_version` / `Core::detect_token_info` use it on every
/// supported network — both mainnet and testnet have it deployed.
pub fn get_token_info_lens(network: Network) -> Option<&'static str> {
    match network {
        Network::Mainnet => Some(addresses::mainnet::v2::TOKEN_INFO_LENS),
        Network::Testnet => Some(addresses::testnet::v2::TOKEN_INFO_LENS),
    }
}

// Re-export commonly used constants for convenience.
pub use addresses::*;
pub use fees::DEFAULT_FEE_TIER;
