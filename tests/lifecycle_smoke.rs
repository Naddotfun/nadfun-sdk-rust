//! Live testnet lifecycle smoke — real on-chain transactions.
//!
//! Exercises the full trading lifecycle end-to-end against a live network:
//! create -> bonding-curve buy -> bonding-curve sell -> graduate -> DEX buy
//! -> DEX sell, for both v1 and v2. These are **state-changing** txs and
//! need a funded wallet, so they are `#[ignore]` by default.
//!
//! Run (testnet, funded key):
//! ```bash
//! TESTNET_RPC_URL=https://dev-node.nadapp.net/ \
//! TESTNET_PRIVATE_KEY=0x... \
//! cargo test --test lifecycle_smoke v2_lifecycle -- --ignored --nocapture
//! ```
//!
//! `NAD_API_KEY` is optional (higher rate limit for the create flow).

use alloy::primitives::{
    utils::{format_ether, parse_ether},
    Address, B256, U256,
};
use alloy::providers::Provider;
use alloy::sol;
use nadfun_sdk::stream::v2::{CurveIndexerV2, NadFunSwapIndexer};
use nadfun_sdk::types::v2::events::V2EventType;
use nadfun_sdk::{
    ActionId, ApiClient, BuyParams, Core, CreateTokenParams, GasPricing, SellParams, SlippageUtils,
    V2BuyWithNativeParams, V2CreatePayment, V2CreateTokenParams, V2DexType, V2SellToNativeParams,
    V2VaultAllocation,
};
use std::time::Duration;

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function approve(address spender, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);
    }
}

/// ERC-20 balance via `Core`'s provider (single nonce domain).
async fn balance_of(core: &Core, token: Address, owner: Address) -> U256 {
    IERC20::new(token, core.provider().clone())
        .balanceOf(owner)
        .call()
        .await
        .expect("balanceOf")
}

/// Approve the router for `amount` if the current allowance is short. Routed
/// through `Core`'s provider so it shares one nonce domain with the trades —
/// a separate `TokenHelper` instance has its own nonce cache and races.
async fn approve_if_needed(core: &Core, token: Address, spender: Address, amount: U256) {
    let erc20 = IERC20::new(token, core.provider().clone());
    let current = erc20
        .allowance(core.wallet_address(), spender)
        .call()
        .await
        .expect("allowance");
    if current < amount {
        let tx = *erc20
            .approve(spender, U256::MAX)
            .send()
            .await
            .expect("approve")
            .tx_hash();
        assert!(wait_receipt(core, tx).await, "approve reverted");
    }
}

const DEFAULT_TESTNET_RPC: &str = "https://dev-node.nadapp.net/";
const DEADLINE: u64 = 9_999_999_999;
const SLIPPAGE: f64 = 10.0;

fn rpc() -> String {
    std::env::var("TESTNET_RPC_URL").unwrap_or_else(|_| DEFAULT_TESTNET_RPC.to_string())
}

fn key() -> Option<String> {
    std::env::var("TESTNET_PRIVATE_KEY").ok()
}

