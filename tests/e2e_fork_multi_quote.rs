//! Monad TESTNET anvil-fork E2E for v2 multi-quote trading.
//!
//! State-changing, so `#[ignore]`. Driven by `scripts/e2e-fork.sh`, which
//! starts a pinned testnet fork, funds a throwaway key (MON + USDC), and runs:
//! ```bash
//! E2E_RPC_URL=http://127.0.0.1:<port> E2E_PRIVATE_KEY=0x... \
//! cargo test --test e2e_fork_multi_quote -- --ignored --nocapture
//! ```
//! Scenarios (each recorded PASS/FAIL in `target/e2e-fork-report.json`):
//!   1. ERC-20 `create` of a USDC-quoted token paid in USDC
//!   2. ERC-20 `buy` + `sell` of that token in USDC; fills == `get_amount_out` previews
//!   3. legacy WMON-quoted `create_with_native` + `buy_with_native` regression
//!   4. (run last) `buy_with_native` on the USDC-quoted token reverts `NativeQuoteMustUseRoute`
//!   5. live api-server `GET /quote_token` matches `quote_tokens(Testnet)` (SKIP if unreachable)
//!   6. `detect_token_info_with_api` with the live dev api-server (SKIP if unreachable):
//!      a pre-fork V2 token and the fork-only USDC token resolve from the chain as V2
//!      (matching the testnet lens); an unregistered address is an API 404 -> `None`
//!   7. large USDC curve trades on a fresh USDC token: buys of ~10% / ~50% / ~90% of the
//!      remaining quote-to-graduation, then a sell of 90% of the wallet's tokens
//!   8. graduation boundary: a buy above the remaining quote (partial fill + refund), then
//!      post-graduation ERC-20 buy/sell routed to the DEX pair
//!   9. large WMON-quoted `buy_with_native` / `sell_to_native` pair
//!
//! Every trade in 7-9: fill == `get_amount_out` preview, `amount_out_min = preview + 1`
//! reverts `InsufficientOutput()` (checked via `estimate_gas`, which sends nothing), and
//! the router holds no token / USDC / WMON / native afterwards.

use alloy::primitives::{
    keccak256,
    utils::{format_units, parse_ether},
    Address, B256, U256,
};
use alloy::providers::Provider;
use alloy::sol;
use nadfun_sdk::types::v2::events::V2EventType;
use nadfun_sdk::{
    quote_tokens, ApiClient, Core, Network, SdkVersion, V2BuyParams, V2BuyWithNativeParams,
    V2CreateParams, V2CreateWithNativeParams, V2DexType, V2GasEstimationParams, V2SellParams,
    V2SellToNativeParams, V2VaultAllocation,
};
use serde_json::{json, Value};
use std::time::Duration;

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function approve(address spender, uint256 amount) external returns (bool);
        function balanceOf(address owner) external view returns (uint256);
    }
}

const DEADLINE: u64 = 9_999_999_999;
const CREATOR_FEE_RATE: u16 = 100;
/// Testnet V2 token created long before the pinned fork block (known to dev api).
const KNOWN_V2_TOKEN: &str = "0xe099B47899BB851D4df6392F7A7B242600aa7777";
/// Not a Nad.fun token on any network (api-server answers 404).
const UNREGISTERED_TOKEN: &str = "0x0000000000000000000000000000000000000001";

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn addr(s: &str) -> Address {
    s.parse().expect("address")
}

async fn balance_of(core: &Core, token: Address, owner: Address) -> U256 {
    IERC20::new(token, core.provider().clone())
        .balanceOf(owner)
        .call()
        .await
        .expect("balanceOf")
}

