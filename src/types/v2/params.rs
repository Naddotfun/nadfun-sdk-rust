//! NadFun contract v2 parameter types (trading + creation).

use alloy::primitives::{Address, Bytes, B256, U256};

use crate::types::GasPricing;

/// Slim off-chain inputs that the v2 token-creation API needs to prepare a
/// deployable token: metadata + creator address + image source. The on-chain
/// fields (vaults, fee rate, dex type, initial buy, deadline) come from a
/// higher-level wrapper added in a later step.
#[derive(Debug, Clone)]
pub struct V2PrepareCreationParams {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image_uri: String,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub creator_address: Address,
}

/// Off-chain preparation result for a v2 token creation flow.
///
/// Contains the IPFS image + metadata URIs, the CREATE2 salt, the predicted
/// token address (mined by the salt server), and the NSFW flag from server
/// detection. Feed into the on-chain `NadFunRouter::create` /
/// `NadFunRouter::createWithNative` call.
#[derive(Debug, Clone)]
pub struct V2PreparedCreation {
    pub image_uri: String,
    pub metadata_uri: String,
    pub salt: B256,
    pub token_address: Address,
    pub is_nsfw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V2DexType {
    NadFun = 0,
}

impl V2DexType {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone)]
pub struct V2VaultAllocation {
    pub vault: Address,
    pub bps: u16,
    pub setup_data: Bytes,
}

