use crate::types::*;
use alloy::{
    primitives::{Address, B256, U256},
    providers::Provider,
    sol,
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

    /// Create a new token - returns tx_hash immediately without waiting for receipt
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        name: String,
        symbol: String,
        token_uri: String,
        amount_out: U256,
        salt: [u8; 32],
        action_id: crate::types::ActionId,
        value: U256,
        gas_limit: Option<u64>,
        gas_price: Option<GasPricing>,
        nonce: Option<u64>,
    ) -> Result<B256> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let params = IBondingCurveRouter::TokenCreationParams {
            name,
            symbol,
            tokenURI: token_uri,
            amountOut: amount_out,
            salt: salt.into(),
            actionId: action_id.as_u8(),
        };

        let mut tx_builder = contract.create(params).value(value);

        if let Some(gas_limit) = gas_limit {
            tx_builder = tx_builder.gas(gas_limit);
        }

        if let Some(gas_price) = &gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    tx_builder = tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn buy(&self, params: BuyParams) -> Result<B256> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let router_params = IBondingCurveRouter::BuyParams {
            amountOutMin: params.amount_out_min,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.buy(router_params).value(params.amount_in);

        if let Some(gas_limit) = params.gas_limit {
            tx_builder = tx_builder.gas(gas_limit);
        }

        if let Some(gas_price) = &params.gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    tx_builder = tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell(&self, params: crate::types::SellParams) -> Result<B256> {
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

        if let Some(gas_price) = &params.gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    tx_builder = tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell_permit(&self, params: crate::types::SellPermitParams) -> Result<B256> {
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

        if let Some(gas_price) = &params.gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    tx_builder = tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_buy(&self, params: crate::types::ExactOutBuyParams) -> Result<B256> {
        let contract = IBondingCurveRouter::new(self.address, self.provider.as_ref());

        let router_params = IBondingCurveRouter::ExactOutBuyParams {
            amountInMax: params.amount_in_max,
            amountOut: params.amount_out,
            token: params.token,
            to: params.to,
            deadline: params.deadline,
        };

        let mut tx_builder = contract
            .exactOutBuy(router_params)
            .value(params.amount_in_max);

        if let Some(gas_limit) = params.gas_limit {
            tx_builder = tx_builder.gas(gas_limit);
        }

        if let Some(gas_price) = &params.gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    tx_builder = tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_sell(&self, params: crate::types::ExactOutSellParams) -> Result<B256> {
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

        if let Some(gas_price) = &params.gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    tx_builder = tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_sell_permit(
        &self,
        params: crate::types::ExactOutSellPermitParams,
    ) -> Result<B256> {
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

        if let Some(gas_price) = &params.gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    tx_builder = tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }
}
