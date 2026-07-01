//! NadFunPair `Swap` event type and decoder.
//!
//! NadFunPair is a Uniswap V2 fork with pair-level fee deduction. The Swap
//! event signature matches Uniswap V2 (`amount0In, amount1In, amount0Out,
//! amount1Out`), distinct from v1's Capricorn CL `Swap` which uses signed
//! `amount0, amount1` + tick info.

// alloy `sol!` expansion exceeds clippy's default too_many_arguments threshold.
#![allow(clippy::too_many_arguments)]

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
///
/// Uses `SolEvent::decode_log(&log.inner)` so indexed `sender` / `to`
/// addresses (which live in topics, not data) are populated correctly.
/// Closes Codex P1 #1.
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

    let e = INadFunPairEvents::Swap::decode_log(&log.inner)?.data;
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

/// Decoded `NadFunPair::Sync` event — the pair's post-trade reserve snapshot.
///
/// `reserve0` / `reserve1` follow the pair's `token0` / `token1` (address-sorted
/// `(token, quote)`) ordering, same as [`crate::contracts::PairReserves`]; map a
/// reserve to a side via the pair's `token0()` or [`crate::CoreV2::quote_token`].
/// `Sync` carries no indexed field — `pair_address` is the emitting contract
/// (`log.address()`).
#[derive(Debug, Clone)]
pub struct NadFunSyncEvent {
    pub pair_address: Address,
    pub reserve0: u128,
    pub reserve1: u128,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// keccak256 hash of the `Sync` event signature.
pub fn nadfun_sync_signature() -> B256 {
    INadFunPairEvents::Sync::SIGNATURE_HASH
}

/// Decode a log into a [`NadFunSyncEvent`]. Errors if topic0 doesn't match
/// `NadFunPair::Sync`.
///
/// `Sync` has no indexed fields, so both reserves live in the data slot;
/// `SolEvent::decode_log(&log.inner)` is used for parity with the swap decoder.
pub fn decode_nadfun_sync_event(log: Log) -> Result<NadFunSyncEvent> {
    let topic0 = log
        .topic0()
        .copied()
        .ok_or_else(|| anyhow!("log has no topics"))?;
    if topic0 != INadFunPairEvents::Sync::SIGNATURE_HASH {
        return Err(anyhow!("topic0 does not match NadFunPair::Sync"));
    }
    let pair_address = log.address();
    let block_number = log.block_number.unwrap_or_default();
    let transaction_hash = log.transaction_hash.unwrap_or_default();
    let transaction_index = log.transaction_index.unwrap_or_default();
    let log_index = log.log_index.unwrap_or_default();

    let e = INadFunPairEvents::Sync::decode_log(&log.inner)?.data;
    Ok(NadFunSyncEvent {
        pair_address,
        reserve0: e.reserve0.to::<u128>(),
        reserve1: e.reserve1.to::<u128>(),
        block_number,
        transaction_hash,
        transaction_index,
        log_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, keccak256, Bytes, LogData};
    use alloy::sol_types::SolValue;

    /// Build a synthetic NadFunPair `Sync` RPC log. `Sync` has no indexed
    /// fields, so both reserves live in the data slot and topics = [sig].
    fn sync_log(
        pair: Address,
        block: u64,
        tx_index: u64,
        log_index: u64,
        r0: u128,
        r1: u128,
    ) -> Log {
        let data: Bytes = (U256::from(r0), U256::from(r1)).abi_encode_params().into();
        let inner = alloy::primitives::Log {
            address: pair,
            data: LogData::new_unchecked(vec![nadfun_sync_signature()], data),
        };
        Log {
            inner,
            block_hash: None,
            block_number: Some(block),
            block_timestamp: None,
            transaction_hash: Some(B256::ZERO),
            transaction_index: Some(tx_index),
            log_index: Some(log_index),
            removed: false,
        }
    }

    #[test]
    fn decode_sync_extracts_reserves_and_metadata() {
        let pair = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let ev = decode_nadfun_sync_event(sync_log(pair, 42, 3, 7, 1_000, 2_000)).expect("decode");
        assert_eq!(ev.pair_address, pair, "pair_address is the emitting contract");
        assert_eq!(ev.reserve0, 1_000);
        assert_eq!(ev.reserve1, 2_000);
        assert_eq!(ev.block_number, 42);
        assert_eq!(ev.transaction_index, 3);
        assert_eq!(ev.log_index, 7);
    }

    #[test]
    fn decode_sync_rejects_non_sync_topic() {
        // A Swap-signature log must NOT decode as Sync — the stream relies on
        // this to drop foreign logs the RPC may leak through the filter.
        let data: Bytes = (U256::from(1u64), U256::from(1u64)).abi_encode_params().into();
        let inner = alloy::primitives::Log {
            address: Address::ZERO,
            data: LogData::new_unchecked(vec![nadfun_swap_signature()], data),
        };
        let log = Log {
            inner,
            block_hash: None,
            block_number: Some(1),
            block_timestamp: None,
            transaction_hash: Some(B256::ZERO),
            transaction_index: Some(0),
            log_index: Some(0),
            removed: false,
        };
        assert!(decode_nadfun_sync_event(log).is_err());
    }

    #[test]
    fn sync_signature_is_canonical_uniswap_v2() {
        // Guards ABI drift: the stream filter and any caller filter must agree
        // on the canonical Uniswap-V2 `Sync` topic that on-chain pairs emit.
        assert_eq!(nadfun_sync_signature(), keccak256("Sync(uint112,uint112)"));
    }
}
