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
