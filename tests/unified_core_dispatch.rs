//! `Core::detect_version` — v1/v2 dispatch primitive.
//!
//! Compile-time checks always run. The live-RPC sanity check is gated on
//! `TESTNET_RPC_URL` + `TESTNET_PRIVATE_KEY` env vars so the suite stays
//! fast and offline by default.

use alloy::primitives::Address;
use nadfun_sdk::{Core, Network, SdkVersion};

/// Surface check — `detect_version` exists and returns `Result<SdkVersion>`.
#[allow(dead_code, unreachable_code, unused_variables)]
async fn _detect_version_surface_compiles(c: &Core) {
    let _: anyhow::Result<SdkVersion> = c.detect_version(Address::ZERO).await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore] // requires testnet env vars
async fn dispatches_v1_and_v2_from_one_core() {
    let Some(rpc) = std::env::var("TESTNET_RPC_URL").ok() else {
        eprintln!("skip: TESTNET_RPC_URL not set");
        return;
    };
    let Some(key) = std::env::var("TESTNET_PRIVATE_KEY").ok() else {
        eprintln!("skip: TESTNET_PRIVATE_KEY not set");
        return;
    };

    let core = Core::new(rpc, key, Network::Testnet).await.unwrap();
    // v2 is always wired on the supported networks; if construction succeeded,
    // v2 surface is reachable.

    // Zero address should never be registered in v2.
    let v = core.detect_version(Address::ZERO).await.unwrap();
    assert_eq!(v, SdkVersion::V1);

    // Cache hit on second call (same result, no panic).
    let v2 = core.detect_version(Address::ZERO).await.unwrap();
    assert_eq!(v, v2);

    // Optional: caller-provided v2 token from TESTNET_V2_TOKEN env.
    if let Ok(addr_s) = std::env::var("TESTNET_V2_TOKEN") {
        let token: Address = addr_s.parse().expect("valid token address");
        let v = core.detect_version(token).await.unwrap();
        assert_eq!(v, SdkVersion::V2, "expected v2 for token from env");
    }
}
