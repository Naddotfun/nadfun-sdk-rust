use crate::types::*;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface IDexRouter {
        struct BuyParams {
            uint256 amountOutMin;
            address token;
            address to;
            uint256 deadline;
        }

        struct SellParams {
            uint256 amountIn;
            uint256 amountOutMin;
            address token;
            address to;
            uint256 deadline;
        }

        struct SellPermitParams {
            uint256 amountIn;
            uint256 amountOutMin;
            uint256 amountAllowance;
            address token;
            address to;
            uint256 deadline;
            uint8 v;
            bytes32 r;
            bytes32 s;
        }

        function buy(BuyParams memory params) external payable returns (uint256);
        function sell(SellParams memory params) external returns (uint256);
        function sellPermit(SellPermitParams memory params) external returns (uint256);
        function getAmountOut(address token, uint256 amountIn, bool isBuy) external view returns (uint256);
        function getAmountIn(address token, uint256 amountOut, bool isBuy) external view returns (uint256);
    }
}

pub struct DexRouter<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> DexRouter<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = IDexRouter::new(self.address, self.provider.as_ref());
        let result = contract
            .getAmountOut(token, amount_in, is_buy)
            .call()
            .await?;
        Ok(result)
    }

    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = IDexRouter::new(self.address, self.provider.as_ref());
        let result = contract
            .getAmountIn(token, amount_out, is_buy)
            .call()
            .await?;
        Ok(result)
    }

    pub async fn buy(&self, params: BuyParams) -> Result<TransactionResult> {
        let contract = IDexRouter::new(self.address, self.provider.as_ref());

        let router_params = IDexRouter::BuyParams {
            amountOutMin: params.amount_out_min,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.buy(router_params).value(params.amount_in);

        if let Some(gas_limit) = params.gas_limit {
            tx_builder = tx_builder.gas(gas_limit);
        }

        if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;

        let receipt = tx.get_receipt().await?;

        Ok(TransactionResult {
            transaction_hash: receipt.transaction_hash,
            block_number: receipt.block_number,
            gas_used: Some(U256::from(receipt.gas_used)),
            status: receipt.status(),
            logs: receipt.logs().to_vec(),
        })
    }

    pub async fn sell(&self, params: crate::types::SellParams) -> Result<TransactionResult> {
        let contract = IDexRouter::new(self.address, self.provider.as_ref());

        let router_params = IDexRouter::SellParams {
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.sell(router_params);

        if let Some(gas_limit) = params.gas_limit {
            tx_builder = tx_builder.gas(gas_limit);
        }

        if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        let receipt = tx.get_receipt().await?;

        Ok(TransactionResult {
            transaction_hash: receipt.transaction_hash,
            block_number: receipt.block_number,
            gas_used: Some(U256::from(receipt.gas_used)),
            status: receipt.status(),
            logs: receipt.logs().to_vec(),
        })
    }

    pub async fn sell_permit(
        &self,
        params: crate::types::SellPermitParams,
    ) -> Result<TransactionResult> {
        let contract = IDexRouter::new(self.address, self.provider.as_ref());

        let router_params = IDexRouter::SellPermitParams {
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            amountAllowance: params.amount_allowance,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
            v: params.v,
            r: params.r,
            s: params.s,
        };

        let mut tx_builder = contract.sellPermit(router_params);

        if let Some(gas_limit) = params.gas_limit {
            tx_builder = tx_builder.gas(gas_limit);
        }

        if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        let receipt = tx.get_receipt().await?;

        Ok(TransactionResult {
            transaction_hash: receipt.transaction_hash,
            block_number: receipt.block_number,
            gas_used: Some(U256::from(receipt.gas_used)),
            status: receipt.status(),
            logs: receipt.logs().to_vec(),
        })
    }
}
