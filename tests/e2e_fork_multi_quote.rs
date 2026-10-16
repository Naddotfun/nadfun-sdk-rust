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
//!   4. `buy_with_native` on the USDC-quoted token reverts `NativeQuoteMustUseRoute`
//!      (runs before 3/7/8/9, which then send on the same `Core`)
//!   5. live api-server `GET /quote_token` matches `quote_tokens(Testnet)` (SKIP if unreachable)
//!   6. `detect_token_info_with_api` with the live dev api-server (SKIP if unreachable):
//!      a pre-fork V2 token and the fork-only USDC token resolve from the chain as V2
//!      (matching the testnet lens); an unregistered address is an API 404 -> `None`
//!   7. large USDC curve trades on a fresh USDC token: buys of ~10% / ~50% / ~90% of the
//!      remaining quote-to-graduation, then a sell of 90% of the wallet's tokens
//!   8. graduation boundary: a buy above the remaining quote (partial fill + refund), then
//!      post-graduation ERC-20 buy/sell routed to the DEX pair
//!   9. large WMON-quoted `buy_with_native` / `sell_to_native` pair
//!  10. one `Core`: a send that reverts at gas estimation, then one the node
//!      rejects, then a concurrent pair, then `TokenHelper` + `Core` on the same key
//!      interleaved, then a caller-supplied nonce followed by an automatic one — every
//!      valid send mines with the next nonce and no two sends share a nonce; finally a
//!      tx dropped from the pool is recovered after the 30 s stall window, and after a
//!      refused send plus 30 s idle two sends with nothing mined get distinct nonces
//!
//! Every trade in 7-9: fill == `get_amount_out` preview, `amount_out_min = preview + 1`
//! reverts `InsufficientOutput()` (checked via `estimate_gas`, which sends nothing), and
//! the router holds no token / USDC / WMON / native afterwards. Those trades send an
//! explicit `gas_limit` (`gas_with_buffer`): bonding curve estimate + 10 %,
//! graduated (DEX) estimate + 50_000 — larger than the documented flat +10 %
//! because anvil uses the Ethereum gas schedule, not Monad's.
//!
//! `E2E_ONLY=8` runs only the listed scenarios (plus prerequisites);
//! `E2E_PROBE_GAS_TS=1` additionally records `probe_gas_vs_timestamp`, the evidence that
//! a bare estimate can run out of gas one second later on a DEX trade.

use alloy::primitives::{
    keccak256,
    utils::{format_units, parse_ether},
    Address, B256, U256,
};
use alloy::providers::Provider;
use alloy::sol;
use nadfun_sdk::types::v2::events::V2EventType;
use nadfun_sdk::{
    quote_tokens, ApiClient, Core, Network, SdkVersion, TokenHelper, V2BuyParams,
    V2BuyWithNativeParams, V2CreateParams, V2CreateWithNativeParams, V2DexType,
    V2GasEstimationParams, V2SellParams, V2SellToNativeParams, V2VaultAllocation,
};
use serde_json::{json, Value};
use std::time::Duration;

sol! {
    // Router trade calldata for the gas probe (`INadFunRouter.BuyParams` / `SellParams`).
    struct TradeParams {
        uint256 amountIn;
        uint256 amountOutMin;
        address token;
        address to;
        uint256 deadline;
    }
    function buy(TradeParams params) external returns (uint256);
    function sell(TradeParams params) external returns (uint256);
    function getCurrentBlockTimestamp() external view returns (uint256);
}

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
            if !r.status {
                anyhow::bail!("tx {tx} reverted: {}", revert_details(core, tx).await);
            }
            return Ok(r);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    anyhow::bail!("receipt not found for {tx}")
}

