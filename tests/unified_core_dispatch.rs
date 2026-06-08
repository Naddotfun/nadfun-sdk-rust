//! `Core::detect_version` / `detect_versions` — v1/v2/None dispatch primitive.
//!
//! Compile-time surface checks always run. The live-RPC smoke is `#[ignore]`d
//! so the default suite stays fast and offline; run it with:
//!
//! ```bash
//! cargo test --test unified_core_dispatch -- --ignored
//! ```
//!
//! It defaults to the public testnet node (`dev-node.nadapp.net`) and an
//! ephemeral read-only key (every call here is an `eth_call`, no tx is sent).
//! Override with `TESTNET_RPC_URL` / `TESTNET_PRIVATE_KEY`. Supply
//! `TESTNET_V1_TOKEN` / `TESTNET_V2_TOKEN` to also assert positive
//! classification for real tokens.

use alloy::primitives::{address, Address};
use nadfun_sdk::{Core, Network, SdkVersion, TokenInfo};

/// Surface check — `detect_version` / `detect_token_info` exist with the
/// expected signatures.
#[allow(dead_code, unreachable_code, unused_variables)]
async fn _detect_surface_compiles(c: &Core) {
    let _: anyhow::Result<SdkVersion> = c.detect_version(Address::ZERO).await;
    let _: anyhow::Result<Vec<SdkVersion>> = c.detect_versions(vec![Address::ZERO]).await;
    let _: anyhow::Result<TokenInfo> = c.detect_token_info(Address::ZERO).await;
    let _: anyhow::Result<Vec<TokenInfo>> = c.detect_token_infos(vec![Address::ZERO]).await;
}

/// Default testnet node used when `TESTNET_RPC_URL` is unset.
const DEFAULT_TESTNET_RPC: &str = "https://dev-node.nadapp.net/";
/// Ephemeral, publicly-known key. Read-only — `detect_*` never sends a tx, so
/// this signer just satisfies `Core::new` and needs no funds.
const EPHEMERAL_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
/// An address that is not a Nad.fun-deployed token (the EVM burn sink).
const ARBITRARY_ERC20: Address = address!("000000000000000000000000000000000000dEaD");

#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires live testnet RPC
async fn detect_version_classifies_via_lens() {
    let rpc = std::env::var("TESTNET_RPC_URL").unwrap_or_else(|_| DEFAULT_TESTNET_RPC.to_string());
    let key = std::env::var("TESTNET_PRIVATE_KEY").unwrap_or_else(|_| EPHEMERAL_KEY.to_string());

    let core = Core::new(rpc, key, Network::Testnet).await.unwrap();

    // With `TokenInfoLens` wired on testnet, an address that isn't a
    // Nad.fun token classifies as `None` (Solidity `Version::None == 0`).
    // This is the Lens path — NOT the pre-Lens fallback, which reported any
    // non-v2 address as `V1`.
    let zero = core.detect_version(Address::ZERO).await.unwrap();
    assert_eq!(zero, SdkVersion::None, "zero address must classify as None");

    let arb = core.detect_version(ARBITRARY_ERC20).await.unwrap();
    assert_eq!(
        arb,
        SdkVersion::None,
        "arbitrary ERC-20 must classify as None"
    );

    // Stateless — calling again hits the chain again and is consistent.
    let zero_again = core.detect_version(Address::ZERO).await.unwrap();
    assert_eq!(zero_again, SdkVersion::None);

    // Batch path: one `getTokenInfos` RPC classifies the whole list, order
    // preserved.
    let batch = core
        .detect_versions(vec![Address::ZERO, ARBITRARY_ERC20])
        .await
        .unwrap();
    assert_eq!(batch, vec![SdkVersion::None, SdkVersion::None]);

    // Empty batch short-circuits without an RPC.
    let empty = core.detect_versions(vec![]).await.unwrap();
    assert!(empty.is_empty());

    // `detect_token_info` carries the on-chain quote token alongside the
    // version. None tokens report a zero quote.
    let none_info = core.detect_token_info(Address::ZERO).await.unwrap();
    assert_eq!(none_info.version, SdkVersion::None);
    assert_eq!(none_info.quote_token, Address::ZERO);

    let infos = core
        .detect_token_infos(vec![Address::ZERO, ARBITRARY_ERC20])
        .await
        .unwrap();
    assert_eq!(infos.len(), 2);
    assert!(infos
        .iter()
        .all(|i| i.version == SdkVersion::None && i.quote_token == Address::ZERO));

    // Optional positive cases when real testnet tokens are provided.
    if let Ok(addr_s) = std::env::var("TESTNET_V1_TOKEN") {
        let token: Address = addr_s.parse().expect("valid TESTNET_V1_TOKEN");
        let info = core.detect_token_info(token).await.unwrap();
        assert_eq!(
            info.version,
            SdkVersion::V1,
            "expected V1 for TESTNET_V1_TOKEN"
        );
        // v1 always quotes against the wrapped native (WMON), never zero.
        assert_ne!(info.quote_token, Address::ZERO, "v1 quote should be WMON");
    }
    if let Ok(addr_s) = std::env::var("TESTNET_V2_TOKEN") {
        let token: Address = addr_s.parse().expect("valid TESTNET_V2_TOKEN");
        let info = core.detect_token_info(token).await.unwrap();
        assert_eq!(
            info.version,
            SdkVersion::V2,
            "expected V2 for TESTNET_V2_TOKEN"
        );
        assert_ne!(
            info.quote_token,
            Address::ZERO,
            "v2 token must have a quote token"
        );
        // `detect_version` is the thin wrapper over `detect_token_info`.
        assert_eq!(core.detect_version(token).await.unwrap(), info.version);
    }
}