/// A unique-ish suffix for token name/symbol without Date/random (forbidden
/// in some harnesses) — derived from the wallet nonce at call time.
async fn unique_suffix(core: &Core) -> u64 {
    core.provider()
        .get_transaction_count(core.wallet_address())
        .await
        .unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // live testnet, funded key + real txs
async fn v2_lifecycle() {
    let Some(pk) = key() else {
        eprintln!("skip: TESTNET_PRIVATE_KEY not set");
        return;
    };
    let core = Core::new(rpc(), pk, nadfun_sdk::Network::Testnet)
        .await
        .expect("core");
    let wallet = core.wallet_address();
    let api = ApiClient::from_env(nadfun_sdk::Network::Testnet);
    let router_v2 = core.v2().router().address;

    let start_block = core.provider().get_block_number().await.unwrap();
    let bal0 = core.provider().get_balance(wallet).await.unwrap();
    println!("wallet {wallet} balance {} MON", format_ether(bal0));
    // Graduating this curve needs ~225k MON of real buys (see step 4). Skip
    // gracefully rather than fail when the funded wallet is short.
    if bal0 <= parse_ether("250000").unwrap() {
        eprintln!(
            "skip: need >250k MON to graduate (have {})",
            format_ether(bal0)
        );
        return;
    }

    // ── 1. create ──────────────────────────────────────────────────────
    let n = unique_suffix(&core).await;
    let burn_vault: Address =
        nadfun_sdk::constants::get_burn_vault_v2(nadfun_sdk::Network::Testnet)
            .unwrap()
            .parse()
            .unwrap();
    let lp_vault: Address = nadfun_sdk::constants::get_lp_vault_v2(nadfun_sdk::Network::Testnet)
        .unwrap()
        .parse()
        .unwrap();
    let initial_buy = parse_ether("1").unwrap();
    let params = V2CreateTokenParams {
        name: format!("Lifecycle V2 {n}"),
        symbol: format!("LCV2{n}"),
        description: "v2 lifecycle smoke".to_string(),
        image_uri: "https://i.imgur.com/0qY8Vp6.png".to_string(),
        website: None,
        twitter: None,
        telegram: None,
        creator_address: wallet,
        creator_fee_rate: 100,
        vaults: vec![
            V2VaultAllocation {
                vault: burn_vault,
                bps: 5_000,
                setup_data: Default::default(),
            },
            V2VaultAllocation {
                vault: lp_vault,
                bps: 5_000,
                setup_data: Default::default(),
            },
        ],
        dex_type: V2DexType::NadFun,
        buy_quote_amount: initial_buy,
        payment: V2CreatePayment::Native,
        deadline: U256::from(DEADLINE),
        gas_limit: None,
        gas_price: Some(GasPricing::Legacy),
        nonce: None,
    };
    let created = core
        .v2()
        .create_token(params, &api)
        .await
        .expect("create_token_v2");
    let token = created.token_address;
    println!("[v2] created {token} (tx {})", created.transaction_hash);
    assert!(
        wait_receipt(&core, created.transaction_hash).await,
        "create tx reverted"
    );
    assert_eq!(
        core.detect_version(token).await.unwrap(),
        nadfun_sdk::SdkVersion::V2
    );
    assert!(
        !core.v2().is_graduated(token).await.unwrap(),
        "fresh token must be pre-graduation"
    );

    // ── 2. bonding-curve buy ───────────────────────────────────────────
    let buy_amt = parse_ether("1").unwrap();
    buy_v2_native(&core, token, buy_amt).await;
    let bal_tok = balance_of(&core, token, wallet).await;
    println!("[v2] token balance after bonding buy: {bal_tok}");
    assert!(bal_tok > U256::ZERO, "should hold tokens after buy");

    // ── 3. bonding-curve sell (approve then sell — full balance) ───────
    let sell_amt = bal_tok; // sell entire holding
    approve_if_needed(&core, token, router_v2, sell_amt).await;
    sell_v2_native(&core, token, sell_amt).await;
    println!("[v2] bonding sell ok");

    // ── 4. graduate via repeated buys ──────────────────────────────────
    // Graduation drains the curve to `minTokenReserve`, which (with the
    // testnet curve config) needs the quote reserve to reach ~295k MON — so
    // ~225k MON of real buys. Large chunks: the curve refunds the excess on
    // the buy that crosses the threshold.
    let mut graduated = core.v2().is_graduated(token).await.unwrap();
    let mut spent = U256::ZERO;
    let chunk = parse_ether("80000").unwrap();
    let cap = parse_ether("400000").unwrap();
    while !graduated && spent < cap {
        buy_v2_native(&core, token, chunk).await;
        spent += chunk;
        graduated = core.v2().is_graduated(token).await.unwrap();
        println!(
            "[v2] graduate progress: spent {} MON, graduated={graduated}",
            format_ether(spent)
        );
    }
    assert!(
        graduated,
        "token did not graduate within {} MON",
        format_ether(cap)
    );

    // ── 5. DEX buy (post-graduation, same auto-routed method) ──────────
    let pre_dex_tok = balance_of(&core, token, wallet).await;
    buy_v2_native(&core, token, parse_ether("1").unwrap()).await;
    let post_dex_tok = balance_of(&core, token, wallet).await;
    assert!(
        post_dex_tok > pre_dex_tok,
        "DEX buy should increase token balance"
    );
    println!("[v2] DEX buy ok (+{} tokens)", post_dex_tok - pre_dex_tok);

    // ── 6. DEX sell (full balance) ─────────────────────────────────────
    let dex_sell = post_dex_tok; // sell entire holding
    approve_if_needed(&core, token, router_v2, dex_sell).await;
    sell_v2_native(&core, token, dex_sell).await;
    println!("[v2] DEX sell ok");

    // ── 7. indexing: read back the real on-chain events (HTTP getLogs) ──
    // Proves the curve + DEX event decoders work against live deployed-
    // contract logs (the WS streams share these decoders).
    let curve_idx = CurveIndexerV2::new(core.provider().clone(), nadfun_sdk::Network::Testnet);
    let curve_events = curve_idx
        .fetch_all_events(
            start_block,
            10_000,
            vec![V2EventType::Buy, V2EventType::Sell, V2EventType::Graduate],
            Some(vec![token]),
        )
        .await
        .expect("curve indexer");
    let buys = curve_events
        .iter()
        .filter(|e| e.event_type() == V2EventType::Buy)
        .count();
    let sells = curve_events
        .iter()
        .filter(|e| e.event_type() == V2EventType::Sell)
        .count();
    println!(
        "[v2] indexed curve events: {buys} buys, {sells} sells, {} total",
        curve_events.len()
    );
    assert!(
        buys > 0 && sells > 0,
        "curve indexer should decode our buys + sells"
    );

    let pair = core.v2().pool_address(token).await.unwrap();
    let swap_idx = NadFunSwapIndexer::new(
        core.provider().clone(),
        vec![pair],
        nadfun_sdk::Network::Testnet,
    );
    let swaps = swap_idx
        .fetch_all_events(start_block, 10_000)
        .await
        .expect("swap indexer");
    println!("[v2] indexed {} DEX swap(s) for pair {pair}", swaps.len());
    assert!(
        !swaps.is_empty(),
        "swap indexer should decode our DEX trades"
    );

    println!("[v2] full v2 lifecycle + indexing PASSED for {token}");
}

/// `Core::get_receipt` is single-shot (returns immediately, no polling), so
/// poll it here until the tx is mined. Returns the on-chain success status.
async fn wait_receipt(core: &Core, tx: B256) -> bool {
    for _ in 0..60 {
        if let Ok(r) = core.get_receipt(tx).await {
            return r.status;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("receipt not found within 30s for {tx}");
}

async fn buy_v2_native(core: &Core, token: Address, value: U256) {
    let expected = core
        .v2()
        .get_amount_out(token, value, true)
        .await
        .expect("quote");
    assert!(expected > U256::ZERO, "zero buy quote");
    let min_out = SlippageUtils::calculate_amount_out_min(expected, SLIPPAGE);
    let tx = core
        .v2()
        .buy_with_native(V2BuyWithNativeParams {
            token,
            to: core.wallet_address(),
            amount_out_min: min_out,
            deadline: U256::from(DEADLINE),
            value,
            // Explicit generous limit: gas_limit=None estimates against the
            // pre-tx state, which can underestimate for a trade right after a
            // prior trade and revert with out-of-gas.
            gas_limit: Some(2_000_000),
            gas_price: Some(GasPricing::Legacy),
            nonce: None,
        })
        .await
        .expect("buy_with_native_v2");
    assert!(wait_receipt(core, tx).await, "buy tx reverted");
}

async fn sell_v2_native(core: &Core, token: Address, amount_in: U256) {
    let expected = core
        .v2()
        .get_amount_out(token, amount_in, false)
        .await
        .expect("sell quote");
    assert!(expected > U256::ZERO, "zero sell quote");
    let min_out = SlippageUtils::calculate_amount_out_min(expected, SLIPPAGE);
    let tx = core
        .v2()
        .sell_to_native(V2SellToNativeParams {
            token,
            to: core.wallet_address(),
            amount_in,
            amount_out_min: min_out,
            deadline: U256::from(DEADLINE),
            gas_limit: Some(2_000_000),
            gas_price: Some(GasPricing::Legacy),
            nonce: None,
        })
        .await
        .expect("sell_to_native_v2");
    assert!(wait_receipt(core, tx).await, "sell tx reverted");
}

// ============================================================================
// v1 lifecycle
// ============================================================================

#[tokio::test(flavor = "multi_thread")]
#[ignore] // live testnet, funded key + real txs
async fn v1_lifecycle() {
    let Some(pk) = key() else {
        eprintln!("skip: TESTNET_PRIVATE_KEY not set");
        return;
    };
    let core = Core::new(rpc(), pk, nadfun_sdk::Network::Testnet)
        .await
        .expect("core");
    let wallet = core.wallet_address();
    let api = ApiClient::from_env(nadfun_sdk::Network::Testnet);

    let bal0 = core.provider().get_balance(wallet).await.unwrap();
    println!("[v1] wallet {wallet} balance {} MON", format_ether(bal0));

    // ── 1. create (v1) ─────────────────────────────────────────────────
    let n = unique_suffix(&core).await;
    let initial_buy = parse_ether("1").unwrap();
    let amount_out = core
        .v1()
        .get_initial_buy_amount_out(initial_buy)
        .await
        .expect("initial buy quote");
    let params = CreateTokenParams {
        name: format!("Lifecycle V1 {n}"),
        symbol: format!("LCV1{n}"),
        description: "v1 lifecycle smoke".to_string(),
        image_uri: "https://i.imgur.com/0qY8Vp6.png".to_string(),
        website: None,
        twitter: None,
        telegram: None,
        creator_address: wallet,
        amount_out,
        value: initial_buy,
        action_id: ActionId::CapricornActor,
    };
    let result = core
        .v1()
        .create_token(params, &api)
        .await
        .expect("create_token");
    let token = result.token_address;
    println!("[v1] created {token} (tx {})", result.transaction_hash);
    assert!(
        wait_receipt(&core, result.transaction_hash).await,
        "v1 create reverted"
    );
    assert_eq!(
        core.detect_version(token).await.unwrap(),
        nadfun_sdk::SdkVersion::V1
    );
    assert!(
        !core.v1().is_graduated(token).await.unwrap(),
        "fresh v1 token is pre-graduation"
    );

    // ── 2. bonding buy ─────────────────────────────────────────────────
    v1_buy(&core, token, parse_ether("1").unwrap()).await;
    let bal_tok = balance_of(&core, token, wallet).await;
    println!("[v1] token balance after bonding buy: {bal_tok}");
    assert!(bal_tok > U256::ZERO, "should hold tokens after v1 buy");

    // ── 3. bonding sell (full balance) ─────────────────────────────────
    v1_sell(&core, token, bal_tok).await;
    println!("[v1] bonding sell ok");

    // ── 4. graduate (adaptive on balance) ──────────────────────────────
    let (_available, required_mon) = core
        .v1()
        .available_buy_tokens(token)
        .await
        .expect("available");
    let bal_now = core.provider().get_balance(wallet).await.unwrap();
    println!(
        "[v1] graduation needs ~{} MON, have {} MON",
        format_ether(required_mon),
        format_ether(bal_now)
    );
    if required_mon == U256::ZERO || required_mon + parse_ether("500").unwrap() >= bal_now {
        println!(
            "[v1] skip graduate+DEX: insufficient balance for {} MON",
            format_ether(required_mon)
        );
        println!("[v1] v1 create + bonding buy/sell PASSED for {token}");
        return;
    }

    // Drain the curve: buy the remaining supply (+5% headroom over the
    // re-quoted remainder) until nothing is left to buy.
    let cap = bal_now - parse_ether("500").unwrap();
    let mut spent = U256::ZERO;
    loop {
        let (_avail, need) = core
            .v1()
            .available_buy_tokens(token)
            .await
            .expect("available");
        if need == U256::ZERO || spent >= cap {
            break;
        }
        let chunk = (need + need / U256::from(20u64)).max(parse_ether("50").unwrap());
        v1_buy(&core, token, chunk).await;
        spent += chunk;
        println!("[v1] drain progress: spent ~{} MON", format_ether(spent));
    }
    println!("[v1] curve drained (~{} MON)", format_ether(spent));

    // v1 graduation (DEX listing) is keeper-driven and lands a few blocks
    // after the curve sells out — poll the on-chain flag.
    let mut graduated = false;
    for _ in 0..60 {
        if core.v1().is_graduated(token).await.unwrap() {
            graduated = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert!(
        graduated,
        "v1 graduation (keeper) did not complete within 120s"
    );
    println!("[v1] graduated");

    // ── 5. DEX buy (auto-routed to Capricorn CL post-graduation) ───────
    let pre = balance_of(&core, token, wallet).await;
    v1_buy(&core, token, parse_ether("1").unwrap()).await;
    let post = balance_of(&core, token, wallet).await;
    assert!(post > pre, "v1 DEX buy should increase balance");
    println!("[v1] DEX buy ok (+{} tokens)", post - pre);

    // ── 6. DEX sell (full balance) ─────────────────────────────────────
    v1_sell(&core, token, post).await;
    println!("[v1] full v1 lifecycle PASSED for {token}");
}

async fn v1_buy(core: &Core, token: Address, value: U256) {
    let (router, expected) = core
        .v1()
        .get_amount_out(token, value, true)
        .await
        .expect("v1 buy quote");
    assert!(expected > U256::ZERO, "zero v1 buy quote");
    let min_out = SlippageUtils::calculate_amount_out_min(expected, SLIPPAGE);
    let tx = core
        .v1()
        .buy(
            BuyParams {
                token,
                amount_in: value,
                amount_out_min: min_out,
                to: core.wallet_address(),
                deadline: U256::from(DEADLINE),
                gas_limit: Some(2_000_000),
                gas_price: Some(GasPricing::Legacy),
                nonce: None,
            },
            router,
        )
        .await
        .expect("v1 buy");
    assert!(wait_receipt(core, tx).await, "v1 buy reverted");
}

async fn v1_sell(core: &Core, token: Address, amount_in: U256) {
    let (router, expected) = core
        .v1()
        .get_amount_out(token, amount_in, false)
        .await
        .expect("v1 sell quote");
    assert!(expected > U256::ZERO, "zero v1 sell quote");
    approve_if_needed(core, token, router.address(), amount_in).await;
    let min_out = SlippageUtils::calculate_amount_out_min(expected, SLIPPAGE);
    let tx = core
        .v1()
        .sell(
            SellParams {
                amount_in,
                amount_out_min: min_out,
                token,
                to: core.wallet_address(),
                deadline: U256::from(DEADLINE),
                gas_limit: Some(2_000_000),
                gas_price: Some(GasPricing::Legacy),
                nonce: None,
            },
            router,
        )
        .await
        .expect("v1 sell");
    assert!(wait_receipt(core, tx).await, "v1 sell reverted");
}
