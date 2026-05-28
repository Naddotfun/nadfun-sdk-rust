//! Mixed-token dispatch helper: receive an arbitrary token, check its
//! version via the Nad.fun API, and route the buy through the correct Core.
//!
//! This is the recommended user-side pattern for wallets / UIs / AI agents
//! that don't know the version of an incoming token up front. The SDK does
//! NOT do this dispatch internally — the choice is yours.
//!
//! Usage:
//!   export PRIVATE_KEY="..." RPC_URL="..."
//!   cargo run --example unified_dispatch -- --token 0xV1Token
//!   cargo run --example unified_dispatch -- --token 0xV2Token

use alloy::primitives::{utils::parse_ether, Address, B256, U256};
use anyhow::Result;
use nadfun_sdk::{
    ApiClient, BuyParams, Core, CoreV2, GasPricing, SdkVersion, SlippageUtils,
    V2BuyWithNativeParams,
};

#[path = "common/mod.rs"]
mod common;
use common::Config;

/// Buy `value` MON worth of `token`, picking v1 or v2 based on the API's
/// version field. Returns the submitted transaction hash.
async fn auto_buy(
    v1: &Core,
    v2: &CoreV2,
    api: &ApiClient,
    token: Address,
    value: U256,
    to: Address,
    deadline: U256,
) -> Result<B256> {
    let info = api.get_token(token).await?;
    println!("token version: {}", info.version);
    match info.version {
        SdkVersion::V1 => {
            let (router, expected) = v1.get_amount_out(token, value, true).await?;
            let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
            v1.buy(
                BuyParams {
                    token,
                    amount_in: value,
                    amount_out_min: min_out,
                    to,
                    deadline,
                    gas_limit: None,
                    gas_price: Some(GasPricing::Legacy),
                    nonce: None,
                },
                router,
            )
            .await
        }
        SdkVersion::V2 => {
            let _ = to; // v2 buy_with_native infers recipient from msg.sender
            let expected = v2.quote(token, value, true).await?;
            let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
            v2.buy_with_native(
                V2BuyWithNativeParams {
                    token,
                    amount_out_min: min_out,
                    deadline,
                    gas_limit: None,
                    gas_price: Some(GasPricing::Legacy),
                    nonce: None,
                },
                value,
            )
            .await
        }
    }
}

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

    let v1 = Core::new(config.rpc_url.clone(), private_key.clone(), config.network).await?;
    let v2 = CoreV2::new(config.rpc_url, private_key, config.network).await?;
    let api = ApiClient::from_env(config.network);
    let wallet = v1.wallet_address();

    let value = parse_ether("0.01")?;
    let deadline = U256::from(9_999_999_999_u64);

    let tx = auto_buy(&v1, &v2, &api, token, value, wallet, deadline).await?;
    println!("tx: {}", tx);

    Ok(())
}

/// Note: starting in 0.4.0 `Core` and `CoreV2` each store their own
/// `Network` — they can run against different networks in the same
/// process without interfering with each other.
#[cfg(test)]
mod _shape_check {
    use super::*;
    fn _check_signature() {
        // Tail-position type check that the function compiles against the
        // public SDK surface from a downstream crate.
        let _: fn(_, _, _, _, _, _, _) -> _ = |a, b, c, d, e, f, g| auto_buy(a, b, c, d, e, f, g);
    }
}
