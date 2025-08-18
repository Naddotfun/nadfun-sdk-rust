//! Uniswap V3 related types
//!
//! Contains all Uniswap V3 event types and pool metadata helpers.

use alloy::{
    primitives::{Address, B256, I256, U256},
    providers::Provider,
    rpc::types::Log,
    sol,
    sol_types::SolEvent,
};
use anyhow::Result;
use std::collections::HashMap;

// Uniswap V3 Pool contract definition
sol! {
    #[sol(rpc)]
    contract UniswapV3Pool {
        /// @notice Emitted by the pool for any swaps between token0 and token1
        event Swap(
            address indexed sender,
            address indexed recipient,
            int256 amount0,
            int256 amount1,
            uint160 sqrtPriceX96,
            uint128 liquidity,
            int24 tick
        );

        /// @notice The first of the two tokens of the pool, sorted by address
        /// @return The token contract address
        function token0() external view returns (address);

        /// @notice The second of the two tokens of the pool, sorted by address
        /// @return The token contract address
        function token1() external view returns (address);
    }
}

/// Uniswap V3 Swap event with NADS-specific analysis methods
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
        let pool = UniswapV3Pool::new(pool_address, provider);
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
        let pool = UniswapV3Pool::new(pool_address, provider);
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

    if *topic0 != UniswapV3Pool::Swap::SIGNATURE_HASH {
        return Err(anyhow::anyhow!("Not a Swap event"));
    }

    let UniswapV3Pool::Swap {
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
pub const SWAP_SIGNATURE: B256 = UniswapV3Pool::Swap::SIGNATURE_HASH;
