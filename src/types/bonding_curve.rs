//! Bonding curve events and related types
//!
//! Contains all bonding curve related event types, enums, and decoding logic.

use alloy::{
    primitives::{Address, B256, U256},
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
};
use anyhow::Result;

// Bonding curve contract interface for events
sol! {
    #[sol(rpc)]
    contract IBondingCurve {
        event CurveCreate(
            address indexed creator,
            address indexed token,
            address indexed pool,
            string name,
            string symbol,
            string tokenURI,
            uint256 virtualMon,
            uint256 virtualToken,
            uint256 targetTokenAmount
        );

        event CurveBuy(
            address indexed sender,
            address indexed token,
            uint256 amountIn,
            uint256 amountOut
        );

        event CurveSell(
            address indexed sender,
            address indexed token,
            uint256 amountIn,
            uint256 amountOut
        );

        event CurveSync(
            address indexed token,
            uint256 realMonReserve,
            uint256 realTokenReserve,
            uint256 virtualMonReserve,
            uint256 virtualTokenReserve
        );

        event CurveTokenLocked(address indexed token);

        event CurveTokenListed(address indexed token, address indexed pool);
    }
}

/// Event types that can be subscribed to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    Create,
    Buy,
    Sell,
    Sync,
    Lock,
    Listed,
}

impl EventType {
    /// Get the signature hash for this event type
    pub fn signature(&self) -> B256 {
        match self {
            EventType::Create => IBondingCurve::CurveCreate::SIGNATURE_HASH,
            EventType::Buy => IBondingCurve::CurveBuy::SIGNATURE_HASH,
            EventType::Sell => IBondingCurve::CurveSell::SIGNATURE_HASH,
            EventType::Sync => IBondingCurve::CurveSync::SIGNATURE_HASH,
            EventType::Lock => IBondingCurve::CurveTokenLocked::SIGNATURE_HASH,
            EventType::Listed => IBondingCurve::CurveTokenListed::SIGNATURE_HASH,
        }
    }
}

/// Create event - when a new token is created
#[derive(Debug, Clone)]
pub struct CreateEvent {
    pub creator: Address,
    pub token: Address,
    pub pool: Address,
    pub name: String,
    pub symbol: String,
    pub token_uri: String,
    pub virtual_mon: U256,
    pub virtual_token: U256,
    pub target_token_amount: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// Buy event - when someone buys tokens with MON
#[derive(Debug, Clone)]
pub struct BuyEvent {
    pub sender: Address,
    pub token: Address,
    pub amount_in: U256,
    pub amount_out: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// Sell event - when someone sells tokens for MON
#[derive(Debug, Clone)]
pub struct SellEvent {
    pub sender: Address,
    pub token: Address,
    pub amount_in: U256,
    pub amount_out: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// Sync event - when pool reserves are updated
#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub token: Address,
    pub real_mon_reserve: U256,
    pub real_token_reserve: U256,
    pub virtual_mon_reserve: U256,
    pub virtual_token_reserve: U256,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// Lock event - when token trading is locked
#[derive(Debug, Clone)]
pub struct LockEvent {
    pub token: Address,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// Listed event - when token is listed on Uniswap
#[derive(Debug, Clone)]
pub struct ListedEvent {
    pub token: Address,
    pub pool: Address,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

/// Unified event type for all bonding curve events
#[derive(Debug, Clone)]
pub enum BondingCurveEvent {
    Create(CreateEvent),
    Buy(BuyEvent),
    Sell(SellEvent),
    Sync(SyncEvent),
    Lock(LockEvent),
    Listed(ListedEvent),
}

impl BondingCurveEvent {
    pub fn token(&self) -> Address {
        match self {
            BondingCurveEvent::Create(e) => e.token,
            BondingCurveEvent::Buy(e) => e.token,
            BondingCurveEvent::Sell(e) => e.token,
            BondingCurveEvent::Sync(e) => e.token,
            BondingCurveEvent::Lock(e) => e.token,
            BondingCurveEvent::Listed(e) => e.token,
        }
    }

    pub fn event_type(&self) -> EventType {
        match self {
            BondingCurveEvent::Create(_) => EventType::Create,
            BondingCurveEvent::Buy(_) => EventType::Buy,
            BondingCurveEvent::Sell(_) => EventType::Sell,
            BondingCurveEvent::Sync(_) => EventType::Sync,
            BondingCurveEvent::Lock(_) => EventType::Lock,
            BondingCurveEvent::Listed(_) => EventType::Listed,
        }
    }

    pub fn block_number(&self) -> u64 {
        match self {
            BondingCurveEvent::Create(e) => e.block_number,
            BondingCurveEvent::Buy(e) => e.block_number,
            BondingCurveEvent::Sell(e) => e.block_number,
            BondingCurveEvent::Sync(e) => e.block_number,
            BondingCurveEvent::Lock(e) => e.block_number,
            BondingCurveEvent::Listed(e) => e.block_number,
        }
    }

    pub fn transaction_index(&self) -> u64 {
        match self {
            BondingCurveEvent::Create(e) => e.transaction_index,
            BondingCurveEvent::Buy(e) => e.transaction_index,
            BondingCurveEvent::Sell(e) => e.transaction_index,
            BondingCurveEvent::Sync(e) => e.transaction_index,
            BondingCurveEvent::Lock(e) => e.transaction_index,
            BondingCurveEvent::Listed(e) => e.transaction_index,
        }
    }

