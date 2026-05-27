//! NadFunPair `Swap` event type and decoder.
//!
//! NadFunPair is a Uniswap V2 fork with pair-level fee deduction. The Swap
//! event signature matches Uniswap V2 (`amount0In, amount1In, amount0Out,
//! amount1Out`), distinct from v1's Capricorn CL `Swap` which uses signed
//! `amount0, amount1` + tick info.

use alloy::{
    primitives::{Address, B256, U256},
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
};
use anyhow::{anyhow, Result};

sol!(
    #[sol(rpc)]
    INadFunPairEvents,
    "abi/v2/NadFunPair.json"
);

/// Decoded `NadFunPair::Swap` event.
#[derive(Debug, Clone)]
pub struct NadFunSwapEvent {
    pub sender: Address,
    pub to: Address,
    pub amount0_in: U256,
    pub amount1_in: U256,
    pub amount0_out: U256,
    pub amount1_out: U256,
    pub pair_address: Address,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

impl NadFunSwapEvent {
    /// Whether this is a buy of `token` from the perspective of someone who
    /// owns the other side of the pair, given which side is `token` (i.e.
    /// `token_is_token0`). A "buy" means `token` came out and the other side
    /// went in.
    pub fn is_token_buy(&self, token_is_token0: bool) -> bool {
        if token_is_token0 {
            self.amount0_out > U256::ZERO && self.amount1_in > U256::ZERO
        } else {
            self.amount1_out > U256::ZERO && self.amount0_in > U256::ZERO
        }
    }

    pub fn is_token_sell(&self, token_is_token0: bool) -> bool {
        if token_is_token0 {
            self.amount0_in > U256::ZERO && self.amount1_out > U256::ZERO
        } else {
            self.amount1_in > U256::ZERO && self.amount0_out > U256::ZERO
        }
    }

    pub fn trade_direction(&self, token_is_token0: bool) -> &'static str {
        if self.is_token_buy(token_is_token0) {
            "BUY"
        } else if self.is_token_sell(token_is_token0) {
            "SELL"
        } else {
            "UNKNOWN"
        }
    }
}

/// keccak256 hash of the `Swap` event signature.
pub fn nadfun_swap_signature() -> B256 {
    INadFunPairEvents::Swap::SIGNATURE_HASH
}

/// Decode a log into a [`NadFunSwapEvent`]. Errors if topic0 doesn't match
/// `NadFunPair::Swap`.
pub fn decode_nadfun_swap_event(log: Log) -> Result<NadFunSwapEvent> {
    let topic0 = log
        .topic0()
        .copied()
        .ok_or_else(|| anyhow!("log has no topics"))?;
    if topic0 != INadFunPairEvents::Swap::SIGNATURE_HASH {
        return Err(anyhow!("topic0 does not match NadFunPair::Swap"));
    }
    let pair_address = log.address();
    let block_number = log.block_number.unwrap_or_default();
    let transaction_hash = log.transaction_hash.unwrap_or_default();
    let transaction_index = log.transaction_index.unwrap_or_default();
    let log_index = log.log_index.unwrap_or_default();

    let e = INadFunPairEvents::Swap::decode_log_data(log.data())?;
    Ok(NadFunSwapEvent {
        sender: e.sender,
        to: e.to,
        amount0_in: e.amount0In,
        amount1_in: e.amount1In,
        amount0_out: e.amount0Out,
        amount1_out: e.amount1Out,
        pair_address,
        block_number,
        transaction_hash,
        transaction_index,
        log_index,
    })
}
