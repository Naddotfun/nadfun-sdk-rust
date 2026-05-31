//! Live testnet drift guards for the Phase 2/3 v2 view surface.
//!
//! These hit a real RPC, so they are `#[ignore]`d by default. Run with:
//!
//! ```bash
//! cargo test --test v2_views_live -- --ignored
//! ```
//!
//! They pin the on-chain field layout of `getCurve` / `getConfig` (the R1
//! risk): if a future contract redeploy reorders the tuple, the
//! constant-product invariant `k == virtualQuoteReserve * virtualTokenReserve`
//! or the WMON-quote/decimals assertions break loudly. Defaults target the
//! public testnet node; override with `TESTNET_RPC_URL` / `TESTNET_V2_TOKEN`.

use alloy::primitives::{address, Address, U256};
use nadfun_sdk::{constants::get_wmon, Core, Network};

const DEFAULT_TESTNET_RPC: &str = "https://dev-node.nadapp.net/";
/// Ephemeral, publicly-known key — every call here is an `eth_call`, no tx sent.
const EPHEMERAL_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
/// A real testnet V2 token (WMON-quoted). Override with `TESTNET_V2_TOKEN`.
const DEFAULT_V2_TOKEN: Address = address!("f9F9dd8015b417112c69BF87974C54265c037777");
/// A graduated testnet V2 token. Graduation is irreversible, so this stays
/// graduated and is a stable fixture for the post-graduation pair views
/// (`is_locked` / `get_reserves`). Override with `TESTNET_V2_GRADUATED_TOKEN`.
const DEFAULT_GRADUATED_V2_TOKEN: Address = address!("78D804f5581020F14eFf18149Cb1c4B50c2C7777");
/// The EVM burn sink — never a registered/graduated v2 token, so it exercises
/// the pre-graduation error path deterministically (it can never graduate).
const UNREGISTERED: Address = address!("000000000000000000000000000000000000dEaD");

async fn testnet_core() -> Core {
    let rpc = std::env::var("TESTNET_RPC_URL").unwrap_or_else(|_| DEFAULT_TESTNET_RPC.to_string());
    let key = std::env::var("TESTNET_PRIVATE_KEY").unwrap_or_else(|_| EPHEMERAL_KEY.to_string());
    Core::new(rpc, key, Network::Testnet).await.unwrap()
}

