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
    interface IBondingCurveRouter {
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
        function availableBuyTokens(address token) external view returns (uint256 availableBuyToken, uint256 requiredMonAmount);
    }
}

sol! {
    #[sol(rpc)]
    interface IBondingCurve {
        function isListed(address token) external view returns (bool);
        function isLocked(address token) external view returns (bool);
        function curves(address token) external view returns (
            uint256 realMonReserve,
            uint256 realTokenReserve,
            uint256 virtualMonReserve,
            uint256 virtualTokenReserve,
            uint256 k,
            uint256 targetTokenAmount,
            uint256 initVirtualMonReserve,
            uint256 initVirtualTokenReserve
        );

        // Events
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

        event CurveTokenLocked(
            address indexed token
        );

        event CurveTokenListed(
            address indexed token,
            address indexed pool
        );
    }
}

pub struct BondingCurveRouter<P> {
    pub address: Address,
    pub bonding_curve_address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> BondingCurveRouter<P> {
    pub fn new(address: Address, bonding_curve_address: Address, provider: Arc<P>) -> Self {
        Self {
            address,
            bonding_curve_address,
            provider,
        }
    }

    pub async fn is_listed(&self, token: Address) -> Result<bool> {
        let contract = IBondingCurve::new(self.bonding_curve_address, self.provider.as_ref());
        let result = contract.isListed(token).call().await?;
        Ok(result)
    }

    pub async fn is_locked(&self, token: Address) -> Result<bool> {
        let contract = IBondingCurve::new(self.bonding_curve_address, self.provider.as_ref());
        let result = contract.isLocked(token).call().await?;
        Ok(result)
    }

    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());
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
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());
        let result = contract
            .getAmountIn(token, amount_out, is_buy)
            .call()
            .await?;
        Ok(result)
    }

    pub async fn buy(&self, params: BuyParams) -> Result<TransactionResult> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let router_params = IBondingCurveRouter::BuyParams {
            amountOutMin: params.amount_out_min,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.buy(router_params).value(params.amount_in);

        if let Some(gas_limit) = params.gas_limit {
            tx_builder = tx_builder.gas(gas_limit.into());
        }

        if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price.into());
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
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());
        let router_params = IBondingCurveRouter::SellParams {
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
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let router_params = IBondingCurveRouter::SellPermitParams {
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

    pub async fn available_buy_tokens(&self, token: Address) -> Result<(U256, U256)> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());
        let result = contract.availableBuyTokens(token).call().await?;
        Ok((result.availableBuyToken, result.requiredMonAmount))
    }

    pub async fn get_curve_state(&self, token: Address) -> Result<CurveState> {
        let contract = IBondingCurve::new(self.bonding_curve_address, self.provider.as_ref());
        let result = contract.curves(token).call().await?;

        Ok(CurveState {
            real_mon_reserve: result.realMonReserve,
            real_token_reserve: result.realTokenReserve,
            virtual_mon_reserve: result.virtualMonReserve,
            virtual_token_reserve: result.virtualTokenReserve,
            k: result.k,
            target_token_amount: result.targetTokenAmount,
            init_virtual_mon_reserve: result.initVirtualMonReserve,
            init_virtual_token_reserve: result.initVirtualTokenReserve,
        })
    }
}
