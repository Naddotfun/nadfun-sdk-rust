use crate::types::*;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
    sol_types::SolEvent,
};
use anyhow::Result;
use std::sync::Arc;

// Load ABIs from JSON files
sol!(
    #[sol(rpc)]
    IBondingCurveRouter,
    "abi/IBondingCurveRouter.json"
);

sol!(
    #[sol(rpc)]
    IBondingCurve,
    "abi/IBondingCurve.json"
);

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

    // Note: is_locked, is_graduated, get_amount_out, get_amount_in, and available_buy_tokens
    // are now handled by LensContract for better gas efficiency and unified interface

    /// Get deploy fee amount from bonding curve contract
    pub async fn get_deploy_fee(&self) -> Result<U256> {
        let contract = IBondingCurve::new(self.bonding_curve_address, self.provider.as_ref());
        let fee_config = contract.feeConfig().call().await?;
        Ok(fee_config.deployFeeAmount)
    }

    /// Create a new token
    pub async fn create(
        &self,
        name: String,
        symbol: String,
        token_uri: String,
        amount_out: U256,
        salt: [u8; 32],
        action_id: u8,
        value: U256,
        gas_limit: Option<u64>,
        gas_price: Option<u128>,
        nonce: Option<u64>,
    ) -> Result<(Address, TransactionResult)> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let params = IBondingCurveRouter::TokenCreationParams {
            name,
            symbol,
            tokenURI: token_uri,
            amountOut: amount_out,
            salt: salt.into(),
            actionId: action_id,
        };

        let mut tx_builder = contract.create(params).value(value);

        if let Some(gas_limit) = gas_limit {
            tx_builder = tx_builder.gas(gas_limit.into());
        }

        if let Some(gas_price) = gas_price {
            tx_builder = tx_builder.gas_price(gas_price.into());
        }

        if let Some(nonce) = nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        let receipt = tx.get_receipt().await?;

        // Parse the CurveCreate event to get the token address
        let mut token_address = Address::ZERO;
        for log in receipt.inner.logs() {
            // Convert RPC log to primitives log
            let primitive_log = alloy::primitives::Log {
                address: log.address(),
                data: log.data().clone(),
            };
            if let Ok(decoded) = IBondingCurve::CurveCreate::decode_log(&primitive_log) {
                token_address = decoded.data.token;
                break;
            }
        }

        Ok((
            token_address,
            TransactionResult {
                transaction_hash: receipt.transaction_hash,
                block_number: receipt.block_number,
                gas_used: Some(U256::from(receipt.gas_used)),
                status: receipt.status(),
                logs: receipt.logs().to_vec(),
            },
        ))
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

    pub async fn exact_out_buy(
        &self,
        params: crate::types::ExactOutBuyParams,
    ) -> Result<TransactionResult> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let router_params = IBondingCurveRouter::ExactOutBuyParams {
            amountInMax: params.amount_in_max,
            amountOut: params.amount_out,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.exactOutBuy(router_params).value(params.amount_in_max);

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

    pub async fn exact_out_sell(
        &self,
        params: crate::types::ExactOutSellParams,
    ) -> Result<TransactionResult> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let router_params = IBondingCurveRouter::ExactOutSellParams {
            amountInMax: params.amount_in_max,
            amountOut: params.amount_out,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.exactOutSell(router_params);

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

    pub async fn exact_out_sell_permit(
        &self,
        params: crate::types::ExactOutSellPermitParams,
    ) -> Result<TransactionResult> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let router_params = IBondingCurveRouter::ExactOutSellPermitParams {
            amountInMax: params.amount_in_max,
            amountOut: params.amount_out,
            amountAllowance: params.amount_allowance,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
            v: params.v,
            r: params.r,
            s: params.s,
        };

        let mut tx_builder = contract.exactOutSellPermit(router_params);

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
