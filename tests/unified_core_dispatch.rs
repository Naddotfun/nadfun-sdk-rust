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
use nadfun_sdk::{Core, Network, SdkVersion};

/// Surface check — `detect_version` exists and returns `Result<SdkVersion>`.
#[allow(dead_code, unreachable_code, unused_variables)]
async fn _detect_version_surface_compiles(c: &Core) {
    let _: anyhow::Result<SdkVersion> = c.detect_version(Address::ZERO).await;
    let _: anyhow::Result<Vec<SdkVersion>> = c.detect_versions(vec![Address::ZERO]).await;
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

    // With `TokenVersionLens` wired on testnet, an address that isn't a
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

    // Batch path: one `getVersions` RPC classifies the whole list, order
    // preserved.
    let batch = core
        .detect_versions(vec![Address::ZERO, ARBITRARY_ERC20])
        .await
        .unwrap();
    assert_eq!(batch, vec![SdkVersion::None, SdkVersion::None]);

    // Empty batch short-circuits without an RPC.
    let empty = core.detect_versions(vec![]).await.unwrap();
    assert!(empty.is_empty());

    // Optional positive cases when real testnet tokens are provided.
    if let Ok(addr_s) = std::env::var("TESTNET_V1_TOKEN") {
        let token: Address = addr_s.parse().expect("valid TESTNET_V1_TOKEN");
        assert_eq!(
            core.detect_version(token).await.unwrap(),
            SdkVersion::V1,
            "expected V1 for TESTNET_V1_TOKEN"
        );
    }
    if let Ok(addr_s) = std::env::var("TESTNET_V2_TOKEN") {
        let token: Address = addr_s.parse().expect("valid TESTNET_V2_TOKEN");
        assert_eq!(
            core.detect_version(token).await.unwrap(),
            SdkVersion::V2,
            "expected V2 for TESTNET_V2_TOKEN"
        );
    }
}
