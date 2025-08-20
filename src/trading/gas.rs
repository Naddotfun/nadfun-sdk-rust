use crate::types::Router;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use anyhow::Result;
use std::sync::Arc;

/// Parameters for gas estimation
#[derive(Debug, Clone)]
pub enum GasEstimationParams {
    Buy {
        token: Address,
        amount_in: U256,
        amount_out_min: U256,
        to: Address,
        deadline: U256,
    },
    Sell {
        token: Address,
        amount_in: U256,
        amount_out_min: U256,
        to: Address,
        deadline: U256,
    },
    SellPermit {
        token: Address,
        amount_in: U256,
        amount_out_min: U256,
        to: Address,
        deadline: U256,
        v: u8,
        r: [u8; 32],
        s: [u8; 32],
    },
}

/// Estimate gas for any trading operation
///
/// This is the main entry point for gas estimation. It takes gas estimation parameters
/// and the router information, then calls the appropriate specialized function.
///
/// # Example
/// ```rust,ignore
/// use nadfun_sdk::{estimate_gas, GasEstimationParams};
///
/// let params = GasEstimationParams::Sell {
///     token,
///     amount_in: token_amount,
///     amount_out_min: U256::from(1),
///     to: wallet,
///     deadline: U256::from(9999999999999999u64),
/// };
///
/// let estimated_gas = estimate_gas(provider, &router, params).await?;
/// ```
pub async fn estimate_gas<P: Provider>(
    provider: Arc<P>,
    router: &Router,
    params: GasEstimationParams,
) -> Result<u64> {
    match params {
        GasEstimationParams::Buy {
            token,
            amount_in,
            amount_out_min,
            to,
            deadline,
        } => {
            estimate_buy_gas(
                provider,
                router,
                token,
                amount_in,
                amount_out_min,
                to,
                deadline,
            )
            .await
        }

        GasEstimationParams::Sell {
            token,
            amount_in,
            amount_out_min,
            to,
            deadline,
        } => {
            estimate_sell_gas(
                provider,
                router,
                token,
                amount_in,
                amount_out_min,
                to,
                deadline,
            )
            .await
        }

        GasEstimationParams::SellPermit {
            token,
            amount_in,
            amount_out_min,
            to,
            deadline,
            v,
            r,
            s,
        } => {
            estimate_sell_permit_gas(
                provider,
                router,
                token,
                amount_in,
                amount_out_min,
                to,
                deadline,
                v,
                r,
                s,
            )
            .await
        }
    }
}

/// Estimate gas for buy operation
pub async fn estimate_buy_gas<P: Provider>(
    provider: Arc<P>,
    router: &Router,
    token: Address,
    amount_in: U256,
    amount_out_min: U256,
    to: Address,
    deadline: U256,
) -> Result<u64> {
    match router {
        Router::BondingCurve(router_addr) => {
            use crate::contracts::bonding_curve::IBondingCurveRouter;

            let contract_params = IBondingCurveRouter::BuyParams {
                amountOutMin: amount_out_min,
                token,
                to,
                deadline,
            };

            let contract = IBondingCurveRouter::new(*router_addr, provider.as_ref());
            let call_builder = contract.buy(contract_params);
            let call_data = call_builder.calldata();

            let gas = provider
                .estimate_gas(
                    TransactionRequest::default()
                        .to(*router_addr)
                        .from(to)
                        .value(amount_in)
                        .input(call_data.clone().into()),
                )
                .await?;

            Ok(gas.try_into().map_err(|_| anyhow::anyhow!("Gas estimation overflow"))?)
        }
        Router::Dex(router_addr) => {
            use crate::contracts::dex::IDexRouter;

            let contract_params = IDexRouter::BuyParams {
                amountOutMin: amount_out_min,
                token,
                to,
                deadline,
            };

            let contract = IDexRouter::new(*router_addr, provider.as_ref());
            let call_builder = contract.buy(contract_params);
            let call_data = call_builder.calldata();

            let gas = provider
                .estimate_gas(
                    TransactionRequest::default()
                        .to(*router_addr)
                        .from(to)
                        .value(amount_in)
                        .input(call_data.clone().into()),
                )
                .await?;

            Ok(gas.try_into().map_err(|_| anyhow::anyhow!("Gas estimation overflow"))?)
        }
    }
}