/// Why `tx` reverted, from the fork's call tracer (error, revert reason, gas).
async fn revert_details(core: &Core, tx: B256) -> String {
    let trace: Result<Value, _> = core
        .provider()
        .raw_request(
            "debug_traceTransaction".into(),
            (tx, json!({ "tracer": "callTracer" })),
        )
        .await;
    match trace {
        Ok(t) => {
            // Innermost failing frame first, then the top-level summary.
            fn failing(frame: &Value) -> Option<&Value> {
                frame["calls"]
                    .as_array()
                    .and_then(|calls| calls.iter().rev().find_map(failing))
                    .or_else(|| frame.get("error").map(|_| frame))
            }
            let inner = failing(&t).unwrap_or(&t);
            json!({
                "error": t["error"],
                "revertReason": t["revertReason"],
                "gas": t["gas"],
                "gasUsed": t["gasUsed"],
                "to": t["to"],
                "selector": t["input"].as_str().map(|i| &i[..10.min(i.len())]),
                "innermost": {
                    "to": inner["to"],
                    "error": inner["error"],
                    "revertReason": inner["revertReason"],
                    "output": inner["output"],
                    "selector": inner["input"].as_str().map(|i| &i[..10.min(i.len())]),
                },
            })
            .to_string()
        }
        Err(e) => format!("(trace unavailable: {e})"),
    }
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
    /// Fork RPC + throwaway key, for scenarios that need their own `Core`.
    rpc: String,
    pk: String,
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
    create_wmon_token_on(&ctx.core, ctx.wallet, ctx.wmon).await
}

