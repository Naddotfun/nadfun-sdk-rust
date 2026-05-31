//! Mixed-token dispatch: receive an arbitrary token and route the buy
//! through the correct v1/v2 path on a single `Core` instance.
//!
//! `core.detect_token_info(token)` does one on-chain `TokenInfoLens` call
//! returning both the version (v1 / v2 / None) and the token's quote token.
//! (`api.get_token(token)` is the off-chain equivalent.) The SDK does not
//! pick a trade method for you — this example shows the v2 quote-routing
//! choice the caller makes (see MIGRATION.md §7 for the full matrix):
//!   quote == wrapped native (WMON) -> core.v2().buy_with_native (send MON)
//!   quote == other ERC-20          -> core.v2().buy (pre-approve the quote token)
//!
//! Usage:
//!   export PRIVATE_KEY="..." RPC_URL="..."
//!   cargo run --example unified_dispatch -- --token 0xV1Token
//!   cargo run --example unified_dispatch -- --token 0xV2Token

use alloy::primitives::{utils::parse_ether, Address, B256, U256};
use anyhow::Result;
use nadfun_sdk::{
    ApiClient, BuyParams, Core, GasPricing, SdkVersion, SlippageUtils, V2BuyParams,
    V2BuyWithNativeParams,
};

#[path = "common/mod.rs"]
mod common;
use common::Config;

/// Buy `value` worth of `token`, picking the v1/v2 path (and, for v2, the
/// native vs ERC-20 quote path) from `Core::detect_token_info`. Returns the
/// submitted transaction hash.
async fn auto_buy(
    core: &Core,
    token: Address,
    value: U256,
    to: Address,
    deadline: U256,
) -> Result<B256> {
    let info = core.detect_token_info(token).await?;
    println!("token info: {:?}", info);
    match info.version {
        SdkVersion::V1 => {
            let (router, expected) = core.v1().get_amount_out(token, value, true).await?;
            let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
            core.v1()
                .buy(
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
            // The quote token decides how the buy is funded. `get_amount_out`
            // is quote-agnostic; the trade method is not.
            let expected = core.v2().get_amount_out(token, value, true).await?;
            let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
            let wrapped_native = core.v2().wrapped_native().await?;

            if info.quote_token == wrapped_native {
                // Native path: send `value` MON, the router wraps it.
                core.v2()
                    .buy_with_native(V2BuyWithNativeParams {
                        token,
                        to,
                        amount_out_min: min_out,
                        deadline,
                        value,
                        gas_limit: None,
                        gas_price: Some(GasPricing::Legacy),
                        nonce: None,
                    })
                    .await
            } else {
                // ERC-20 quote (e.g. USDT): `value` is the quote-token amount and
                // must already be approved to the v2 router. (LvMON is a special
                // case that mints from native on buy — see MIGRATION.md §7.)
                core.v2()
                    .buy(V2BuyParams {
                        token,
                        to,
                        amount_in: value,
                        amount_out_min: min_out,
                        deadline,
                        gas_limit: None,
                        gas_price: Some(GasPricing::Legacy),
                        nonce: None,
                    })
                    .await
            }
        }
        SdkVersion::None => {
            anyhow::bail!("token {token} is not registered on either v1 or v2 — refusing to trade");
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
/// `core.v1().buy(...)` for v1 paths and `core.v2().buy(...)` (or any other
/// `core.v2().*` method) for v2-only paths.
#[cfg(test)]
mod _shape_check {
    use super::*;
    fn _check_signature() {
        // Tail-position type check that the function compiles against the
        // public SDK surface from a downstream crate.
        let _: fn(_, _, _, _, _) -> _ = |a, b, c, d, e| auto_buy(a, b, c, d, e);
    }
}