#[derive(Debug, Clone)]
pub struct V2CreateParams {
    pub name: String,
    pub symbol: String,
    pub token_uri: String,
    pub quote_token: Address,
    pub creator_fee_rate: u16,
    pub vaults: Vec<V2VaultAllocation>,
    pub salt: B256,
    pub dex_type: V2DexType,
    pub buy_quote_amount: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2CreateWithNativeParams {
    pub name: String,
    pub symbol: String,
    pub token_uri: String,
    pub creator_fee_rate: u16,
    pub vaults: Vec<V2VaultAllocation>,
    pub salt: B256,
    pub dex_type: V2DexType,
    pub buy_quote_amount: U256,
    pub native_value: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2BuyParams {
    pub token: Address,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2BuyWithNativeParams {
    pub token: Address,
    pub amount_out_min: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2SellParams {
    pub token: Address,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2PermitParams {
    pub v: u8,
    pub r: B256,
    pub s: B256,
}

#[derive(Debug, Clone)]
pub struct V2BuyWithPermitParams {
    pub token: Address,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub deadline: U256,
    pub permit: V2PermitParams,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2SellWithPermitParams {
    pub token: Address,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub deadline: U256,
    pub permit: V2PermitParams,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2ExactOutBuyParams {
    pub token: Address,
    pub amount_out: U256,
    pub amount_in_max: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2ExactOutBuyWithNativeParams {
    pub token: Address,
    pub amount_out: U256,
    pub amount_in_max: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct V2ExactOutSellParams {
    pub token: Address,
    pub amount_in_max: U256,
    pub amount_out: U256,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

pub type V2SellToNativeParams = V2SellParams;
pub type V2SellToNativeWithPermitParams = V2SellWithPermitParams;
pub type V2ExactOutSellToNativeParams = V2ExactOutSellParams;

// ============================================================================
// High-level token creation wrapper
// ============================================================================

/// How the creator funds the initial buy on a v2 token creation.
#[derive(Debug, Clone)]
pub enum V2CreatePayment {
    /// Pay with the native chain currency (MON). `value` is sent as `msg.value`
    /// and covers the initial buy plus any deploy fee. Routes to
    /// `NadFunRouter::createWithNative`.
    Native {
        /// Native amount to send as `msg.value`.
        value: U256,
    },
    /// Pay with an ERC-20 quote token (e.g. WMON, USDT, ...). The caller must
    /// have approved the router for at least `buy_quote_amount` worth of
    /// `quote_token` beforehand. Routes to `NadFunRouter::create`.
    Erc20 {
        /// ERC-20 quote token used for the initial buy.
        quote_token: Address,
    },
}

/// High-level parameters for `CoreV2::create_token`.
///
/// Bundles off-chain (metadata, image, socials) and on-chain (vaults, fees,
/// dex type, initial buy, deadline) inputs into one struct so the SDK can
/// drive the full create flow:
///   1. Off-chain: image upload + metadata + salt mining via `ApiClient`.
///   2. On-chain: `NadFunRouter::create` or `NadFunRouter::createWithNative`
///      depending on [`V2CreatePayment`].
#[derive(Debug, Clone)]
pub struct V2CreateTokenParams {
    // --- Off-chain (metadata) ---
    /// Human-readable token name.
    pub name: String,
    /// Ticker symbol (typically 3-10 chars).
    pub symbol: String,
    /// Free-form description shown in token cards.
    pub description: String,
    /// Source URL for the token image. Downloaded by the metadata server,
    /// validated, and re-uploaded to IPFS.
    pub image_uri: String,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    /// EIP-55 creator address. Used as a CREATE2 salt input and recorded as
    /// the token's creator on-chain.
    pub creator_address: Address,

    // --- On-chain (curve config) ---
    /// Creator fee rate in basis points (max 10_000 = 100%).
    pub creator_fee_rate: u16,
    /// Vault allocations (BurnVault / LPVault / CreatorFeeVault / GiftVault).
    /// Bps must sum to 10_000.
    pub vaults: Vec<V2VaultAllocation>,
    /// DEX type for post-graduation routing (currently always `NadFun`).
    pub dex_type: V2DexType,
    /// Initial buy size denominated in the quote token. For `Native`
    /// payment, this is the MON amount the curve uses to buy tokens for the
    /// creator at deploy time; for `Erc20`, it's the ERC-20 amount.
    pub buy_quote_amount: U256,
    /// Whether the initial buy is funded by native MON (`msg.value`) or by
    /// an ERC-20 quote token (pre-approved).
    pub payment: V2CreatePayment,
    /// Deadline timestamp (seconds since unix epoch).
    pub deadline: U256,

    // --- Transaction options ---
    pub gas_limit: Option<u64>,
    pub gas_price: Option<GasPricing>,
    pub nonce: Option<u64>,
}

/// Result of `CoreV2::create_token`.
#[derive(Debug, Clone)]
pub struct V2TokenCreationResult {
    /// Address of the newly deployed token (CREATE2-predicted, matches the
    /// salt server output).
    pub token_address: Address,
    /// IPFS metadata URI.
    pub metadata_uri: String,
    /// IPFS image URI.
    pub image_uri: String,
    /// CREATE2 salt used at deployment.
    pub salt: B256,
    /// Hash of the on-chain create transaction.
    pub transaction_hash: B256,
    /// Server-side NSFW detection flag.
    pub is_nsfw: bool,
}

// ============================================================================
// Gas estimation
// ============================================================================

/// All v2 operations the SDK can estimate gas for, packaged in a single enum
/// so `CoreV2::estimate_gas` has a uniform entry point.
///
/// Mirrors the trade/create method matrix on `NadFunRouter`. Each variant
/// carries the same parameter struct the equivalent trade method takes.
#[derive(Debug, Clone)]
pub enum V2GasEstimationParams {
    Create(V2CreateParams),
    CreateWithNative(V2CreateWithNativeParams),
    Buy(V2BuyParams),
    BuyWithNative {
        params: V2BuyWithNativeParams,
        /// Native amount sent as `msg.value`.
        value: U256,
    },
    BuyWithPermit(V2BuyWithPermitParams),
    Sell(V2SellParams),
    SellToNative(V2SellToNativeParams),
    SellWithPermit(V2SellWithPermitParams),
    SellToNativeWithPermit(V2SellToNativeWithPermitParams),
    ExactOutBuy(V2ExactOutBuyParams),
    ExactOutBuyWithNative(V2ExactOutBuyWithNativeParams),
    ExactOutSell(V2ExactOutSellParams),
    ExactOutSellToNative(V2ExactOutSellToNativeParams),
}