async fn create_wmon_token_on(
    core: &Core,
    wallet: Address,
    wmon: Address,
) -> anyhow::Result<(Address, B256)> {
    let buy_quote_amount = parse_ether("1")?;
    let deploy_fee = core.v2().deploy_fee(wmon).await?;
    let nonce = core.provider().get_transaction_count(wallet).await?;
    let tx = core
        .v2()
        .create_with_native(V2CreateWithNativeParams {
            name: format!("E2E Legacy {nonce}"),
            symbol: format!("EL{nonce}"),
            token_uri: "ipfs://e2e-legacy".to_string(),
            quote_token: wmon,
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
/// router's `InsufficientOutput()` (checked by estimation, which sends nothing).
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

/// Explicit gas limit for fork trades: bonding curve estimate + 10 %, graduated
/// (DEX) estimate + 50_000. Larger than the docs' flat +10 % on purpose: anvil
/// runs the Ethereum gas schedule, not Monad's, and there the first DEX trade
/// after graduation needs +15.6 % (~48 k) once the block timestamp moves past
/// the pair's last update (cold zero → nonzero accumulator SSTOREs). On Monad
/// the same case measured +6.22 % (testnet) / +6.34 % (mainnet), which the
/// documented +10 % covers.
async fn gas_with_buffer(
    core: &Core,
    token: Address,
    params: V2GasEstimationParams,
) -> anyhow::Result<u64> {
    let estimate = core.v2().estimate_gas(params).await?;
    Ok(if core.v2().is_graduated(token).await? {
        estimate + 50_000
    } else {
        estimate + estimate / 10
    })
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
    let mut buy = params(preview);
    buy.gas_limit =
        Some(gas_with_buffer(core, token, V2GasEstimationParams::Buy(buy.clone())).await?);
    let tx = core.v2().buy(buy).await?;
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
    let mut sell = params(preview);
    sell.gas_limit =
        Some(gas_with_buffer(core, token, V2GasEstimationParams::Sell(sell.clone())).await?);
    let tx = core.v2().sell(sell).await?;
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
    let mut boundary = V2BuyParams {
        token,
        to: ctx.wallet,
        amount_in,
        amount_out_min: preview,
        deadline: U256::from(DEADLINE),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    boundary.gas_limit =
        Some(gas_with_buffer(core, token, V2GasEstimationParams::Buy(boundary.clone())).await?);
    let tx = core.v2().buy(boundary).await?;
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

    // Opt-in: gas at T vs T + 1 for the first DEX trades after graduation, while
    // the pair's price accumulators are still zero.
    let first_dex_gas = if env("E2E_PROBE_GAS_TS").is_some() {
        measure_gas_vs_timestamp(ctx, token, None).await?
    } else {
        Value::Null
    };

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
        "gas_measure_first_dex_trade_after_graduation": first_dex_gas,
        "dex_buy": dex_buy,
        "dex_sell": dex_sell,
        "pair_reserves_after": [reserves_after.reserve0.to_string(), reserves_after.reserve1.to_string()],
    }))
}

/// Diagnostic for the scenario-8 flake (`E2E_PROBE_GAS_TS=1`): a NadFunPair
/// swap costs more gas when the block timestamp moved since the pair's last
/// update (price accumulators are written). Estimate at the pair's own
/// timestamp, execute one second later with exactly that gas.
async fn probe_gas_vs_timestamp(ctx: &Ctx, token: Address) -> anyhow::Result<Value> {
    let core = &ctx.core;
    let rpc = |method: &'static str, params: Value| async move {
        core.provider()
            .raw_request::<_, Value>(method.into(), params)
            .await
    };
    let amount = U256::from(1_000_000u64); // 1 USDC per swap
    approve(core, ctx.usdc, ctx.router, amount * U256::from(4)).await?;
    let params = |gas_limit| V2BuyParams {
        token,
        to: ctx.wallet,
        amount_in: amount,
        amount_out_min: U256::ZERO,
        deadline: U256::from(DEADLINE),
        gas_limit,
        gas_price: None,
        nonce: None,
    };
    let outcome = |r: anyhow::Result<nadfun_sdk::TransactionResult>| match r {
        Ok(_) => "mined".to_string(),
        Err(e) => format!("{e:#}"),
    };
    // Stamp the pair with the latest block's timestamp T.
    wait_ok(core, core.v2().buy(params(None)).await?).await?;
    let t = core.v2().get_reserves(token).await?.block_timestamp_last as u64;
    // Gas needed when no time passed since the pair's last update.
    rpc("evm_setNextBlockTimestamp", json!([t])).await?;
    let g = core
        .v2()
        .estimate_gas(V2GasEstimationParams::Buy(params(None)))
        .await?;
    // (1) same second as the pair's last update, exactly `g` gas.
    rpc("evm_setNextBlockTimestamp", json!([t])).await?;
    let same = outcome(wait_ok(core, core.v2().buy(params(Some(g))).await?).await);
    // (2) one second later, exactly `g` gas.
    rpc("evm_setNextBlockTimestamp", json!([t + 1])).await?;
    let later = outcome(wait_ok(core, core.v2().buy(params(Some(g))).await?).await);
    let used_later = {
        let r = core
            .provider()
            .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
            .await?;
        r.map(|b| b.header.gas_used)
    };
    Ok(json!({
        "pair_timestamp": t,
        "gas_estimate": g,
        "exec_same_second_with_estimate": same,
        "exec_next_second_with_estimate": later,
        "block_gas_used_next_second": used_later,
    }))
}

/// `eth_call` of `data` from the wallet with exactly `gas`, as if mined at
/// block timestamp `time` (block override). `Ok` = the call succeeded.
async fn call_at(
    ctx: &Ctx,
    to: Address,
    data: &[u8],
    gas: u64,
    time: u64,
) -> Result<Value, alloy::transports::TransportError> {
    let tx = json!({
        "from": ctx.wallet,
        "to": to,
        "data": alloy::primitives::hex::encode_prefixed(data),
        "gas": format!("0x{gas:x}"),
    });
    ctx.core
        .provider()
        .raw_request(
            "eth_call".into(),
            (
                tx,
                "latest",
                json!({}),
                json!({ "time": format!("0x{time:x}") }),
            ),
        )
        .await
}