    pub fn log_index(&self) -> u64 {
        match self {
            BondingCurveEvent::Create(e) => e.log_index,
            BondingCurveEvent::Buy(e) => e.log_index,
            BondingCurveEvent::Sell(e) => e.log_index,
            BondingCurveEvent::Sync(e) => e.log_index,
            BondingCurveEvent::Lock(e) => e.log_index,
            BondingCurveEvent::Listed(e) => e.log_index,
        }
    }
}

/// Decode a log into a BondingCurveEvent
pub fn decode_bonding_curve_event(log: Log) -> Result<BondingCurveEvent> {
    let topic0 = log
        .topics()
        .first()
        .ok_or_else(|| anyhow::anyhow!("No topic0 found"))?;

    if *topic0 == IBondingCurve::CurveCreate::SIGNATURE_HASH {
        let IBondingCurve::CurveCreate {
            creator,
            token,
            pool,
            name,
            symbol,
            tokenURI,
            virtualMon,
            virtualToken,
            targetTokenAmount,
        } = log.log_decode()?.inner.data;

        Ok(BondingCurveEvent::Create(CreateEvent {
            creator,
            token,
            pool,
            name,
            symbol,
            token_uri: tokenURI,
            virtual_mon: virtualMon,
            virtual_token: virtualToken,
            target_token_amount: targetTokenAmount,
            block_number: log.block_number.unwrap_or(0),
            transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
            transaction_index: log.transaction_index.unwrap_or(0),
            log_index: log.log_index.unwrap_or(0),
        }))
    } else if *topic0 == IBondingCurve::CurveBuy::SIGNATURE_HASH {
        let IBondingCurve::CurveBuy {
            sender,
            token,
            amountIn,
            amountOut,
        } = log.log_decode()?.inner.data;

        Ok(BondingCurveEvent::Buy(BuyEvent {
            sender,
            token,
            amount_in: amountIn,
            amount_out: amountOut,
            block_number: log.block_number.unwrap_or(0),
            transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
            transaction_index: log.transaction_index.unwrap_or(0),
            log_index: log.log_index.unwrap_or(0),
        }))
    } else if *topic0 == IBondingCurve::CurveSell::SIGNATURE_HASH {
        let IBondingCurve::CurveSell {
            sender,
            token,
            amountIn,
            amountOut,
        } = log.log_decode()?.inner.data;

        Ok(BondingCurveEvent::Sell(SellEvent {
            sender,
            token,
            amount_in: amountIn,
            amount_out: amountOut,
            block_number: log.block_number.unwrap_or(0),
            transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
            transaction_index: log.transaction_index.unwrap_or(0),
            log_index: log.log_index.unwrap_or(0),
        }))
    } else if *topic0 == IBondingCurve::CurveSync::SIGNATURE_HASH {
        let IBondingCurve::CurveSync {
            token,
            realMonReserve,
            realTokenReserve,
            virtualMonReserve,
            virtualTokenReserve,
        } = log.log_decode()?.inner.data;

        Ok(BondingCurveEvent::Sync(SyncEvent {
            token,
            real_mon_reserve: realMonReserve,
            real_token_reserve: realTokenReserve,
            virtual_mon_reserve: virtualMonReserve,
            virtual_token_reserve: virtualTokenReserve,
            block_number: log.block_number.unwrap_or(0),
            transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
            transaction_index: log.transaction_index.unwrap_or(0),
            log_index: log.log_index.unwrap_or(0),
        }))
    } else if *topic0 == IBondingCurve::CurveTokenLocked::SIGNATURE_HASH {
        let IBondingCurve::CurveTokenLocked { token } = log.log_decode()?.inner.data;

        Ok(BondingCurveEvent::Lock(LockEvent {
            token,
            block_number: log.block_number.unwrap_or(0),
            transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
            transaction_index: log.transaction_index.unwrap_or(0),
            log_index: log.log_index.unwrap_or(0),
        }))
    } else if *topic0 == IBondingCurve::CurveTokenListed::SIGNATURE_HASH {
        let IBondingCurve::CurveTokenListed { token, pool } = log.log_decode()?.inner.data;

        Ok(BondingCurveEvent::Listed(ListedEvent {
            token,
            pool,
            block_number: log.block_number.unwrap_or(0),
            transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
            transaction_index: log.transaction_index.unwrap_or(0),
            log_index: log.log_index.unwrap_or(0),
        }))
    } else {
        Err(anyhow::anyhow!("Unknown event signature: {:?}", topic0))
    }
}

// Export event signature constants for convenience
pub const CURVE_CREATE_SIGNATURE: B256 = IBondingCurve::CurveCreate::SIGNATURE_HASH;
pub const CURVE_BUY_SIGNATURE: B256 = IBondingCurve::CurveBuy::SIGNATURE_HASH;
pub const CURVE_SELL_SIGNATURE: B256 = IBondingCurve::CurveSell::SIGNATURE_HASH;
pub const CURVE_SYNC_SIGNATURE: B256 = IBondingCurve::CurveSync::SIGNATURE_HASH;
pub const CURVE_TOKEN_LOCKED_SIGNATURE: B256 = IBondingCurve::CurveTokenLocked::SIGNATURE_HASH;
pub const CURVE_TOKEN_LISTED_SIGNATURE: B256 = IBondingCurve::CurveTokenListed::SIGNATURE_HASH;
