//! v2 sell tokens for native MON via the unified `Core` (`core.v2()` handle).

use alloy::primitives::{Address, U256};
use anyhow::Result;
use nadfun_sdk::{Core, GasPricing, SlippageUtils, V2SellToNativeParams};

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

    let core = Core::new(config.rpc_url, private_key, config.network).await?;
    println!("wallet: {}", core.wallet_address());

    // Sell 100 tokens (assumes 18 decimals).
    let amount_in: U256 = U256::from(100u64) * U256::from(10).pow(U256::from(18u64));

    let expected = core.v2().get_amount_out(token, amount_in, false).await?;
    println!("expected MON out: {}", expected);
    if expected == U256::ZERO {
        anyhow::bail!("zero quote — token not sellable on v2 surface");
    }
    let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);

    // Note: caller must have approved the router for `amount_in` of `token`
    // beforehand (e.g. via TokenHelper). Use `sell_to_native_with_permit` for
    // a gasless approval flow.
    let tx_hash = core
        .v2()
        .sell_to_native(V2SellToNativeParams {
            token,
            to: core.wallet_address(),
            amount_in,
            amount_out_min: min_out,
            deadline: U256::from(9_999_999_999_u64),
            gas_limit: None,
            gas_price: Some(GasPricing::Legacy),
            nonce: None,
        })
        .await?;
    println!("tx: {}", tx_hash);

    let receipt = core.get_receipt(tx_hash).await?;
    println!(
        "status: {}, block: {:?}",
        receipt.status, receipt.block_number
    );
    Ok(())
}
