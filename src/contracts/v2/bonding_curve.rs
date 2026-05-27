//! NadFun v2 bonding curve binding.
//!
//! Wraps `BondingCurve.sol` view methods. Trading happens through
//! `NadFunRouter` (which calls into this contract); the SDK uses this binding
//! only for quoting, curve state inspection, and event decoding.

use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol!(
    #[sol(rpc)]
    IBondingCurveV2,
    "abi/v2/BondingCurve.json"
);

pub struct BondingCurveV2<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> BondingCurveV2<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Quote: expected output for `amountIn` of input. `is_buy = true` quotes
    /// quote→token; `false` quotes token→quote.
    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let amount_out = contract
            .getAmountOut(token, amount_in, is_buy)
            .call()
            .await?;
        Ok(amount_out)
    }

    /// Inverse quote: required input to receive `amountOut` of output.
    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let amount_in = contract
            .getAmountIn(token, amount_out, is_buy)
            .call()
            .await?;
        Ok(amount_in)
    }

    /// Sniping penalty rate (BPS) for a buy at the current block, used to
    /// preview the anti-sniping fee. Driven by `block.number - createdAtBlock`
    /// against the ProtocolManager's lookup table.
    pub async fn get_sniping_penalty(&self, token: Address) -> Result<U256> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let bps = contract.getSnipingPenalty(token).call().await?;
        Ok(bps)
    }

    /// Quote token address for `token` (e.g. WMON or USDT depending on config).
    pub async fn get_quote_token(&self, token: Address) -> Result<Address> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let quote = contract.getQuoteToken(token).call().await?;
        Ok(quote)
    }

    /// Whether the protocol is paused (no trading allowed).
    pub async fn is_halted(&self) -> Result<bool> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let halted = contract.isHalted().call().await?;
        Ok(halted)
    }

    /// Contract version (matches the `CurveVersion` enum on-chain).
    pub async fn version(&self) -> Result<u8> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let v = contract.VERSION().call().await?;
        Ok(v)
    }

    /// Linked CreatorFeeProcessor address (where creator fees are routed).
    pub async fn creator_fee_processor(&self) -> Result<Address> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let addr = contract.creatorFeeProcessor().call().await?;
        Ok(addr)
    }
}
