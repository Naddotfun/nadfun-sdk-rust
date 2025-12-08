use crate::types::*;
use alloy::{
    primitives::{Address, B256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

// Load ABI from JSON file
sol!(
    #[sol(rpc)]
    IDexRouter,
    "abi/IDexRouter.json"
);

pub struct DexRouter<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> DexRouter<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    // Note: get_amount_out and get_amount_in are now handled by LensContract
    // for better gas efficiency and unified interface

    pub async fn buy(&self, params: BuyParams) -> Result<B256> {
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

        // EIP-1559 gas pricing takes priority over legacy gas_price
        if let Some(gas_pricing) = &params.gas_pricing {
            match gas_pricing {
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
        } else if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell(&self, params: crate::types::SellParams) -> Result<B256> {
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

        // EIP-1559 gas pricing takes priority over legacy gas_price
        if let Some(gas_pricing) = &params.gas_pricing {
            match gas_pricing {
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
        } else if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell_permit(
        &self,
        params: crate::types::SellPermitParams,
    ) -> Result<B256> {
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

        // EIP-1559 gas pricing takes priority over legacy gas_price
        if let Some(gas_pricing) = &params.gas_pricing {
            match gas_pricing {
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
        } else if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_buy(
        &self,
        params: crate::types::ExactOutBuyParams,
    ) -> Result<B256> {
        let contract = IDexRouter::new(self.address, self.provider.as_ref());

        let router_params = IDexRouter::ExactOutBuyParams {
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

        // EIP-1559 gas pricing takes priority over legacy gas_price
        if let Some(gas_pricing) = &params.gas_pricing {
            match gas_pricing {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    tx_builder = tx_builder.gas_price((*gas_price).into());
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
        } else if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price.into());
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_sell(
        &self,
        params: crate::types::ExactOutSellParams,
    ) -> Result<B256> {
        let contract = IDexRouter::new(self.address, self.provider.as_ref());

        let router_params = IDexRouter::ExactOutSellParams {
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

        // EIP-1559 gas pricing takes priority over legacy gas_price
        if let Some(gas_pricing) = &params.gas_pricing {
            match gas_pricing {
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
        } else if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
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
        let contract = IDexRouter::new(self.address, self.provider.as_ref());

        let router_params = IDexRouter::ExactOutSellPermitParams {
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

        // EIP-1559 gas pricing takes priority over legacy gas_price
        if let Some(gas_pricing) = &params.gas_pricing {
            match gas_pricing {
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
        } else if let Some(gas_price) = params.gas_price {
            tx_builder = tx_builder.gas_price(gas_price);
        }

        if let Some(nonce) = params.nonce {
            tx_builder = tx_builder.nonce(nonce);
        }

        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }
}
