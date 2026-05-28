//! Mixed-token dispatch: receive an arbitrary token and route the buy
//! through the correct v1/v2 path on a single `Core` instance.
//!
//! Two patterns shown:
//!   1. `core.detect_version(token)` — on-chain probe of
//!      `TokenRegistryV2::isRegistered`, with in-process caching.
//!   2. `api.get_token(token).version` — same answer from the API.
//!
//! Either is fine. The on-chain probe avoids the API hop; the API answer
//! avoids the RPC hop. Pick whichever fits your latency budget.
//!
//! Usage:
//!   export PRIVATE_KEY="..." RPC_URL="..."
//!   cargo run --example unified_dispatch -- --token 0xV1Token
//!   cargo run --example unified_dispatch -- --token 0xV2Token

use alloy::primitives::{utils::parse_ether, Address, B256, U256};
use anyhow::Result;
use nadfun_sdk::{
    ApiClient, BuyParams, Core, GasPricing, SdkVersion, SlippageUtils, V2BuyWithNativeParams,
};

#[path = "common/mod.rs"]
mod common;
use common::Config;

/// Buy `value` MON worth of `token`, picking v1 or v2 based on
/// `Core::detect_version`. Returns the submitted transaction hash.
async fn auto_buy(
    core: &Core,
    token: Address,
    value: U256,
    to: Address,
    deadline: U256,
) -> Result<B256> {
    let version = core.detect_version(token).await?;
    println!("token version: {:?}", version);
    match version {
        SdkVersion::V1 => {
            let (router, expected) = core.get_amount_out(token, value, true).await?;
            let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
            core.buy(
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
            let expected = core.quote_v2(token, value, true).await?;
            let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
            core.buy_with_native_v2(
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

    let core = Core::new(config.rpc_url, private_key, config.network).await?;
    let _api = ApiClient::from_env(config.network); // available if you prefer API-side version detection
    let wallet = core.wallet_address();

    let value = parse_ether("0.01")?;
    let deadline = U256::from(9_999_999_999_u64);

    let tx = auto_buy(&core, token, value, wallet, deadline).await?;
    println!("tx: {}", tx);

    Ok(())
}

/// Note: starting in 0.4.0 the unified `Core` handles v1 and v2 from a
/// single instance — no need for two side-by-side clients. Use
/// `core.buy(...)` for v1 paths and `core.buy_v2(...)` (or any other
/// `*_v2` method) for v2-only paths.
#[cfg(test)]
mod _shape_check {
    use super::*;
    fn _check_signature() {
        // Tail-position type check that the function compiles against the
        // public SDK surface from a downstream crate.
        let _: fn(_, _, _, _, _) -> _ = |a, b, c, d, e| auto_buy(a, b, c, d, e);
    }
}
