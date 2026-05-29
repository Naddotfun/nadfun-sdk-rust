//! `ProtocolManager` v2 binding (read-only slice used by the SDK).
//!
//! The protocol manager holds per-quote-token protocol config. The SDK reads
//! `deployFee(quoteToken)` to fund v2 token creation correctly — the on-chain
//! `NadFunRouter::create` / `createWithNative` requires
//! `msg.value >= deployFee(quoteToken) + buyQuoteAmount`.

use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface IProtocolManagerV2 {
        function deployFee(address quoteToken) external view returns (uint256);
    }
}

/// SDK wrapper around the v2 `ProtocolManager` view surface.
pub struct ProtocolManagerV2<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> ProtocolManagerV2<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// One-time deploy fee charged when creating a token quoted in
    /// `quote_token`. Denominated in the quote token.
    pub async fn deploy_fee(&self, quote_token: Address) -> Result<U256> {
        let contract = IProtocolManagerV2::new(self.address, self.provider.as_ref());
        Ok(contract.deployFee(quote_token).call().await?)
    }
}
