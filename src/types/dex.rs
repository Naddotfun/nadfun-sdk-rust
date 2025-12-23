//! DEX pool related types
//!
//! Contains all DEX (Capricorn CL) event types and pool metadata helpers.

use alloy::{
    primitives::{Address, B256, I256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
};
use anyhow::Result;
use std::collections::HashMap;

// Capricorn CL Pool contract definition
sol! {
    #[sol(rpc)]
    ICapricornCLPool,
    "abi/ICapricornCLPool.json"
}

/// DEX Swap event with Nad.fun-specific analysis methods
#[derive(Debug, Clone)]
pub struct SwapEvent {
    pub sender: Address,
    pub recipient: Address,
    pub amount0: I256,
    pub amount1: I256,
    pub sqrt_price_x96: U256, // uint160 fits in U256
    pub liquidity: u128,
    pub tick: i32, // int24 fits in i32
    pub pool_address: Address,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

impl SwapEvent {
    /// Get WMON amount from the swap (needs pool metadata to determine which is WMON)
    /// Returns positive for WMON received, negative for WMON spent
    pub fn wmon_amount(&self, wmon_is_token0: bool) -> I256 {
        if wmon_is_token0 {
            self.amount0
        } else {
            self.amount1
        }
    }

    /// Get token amount from the swap (needs pool metadata to determine which is token)
    /// Returns positive for token received, negative for token spent
    pub fn token_amount(&self, wmon_is_token0: bool) -> I256 {
        if wmon_is_token0 {
            self.amount1
        } else {
            self.amount0
        }
    }

    /// Get absolute WMON volume
    pub fn abs_wmon_amount(&self, wmon_is_token0: bool) -> U256 {
        let amount = self.wmon_amount(wmon_is_token0);
        U256::from(amount.abs())
    }

    /// Get absolute token volume  
    pub fn abs_token_amount(&self, wmon_is_token0: bool) -> U256 {
        let amount = self.token_amount(wmon_is_token0);
        U256::from(amount.abs())
    }

    /// Check if this is a token buy (WMON spent, token received)
    pub fn is_token_buy(&self, wmon_is_token0: bool) -> bool {
        let wmon_amount = self.wmon_amount(wmon_is_token0);
        let token_amount = self.token_amount(wmon_is_token0);
        wmon_amount < I256::ZERO && token_amount > I256::ZERO
    }

    /// Check if this is a token sell (token spent, WMON received)
    pub fn is_token_sell(&self, wmon_is_token0: bool) -> bool {
        let wmon_amount = self.wmon_amount(wmon_is_token0);
        let token_amount = self.token_amount(wmon_is_token0);
        wmon_amount > I256::ZERO && token_amount < I256::ZERO
    }

    /// Get trade direction as string
    pub fn trade_direction(&self, wmon_is_token0: bool) -> &'static str {
        if self.is_token_buy(wmon_is_token0) {
            "BUY"
        } else if self.is_token_sell(wmon_is_token0) {
            "SELL"
        } else {
            "UNKNOWN"
        }
    }
}

/// Pool metadata helper for determining which token is WMON
pub struct PoolMetadata {
    /// Cache of pool address -> whether WMON is token0
    wmon_is_token0_cache: HashMap<Address, bool>,
}

impl Default for PoolMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolMetadata {
    pub fn new() -> Self {
        Self {
            wmon_is_token0_cache: HashMap::new(),
        }
    }

    /// Check if WMON is token0 in the given pool
    pub async fn is_wmon_token0<P: Provider + Clone>(
        &mut self,
        provider: &P,
        pool_address: Address,
    ) -> Result<bool> {
        // Check cache first
        if let Some(&cached) = self.wmon_is_token0_cache.get(&pool_address) {
            return Ok(cached);
        }

        // Query the pool contract
        let pool = ICapricornCLPool::new(pool_address, provider);
        let token0 = pool.token0().call().await?;
        let wmon_address: Address = crate::constants::WMON.parse()?;

        let is_wmon_token0 = token0 == wmon_address;

        // Cache the result
        self.wmon_is_token0_cache
            .insert(pool_address, is_wmon_token0);

        Ok(is_wmon_token0)
    }

    /// Get token addresses for a pool (token0, token1)
    pub async fn get_pool_tokens<P: Provider + Clone>(
        &self,
        provider: &P,
        pool_address: Address,
    ) -> Result<(Address, Address)> {
        let pool = ICapricornCLPool::new(pool_address, provider);
        let token0 = pool.token0().call().await?;
        let token1 = pool.token1().call().await?;
        Ok((token0, token1))
    }
}

/// Decode a log into a SwapEvent
pub fn decode_swap_event(log: Log) -> Result<SwapEvent> {
    let pool_address = log.address();

    // Verify this is a Swap event
    let topic0 = log
        .topics()
        .first()
        .ok_or_else(|| anyhow::anyhow!("No topic0 found"))?;

    if *topic0 != ICapricornCLPool::Swap::SIGNATURE_HASH {
        return Err(anyhow::anyhow!("Not a Swap event"));
    }

    let ICapricornCLPool::Swap {
        sender,
        recipient,
        amount0,
        amount1,
        sqrtPriceX96,
        liquidity,
        tick,
    } = log.log_decode()?.inner.data;

    Ok(SwapEvent {
        sender,
        recipient,
        amount0,
        amount1,
        sqrt_price_x96: U256::from(sqrtPriceX96),
        liquidity,
        tick: tick.try_into().unwrap_or(0), // int24 -> i32
        pool_address,
        block_number: log.block_number.unwrap_or(0),
        transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
        transaction_index: log.transaction_index.unwrap_or(0),
        log_index: log.log_index.unwrap_or(0),
    })
}

// Export swap event signature for convenience
pub const SWAP_SIGNATURE: B256 = ICapricornCLPool::Swap::SIGNATURE_HASH;

/// DEX Mint event (liquidity added)
#[derive(Debug, Clone)]
pub struct MintEvent {
    pub sender: Address,
    pub owner: Address,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub amount: u128,
    pub amount0: U256,
    pub amount1: U256,
    pub pool_address: Address,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

impl MintEvent {
    /// Get WMON amount added as liquidity
    pub fn wmon_amount(&self, wmon_is_token0: bool) -> U256 {
        if wmon_is_token0 {
            self.amount0
        } else {
            self.amount1
        }
    }

    /// Get token amount added as liquidity
    pub fn token_amount(&self, wmon_is_token0: bool) -> U256 {
        if wmon_is_token0 {
            self.amount1
        } else {
            self.amount0
        }
    }
}

/// DEX Burn event (liquidity removed)
#[derive(Debug, Clone)]
pub struct BurnEvent {
    pub owner: Address,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub amount: u128,
    pub amount0: U256,
    pub amount1: U256,
    pub pool_address: Address,
    pub block_number: u64,
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub log_index: u64,
}

impl BurnEvent {
    /// Get WMON amount removed from liquidity
    pub fn wmon_amount(&self, wmon_is_token0: bool) -> U256 {
        if wmon_is_token0 {
            self.amount0
        } else {
            self.amount1
        }
    }

    /// Get token amount removed from liquidity
    pub fn token_amount(&self, wmon_is_token0: bool) -> U256 {
        if wmon_is_token0 {
            self.amount1
        } else {
            self.amount0
        }
    }
}

/// Decode a log into a MintEvent
pub fn decode_mint_event(log: Log) -> Result<MintEvent> {
    let pool_address = log.address();

    let topic0 = log
        .topics()
        .first()
        .ok_or_else(|| anyhow::anyhow!("No topic0 found"))?;

    if *topic0 != ICapricornCLPool::Mint::SIGNATURE_HASH {
        return Err(anyhow::anyhow!("Not a Mint event"));
    }

    let ICapricornCLPool::Mint {
        sender,
        owner,
        tickLower,
        tickUpper,
        amount,
        amount0,
        amount1,
    } = log.log_decode()?.inner.data;

    Ok(MintEvent {
        sender,
        owner,
        tick_lower: tickLower.try_into().unwrap_or(0),
        tick_upper: tickUpper.try_into().unwrap_or(0),
        amount,
        amount0,
        amount1,
        pool_address,
        block_number: log.block_number.unwrap_or(0),
        transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
        transaction_index: log.transaction_index.unwrap_or(0),
        log_index: log.log_index.unwrap_or(0),
    })
}

/// Decode a log into a BurnEvent
pub fn decode_burn_event(log: Log) -> Result<BurnEvent> {
    let pool_address = log.address();

    let topic0 = log
        .topics()
        .first()
        .ok_or_else(|| anyhow::anyhow!("No topic0 found"))?;

    if *topic0 != ICapricornCLPool::Burn::SIGNATURE_HASH {
        return Err(anyhow::anyhow!("Not a Burn event"));
    }

    let ICapricornCLPool::Burn {
        owner,
        tickLower,
        tickUpper,
        amount,
        amount0,
        amount1,
    } = log.log_decode()?.inner.data;

    Ok(BurnEvent {
        owner,
        tick_lower: tickLower.try_into().unwrap_or(0),
        tick_upper: tickUpper.try_into().unwrap_or(0),
        amount,
        amount0,
        amount1,
        pool_address,
        block_number: log.block_number.unwrap_or(0),
        transaction_hash: log.transaction_hash.unwrap_or(B256::ZERO),
        transaction_index: log.transaction_index.unwrap_or(0),
        log_index: log.log_index.unwrap_or(0),
    })
}

// Export event signatures
pub const MINT_SIGNATURE: B256 = ICapricornCLPool::Mint::SIGNATURE_HASH;
pub const BURN_SIGNATURE: B256 = ICapricornCLPool::Burn::SIGNATURE_HASH;

/// Unified DEX event enum
#[derive(Debug, Clone)]
pub enum DexEvent {
    Swap(SwapEvent),
    Mint(MintEvent),
    Burn(BurnEvent),
}

impl DexEvent {
    /// Get pool address from any event type
    pub fn pool_address(&self) -> Address {
        match self {
            DexEvent::Swap(e) => e.pool_address,
            DexEvent::Mint(e) => e.pool_address,
            DexEvent::Burn(e) => e.pool_address,
        }
    }

    /// Get block number from any event type
    pub fn block_number(&self) -> u64 {
        match self {
            DexEvent::Swap(e) => e.block_number,
            DexEvent::Mint(e) => e.block_number,
            DexEvent::Burn(e) => e.block_number,
        }
    }

    /// Get transaction hash from any event type
    pub fn transaction_hash(&self) -> B256 {
        match self {
            DexEvent::Swap(e) => e.transaction_hash,
            DexEvent::Mint(e) => e.transaction_hash,
            DexEvent::Burn(e) => e.transaction_hash,
        }
    }

    /// Get event type as string
    pub fn event_type(&self) -> &'static str {
        match self {
            DexEvent::Swap(_) => "Swap",
            DexEvent::Mint(_) => "Mint",
            DexEvent::Burn(_) => "Burn",
        }
    }
}

/// Decode a log into a DexEvent (Swap, Mint, or Burn)
pub fn decode_dex_event(log: Log) -> Result<DexEvent> {
    let topic0 = log
        .topics()
        .first()
        .ok_or_else(|| anyhow::anyhow!("No topic0 found"))?;

    match *topic0 {
        SWAP_SIGNATURE => Ok(DexEvent::Swap(decode_swap_event(log)?)),
        MINT_SIGNATURE => Ok(DexEvent::Mint(decode_mint_event(log)?)),
        BURN_SIGNATURE => Ok(DexEvent::Burn(decode_burn_event(log)?)),
        _ => Err(anyhow::anyhow!("Unknown DEX event: {:?}", topic0)),
    }
}
