use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

// Load ABI from JSON file
sol!(
    #[sol(rpc)]
    ILensContract,
    "abi/ILens.json"
);

pub struct Lens<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> Lens<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Get amount in and the router address to use
    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<(Address, U256)> {
        let contract = ILensContract::new(self.address, self.provider.as_ref());
        let result = contract
            .getAmountIn(token, amount_out, is_buy)
            .call()
            .await?;
        Ok((result.router, result.amountIn))
    }

    /// Get amount out and the router address to use
    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<(Address, U256)> {
        let contract = ILensContract::new(self.address, self.provider.as_ref());
        let result = contract
            .getAmountOut(token, amount_in, is_buy)
            .call()
            .await?;
        Ok((result.router, result.amountOut))
    }

    /// Check if token is locked
    pub async fn is_locked(&self, token: Address) -> Result<bool> {
        let contract = ILensContract::new(self.address, self.provider.as_ref());
        let result = contract.isLocked(token).call().await?;
        Ok(result)
    }

    /// Check if token is graduated (listed on DEX)
    pub async fn is_graduated(&self, token: Address) -> Result<bool> {
        let contract = ILensContract::new(self.address, self.provider.as_ref());
        let result = contract.isGraduated(token).call().await?;
        Ok(result)
    }

    /// Get available buy tokens and required MON amount
    pub async fn available_buy_tokens(&self, token: Address) -> Result<(U256, U256)> {
        let contract = ILensContract::new(self.address, self.provider.as_ref());
        let result = contract.availableBuyTokens(token).call().await?;
        Ok((result.availableBuyToken, result.requiredMonAmount))
    }

    /// Get initial buy amount out for token creation
    ///
    /// Calculate how many tokens you'll receive when creating a new token
    /// with a given MON amount (typically used during token creation).
    pub async fn get_initial_buy_amount_out(&self, amount_in: U256) -> Result<U256> {
        let contract = ILensContract::new(self.address, self.provider.as_ref());
        let amount_out = contract.getInitialBuyAmountOut(amount_in).call().await?;
        Ok(amount_out)
    }

    /// Get bonding curve progress percentage
    ///
    /// Returns the progress of the bonding curve towards graduation (0-10000 = 0-100%)
    pub async fn get_progress(&self, token: Address) -> Result<U256> {
        let contract = ILensContract::new(self.address, self.provider.as_ref());
        let progress = contract.getProgress(token).call().await?;
        Ok(progress)
    }
}
