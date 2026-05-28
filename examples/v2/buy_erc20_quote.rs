//! Buy a v2 token using an ERC-20 quote (e.g. USDT) instead of native MON.
//!
//! Requires caller to have pre-approved the router for at least
//! `amount_in` worth of the quote token. v1 had no analogue — quote was
//! always MON.

use alloy::primitives::{Address, U256};
use anyhow::Result;
use nadfun_sdk::{Core, GasPricing, SlippageUtils, V2BuyParams};

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

    // 10 quote-token units (assumes 18 decimals; adjust for USDT 6).
    let amount_in: U256 = U256::from(10u64) * U256::from(10).pow(U256::from(18u64));

    let expected = core.quote_v2(token, amount_in, true).await?;
    println!("expected token out: {}", expected);
    if expected == U256::ZERO {
        anyhow::bail!("zero quote — token may not be tradeable with this quote");
    }
    let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);

    let tx_hash = core
        .buy_v2(V2BuyParams {
            token,
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