fn v2_token() -> Address {
    std::env::var("TESTNET_V2_TOKEN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_V2_TOKEN)
}

fn graduated_v2_token() -> Address {
    std::env::var("TESTNET_V2_GRADUATED_TOKEN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_GRADUATED_V2_TOKEN)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn get_curve_holds_constant_product_invariant() {
    let core = testnet_core().await;
    let token = v2_token();

    let curve = core.v2().get_curve(token).await.unwrap();

    // The invariant that catches any getCurve field-order regression.
    // Pre-graduation only (post-graduation k is frozen at genesis); the default
    // fixture token is not graduated.
    if !curve.graduated {
        assert_eq!(
            curve.k,
            curve.virtual_quote_reserve * curve.virtual_token_reserve,
            "k must equal virtualQuoteReserve * virtualTokenReserve pre-graduation"
        );
    }
    assert_eq!(curve.token, token, "curve.token must echo the input");
    assert_ne!(
        curve.quote_token,
        Address::ZERO,
        "curve must have a quote token"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn quote_config_decodes_for_wmon() {
    let core = testnet_core().await;
    let wmon: Address = get_wmon(Network::Testnet).parse().unwrap();

    let cfg = core.v2().quote_config(wmon).await.unwrap();

    assert!(cfg.active, "WMON must be an active quote token on testnet");
    assert_eq!(cfg.decimals, 18, "WMON has 18 decimals");
    assert!(
        cfg.virtual_reserve > U256::ZERO,
        "genesis virtual reserve must be set"
    );
    assert!(
        cfg.min_token_reserve < cfg.virtual_token_reserve,
        "minTokenReserve must be below virtualTokenReserve (graduation room)"
    );
}

/// Every passthrough view resolves against a real registered token, and the
/// values cross-agree (e.g. `quote_token` matches `get_curve().quote_token`).
/// Uses a graduated token so the post-graduation pair views are also exercised.
#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn all_passthrough_views_resolve_for_graduated_token() {
    let core = testnet_core().await;
    let v2 = core.v2();
    let token = graduated_v2_token();

    // Protocol-level state.
    let _halted: bool = v2.is_halted().await.unwrap();

    // Registry views.
    assert!(
        v2.is_registered(token).await.unwrap(),
        "graduated token must be registered"
    );
    let _dex_type: u8 = v2.get_dex_type(token).await.unwrap();

    // Bonding-curve views.
    let quote = v2.quote_token(token).await.unwrap();
    assert_ne!(quote, Address::ZERO, "graduated token has a quote token");
    let _penalty: U256 = v2.get_sniping_penalty(token).await.unwrap();

    // Full curve state — graduated, with a live pair. (k == vQuote*vToken only
    // holds PRE-graduation, so it is checked in the non-graduated test.)
    let curve = v2.get_curve(token).await.unwrap();
    assert!(curve.graduated, "fixture token must be graduated");
    assert_ne!(curve.pair, Address::ZERO, "graduated curve has a pair");
    assert_eq!(
        quote, curve.quote_token,
        "quote_token() must agree with get_curve().quote_token"
    );

    // Post-graduation pair views — succeed for a graduated token.
    let _locked: bool = v2.is_locked(token).await.unwrap();
    let reserves = v2.get_reserves(token).await.unwrap();
    assert!(
        reserves.reserve0 > 0 || reserves.reserve1 > 0,
        "a graduated pair holds liquidity"
    );
}

/// The pre-graduation gate (Codex review): `is_locked` / `get_reserves` must
/// Err for a token that is not a live graduated pair. Uses the registered-but-
/// not-graduated default token — exactly the case the `get_pair != ZERO` guard
/// failed to catch (the registry assigns a pair at creation), so this is the
/// regression test for `resolve_graduated_pair` gating on graduation.
#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn pair_views_error_before_graduation() {
    let core = testnet_core().await;
    let v2 = core.v2();
    let token = v2_token(); // registered, NOT graduated, but has a registry pair

    assert!(
        !v2.is_graduated(token).await.unwrap(),
        "fixture token must be registered-but-not-graduated"
    );
    assert!(
        v2.is_locked(token).await.is_err(),
        "is_locked must Err before graduation even though the registry has a pair"
    );
    assert!(
        v2.get_reserves(token).await.is_err(),
        "get_reserves must Err before graduation even though the registry has a pair"
    );

    // The burn sink is unregistered — `is_graduated` itself reverts there, and
    // that error must propagate (not silently become Ok) through the pair views.
    assert!(
        v2.is_graduated(UNREGISTERED).await.is_err(),
        "is_graduated reverts for an unregistered token"
    );
    assert!(
        v2.is_locked(UNREGISTERED).await.is_err(),
        "is_locked must surface the unregistered-token revert"
    );
    assert!(
        !v2.is_registered(UNREGISTERED).await.unwrap(),
        "burn sink is not registered"
    );
}

/// `is_graduated` (the gate) agrees with `get_curve().graduated` and with
/// pair presence — the consistency that makes `resolve_graduated_pair`
/// correct. Pins the three signals together so a future contract change that
/// desyncs them fails loudly.
#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn graduation_signals_agree() {
    let core = testnet_core().await;
    let v2 = core.v2();
    let token = graduated_v2_token();

    let is_grad = v2.is_graduated(token).await.unwrap();
    let curve = v2.get_curve(token).await.unwrap();
    let pool = v2.pool_address(token).await.unwrap();

    assert_eq!(
        is_grad, curve.graduated,
        "router.is_graduated must agree with get_curve().graduated"
    );
    assert_eq!(
        curve.pair, pool,
        "get_curve().pair must agree with pool_address() (registry pair)"
    );
    if is_grad {
        assert_ne!(pool, Address::ZERO, "graduated token must have a pair");
    }
}

/// `get_initial_buy_amount_out(quote, amt, creator_fee_rate)` returns the exact
/// create-time initial-buy output (combined protocol + creator fee, then the
/// constant-product / supply-cap math; anti-sniping exempt). The exact fee +
/// ceil-div arithmetic — including the create-time golden values proven on
/// testnet — is pinned by the `calc` unit tests; this live test only guards that
/// the genesis config feeds through sanely (with `creator_fee_rate = 0`, the
/// protocol-only case), staying positive and below the genesis sellable supply.
#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn initial_buy_is_bounded_by_genesis_supply() {
    let core = testnet_core().await;
    let v2 = core.v2();
    let wmon: Address = get_wmon(Network::Testnet).parse().unwrap();
    let amount = U256::from(1_000_000_000_000_000_000u64); // 1e18

    let computed = v2
        .get_initial_buy_amount_out(wmon, amount, 0)
        .await
        .unwrap();
    assert!(computed > U256::ZERO, "initial buy must yield tokens");

    let cfg = v2.quote_config(wmon).await.unwrap();
    let genesis_available = cfg.virtual_token_reserve - cfg.min_token_reserve;
    // A 1e18 buy is tiny vs genesis reserves, so it must be well below the cap.
    assert!(
        computed < genesis_available,
        "a small buy should not consume the entire genesis supply"
    );
}

/// `get_progress` reads 10000 for a graduated curve and stays within the
/// basis-points range for a live one; `available_buy_tokens` is `(0, 0)` once
/// graduated and `virtual − min` (>0) otherwise.
#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn progress_and_available_match_curve_state() {
    let core = testnet_core().await;
    let v2 = core.v2();

    // Graduated token: progress 10000, nothing left to buy.
    let grad = graduated_v2_token();
    assert_eq!(
        v2.get_progress(grad).await.unwrap(),
        U256::from(10_000u64),
        "graduated token is 100% progressed"
    );
    assert_eq!(
        v2.available_buy_tokens(grad).await.unwrap(),
        (U256::ZERO, U256::ZERO),
        "graduated token has nothing left to buy"
    );

    // Non-graduated token: progress in [0, 10000], available matches curve.
    let token = v2_token();
    let curve = v2.get_curve(token).await.unwrap();
    if !curve.graduated {
        let progress = v2.get_progress(token).await.unwrap();
        assert!(progress <= U256::from(10_000u64), "progress within range");
        let (available, required) = v2.available_buy_tokens(token).await.unwrap();
        assert_eq!(
            available,
            curve.virtual_token_reserve - curve.min_token_reserve,
            "available == virtual − min"
        );
        if available > U256::ZERO {
            assert!(required > U256::ZERO, "buying tokens costs quote");
        }
    }
}
