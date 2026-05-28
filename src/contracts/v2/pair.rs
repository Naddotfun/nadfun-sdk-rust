//! NadFun v2 pair binding (Uniswap V2 fork with pair-level fee).
//!
//! Wraps `NadFunPair.sol` view methods for quoting, reserves, and ERC20-permit
//! metadata. The pair also doubles as an ERC20 LP token (mint/burn/transfer)
//! and an EIP-2612 permit-enabled contract; only the read surface needed by the
//! SDK is exposed here.

// alloy `sol!` expansion exceeds clippy's default too_many_arguments threshold;
// dead_code fires on view methods reserved for advanced callers (NadFunPair is
// exposed via `contracts::v2::*` but no internal SDK code uses it yet).
#![allow(clippy::too_many_arguments, dead_code)]

use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol!(
    #[sol(rpc)]
    INadFunPair,
    "abi/v2/NadFunPair.json"
);

/// Reserves snapshot of a NadFunPair.
#[derive(Debug, Clone, Copy)]
pub struct PairReserves {
    pub reserve0: u128,
    pub reserve1: u128,
    pub block_timestamp_last: u32,
}

pub struct NadFunPair<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> NadFunPair<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Quote: how much of `tokenIn` worth `amountIn` you get out the other side.
    /// Fee-aware (LP fee + pair-level protocol fee).
    pub async fn get_amount_out(&self, token_in: Address, amount_in: U256) -> Result<U256> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let amount_out = contract.getAmountOut(token_in, amount_in).call().await?;
        Ok(amount_out)
    }

    /// Quote: how much of the input side you need to receive `amountOut` of `tokenOut`.
    pub async fn get_amount_in(&self, token_out: Address, amount_out: U256) -> Result<U256> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let amount_in = contract.getAmountIn(token_out, amount_out).call().await?;
        Ok(amount_in)
    }

    /// Pair reserves (token0, token1, last-update timestamp).
    pub async fn get_reserves(&self) -> Result<PairReserves> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let result = contract.getReserves().call().await?;
        Ok(PairReserves {
            reserve0: result.reserve0.to::<u128>(),
            reserve1: result.reserve1.to::<u128>(),
            block_timestamp_last: result.blockTimestampLast,
        })
    }

    /// Address of token0 (lexicographically lower of the pair).
    pub async fn token0(&self) -> Result<Address> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let token = contract.token0().call().await?;
        Ok(token)
    }

    /// Address of token1.
    pub async fn token1(&self) -> Result<Address> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let token = contract.token1().call().await?;
        Ok(token)
    }

    /// Address of the deploying factory.
    pub async fn factory(&self) -> Result<Address> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let factory = contract.factory().call().await?;
        Ok(factory)
    }

    /// Whether the pair is locked (no trading / liquidity changes allowed).
    pub async fn is_locked(&self) -> Result<bool> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let locked = contract.isLocked().call().await?;
        Ok(locked)
    }

    /// LP fee rate in basis points (constant per-pair).
    pub async fn lp_fee_rate(&self) -> Result<U256> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let rate = contract.LP_FEE_RATE().call().await?;
        Ok(rate)
    }

    /// LP token total supply.
    pub async fn total_supply(&self) -> Result<U256> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let supply = contract.totalSupply().call().await?;
        Ok(supply)
    }

    /// LP token balance of `account`.
    pub async fn balance_of(&self, account: Address) -> Result<U256> {
        let contract = INadFunPair::new(self.address, self.provider.as_ref());
        let bal = contract.balanceOf(account).call().await?;
        Ok(bal)
    }
}
