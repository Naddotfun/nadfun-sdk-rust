//! Mixed-token dispatch: receive an arbitrary token and route the buy
//! through the correct v1/v2 path on a single `Core` instance.
//!
//! `core.detect_token_info_with_api(&api, token)` returns the version
//! (v1 / v2 / None) and quote token: v2 registry first, the api-server for
//! unregistered tokens. It works on every network; the lens-based `detect_token_info` does
//! not work on mainnet (the shipped lens address holds another contract).
//! The SDK does not pick a trade method for you — this example shows the v2
//! quote-routing choice the caller makes (see MIGRATION.md §7):
//!   quote is native (`is_native`: WMON, LVMON) -> core.v2().buy_with_native (send MON)
//!   any other quote (USDC, aStocks, ...)       -> core.v2().buy (pre-approve the quote token)
//!
//! Usage:
//!   export PRIVATE_KEY="..." RPC_URL="..."
//!   cargo run --example unified_dispatch -- --token 0xV1Token
//!   cargo run --example unified_dispatch -- --token 0xV2Token

use alloy::primitives::{
    utils::{parse_ether, parse_units},
    Address, B256, U256,
};
use anyhow::Result;
use nadfun_sdk::{
    quote_tokens, ApiClient, BuyParams, Core, GasPricing, SdkVersion, SlippageUtils, V2BuyParams,
    V2BuyWithNativeParams,
};

#[path = "common/mod.rs"]
mod common;
use common::Config;

/// Buy `amount` (human units of whatever the token is paid in) of `token`,
/// picking the v1/v2 path (and, for v2, the native vs ERC-20 quote path) from
/// `Core::detect_token_info_with_api`. Returns the submitted transaction hash.
async fn auto_buy(
    core: &Core,
    api: &ApiClient,
    token: Address,
    amount: &str,
    to: Address,
    deadline: U256,
) -> Result<B256> {
    let info = core.detect_token_info_with_api(api, token).await?;
    println!("token info: {:?}", info);
    match info.version {
        SdkVersion::V1 => {
            let value = parse_ether(amount)?;
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
            // The quote token decides how the buy is funded and in which
            // decimals `amount` is denominated.
            let native_quote = quote_tokens(core.network()).iter().any(|q| {
                q.is_native && q.address.parse::<Address>().ok() == Some(info.quote_token)
            });

            if native_quote {
                // Native path: send `amount` MON; the router wraps (WMON) or
                // mints (LVMON) it into the quote.
                let value = parse_ether(amount)?;
                let expected = core.v2().get_amount_out(token, value, true).await?;
                let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
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
                // ERC-20 quote (e.g. USDC, 6 decimals): scale `amount` by the
                // quote's on-chain decimals; the v2 router must already be
                // approved for it. `buy_with_native` would revert
                // `NativeQuoteMustUseRoute`.
                let decimals = core.v2().quote_config(info.quote_token).await?.decimals;
                let amount_in = parse_units(amount, decimals)?.get_absolute();
                let expected = core.v2().get_amount_out(token, amount_in, true).await?;
                let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
                core.v2()
                    .buy(V2BuyParams {
                        token,
                        to,
                        amount_in,
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
    let api = ApiClient::from_env(config.network);
    let wallet = core.wallet_address();

    // Human amount: MON for v1 / native-quoted v2, else the quote token.
    let amount = "0.01";
    let deadline = U256::from(9_999_999_999_u64);

    let tx = auto_buy(&core, &api, token, amount, wallet, deadline).await?;
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
        let _: fn(_, _, _, _, _, _) -> _ = |a, b, c, d, e, f| auto_buy(a, b, c, d, e, f);
    }
}