/// Surface check — the v1 namespace handle exposes the v1 query/escape surface.
#[allow(dead_code, unreachable_code, unused_variables)]
async fn _core_v1_methods_compile(c: &nadfun_sdk::Core) {
    use alloy::primitives::{Address, U256};
    let t = Address::ZERO;
    let a = U256::ZERO;
    let _: anyhow::Result<(nadfun_sdk::Router, U256)> = c.v1().get_amount_out(t, a, true).await;
    let _: anyhow::Result<(nadfun_sdk::Router, U256)> = c.v1().get_amount_in(t, a, true).await;
    let _: anyhow::Result<(U256, U256)> = c.v1().available_buy_tokens(t).await;
    let _: anyhow::Result<bool> = c.v1().is_locked(t).await;
    let _: anyhow::Result<bool> = c.v1().is_graduated(t).await;
    let _: anyhow::Result<U256> = c.v1().get_initial_buy_amount_out(a).await;
    let _: anyhow::Result<U256> = c.v1().get_deploy_fee().await;
    let _: anyhow::Result<U256> = c.v1().get_progress(t).await;
    let _bcr = c.v1().bonding_curve_router();
    let _dxr = c.v1().dex_router();
    let _lens = c.v1().lens();
}

/// Regression (Codex review [P2], 2026-05-31): namespace handles must support
/// the common pattern where a future or an escape-hatch reference outlives the
/// temporary `core.v1()` / `core.v2()` handle that produced it. The handles are
/// `Copy` and take `self` by value, and escape hatches return `&'a _` tied to
/// the underlying `&Core`, so storing a result across the temporary compiles.
/// Before the fix these lines failed with E0716 (temporary dropped while
/// borrowed). The earlier surface checks never *use* the stored value after a
/// subsequent statement, so they did not catch this.
#[allow(dead_code, unreachable_code, unused_variables)]
async fn _stored_handle_results_compile(c: &nadfun_sdk::Core) {
    use alloy::primitives::{Address, U256};
    // Future stored across the temporary v1()/v2() handle, awaited later.
    let fut_v1 = c.v1().get_amount_out(Address::ZERO, U256::ZERO, true);
    let _ = fut_v1.await;
    let fut_v2 = c.v2().get_amount_out(Address::ZERO, U256::ZERO, true);
    let _ = fut_v2.await;
    // Escape-hatch reference stored, then used after another statement.
    let lens = c.v1().lens();
    let router = c.v2().router();
    let _ = lens.address;
    let _ = router.address;
}
