//! v2 BondingCurve event types and decoder.
//!
//! Mirrors the events emitted by the v2 BondingCurve contract
//! (`nadfun-contract-v2/src/core/BondingCurve.sol`). The router's Buy/Sell
//! events (which carry the `graduated: bool` flag) are decoded separately —
//! these are the lower-level curve events the SDK consumes for indexing.

use alloy::{
    primitives::{Address, B256, U256},
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
};
use anyhow::{anyhow, Result};

sol!(
    #[sol(rpc)]
    IBondingCurveV2Events,
    "abi/v2/BondingCurve.json"
);

/// Event-type discriminator for v2 BondingCurve filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum V2EventType {
    Create,
    Buy,
    Sell,
    Sync,
    Graduate,
    SnipingPenalty,
}

impl V2EventType {
    /// keccak256 hash of the event signature — used for `eth_getLogs` topic[0] filtering.
    pub fn signature(&self) -> B256 {
        match self {
            V2EventType::Create => IBondingCurveV2Events::Create::SIGNATURE_HASH,
            V2EventType::Buy => IBondingCurveV2Events::Buy::SIGNATURE_HASH,
            V2EventType::Sell => IBondingCurveV2Events::Sell::SIGNATURE_HASH,
            V2EventType::Sync => IBondingCurveV2Events::Sync::SIGNATURE_HASH,
            V2EventType::Graduate => IBondingCurveV2Events::Graduate::SIGNATURE_HASH,
            V2EventType::SnipingPenalty => IBondingCurveV2Events::SnipingPenalty::SIGNATURE_HASH,
        }
    }