/// Poll until the tx is mined; errors if it reverted.
async fn wait_ok(core: &Core, tx: B256) -> anyhow::Result<nadfun_sdk::TransactionResult> {
    for _ in 0..60 {
        if let Ok(r) = core.get_receipt(tx).await {
            anyhow::ensure!(r.status, "tx {tx} reverted");
            return Ok(r);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    anyhow::bail!("receipt not found for {tx}")
}

async fn approve(
    core: &Core,
    token: Address,
    spender: Address,
    amount: U256,
) -> anyhow::Result<()> {
    let tx = *IERC20::new(token, core.provider().clone())
        .approve(spender, amount)
        .send()
        .await?
        .tx_hash();
    wait_ok(core, tx).await.map(|_| ())
}

/// Mine until the anti-sniping penalty is gone so a preview at block N equals
/// the fill at block N+1.
async fn mine_past_penalty(core: &Core, token: Address) -> anyhow::Result<u64> {
    for mined in 0..200 {
        if core.v2().get_sniping_penalty(token).await?.is_zero() {
            return Ok(mined);
        }
        let _: Value = core
            .provider()
            .raw_request("anvil_mine".into(), (U256::from(1),))
            .await?;
    }
    anyhow::bail!("sniping penalty still active after 200 blocks")
}

fn created_token(receipt: &nadfun_sdk::TransactionResult) -> anyhow::Result<Address> {
    let sig = V2EventType::Create.signature();
    let log = receipt
        .logs
        .iter()
        .find(|l| l.topic0() == Some(&sig))
        .ok_or_else(|| anyhow::anyhow!("no Create log"))?;
    Ok(Address::from_word(log.topics()[2]))
}

fn vaults() -> Vec<V2VaultAllocation> {
    let burn = addr(nadfun_sdk::constants::get_burn_vault_v2(Network::Testnet));
    let lp = addr(nadfun_sdk::constants::get_lp_vault_v2(Network::Testnet));
    vec![
        V2VaultAllocation {
            vault: burn,
            bps: 5_000,
            setup_data: Default::default(),
        },
        V2VaultAllocation {
            vault: lp,
            bps: 5_000,
            setup_data: Default::default(),
        },
    ]
}

fn salt(tag: &str, nonce: u64) -> B256 {
    keccak256(format!("nadfun-sdk-e2e-fork/{tag}/{nonce}"))
}

struct Ctx {
    core: Core,
    wallet: Address,
    router: Address,
    usdc: Address,
    wmon: Address,
}

async fn scenario_create_usdc(ctx: &Ctx) -> anyhow::Result<(Address, Value)> {
    let core = &ctx.core;
    let buy_quote_amount = U256::from(50_000u64); // 0.05 USDC (6 decimals)
    let deploy_fee = core.v2().deploy_fee(ctx.usdc).await?;
    let expected = core
        .v2()
        .get_initial_buy_amount_out(ctx.usdc, buy_quote_amount, CREATOR_FEE_RATE)
        .await?;
    approve(core, ctx.usdc, ctx.router, deploy_fee + buy_quote_amount).await?;
    let usdc_before = balance_of(core, ctx.usdc, ctx.wallet).await;
    let nonce = core.provider().get_transaction_count(ctx.wallet).await?;
    let tx = core
        .v2()
        .create(V2CreateParams {
            name: format!("E2E USDC {nonce}"),
            symbol: format!("EU{nonce}"),
            token_uri: "ipfs://e2e-usdc".to_string(),
            quote_token: ctx.usdc,
            creator_fee_rate: CREATOR_FEE_RATE,
            vaults: vaults(),
            salt: salt("usdc", nonce),
            dex_type: V2DexType::NadFun,
            buy_quote_amount,
            deadline: U256::from(DEADLINE),
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await?;
    let token = created_token(&wait_ok(core, tx).await?)?;
    let quote = core.v2().quote_token(token).await?;
    anyhow::ensure!(quote == ctx.usdc, "token quote {quote} != USDC");
    let usdc_spent = usdc_before - balance_of(core, ctx.usdc, ctx.wallet).await;
    let minted = balance_of(core, token, ctx.wallet).await;
    anyhow::ensure!(
        usdc_spent == deploy_fee + buy_quote_amount,
        "USDC spent {usdc_spent} != deployFee + buyQuoteAmount"
    );
    anyhow::ensure!(
        minted == expected,
        "initial buy {minted} != expected {expected}"
    );
    Ok((
        token,
        json!({
            "tx": tx.to_string(),
            "token": token.to_string(),
            "deploy_fee": deploy_fee.to_string(),
            "buy_quote_amount": buy_quote_amount.to_string(),
            "usdc_spent": usdc_spent.to_string(),
            "expected_initial_buy_tokens": expected.to_string(),
            "initial_buy_tokens": minted.to_string(),
        }),
    ))
}

async fn scenario_buy_sell_usdc(ctx: &Ctx, token: Address) -> anyhow::Result<Value> {
    let core = &ctx.core;
    let mined = mine_past_penalty(core, token).await?;

    let amount_in = U256::from(10_000u64); // 0.01 USDC
    approve(core, ctx.usdc, ctx.router, amount_in).await?;
    let buy_preview = core.v2().get_amount_out(token, amount_in, true).await?;
    let token_before = balance_of(core, token, ctx.wallet).await;
    let usdc_before = balance_of(core, ctx.usdc, ctx.wallet).await;
    let buy = core
        .v2()
        .buy(V2BuyParams {
            token,
            to: ctx.wallet,
            amount_in,
            amount_out_min: buy_preview,
            deadline: U256::from(DEADLINE),
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await?;
    wait_ok(core, buy).await?;
    let bought = balance_of(core, token, ctx.wallet).await - token_before;
    let usdc_spent = usdc_before - balance_of(core, ctx.usdc, ctx.wallet).await;
    anyhow::ensure!(
        bought == buy_preview,
        "buy {bought} != preview {buy_preview}"
    );
    anyhow::ensure!(
        usdc_spent <= amount_in && !usdc_spent.is_zero(),
        "bad USDC spend {usdc_spent}"
    );

    let sell_amount = bought / U256::from(2);
    approve(core, token, ctx.router, sell_amount).await?;
    let sell_preview = core.v2().get_amount_out(token, sell_amount, false).await?;
    let usdc_before = balance_of(core, ctx.usdc, ctx.wallet).await;
    let sell = core
        .v2()
        .sell(V2SellParams {
            token,
            to: ctx.wallet,
            amount_in: sell_amount,
            amount_out_min: sell_preview,
            deadline: U256::from(DEADLINE),
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await?;
    wait_ok(core, sell).await?;
    let usdc_received = balance_of(core, ctx.usdc, ctx.wallet).await - usdc_before;
    anyhow::ensure!(
        usdc_received == sell_preview,
        "sell {usdc_received} != preview {sell_preview}"
    );
    anyhow::ensure!(!usdc_received.is_zero(), "sell returned no USDC");
    Ok(json!({
        "blocks_mined_for_penalty": mined,
        "buy_tx": buy.to_string(),
        "buy_usdc_in": amount_in.to_string(),
        "buy_usdc_spent": usdc_spent.to_string(),
        "buy_preview_tokens": buy_preview.to_string(),
        "buy_received_tokens": bought.to_string(),
        "sell_tx": sell.to_string(),
        "sell_tokens_in": sell_amount.to_string(),
        "sell_preview_usdc": sell_preview.to_string(),
        "sell_received_usdc": usdc_received.to_string(),
    }))
}

async fn create_wmon_token(ctx: &Ctx) -> anyhow::Result<(Address, B256)> {
    let core = &ctx.core;
    let buy_quote_amount = parse_ether("1")?;
    let deploy_fee = core.v2().deploy_fee(ctx.wmon).await?;
    let nonce = core.provider().get_transaction_count(ctx.wallet).await?;
    let tx = core
        .v2()
        .create_with_native(V2CreateWithNativeParams {
            name: format!("E2E Legacy {nonce}"),
            symbol: format!("EL{nonce}"),
            token_uri: "ipfs://e2e-legacy".to_string(),
            quote_token: ctx.wmon,
            creator_fee_rate: CREATOR_FEE_RATE,
            vaults: vaults(),
            salt: salt("legacy", nonce),
            dex_type: V2DexType::NadFun,
            buy_quote_amount,
            native_value: deploy_fee + buy_quote_amount,
            deadline: U256::from(DEADLINE),
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await?;
    let token = created_token(&wait_ok(core, tx).await?)?;
    Ok((token, tx))
}

async fn scenario_legacy_wmon(ctx: &Ctx) -> anyhow::Result<Value> {
    let core = &ctx.core;
    let (token, tx) = create_wmon_token(ctx).await?;
    mine_past_penalty(core, token).await?;
    let value = parse_ether("1")?;
    let preview = core.v2().get_amount_out(token, value, true).await?;
    let before = balance_of(core, token, ctx.wallet).await;
    let buy = core
        .v2()
        .buy_with_native(V2BuyWithNativeParams {
            token,
            to: ctx.wallet,
            amount_out_min: preview,
            deadline: U256::from(DEADLINE),
            value,
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await?;
    wait_ok(core, buy).await?;
    let received = balance_of(core, token, ctx.wallet).await - before;
    anyhow::ensure!(
        received == preview,
        "received {received} != preview {preview}"
    );
    Ok(json!({
        "create_tx": tx.to_string(),
        "token": token.to_string(),
        "buy_tx": buy.to_string(),
        "native_in": value.to_string(),
        "preview_token_out": preview.to_string(),
        "received_tokens": received.to_string(),
    }))
}

async fn scenario_native_reverts(ctx: &Ctx, token: Address) -> anyhow::Result<Value> {
    let selector = &keccak256("NativeQuoteMustUseRoute()")[..4];
    let err = match ctx
        .core
        .v2()
        .buy_with_native(V2BuyWithNativeParams {
            token,
            to: ctx.wallet,
            amount_out_min: U256::ZERO,
            deadline: U256::from(DEADLINE),
            value: parse_ether("1")?,
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await
    {
        Ok(tx) => anyhow::bail!("buy_with_native on USDC token was accepted: {tx}"),
        Err(e) => e,
    };
    let revert = err
        .downcast_ref::<alloy::contract::Error>()
        .and_then(|e| e.as_revert_data())
        .ok_or_else(|| anyhow::anyhow!("no revert data in error: {err:#}"))?;
    anyhow::ensure!(
        revert.as_ref() == selector,
        "revert data {revert} is not NativeQuoteMustUseRoute()"
    );
    Ok(json!({ "revert_data": revert.to_string(), "decoded": "NativeQuoteMustUseRoute()" }))
}

async fn scenario_api_catalog(api: &ApiClient) -> anyhow::Result<Value> {
    let live: Vec<_> = api
        .get_quote_tokens()
        .await?
        .into_iter()
        .map(|q| (q.quote_id, q.symbol, q.decimals))
        .collect();
    let baked: Vec<_> = quote_tokens(Network::Testnet)
        .iter()
        .map(|q| (addr(q.address), q.symbol.to_string(), q.decimals))
        .collect();
    anyhow::ensure!(live == baked, "api {live:?} != constants {baked:?}");
    Ok(json!({ "count": live.len() }))
}

async fn scenario_api_detection(
    ctx: &Ctx,
    api: &ApiClient,
    fork_only_token: Option<Address>,
) -> anyhow::Result<Value> {
    // Registered tokens resolve from the chain and must agree with the testnet lens.
    let known = addr(KNOWN_V2_TOKEN);
    let mut registered = vec![known];
    registered.extend(fork_only_token);
    let mut seen = Vec::new();
    for token in registered {
        let via_api = ctx.core.detect_token_info_with_api(api, token).await?;
        let via_lens = ctx.core.detect_token_info(token).await?;
        anyhow::ensure!(via_api.version == SdkVersion::V2, "{token}: {via_api:?}");
        anyhow::ensure!(
            via_api == via_lens,
            "{token}: api {via_api:?} != lens {via_lens:?}"
        );
        seen.push(json!({
            "token": token.to_string(),
            "version": via_api.version.to_string(),
            "quote": via_api.quote_token.to_string(),
            "matches_testnet_lens": true,
        }));
    }
    if fork_only_token.is_some() {
        let quote = seen[1]["quote"].as_str().unwrap().parse::<Address>()?;
        anyhow::ensure!(quote == ctx.usdc, "fork-only token quote {quote} != USDC");
    }
    // Unregistered on chain -> asks the live API, whose 404 maps to None.
    let unknown = addr(UNREGISTERED_TOKEN);
    let info = ctx.core.detect_token_info_with_api(api, unknown).await?;
    anyhow::ensure!(
        info.version == SdkVersion::None && info.quote_token == Address::ZERO,
        "unregistered address should be None, got {info:?}"
    );
    Ok(json!({
        "registered": seen,
        "unregistered": { "token": unknown.to_string(), "version": info.version.to_string() },
    }))
}

const USDC_DECIMALS: u8 = 6;
const TOKEN_DECIMALS: u8 = 18;

fn human(v: U256, decimals: u8) -> String {
    format_units(v, decimals).unwrap_or_else(|_| v.to_string())
}

/// Raw + human-readable amount for the report.
fn amt(v: U256, decimals: u8) -> Value {
    json!({ "raw": v.to_string(), "human": human(v, decimals) })
}

/// The router must not retain any of the traded assets after a trade.
async fn assert_router_clean(ctx: &Ctx, token: Address) -> anyhow::Result<()> {
    let core = &ctx.core;
    let held = [
        ("token", balance_of(core, token, ctx.router).await),
        ("usdc", balance_of(core, ctx.usdc, ctx.router).await),
        ("wmon", balance_of(core, ctx.wmon, ctx.router).await),
        ("native", core.provider().get_balance(ctx.router).await?),
    ];
    for (what, v) in held {
        anyhow::ensure!(v.is_zero(), "router holds {v} {what} dust");
    }
    Ok(())
}

/// `estimate_gas` with `amount_out_min = preview + 1` must revert with the
/// router's `InsufficientOutput()`. Estimation sends nothing, so it does not
/// disturb the provider's nonce cache.
async fn assert_slippage_reverts(
    ctx: &Ctx,
    params: V2GasEstimationParams,
) -> anyhow::Result<String> {
    let selector = &keccak256("InsufficientOutput()")[..4];
    let err = match ctx.core.v2().estimate_gas(params).await {
        Ok(gas) => anyhow::bail!("preview + 1 was accepted (gas {gas})"),
        Err(e) => e,
    };
    let revert = err
        .downcast_ref::<alloy::contract::Error>()
        .and_then(|e| e.as_revert_data())
        .ok_or_else(|| anyhow::anyhow!("no revert data in error: {err:#}"))?;
    anyhow::ensure!(
        revert.as_ref() == selector,
        "slippage revert {revert} is not InsufficientOutput()"
    );
    Ok("InsufficientOutput()".to_string())
}

/// ERC-20 buy of `amount_in` USDC: slippage guard, exact preview fill, clean router.
async fn usdc_buy(ctx: &Ctx, token: Address, amount_in: U256) -> anyhow::Result<Value> {
    let core = &ctx.core;
    approve(core, ctx.usdc, ctx.router, amount_in).await?;
    let preview = core.v2().get_amount_out(token, amount_in, true).await?;
    let params = |amount_out_min| V2BuyParams {
        token,
        to: ctx.wallet,
        amount_in,
        amount_out_min,
        deadline: U256::from(DEADLINE),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    let slippage = assert_slippage_reverts(
        ctx,
        V2GasEstimationParams::Buy(params(preview + U256::from(1))),
    )
    .await?;
    let token_before = balance_of(core, token, ctx.wallet).await;
    let usdc_before = balance_of(core, ctx.usdc, ctx.wallet).await;
    let tx = core.v2().buy(params(preview)).await?;
    wait_ok(core, tx).await?;
    let received = balance_of(core, token, ctx.wallet).await - token_before;
    let spent = usdc_before - balance_of(core, ctx.usdc, ctx.wallet).await;
    anyhow::ensure!(received == preview, "buy {received} != preview {preview}");
    anyhow::ensure!(spent <= amount_in, "spent {spent} > amount_in {amount_in}");
    assert_router_clean(ctx, token).await?;
    Ok(json!({
        "tx": tx.to_string(),
        "usdc_in": amt(amount_in, USDC_DECIMALS),
        "usdc_spent": amt(spent, USDC_DECIMALS),
        "preview_tokens": amt(preview, TOKEN_DECIMALS),
        "received_tokens": amt(received, TOKEN_DECIMALS),
        "slippage_revert": slippage,
    }))
}

/// ERC-20 sell of `amount_in` tokens for USDC: slippage guard, exact preview fill, clean router.
async fn usdc_sell(ctx: &Ctx, token: Address, amount_in: U256) -> anyhow::Result<Value> {
    let core = &ctx.core;
    approve(core, token, ctx.router, amount_in).await?;
    let preview = core.v2().get_amount_out(token, amount_in, false).await?;
    let params = |amount_out_min| V2SellParams {
        token,
        to: ctx.wallet,
        amount_in,
        amount_out_min,
        deadline: U256::from(DEADLINE),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    let slippage = assert_slippage_reverts(
        ctx,
        V2GasEstimationParams::Sell(params(preview + U256::from(1))),
    )
    .await?;
    let usdc_before = balance_of(core, ctx.usdc, ctx.wallet).await;
    let tx = core.v2().sell(params(preview)).await?;
    wait_ok(core, tx).await?;
    let received = balance_of(core, ctx.usdc, ctx.wallet).await - usdc_before;
    anyhow::ensure!(received == preview, "sell {received} != preview {preview}");
    anyhow::ensure!(!received.is_zero(), "sell returned no USDC");
    assert_router_clean(ctx, token).await?;
    Ok(json!({
        "tx": tx.to_string(),
        "tokens_in": amt(amount_in, TOKEN_DECIMALS),
        "preview_usdc": amt(preview, USDC_DECIMALS),
        "received_usdc": amt(received, USDC_DECIMALS),
        "slippage_revert": slippage,
    }))
}

async fn scenario_large_usdc(ctx: &Ctx) -> anyhow::Result<(Address, Value)> {
    let core = &ctx.core;
    let (token, _) = scenario_create_usdc(ctx).await?;
    mine_past_penalty(core, token).await?;
    let mut buys = Vec::new();
    for pct in [10u64, 50, 90] {
        let (tokens_left, quote_left) = core.v2().available_buy_tokens(token).await?;
        let amount_in = quote_left * U256::from(pct) / U256::from(100);
        let mut leg = usdc_buy(ctx, token, amount_in).await?;
        leg["pct_of_remaining_quote"] = json!(pct);
        leg["remaining_before"] = json!({
            "tokens": amt(tokens_left, TOKEN_DECIMALS),
            "quote": amt(quote_left, USDC_DECIMALS),
        });
        buys.push(leg);
    }
    anyhow::ensure!(!core.v2().is_graduated(token).await?, "graduated early");
    let wallet_tokens = balance_of(core, token, ctx.wallet).await;
    let sell = usdc_sell(ctx, token, wallet_tokens * U256::from(9) / U256::from(10)).await?;
    Ok((
        token,
        json!({ "token": token.to_string(), "buys": buys, "sell_90pct_of_wallet": sell }),
    ))
}

async fn scenario_graduation_boundary(ctx: &Ctx, token: Address) -> anyhow::Result<Value> {
    let core = &ctx.core;
    let (tokens_left, quote_left) = core.v2().available_buy_tokens(token).await?;
    let amount_in = quote_left * U256::from(3) / U256::from(2); // 150% of what is left
    approve(core, ctx.usdc, ctx.router, amount_in).await?;
    let preview = core.v2().get_amount_out(token, amount_in, true).await?;
    anyhow::ensure!(
        preview == tokens_left,
        "preview {preview} != available_buy_tokens {tokens_left}"
    );
    let token_before = balance_of(core, token, ctx.wallet).await;
    let usdc_before = balance_of(core, ctx.usdc, ctx.wallet).await;
    let tx = core
        .v2()
        .buy(V2BuyParams {
            token,
            to: ctx.wallet,
            amount_in,
            amount_out_min: preview,
            deadline: U256::from(DEADLINE),
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await?;
    wait_ok(core, tx).await?;
    let received = balance_of(core, token, ctx.wallet).await - token_before;
    let spent = usdc_before - balance_of(core, ctx.usdc, ctx.wallet).await;
    anyhow::ensure!(
        received == tokens_left,
        "received {received} != available {tokens_left}"
    );
    anyhow::ensure!(
        spent == quote_left,
        "spent {spent} != required quote {quote_left}"
    );
    anyhow::ensure!(
        core.v2().is_graduated(token).await?,
        "not graduated after boundary buy"
    );
    assert_router_clean(ctx, token).await?;

    // Post-graduation: the same SDK calls now route to the NadFunPair.
    let reserves_before = core.v2().get_reserves(token).await?;
    let dex_buy = usdc_buy(ctx, token, U256::from(100_000_000u64)).await?; // 100 USDC
    let reserves_mid = core.v2().get_reserves(token).await?;
    anyhow::ensure!(
        (reserves_mid.reserve0, reserves_mid.reserve1)
            != (reserves_before.reserve0, reserves_before.reserve1),
        "post-graduation buy did not move the pair reserves"
    );
    let bought = dex_buy["received_tokens"]["raw"]
        .as_str()
        .unwrap()
        .parse::<U256>()?;
    let dex_sell = usdc_sell(ctx, token, bought / U256::from(2)).await?;
    let reserves_after = core.v2().get_reserves(token).await?;
    anyhow::ensure!(
        (reserves_after.reserve0, reserves_after.reserve1)
            != (reserves_mid.reserve0, reserves_mid.reserve1),
        "post-graduation sell did not move the pair reserves"
    );
    Ok(json!({
        "observed": "partial fill + refund: router takes getAmountIn(remaining tokens), refunds the rest, curve graduates in the same tx",
        "boundary_tx": tx.to_string(),
        "usdc_in": amt(amount_in, USDC_DECIMALS),
        "required_quote_available_buy_tokens": amt(quote_left, USDC_DECIMALS),
        "usdc_spent": amt(spent, USDC_DECIMALS),
        "usdc_refunded": amt(amount_in - spent, USDC_DECIMALS),
        "available_tokens": amt(tokens_left, TOKEN_DECIMALS),
        "preview_tokens": amt(preview, TOKEN_DECIMALS),
        "received_tokens": amt(received, TOKEN_DECIMALS),
        "graduated": true,
        "pair_reserves_before": [reserves_before.reserve0.to_string(), reserves_before.reserve1.to_string()],
        "dex_buy": dex_buy,
        "dex_sell": dex_sell,
        "pair_reserves_after": [reserves_after.reserve0.to_string(), reserves_after.reserve1.to_string()],
    }))
}

/// Native MON paid for gas by `tx`.
async fn gas_cost(ctx: &Ctx, tx: B256) -> anyhow::Result<U256> {
    let r = ctx
        .core
        .provider()
        .get_transaction_receipt(tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no receipt for {tx}"))?;
    Ok(U256::from(r.gas_used) * U256::from(r.effective_gas_price))
}

async fn scenario_large_wmon(ctx: &Ctx) -> anyhow::Result<Value> {
    let core = &ctx.core;
    let (token, _) = create_wmon_token(ctx).await?;
    mine_past_penalty(core, token).await?;

    let (_, quote_left) = core.v2().available_buy_tokens(token).await?;
    let value = quote_left / U256::from(2);
    let preview = core.v2().get_amount_out(token, value, true).await?;
    let buy_params = |amount_out_min| V2BuyWithNativeParams {
        token,
        to: ctx.wallet,
        amount_out_min,
        deadline: U256::from(DEADLINE),
        value,
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    let buy_slippage = assert_slippage_reverts(
        ctx,
        V2GasEstimationParams::BuyWithNative(buy_params(preview + U256::from(1))),
    )
    .await?;
    let token_before = balance_of(core, token, ctx.wallet).await;
    let native_before = core.provider().get_balance(ctx.wallet).await?;
    let buy = core.v2().buy_with_native(buy_params(preview)).await?;
    wait_ok(core, buy).await?;
    let received = balance_of(core, token, ctx.wallet).await - token_before;
    let native_spent =
        native_before - core.provider().get_balance(ctx.wallet).await? - gas_cost(ctx, buy).await?;
    anyhow::ensure!(received == preview, "buy {received} != preview {preview}");
    anyhow::ensure!(
        native_spent <= value,
        "spent {native_spent} > value {value}"
    );
    assert_router_clean(ctx, token).await?;

    let sell_amount = balance_of(core, token, ctx.wallet).await * U256::from(9) / U256::from(10);
    approve(core, token, ctx.router, sell_amount).await?;
    let sell_preview = core.v2().get_amount_out(token, sell_amount, false).await?;
    let sell_params = |amount_out_min| V2SellToNativeParams {
        token,
        to: ctx.wallet,
        amount_in: sell_amount,
        amount_out_min,
        deadline: U256::from(DEADLINE),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    let sell_slippage = assert_slippage_reverts(
        ctx,
        V2GasEstimationParams::SellToNative(sell_params(sell_preview + U256::from(1))),
    )
    .await?;
    let native_before = core.provider().get_balance(ctx.wallet).await?;
    let sell = core.v2().sell_to_native(sell_params(sell_preview)).await?;
    wait_ok(core, sell).await?;
    let native_received =
        core.provider().get_balance(ctx.wallet).await? + gas_cost(ctx, sell).await? - native_before;
    anyhow::ensure!(
        native_received == sell_preview,
        "sell {native_received} != preview {sell_preview}"
    );
    assert_router_clean(ctx, token).await?;
    Ok(json!({
        "token": token.to_string(),
        "remaining_quote_before": amt(quote_left, TOKEN_DECIMALS),
        "buy": {
            "tx": buy.to_string(),
            "mon_in": amt(value, TOKEN_DECIMALS),
            "mon_spent_ex_gas": amt(native_spent, TOKEN_DECIMALS),
            "preview_tokens": amt(preview, TOKEN_DECIMALS),
            "received_tokens": amt(received, TOKEN_DECIMALS),
            "slippage_revert": buy_slippage,
        },
        "sell": {
            "tx": sell.to_string(),
            "tokens_in": amt(sell_amount, TOKEN_DECIMALS),
            "preview_mon": amt(sell_preview, TOKEN_DECIMALS),
            "received_mon_ex_gas": amt(native_received, TOKEN_DECIMALS),
            "slippage_revert": sell_slippage,
        },
    }))
}

fn record(report: &mut Value, name: &str, result: anyhow::Result<Value>) -> bool {
    let (pass, entry) = match result {
        Ok(details) => (
            true,
            json!({ "name": name, "result": "PASS", "details": details }),
        ),
        Err(e) => (
            false,
            json!({ "name": name, "result": "FAIL", "error": format!("{e:#}") }),
        ),
    };
    println!("[e2e] {name}: {}", if pass { "PASS" } else { "FAIL" });
    report["scenarios"].as_array_mut().unwrap().push(entry);
    pass
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // anvil testnet fork + state-changing txs; run via scripts/e2e-fork.sh
async fn v2_multi_quote_fork_e2e() {
    let (Some(rpc), Some(pk)) = (env("E2E_RPC_URL"), env("E2E_PRIVATE_KEY")) else {
        panic!("E2E_RPC_URL and E2E_PRIVATE_KEY are required (run scripts/e2e-fork.sh)");
    };
    let report_path = env("E2E_REPORT_PATH")
        .unwrap_or_else(|| format!("{}/target/e2e-fork-report.json", env!("CARGO_MANIFEST_DIR")));
    let core = Core::new(rpc, pk, Network::Testnet).await.expect("core");
    // Refuse to send anything unless this is an anvil fork of Monad testnet.
    let chain_id = core.provider().get_chain_id().await.expect("chain_id");
    assert_eq!(
        chain_id, 10143,
        "E2E_RPC_URL is not Monad testnet (chain {chain_id})"
    );
    let node_info: Value = core
        .provider()
        .raw_request("anvil_nodeInfo".into(), ())
        .await
        .expect("anvil_nodeInfo failed: E2E_RPC_URL must be an anvil fork, not a live node");
    let wallet = core.wallet_address();
    let router = core.v2().router().address;
    let usdc = addr(
        quote_tokens(Network::Testnet)
            .iter()
            .find(|q| q.symbol == "USDC")
            .expect("testnet USDC quote token")
            .address,
    );
    let wmon = core.v2().wrapped_native().await.expect("wrapped_native");
    let fork_block = core.provider().get_block_number().await.expect("block");

    let mut report = json!({
        "network": "monad-testnet-fork",
        "fork_block_pin": env("E2E_FORK_BLOCK"),
        "fork_head_block_at_start": fork_block,
        "anvil_fork_block": node_info["forkConfig"]["forkBlockNumber"],
        "wallet": wallet.to_string(),
        "router": router.to_string(),
        "usdc": usdc.to_string(),
        "wmon": wmon.to_string(),
        "scenarios": [],
    });
    let ctx = Ctx {
        core,
        wallet,
        router,
        usdc,
        wmon,
    };

    let mut all = true;
    let usdc_token = match scenario_create_usdc(&ctx).await {
        Ok((token, details)) => {
            record(&mut report, "1_erc20_create_usdc_quoted", Ok(details));
            Some(token)
        }
        Err(e) => {
            all &= record(&mut report, "1_erc20_create_usdc_quoted", Err(e));
            None
        }
    };
    if let Some(token) = usdc_token {
        all &= record(
            &mut report,
            "2_erc20_buy_sell_usdc_match_preview",
            scenario_buy_sell_usdc(&ctx, token).await,
        );
    }
    all &= record(
        &mut report,
        "3_legacy_wmon_native_create_and_buy",
        scenario_legacy_wmon(&ctx).await,
    );
    match scenario_large_usdc(&ctx).await {
        Ok((token, details)) => {
            all &= record(&mut report, "7_large_usdc_curve_trades", Ok(details));
            all &= record(
                &mut report,
                "8_graduation_boundary_and_dex",
                scenario_graduation_boundary(&ctx, token).await,
            );
        }
        Err(e) => all &= record(&mut report, "7_large_usdc_curve_trades", Err(e)),
    }
    all &= record(
        &mut report,
        "9_large_wmon_native_buy_sell",
        scenario_large_wmon(&ctx).await,
    );
    // Last: the provider's cached nonce manager consumes a nonce on a failed
    // estimate, so any tx sent after this expected revert would stall.
    if let Some(token) = usdc_token {
        all &= record(
            &mut report,
            "4_buy_with_native_on_usdc_reverts_must_use_route",
            scenario_native_reverts(&ctx, token).await,
        );
    }

    let mut api = ApiClient::new(Network::Testnet);
    if let Some(url) = env("E2E_API_URL") {
        api = api.with_api_url(url);
    }
    match scenario_api_catalog(&api).await {
        Err(e) if e.downcast_ref::<reqwest::Error>().is_some() => {
            println!("[e2e] 5_api_quote_tokens_match_constants: SKIP ({e:#})");
            report["scenarios"].as_array_mut().unwrap().push(json!({
                "name": "5_api_quote_tokens_match_constants",
                "result": "SKIP",
                "error": format!("{e:#}"),
            }));
        }
        result => all &= record(&mut report, "5_api_quote_tokens_match_constants", result),
    }

    match scenario_api_detection(&ctx, &api, usdc_token).await {
        Err(e) if e.downcast_ref::<reqwest::Error>().is_some() => {
            println!("[e2e] 6_api_detection: SKIP ({e:#})");
            report["scenarios"].as_array_mut().unwrap().push(json!({
                "name": "6_api_detection",
                "result": "SKIP",
                "error": format!("{e:#}"),
            }));
        }
        result => all &= record(&mut report, "6_api_detection", result),
    }

    report["all_pass"] = json!(all);
    std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap())
        .expect("write report");
    println!("[e2e] report -> {report_path}");
    assert!(
        all,
        "fork E2E failed: {}",
        serde_json::to_string_pretty(&report).unwrap()
    );
}
