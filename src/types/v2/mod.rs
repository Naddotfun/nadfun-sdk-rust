//! NadFun contract v2 types.

use alloy::primitives::{Address, Bytes, B256, U256};

use crate::types::GasPricing;

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