    /// All v2 event types (handy for callers that want the full feed).
    pub fn all() -> Vec<V2EventType> {
        vec![
            V2EventType::Create,
            V2EventType::Buy,
            V2EventType::Sell,
            V2EventType::Sync,
            V2EventType::Graduate,
            V2EventType::SnipingPenalty,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct V2CreateEvent {
    pub creator: Address,
    pub token: Address,
    pub pair: Address,
    pub quote_token: Address,
    pub name: String,
    pub symbol: String,
    pub token_uri: String,
    pub virtual_quote_reserve: U256,
    pub virtual_token_reserve: U256,
    pub min_token_reserve: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

#[derive(Debug, Clone)]
pub struct V2BuyEvent {
    pub token: Address,
    pub buyer: Address,
    pub quote_in: U256,
    pub token_out: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

#[derive(Debug, Clone)]
pub struct V2SellEvent {
    pub token: Address,
    pub seller: Address,
    pub token_in: U256,
    pub quote_out: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

#[derive(Debug, Clone)]
pub struct V2SyncEvent {
    pub token: Address,
    pub real_quote_reserve: U256,
    pub real_token_reserve: U256,
    pub virtual_quote_reserve: U256,
    pub virtual_token_reserve: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

#[derive(Debug, Clone)]
pub struct V2GraduateEvent {
    pub token: Address,
    pub pair: Address,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

#[derive(Debug, Clone)]
pub struct V2SnipingPenaltyEvent {
    pub token: Address,
    pub buyer: Address,
    pub sniping_fee: U256,
    pub penalty_bps: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// Unified enum over all v2 BondingCurve events.
///
/// Intentionally distinct from v1's `BondingCurveEvent` so v1 callers'
/// exhaustive `match` blocks keep compiling.
#[derive(Debug, Clone)]
pub enum V2BondingCurveEvent {
    Create(V2CreateEvent),
    Buy(V2BuyEvent),
    Sell(V2SellEvent),
    Sync(V2SyncEvent),
    Graduate(V2GraduateEvent),
    SnipingPenalty(V2SnipingPenaltyEvent),
}

impl V2BondingCurveEvent {
    pub fn token(&self) -> Address {
        match self {
            V2BondingCurveEvent::Create(e) => e.token,
            V2BondingCurveEvent::Buy(e) => e.token,
            V2BondingCurveEvent::Sell(e) => e.token,
            V2BondingCurveEvent::Sync(e) => e.token,
            V2BondingCurveEvent::Graduate(e) => e.token,
            V2BondingCurveEvent::SnipingPenalty(e) => e.token,
        }
    }

    pub fn event_type(&self) -> V2EventType {
        match self {
            V2BondingCurveEvent::Create(_) => V2EventType::Create,
            V2BondingCurveEvent::Buy(_) => V2EventType::Buy,
            V2BondingCurveEvent::Sell(_) => V2EventType::Sell,
            V2BondingCurveEvent::Sync(_) => V2EventType::Sync,
            V2BondingCurveEvent::Graduate(_) => V2EventType::Graduate,
            V2BondingCurveEvent::SnipingPenalty(_) => V2EventType::SnipingPenalty,
        }
    }

    pub fn block_number(&self) -> u64 {
        match self {
            V2BondingCurveEvent::Create(e) => e.block_number,
            V2BondingCurveEvent::Buy(e) => e.block_number,
            V2BondingCurveEvent::Sell(e) => e.block_number,
            V2BondingCurveEvent::Sync(e) => e.block_number,
            V2BondingCurveEvent::Graduate(e) => e.block_number,
            V2BondingCurveEvent::SnipingPenalty(e) => e.block_number,
        }
    }

    pub fn transaction_hash(&self) -> B256 {
        match self {
            V2BondingCurveEvent::Create(e) => e.transaction_hash,
            V2BondingCurveEvent::Buy(e) => e.transaction_hash,
            V2BondingCurveEvent::Sell(e) => e.transaction_hash,
            V2BondingCurveEvent::Sync(e) => e.transaction_hash,
            V2BondingCurveEvent::Graduate(e) => e.transaction_hash,
            V2BondingCurveEvent::SnipingPenalty(e) => e.transaction_hash,
        }
    }

    pub fn log_index(&self) -> u64 {
        match self {
            V2BondingCurveEvent::Create(e) => e.log_index,
            V2BondingCurveEvent::Buy(e) => e.log_index,
            V2BondingCurveEvent::Sell(e) => e.log_index,
            V2BondingCurveEvent::Sync(e) => e.log_index,
            V2BondingCurveEvent::Graduate(e) => e.log_index,
            V2BondingCurveEvent::SnipingPenalty(e) => e.log_index,
        }
    }
}

/// Decode an arbitrary log into a typed v2 BondingCurve event. Returns an
/// error if the log doesn't match any of the v2 event signatures.
///
/// Uses `SolEvent::decode_log(&log.inner)` so indexed fields (which live in
/// the topics, not the data slot) are populated correctly. The previous
/// `decode_log_data(log.data())` form silently zeroed every indexed field
/// (`creator`, `token`, `buyer`, `seller`, `pair`, …) — see Codex P1 #1.
pub fn decode_v2_bonding_curve_event(log: Log) -> Result<V2BondingCurveEvent> {
    let topic0 = log
        .topic0()
        .copied()
        .ok_or_else(|| anyhow!("log has no topics"))?;
    let block_number = log.block_number.unwrap_or_default();
    let transaction_hash = log.transaction_hash.unwrap_or_default();
    let transaction_index = log.transaction_index.unwrap_or_default();
    let log_index = log.log_index.unwrap_or_default();
    let inner = &log.inner;

    if topic0 == IBondingCurveV2Events::Create::SIGNATURE_HASH {
        let e = IBondingCurveV2Events::Create::decode_log(inner)?.data;
        Ok(V2BondingCurveEvent::Create(V2CreateEvent {
            creator: e.creator,
            token: e.token,
            pair: e.pair,
            quote_token: e.quoteToken,
            name: e.name,
            symbol: e.symbol,
            token_uri: e.tokenURI,
            virtual_quote_reserve: e.virtualQuoteReserve,
            virtual_token_reserve: e.virtualTokenReserve,
            min_token_reserve: e.minTokenReserve,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
        }))
    } else if topic0 == IBondingCurveV2Events::Buy::SIGNATURE_HASH {
        let e = IBondingCurveV2Events::Buy::decode_log(inner)?.data;
        Ok(V2BondingCurveEvent::Buy(V2BuyEvent {
            token: e.token,
            buyer: e.buyer,
            quote_in: e.quoteIn,
            token_out: e.tokenOut,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
        }))
    } else if topic0 == IBondingCurveV2Events::Sell::SIGNATURE_HASH {
        let e = IBondingCurveV2Events::Sell::decode_log(inner)?.data;
        Ok(V2BondingCurveEvent::Sell(V2SellEvent {
            token: e.token,
            seller: e.seller,
            token_in: e.tokenIn,
            quote_out: e.quoteOut,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
        }))
    } else if topic0 == IBondingCurveV2Events::Sync::SIGNATURE_HASH {
        let e = IBondingCurveV2Events::Sync::decode_log(inner)?.data;
        Ok(V2BondingCurveEvent::Sync(V2SyncEvent {
            token: e.token,
            real_quote_reserve: e.realQuoteReserve,
            real_token_reserve: e.realTokenReserve,
            virtual_quote_reserve: e.virtualQuoteReserve,
            virtual_token_reserve: e.virtualTokenReserve,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
        }))
    } else if topic0 == IBondingCurveV2Events::Graduate::SIGNATURE_HASH {
        let e = IBondingCurveV2Events::Graduate::decode_log(inner)?.data;
        Ok(V2BondingCurveEvent::Graduate(V2GraduateEvent {
            token: e.token,
            pair: e.pair,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
        }))
    } else if topic0 == IBondingCurveV2Events::SnipingPenalty::SIGNATURE_HASH {
        let e = IBondingCurveV2Events::SnipingPenalty::decode_log(inner)?.data;
        Ok(V2BondingCurveEvent::SnipingPenalty(V2SnipingPenaltyEvent {
            token: e.token,
            buyer: e.buyer,
            sniping_fee: e.snipingFee,
            penalty_bps: e.penaltyBps,
            block_number,
            transaction_hash,
            transaction_index,
            log_index,
        }))
    } else {
        Err(anyhow!(
            "log topic0 {:?} does not match any v2 BondingCurve event",
            topic0
        ))
    }
}
