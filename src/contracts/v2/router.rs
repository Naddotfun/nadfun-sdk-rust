use crate::types::{
    GasPricing, V2BuyParams, V2BuyWithNativeParams, V2BuyWithPermitParams, V2CreateParams,
    V2CreateWithNativeParams, V2ExactOutBuyParams, V2ExactOutBuyWithNativeParams,
    V2ExactOutSellParams, V2ExactOutSellToNativeParams, V2GasEstimationParams, V2SellParams,
    V2SellToNativeParams, V2SellToNativeWithPermitParams, V2SellWithPermitParams,
    V2VaultAllocation,
};
use alloy::{
    primitives::{Address, B256, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol!(
    #[sol(rpc)]
    INadFunRouter,
    "abi/v2/NadFunRouter.json"
);

use IBondingCurve::VaultAllocation;

macro_rules! apply_tx_options {
    ($tx_builder:ident, $params:ident) => {
        if let Some(gas_limit) = $params.gas_limit {
            $tx_builder = $tx_builder.gas(gas_limit);
        }

        if let Some(gas_price) = &$params.gas_price {
            match gas_price {
                GasPricing::Legacy => {}
                GasPricing::LegacyWithPrice { gas_price } => {
                    $tx_builder = $tx_builder.gas_price(*gas_price);
                }
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => {
                    $tx_builder = $tx_builder
                        .max_fee_per_gas(*max_fee_per_gas)
                        .max_priority_fee_per_gas(*max_priority_fee_per_gas);
                }
            }
        }

        if let Some(nonce) = $params.nonce {
            $tx_builder = $tx_builder.nonce(nonce);
        }
    };
}

pub struct NadFunRouter<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> NadFunRouter<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    pub async fn create(&self, params: V2CreateParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::CreateParams {
            name: params.name,
            symbol: params.symbol,
            tokenURI: params.token_uri,
            quoteToken: params.quote_token,
            creatorFeeRate: params.creator_fee_rate,
            vaults: params
                .vaults
                .into_iter()
                .map(|vault| VaultAllocation {
                    vault: vault.vault,
                    bps: vault.bps,
                    setupData: vault.setup_data,
                })
                .collect(),
            salt: params.salt,
            dexType: params.dex_type.as_u8(),
            buyQuoteAmount: params.buy_quote_amount,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.create(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn create_with_native(&self, params: V2CreateWithNativeParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::CreateParams {
            name: params.name,
            symbol: params.symbol,
            tokenURI: params.token_uri,
            quoteToken: Address::ZERO,
            creatorFeeRate: params.creator_fee_rate,
            vaults: params
                .vaults
                .into_iter()
                .map(|vault| VaultAllocation {
                    vault: vault.vault,
                    bps: vault.bps,
                    setupData: vault.setup_data,
                })
                .collect(),
            salt: params.salt,
            dexType: params.dex_type.as_u8(),
            buyQuoteAmount: params.buy_quote_amount,
            deadline: params.deadline,
        };

        let mut tx_builder = contract
            .createWithNative(router_params)
            .value(params.native_value);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn buy(&self, params: V2BuyParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::BuyParams {
            token: params.token,
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.buy(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn buy_with_native(
        &self,
        params: V2BuyWithNativeParams,
        value: U256,
    ) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::BuyWithNativeParams {
            token: params.token,
            amountOutMin: params.amount_out_min,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.buyWithNative(router_params).value(value);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn buy_with_permit(&self, params: V2BuyWithPermitParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::BuyWithPermitParams {
            token: params.token,
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            deadline: params.deadline,
            v: params.permit.v,
            r: params.permit.r,
            s: params.permit.s,
        };

        let mut tx_builder = contract.buyWithPermit(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell(&self, params: V2SellParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::SellParams {
            token: params.token,
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.sell(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell_to_native(&self, params: V2SellToNativeParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::SellToNativeParams {
            token: params.token,
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.sellToNative(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell_with_permit(&self, params: V2SellWithPermitParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::SellWithPermitParams {
            token: params.token,
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            deadline: params.deadline,
            v: params.permit.v,
            r: params.permit.r,
            s: params.permit.s,
        };

        let mut tx_builder = contract.sellWithPermit(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn sell_to_native_with_permit(
        &self,
        params: V2SellToNativeWithPermitParams,
    ) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::SellToNativeWithPermitParams {
            token: params.token,
            amountIn: params.amount_in,
            amountOutMin: params.amount_out_min,
            deadline: params.deadline,
            v: params.permit.v,
            r: params.permit.r,
            s: params.permit.s,
        };

        let mut tx_builder = contract.sellToNativeWithPermit(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_buy(&self, params: V2ExactOutBuyParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::ExactOutBuyParams {
            token: params.token,
            amountOut: params.amount_out,
            amountInMax: params.amount_in_max,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.exactOutBuy(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_buy_with_native(
        &self,
        params: V2ExactOutBuyWithNativeParams,
    ) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::ExactOutBuyWithNativeParams {
            token: params.token,
            amountOut: params.amount_out,
            deadline: params.deadline,
        };

        let mut tx_builder = contract
            .exactOutBuyWithNative(router_params)
            .value(params.amount_in_max);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_sell(&self, params: V2ExactOutSellParams) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::ExactOutSellParams {
            token: params.token,
            amountInMax: params.amount_in_max,
            amountOut: params.amount_out,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.exactOutSell(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn exact_out_sell_to_native(
        &self,
        params: V2ExactOutSellToNativeParams,
    ) -> Result<B256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let router_params = INadFunRouter::ExactOutSellToNativeParams {
            token: params.token,
            amountInMax: params.amount_in_max,
            amountOut: params.amount_out,
            deadline: params.deadline,
        };

        let mut tx_builder = contract.exactOutSellToNative(router_params);
        apply_tx_options!(tx_builder, params);
        let tx = tx_builder.send().await?;
        Ok(*tx.tx_hash())
    }

    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract
            .getAmountOut(token, amount_in, is_buy)
            .call()
            .await?)
    }

    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract
            .getAmountIn(token, amount_out, is_buy)
            .call()
            .await?)
    }

    pub async fn get_bonding_curve_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract
            .getBondingCurveAmountOut(token, amount_in, is_buy)
            .call()
            .await?)
    }

    pub async fn get_bonding_curve_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract
            .getBondingCurveAmountIn(token, amount_out, is_buy)
            .call()
            .await?)
    }

    pub async fn get_dex_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract
            .getDexAmountOut(token, amount_in, is_buy)
            .call()
            .await?)
    }

    pub async fn get_dex_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract
            .getDexAmountIn(token, amount_out, is_buy)
            .call()
            .await?)
    }

    pub async fn is_graduated(&self, token: Address) -> Result<bool> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract.isGraduated(token).call().await?)
    }

    pub async fn bonding_curve(&self) -> Result<Address> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract.bondingCurve().call().await?)
    }

    pub async fn token_registry(&self) -> Result<Address> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract.tokenRegistry().call().await?)
    }

    pub async fn wrapped_native(&self) -> Result<Address> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        Ok(contract.wrappedNative().call().await?)
    }

    /// Estimate gas for any v2 trading or creation operation.
    ///
    /// Builds the same calldata the equivalent send method would produce, but
    /// runs `eth_estimateGas` (with the caller's `from` address so balance /
    /// allowance checks succeed) instead of broadcasting. Caller should apply
    /// their own buffer (15-25% is typical) on top of the returned value.
    pub async fn estimate_gas(
        &self,
        params: V2GasEstimationParams,
        from: Address,
    ) -> Result<u64> {
        let contract = INadFunRouter::new(self.address, self.provider.as_ref());
        let map_vaults = |vs: Vec<V2VaultAllocation>| {
            vs.into_iter()
                .map(|v| VaultAllocation {
                    vault: v.vault,
                    bps: v.bps,
                    setupData: v.setup_data,
                })
                .collect::<Vec<_>>()
        };
        let gas = match params {
            V2GasEstimationParams::Create(p) => {
                let rp = INadFunRouter::CreateParams {
                    name: p.name,
                    symbol: p.symbol,
                    tokenURI: p.token_uri,
                    quoteToken: p.quote_token,
                    creatorFeeRate: p.creator_fee_rate,
                    vaults: map_vaults(p.vaults),
                    salt: p.salt,
                    dexType: p.dex_type.as_u8(),
                    buyQuoteAmount: p.buy_quote_amount,
                    deadline: p.deadline,
                };
                contract.create(rp).from(from).estimate_gas().await?
            }
            V2GasEstimationParams::CreateWithNative(p) => {
                let rp = INadFunRouter::CreateParams {
                    name: p.name,
                    symbol: p.symbol,
                    tokenURI: p.token_uri,
                    quoteToken: Address::ZERO,
                    creatorFeeRate: p.creator_fee_rate,
                    vaults: map_vaults(p.vaults),
                    salt: p.salt,
                    dexType: p.dex_type.as_u8(),
                    buyQuoteAmount: p.buy_quote_amount,
                    deadline: p.deadline,
                };
                contract
                    .createWithNative(rp)
                    .from(from)
                    .value(p.native_value)
                    .estimate_gas()
                    .await?
            }
            V2GasEstimationParams::Buy(p) => {
                let rp = INadFunRouter::BuyParams {
                    token: p.token,
                    amountIn: p.amount_in,
                    amountOutMin: p.amount_out_min,
                    deadline: p.deadline,
                };
                contract.buy(rp).from(from).estimate_gas().await?
            }
            V2GasEstimationParams::BuyWithNative { params, value } => {
                let rp = INadFunRouter::BuyWithNativeParams {
                    token: params.token,
                    amountOutMin: params.amount_out_min,
                    deadline: params.deadline,
                };
                contract
                    .buyWithNative(rp)
                    .from(from)
                    .value(value)
                    .estimate_gas()
                    .await?
            }
            V2GasEstimationParams::BuyWithPermit(p) => {
                let rp = INadFunRouter::BuyWithPermitParams {
                    token: p.token,
                    amountIn: p.amount_in,
                    amountOutMin: p.amount_out_min,
                    deadline: p.deadline,
                    v: p.permit.v,
                    r: p.permit.r,
                    s: p.permit.s,
                };
                contract.buyWithPermit(rp).from(from).estimate_gas().await?
            }
            V2GasEstimationParams::Sell(p) => {
                let rp = INadFunRouter::SellParams {
                    token: p.token,
                    amountIn: p.amount_in,
                    amountOutMin: p.amount_out_min,
                    deadline: p.deadline,
                };
                contract.sell(rp).from(from).estimate_gas().await?
            }
            V2GasEstimationParams::SellToNative(p) => {
                let rp = INadFunRouter::SellToNativeParams {
                    token: p.token,
                    amountIn: p.amount_in,
                    amountOutMin: p.amount_out_min,
                    deadline: p.deadline,
                };
                contract.sellToNative(rp).from(from).estimate_gas().await?
            }
            V2GasEstimationParams::SellWithPermit(p) => {
                let rp = INadFunRouter::SellWithPermitParams {
                    token: p.token,
                    amountIn: p.amount_in,
                    amountOutMin: p.amount_out_min,
                    deadline: p.deadline,
                    v: p.permit.v,
                    r: p.permit.r,
                    s: p.permit.s,
                };
                contract
                    .sellWithPermit(rp)
                    .from(from)
                    .estimate_gas()
                    .await?
            }
            V2GasEstimationParams::SellToNativeWithPermit(p) => {
                let rp = INadFunRouter::SellToNativeWithPermitParams {
                    token: p.token,
                    amountIn: p.amount_in,
                    amountOutMin: p.amount_out_min,
                    deadline: p.deadline,
                    v: p.permit.v,
                    r: p.permit.r,
                    s: p.permit.s,
                };
                contract
                    .sellToNativeWithPermit(rp)
                    .from(from)
                    .estimate_gas()
                    .await?
            }
            V2GasEstimationParams::ExactOutBuy(p) => {
                let rp = INadFunRouter::ExactOutBuyParams {
                    token: p.token,
                    amountOut: p.amount_out,
                    amountInMax: p.amount_in_max,
                    deadline: p.deadline,
                };
                contract.exactOutBuy(rp).from(from).estimate_gas().await?
            }
            V2GasEstimationParams::ExactOutBuyWithNative(p) => {
                let rp = INadFunRouter::ExactOutBuyWithNativeParams {
                    token: p.token,
                    amountOut: p.amount_out,
                    deadline: p.deadline,
                };
                contract
                    .exactOutBuyWithNative(rp)
                    .from(from)
                    .value(p.amount_in_max)
                    .estimate_gas()
                    .await?
            }
            V2GasEstimationParams::ExactOutSell(p) => {
                let rp = INadFunRouter::ExactOutSellParams {
                    token: p.token,
                    amountInMax: p.amount_in_max,
                    amountOut: p.amount_out,
                    deadline: p.deadline,
                };
                contract.exactOutSell(rp).from(from).estimate_gas().await?
            }
            V2GasEstimationParams::ExactOutSellToNative(p) => {
                let rp = INadFunRouter::ExactOutSellToNativeParams {
                    token: p.token,
                    amountInMax: p.amount_in_max,
                    amountOut: p.amount_out,
                    deadline: p.deadline,
                };
                contract
                    .exactOutSellToNative(rp)
                    .from(from)
                    .estimate_gas()
                    .await?
            }
        };
        Ok(gas)
    }
}
