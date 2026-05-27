//! Exact-output buy: "I want exactly N tokens; spend at most M MON".

use alloy::primitives::{utils::parse_ether, Address, U256};
use anyhow::Result;
use nadfun_sdk::{CoreV2, GasPricing, V2ExactOutBuyWithNativeParams};

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

    let core = CoreV2::new(config.rpc_url, private_key, config.network).await?;

    // Target: exactly 1 token (18 decimals).
    let amount_out: U256 = U256::from(10).pow(U256::from(18u64));
    let max_in = parse_ether("1")?; // willing to spend up to 1 MON

    // Optional sanity-check via inverse quote.
    let required = core.quote_in(token, amount_out, true).await?;
    println!("required MON for exactly 1 token: {}", required);
    if required > max_in {
        anyhow::bail!("would cost more than max_in ({} > {})", required, max_in);
    }

    let tx_hash = core
        .exact_out_buy_with_native(V2ExactOutBuyWithNativeParams {
            token,
            amount_out,
            amount_in_max: max_in,
            deadline: U256::from(9_999_999_999_u64),
            gas_limit: None,
            gas_price: Some(GasPricing::Legacy),
            nonce: None,
        })
        .await?;
    println!("tx: {}", tx_hash);

    let receipt = core.get_receipt(tx_hash).await?;
    println!("status: {}, block: {:?}", receipt.status, receipt.block_number);
    Ok(())
}
