use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface ILensContract {
        function getAmountIn(address token, uint256 amountOut, bool isBuy) external returns (address, uint256);
        function getAmountOut(address token, uint256 amountIn, bool isBuy) external returns (address, uint256);
    }
}

pub struct LensContract<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> LensContract<P> {
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
        Ok((result._0, result._1))
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
        Ok((result._0, result._1))
    }
}