/// Smallest gas limit with which `data` succeeds at block timestamp `time`.
async fn min_gas_at(ctx: &Ctx, data: &[u8], time: u64) -> anyhow::Result<u64> {
    let (mut lo, mut hi) = (21_000u64, 3_000_000u64);
    anyhow::ensure!(
        call_at(ctx, ctx.router, data, hi, time).await.is_ok(),
        "fails even with {hi} gas: {:?}",
        call_at(ctx, ctx.router, data, hi, time).await.err()
    );
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if call_at(ctx, ctx.router, data, mid, time).await.is_ok() {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(hi)
}

/// Minimum successful gas at timestamp T vs T + 1 for DEX and curve buy/sell,
/// against the default `estimate_gas` and 7.5 % / 10 % buffers.
async fn measure_gas_vs_timestamp(
    ctx: &Ctx,
    dex_token: Address,
    curve_token: Option<Address>,
) -> anyhow::Result<Value> {
    use alloy::sol_types::SolCall;
    let core = &ctx.core;
    // The block override must reach `block.timestamp`.
    let multicall = addr("0xcA11bde05977b3631167028862bE2a173976CA11");
    let probe_ts = 1_900_000_000u64;
    let echoed = ctx
        .core
        .provider()
        .raw_request::<_, Value>(
            "eth_call".into(),
            (
                json!({ "to": multicall, "data": alloy::primitives::hex::encode_prefixed(getCurrentBlockTimestampCall {}.abi_encode()) }),
                "latest",
                json!({}),
                json!({ "time": format!("0x{probe_ts:x}") }),
            ),
        )
        .await?;
    let echoed = U256::from_str_radix(
        echoed.as_str().unwrap_or("0x0").trim_start_matches("0x"),
        16,
    )?;
    anyhow::ensure!(
        echoed == U256::from(probe_ts),
        "time override ignored: {echoed}"
    );

    let usdc_in = U256::from(1_000_000u64); // 1 USDC
    approve(core, ctx.usdc, ctx.router, usdc_in * U256::from(4)).await?;
    let dex_tokens = balance_of(core, dex_token, ctx.wallet).await / U256::from(10);
    approve(core, dex_token, ctx.router, dex_tokens).await?;
    let curve_tokens = match curve_token {
        Some(t) => {
            let amount = balance_of(core, t, ctx.wallet).await / U256::from(10);
            approve(core, t, ctx.router, amount).await?;
            amount
        }
        None => U256::ZERO,
    };

    let latest = core
        .provider()
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no latest block"))?
        .header
        .timestamp;
    let pair_ts = core
        .v2()
        .get_reserves(dex_token)
        .await?
        .block_timestamp_last as u64;
    let trade = |token, amount_in| TradeParams {
        amountIn: amount_in,
        amountOutMin: U256::ZERO,
        token,
        to: ctx.wallet,
        deadline: U256::from(DEADLINE),
    };
    let curve_token = curve_token.unwrap_or_default();
    let mut cases = vec![
        (
            "dex_buy",
            buyCall {
                params: trade(dex_token, usdc_in),
            }
            .abi_encode(),
            pair_ts,
            V2GasEstimationParams::Buy(V2BuyParams {
                token: dex_token,
                to: ctx.wallet,
                amount_in: usdc_in,
                amount_out_min: U256::ZERO,
                deadline: U256::from(DEADLINE),
                gas_limit: None,
                gas_price: None,
                nonce: None,
            }),
        ),
        (
            "dex_sell",
            sellCall {
                params: trade(dex_token, dex_tokens),
            }
            .abi_encode(),
            pair_ts,
            V2GasEstimationParams::Sell(V2SellParams {
                token: dex_token,
                to: ctx.wallet,
                amount_in: dex_tokens,
                amount_out_min: U256::ZERO,
                deadline: U256::from(DEADLINE),
                gas_limit: None,
                gas_price: None,
                nonce: None,
            }),
        ),
        (
            "curve_buy",
            buyCall {
                params: trade(curve_token, usdc_in),
            }
            .abi_encode(),
            latest,
            V2GasEstimationParams::Buy(V2BuyParams {
                token: curve_token,
                to: ctx.wallet,
                amount_in: usdc_in,
                amount_out_min: U256::ZERO,
                deadline: U256::from(DEADLINE),
                gas_limit: None,
                gas_price: None,
                nonce: None,
            }),
        ),
        (
            "curve_sell",
            sellCall {
                params: trade(curve_token, curve_tokens),
            }
            .abi_encode(),
            latest,
            V2GasEstimationParams::Sell(V2SellParams {
                token: curve_token,
                to: ctx.wallet,
                amount_in: curve_tokens,
                amount_out_min: U256::ZERO,
                deadline: U256::from(DEADLINE),
                gas_limit: None,
                gas_price: None,
                nonce: None,
            }),
        ),
    ];
    if curve_token == Address::ZERO {
        cases.truncate(2);
    }
    let mut rows = Vec::new();
    for (name, data, t, est_params) in cases {
        let at_t = min_gas_at(ctx, &data, t).await?;
        let at_t1 = min_gas_at(ctx, &data, t + 1).await?;
        let estimate = core.v2().estimate_gas(est_params).await?;
        let extra = at_t1 as i64 - at_t as i64;
        rows.push(json!({
            "op": name,
            "t": t,
            "min_gas_at_t": at_t,
            "min_gas_at_t_plus_1": at_t1,
            "extra_gas": extra,
            "extra_pct_of_min_at_t": format!("{:.2}", extra as f64 * 100.0 / at_t as f64),
            "default_estimate_now": estimate,
            "min_at_t_plus_7_5pct_covers_t1": at_t + at_t * 75 / 1000 >= at_t1,
            "min_at_t_plus_10pct_covers_t1": at_t + at_t / 10 >= at_t1,
        }));
    }
    Ok(json!({ "time_override_verified": true, "rows": rows }))
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
    let mut native_buy = buy_params(preview);
    native_buy.gas_limit = Some(
        gas_with_buffer(
            core,
            token,
            V2GasEstimationParams::BuyWithNative(native_buy.clone()),
        )
        .await?,
    );
    let buy = core.v2().buy_with_native(native_buy).await?;
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
    let mut native_sell = sell_params(sell_preview);
    native_sell.gas_limit = Some(
        gas_with_buffer(
            core,
            token,
            V2GasEstimationParams::SellToNative(native_sell.clone()),
        )
        .await?,
    );
    let sell = core.v2().sell_to_native(native_sell).await?;
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

/// Account nonce (mined transaction count) of `who`.
async fn mined_nonce(core: &Core, who: Address) -> anyhow::Result<u64> {
    Ok(core.provider().get_transaction_count(who).await?)
}

/// Nonce the mined transaction `tx` was sent with.
async fn tx_nonce(core: &Core, tx: B256) -> anyhow::Result<u64> {
    use alloy::consensus::Transaction as _;
    let t = core
        .provider()
        .get_transaction_by_hash(tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("tx {tx} not found"))?;
    Ok(t.nonce())
}

fn native_buy(token: Address, to: Address, value: U256) -> V2BuyWithNativeParams {
    V2BuyWithNativeParams {
        token,
        to,
        amount_out_min: U256::ZERO,
        deadline: U256::from(DEADLINE),
        value,
        gas_limit: None,
        gas_price: None,
        nonce: None,
    }
}

/// A failed send must not consume a nonce on the SDK's signing provider.
/// Runs on a fresh `Core` (one provider, one nonce cache) so earlier
/// scenarios cannot influence it.
async fn scenario_nonce_after_failed_sends(
    ctx: &Ctx,
    usdc_token: Address,
) -> anyhow::Result<Value> {
    let core = Core::new(ctx.rpc.clone(), ctx.pk.clone(), Network::Testnet).await?;
    let wallet = ctx.wallet;
    let n0 = mined_nonce(&core, wallet).await?;

    // (a) reverts at gas estimation (USDC-quoted token on the native path).
    let estimate_err = core
        .v2()
        .buy_with_native(native_buy(usdc_token, wallet, parse_ether("1")?))
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("estimate-reverting send was accepted"))?;
    // (b) next valid send must mine with the very next nonce.
    let (token, create_tx) = create_wmon_token_on(&core, wallet, ctx.wmon).await?;
    let n1 = mined_nonce(&core, wallet).await?;
    anyhow::ensure!(
        n1 == n0 + 1,
        "after estimate failure: nonce {n0} -> {n1}, want +1"
    );

    // (c) node rejects the signed tx (value > balance, explicit gas skips estimation).
    let balance = core.provider().get_balance(wallet).await?;
    let mut too_big = native_buy(token, wallet, balance + parse_ether("1")?);
    too_big.gas_limit = Some(1_000_000);
    let rpc_err = core
        .v2()
        .buy_with_native(too_big)
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("over-balance send was accepted"))?;
    let buy = core
        .v2()
        .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?))
        .await?;
    wait_ok(&core, buy).await?;
    let n2 = mined_nonce(&core, wallet).await?;
    anyhow::ensure!(
        n2 == n1 + 1,
        "after RPC rejection: nonce {n1} -> {n2}, want +1"
    );

    // (d) two sends issued concurrently must get distinct nonces and both mine.
    let (a, b) = tokio::join!(
        core.v2()
            .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?)),
        core.v2()
            .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?)),
    );
    let (a, b) = (a?, b?);
    wait_ok(&core, a).await?;
    wait_ok(&core, b).await?;
    let (na, nb) = (tx_nonce(&core, a).await?, tx_nonce(&core, b).await?);
    let n3 = mined_nonce(&core, wallet).await?;
    anyhow::ensure!(na != nb, "concurrent sends reused nonce {na}");
    anyhow::ensure!(
        n3 == n2 + 2,
        "after concurrent pair: nonce {n2} -> {n3}, want +2"
    );

    // (e) TokenHelper and Core on the same key, interleaved: approve via the
    // helper, sell via Core, approve again, buy via Core — all must mine.
    let helper = TokenHelper::new(ctx.rpc.clone(), ctx.pk.clone()).await?;
    let sell_amount = balance_of(&core, token, wallet).await / U256::from(4);
    let approve1 = helper.approve(token, ctx.router, sell_amount).await?;
    let sell = core
        .v2()
        .sell(V2SellParams {
            token,
            to: wallet,
            amount_in: sell_amount,
            amount_out_min: U256::ZERO,
            deadline: U256::from(DEADLINE),
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await?;
    wait_ok(&core, sell).await?;
    let approve2 = helper.approve(token, ctx.router, U256::ZERO).await?;
    let buy2 = core
        .v2()
        .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?))
        .await?;
    wait_ok(&core, buy2).await?;
    let n4 = mined_nonce(&core, wallet).await?;
    anyhow::ensure!(
        n4 == n3 + 4,
        "after helper/core interleave: nonce {n3} -> {n4}, want +4"
    );
    let interleave_nonces = [
        tx_nonce(&core, approve1).await?,
        tx_nonce(&core, sell).await?,
        tx_nonce(&core, approve2).await?,
        tx_nonce(&core, buy2).await?,
    ];

    // (f) caller-supplied nonce, then an automatic one.
    let mut fixed = native_buy(token, wallet, parse_ether("0.1")?);
    fixed.nonce = Some(n4);
    let fixed_tx = core.v2().buy_with_native(fixed).await?;
    wait_ok(&core, fixed_tx).await?;
    let auto_tx = core
        .v2()
        .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?))
        .await?;
    wait_ok(&core, auto_tx).await?;
    let n5 = mined_nonce(&core, wallet).await?;
    anyhow::ensure!(
        n5 == n4 + 2,
        "after caller nonce + auto: nonce {n4} -> {n5}, want +2"
    );

    // (g) a tx the node accepted and then dropped: with automine off, send,
    // drop it from the pool, re-enable mining and wait past the SDK's 30 s
    // stall window — the next send must reuse the dropped nonce and mine.
    let rpc = |method: &'static str, params: Value| {
        let core = &core;
        async move {
            core.provider()
                .raw_request::<_, Value>(method.into(), params)
                .await
        }
    };
    rpc("evm_setAutomine", json!([false])).await?;
    let dropped = core
        .v2()
        .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?))
        .await;
    rpc("anvil_dropTransaction", json!([dropped.as_ref().ok()])).await?;
    rpc("evm_setAutomine", json!([true])).await?;
    let dropped = dropped?;
    anyhow::ensure!(
        core.provider()
            .get_transaction_by_hash(dropped)
            .await?
            .is_none(),
        "dropped tx {dropped} is still known to the node"
    );
    tokio::time::sleep(Duration::from_secs(31)).await;
    let recovered = core
        .v2()
        .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?))
        .await?;
    wait_ok(&core, recovered).await?;
    let recovered_nonce = tx_nonce(&core, recovered).await?;
    let n6 = mined_nonce(&core, wallet).await?;
    anyhow::ensure!(
        recovered_nonce == n5 && n6 == n5 + 1,
        "after dropped tx: recovered with nonce {recovered_nonce}, account {n5} -> {n6}"
    );

    // (h) a refused send, idle past the stall window, then two sends while
    // nothing is mined (`latest` lags): the second must not reuse the first's
    // nonce — our own accepted send restarts the stall clock.
    let balance = core.provider().get_balance(wallet).await?;
    let mut refused = native_buy(token, wallet, balance + parse_ether("1")?);
    refused.gas_limit = Some(1_000_000);
    anyhow::ensure!(
        core.v2().buy_with_native(refused).await.is_err(),
        "over-balance send was accepted"
    );
    tokio::time::sleep(Duration::from_secs(31)).await;
    rpc("evm_setAutomine", json!([false])).await?;
    let pair = async {
        let a = core
            .v2()
            .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?))
            .await?;
        let b = core
            .v2()
            .buy_with_native(native_buy(token, wallet, parse_ether("0.1")?))
            .await?;
        anyhow::Ok((a, b))
    }
    .await;
    rpc("evm_setAutomine", json!([true])).await?;
    rpc("evm_mine", json!([])).await?;
    let (lag_a, lag_b) = pair?;
    wait_ok(&core, lag_a).await?;
    wait_ok(&core, lag_b).await?;
    let lag_nonces = [tx_nonce(&core, lag_a).await?, tx_nonce(&core, lag_b).await?];
    let n7 = mined_nonce(&core, wallet).await?;
    anyhow::ensure!(
        lag_nonces == [n6, n6 + 1] && n7 == n6 + 2,
        "after refused + idle + lagging pair: nonces {lag_nonces:?}, account {n6} -> {n7}"
    );

    Ok(json!({
        "start_nonce": n0,
        "estimate_failure": format!("{estimate_err:#}").chars().take(160).collect::<String>(),
        "create_tx": create_tx.to_string(),
        "create_tx_nonce": tx_nonce(&core, create_tx).await?,
        "nonce_after_estimate_failure_then_send": n1,
        "rpc_rejection": format!("{rpc_err:#}").chars().take(160).collect::<String>(),
        "buy_tx": buy.to_string(),
        "buy_tx_nonce": tx_nonce(&core, buy).await?,
        "nonce_after_rpc_rejection_then_send": n2,
        "concurrent_txs": [a.to_string(), b.to_string()],
        "concurrent_tx_nonces": [na, nb],
        "nonce_after_concurrent_pair": n3,
        "interleave_txs_approve_sell_approve_buy": [
            approve1.to_string(), sell.to_string(), approve2.to_string(), buy2.to_string()
        ],
        "interleave_tx_nonces": interleave_nonces,
        "nonce_after_interleave": n4,
        "caller_nonce_tx": fixed_tx.to_string(),
        "caller_nonce_tx_nonce": tx_nonce(&core, fixed_tx).await?,
        "auto_after_caller_tx": auto_tx.to_string(),
        "auto_after_caller_tx_nonce": tx_nonce(&core, auto_tx).await?,
        "dropped_tx": dropped.to_string(),
        "recovered_tx": recovered.to_string(),
        "recovered_tx_nonce": recovered_nonce,
        "lagging_pair_txs": [lag_a.to_string(), lag_b.to_string()],
        "lagging_pair_tx_nonces": lag_nonces,
        "final_nonce": n7,
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
    let core = Core::new(rpc.clone(), pk.clone(), Network::Testnet)
        .await
        .expect("core");
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
        rpc,
        pk,
        wallet,
        router,
        usdc,
        wmon,
    };

    // `E2E_ONLY=8,10` runs only those scenarios (plus what they depend on).
    let only: Option<Vec<u32>> =
        env("E2E_ONLY").map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect());
    let want = |n: u32| only.as_ref().is_none_or(|o| o.contains(&n));
    report["only"] = json!(only);

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
    if let Some(token) = usdc_token.filter(|_| want(2)) {
        all &= record(
            &mut report,
            "2_erc20_buy_sell_usdc_match_preview",
            scenario_buy_sell_usdc(&ctx, token).await,
        );
    }
    // Expected revert first: every later send on this same Core proves the
    // failed send left no nonce gap.
    if let Some(token) = usdc_token.filter(|_| want(4)) {
        all &= record(
            &mut report,
            "4_buy_with_native_on_usdc_reverts_must_use_route",
            scenario_native_reverts(&ctx, token).await,
        );
    }
    if want(3) {
        all &= record(
            &mut report,
            "3_legacy_wmon_native_create_and_buy",
            scenario_legacy_wmon(&ctx).await,
        );
    }
    if want(7) || want(8) {
        match scenario_large_usdc(&ctx).await {
            Ok((token, details)) => {
                all &= record(&mut report, "7_large_usdc_curve_trades", Ok(details));
                if want(8) {
                    all &= record(
                        &mut report,
                        "8_graduation_boundary_and_dex",
                        scenario_graduation_boundary(&ctx, token).await,
                    );
                    // Opt-in evidence for the bare-estimate DEX revert (see
                    // `probe_gas_vs_timestamp`); its result is recorded, not asserted.
                    if env("E2E_PROBE_GAS_TS").is_some() {
                        let probe = probe_gas_vs_timestamp(&ctx, token).await;
                        record(&mut report, "probe_gas_vs_timestamp", probe);
                        let table = measure_gas_vs_timestamp(&ctx, token, usdc_token).await;
                        record(&mut report, "measure_gas_vs_timestamp", table);
                    }
                }
            }
            Err(e) => all &= record(&mut report, "7_large_usdc_curve_trades", Err(e)),
        }
    }
    if want(9) {
        all &= record(
            &mut report,
            "9_large_wmon_native_buy_sell",
            scenario_large_wmon(&ctx).await,
        );
    }

    let mut api = ApiClient::new(Network::Testnet);
    if let Some(url) = env("E2E_API_URL") {
        api = api.with_api_url(url);
    }
    if want(5) {
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
    }

    if want(6) {
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
    }

    if let Some(token) = usdc_token.filter(|_| want(10)) {
        all &= record(
            &mut report,
            "10_nonce_survives_failed_sends",
            scenario_nonce_after_failed_sends(&ctx, token).await,
        );
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
