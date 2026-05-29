//! v2 buy with native MON via the unified `Core` (`*_v2` surface).
//!
//! Usage:
//!   export PRIVATE_KEY="..." RPC_URL="..." TOKEN="0x..."
//!   cargo run --example v2_buy
//!
//! Or with args:
//!   cargo run --example v2_buy -- --private-key 0x... --rpc-url https://... --token 0x...

use alloy::primitives::{utils::parse_ether, Address, U256};
use anyhow::Result;
use nadfun_sdk::{Core, GasPricing, SlippageUtils, V2BuyWithNativeParams, V2GasEstimationParams};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    let private_key = config.require_private_key()?;
    let token: Address = config
        .token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--token required"))?
        .parse()?;
    let mon_amount = parse_ether("0.01")?;

    let core = Core::new(config.rpc_url, private_key, config.network).await?;
    let wallet = core.wallet_address();
    println!("wallet: {}, router: {}", wallet, core.router_v2().address);

    // Quote: router auto-routes BC vs DEX based on graduation.
    let expected = core.get_amount_out_v2(token, mon_amount, true).await?;
    println!("expected tokens out: {}", expected);
    if expected == U256::ZERO {
        anyhow::bail!("Zero quote — token may not be tradeable on v2");
    }

    let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
    let deadline = U256::from(9_999_999_999_u64);
    let params = V2BuyWithNativeParams {
        token,
        to: wallet,
        amount_out_min: min_out,
        deadline,
        value: mon_amount,
        gas_limit: None,
        gas_price: Some(GasPricing::Legacy),
        nonce: None,
    };

    // Estimate gas with the same params we'll send.
    let gas_estimate = core
        .estimate_gas_v2(V2GasEstimationParams::BuyWithNative(params.clone()))
        .await
        .unwrap_or(400_000);
    let gas_with_buffer = gas_estimate * 120 / 100;
    println!(
        "estimated gas: {} (with 20% buffer: {})",
        gas_estimate, gas_with_buffer
    );

    let mut params = params;
    params.gas_limit = Some(gas_with_buffer);

    let tx_hash = core.buy_with_native_v2(params).await?;
    println!("tx: {}", tx_hash);

    let receipt = core.get_receipt(tx_hash).await?;
    println!(
        "status: {}, block: {:?}",
        receipt.status, receipt.block_number
    );

    Ok(())
}