/// Estimate gas for sell operation
pub async fn estimate_sell_gas<P: Provider>(
    provider: Arc<P>,
    router: &Router,
    token: Address,
    amount_in: U256,
    amount_out_min: U256,
    to: Address,
    deadline: U256,
) -> Result<u64> {
    match router {
        Router::BondingCurve(router_addr) => {
            use crate::contracts::bonding_curve::IBondingCurveRouter;

            let contract_params = IBondingCurveRouter::SellParams {
                amountIn: amount_in,
                amountOutMin: amount_out_min,
                token,
                to,
                deadline,
            };

            let contract = IBondingCurveRouter::new(*router_addr, provider.as_ref());
            let call_builder = contract.sell(contract_params);
            let call_data = call_builder.calldata();

            let gas = provider
                .estimate_gas(
                    TransactionRequest::default()
                        .to(*router_addr)
                        .from(to)
                        .input(call_data.clone().into()),
                )
                .await?;

            Ok(gas.try_into().map_err(|_| anyhow::anyhow!("Gas estimation overflow"))?)
        }
        Router::Dex(router_addr) => {
            use crate::contracts::dex::IDexRouter;

            let contract_params = IDexRouter::SellParams {
                amountIn: amount_in,
                amountOutMin: amount_out_min,
                token,
                to,
                deadline,
            };

            let contract = IDexRouter::new(*router_addr, provider.as_ref());
            let call_builder = contract.sell(contract_params);
            let call_data = call_builder.calldata();

            let gas = provider
                .estimate_gas(
                    TransactionRequest::default()
                        .to(*router_addr)
                        .from(to)
                        .input(call_data.clone().into()),
                )
                .await?;

            Ok(gas.try_into().map_err(|_| anyhow::anyhow!("Gas estimation overflow"))?)
        }
    }
}

/// Estimate gas for sell permit operation
pub async fn estimate_sell_permit_gas<P: Provider>(
    provider: Arc<P>,
    router: &Router,
    token: Address,
    amount_in: U256,
    amount_out_min: U256,
    to: Address,
    deadline: U256,
    v: u8,
    r: [u8; 32],
    s: [u8; 32],
) -> Result<u64> {
    match router {
        Router::BondingCurve(router_addr) => {
            use crate::contracts::bonding_curve::IBondingCurveRouter;

            let contract_params = IBondingCurveRouter::SellPermitParams {
                amountIn: amount_in,
                amountOutMin: amount_out_min,
                amountAllowance: amount_in, // Same as amount_in
                token,
                to,
                deadline,
                v,
                r: r.into(),
                s: s.into(),
            };

            let contract = IBondingCurveRouter::new(*router_addr, provider.as_ref());
            let call_builder = contract.sellPermit(contract_params);
            let call_data = call_builder.calldata();

            let gas = provider
                .estimate_gas(
                    TransactionRequest::default()
                        .to(*router_addr)
                        .from(to)
                        .input(call_data.clone().into()),
                )
                .await?;

            Ok(gas
                .try_into()
                .map_err(|_| anyhow::anyhow!("Gas estimation overflow"))?)
        }
        Router::Dex(router_addr) => {
            use crate::contracts::dex::IDexRouter;

            let contract_params = IDexRouter::SellPermitParams {
                amountIn: amount_in,
                amountOutMin: amount_out_min,
                amountAllowance: amount_in, // Same as amount_in
                token,
                to,
                deadline,
                v,
                r: r.into(),
                s: s.into(),
            };

            let contract = IDexRouter::new(*router_addr, provider.as_ref());
            let call_builder = contract.sellPermit(contract_params);
            let call_data = call_builder.calldata();

            let gas = provider
                .estimate_gas(
                    TransactionRequest::default()
                        .to(*router_addr)
                        .from(to)
                        .input(call_data.clone().into()),
                )
                .await?;

            Ok(gas.try_into().map_err(|_| anyhow::anyhow!("Gas estimation overflow"))?)
        }
    }
}

