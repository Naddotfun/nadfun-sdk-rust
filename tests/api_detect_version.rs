//! Wiremock tests for API-based token detection
//! (`Core::detect_token_info_with_api` / `Core::detect_version_with_api`).
//!
//! One mock server plays both the api-server (`GET /token/{token}`) and the
//! JSON-RPC endpoint (`eth_call` to `TokenRegistry.getQuoteToken`). Rules
//! pinned here: the chain is read first — a registry entry means V2 with that
//! quote whatever the API says (or without asking it); only an unregistered
//! token falls back to the API (V1 -> WMON, V2 -> `Err`, api-server 404 ->
//! `None`); a 404 that is not the api-server's error body, any other API error,
//! and an ApiClient/Core network mismatch are `Err`.

use alloy::primitives::Address;
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use nadfun_sdk::{ApiClient, Core, Network, SdkVersion};
use serde_json::{json, Value};
use std::sync::Arc;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const TOKEN_RAW: &str = "0xabcdef0123456789abcdef0123456789abcdef01";
const USDC_RAW: &str = "0x754704Bc059F8C67012fEd69BC8A327a5aafb603";

fn token() -> Address {
    TOKEN_RAW.parse().unwrap()
}

/// Read-only `Core` on `network` whose RPC is the mock server.
fn core_for(server: &MockServer, network: Network) -> Core {
    let provider = ProviderBuilder::new().connect_http(server.uri().parse().unwrap());
    Core::with_provider(
        Arc::new(DynProvider::new(provider.erased())),
        Address::ZERO,
        network,
    )
    .unwrap()
}

fn api_for(server: &MockServer, network: Network) -> ApiClient {
    ApiClient::new(network).with_api_url(server.uri())
}

async fn mock_token(server: &MockServer, status: u16, body: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/token/{:?}", token())))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(server)
        .await;
}

/// Every `eth_call` returns `quote` ABI-encoded (TokenRegistry.getQuoteToken).
async fn mock_registry_quote(server: &MockServer, quote: Address) {
    Mock::given(method("POST"))
        .and(body_partial_json(json!({ "method": "eth_call" })))
        .respond_with(move |req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": body["id"],
                "result": format!("0x{:0>64}", hex_addr(quote)),
            }))
        })
        .mount(server)
        .await;
}

fn hex_addr(a: Address) -> String {
    format!("{a:x}").trim_start_matches("0x").to_string()
}

/// Live api-server shape: `{"token_info": {...}}` (api-server `TokenResponse`).
fn token_body(version: &str) -> Value {
    json!({
        "token_info": {
            "token_id": TOKEN_RAW,
            "name": "T",
            "symbol": "T",
            "image_uri": "",
            "is_graduated": false,
            "version": version
        }
    })
}

/// Mounts `GET /token/{token}` that must never be called.
async fn forbid_token_lookup(server: &MockServer) {
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(token_body("V1")))
        .expect(0)
        .mount(server)
        .await;
}

fn wmon() -> Address {
    nadfun_sdk::constants::get_wmon(Network::Mainnet)
        .parse()
        .unwrap()
}

#[tokio::test]
async fn registered_token_is_v2_without_asking_api() {
    let server = MockServer::start().await;
    let usdc: Address = USDC_RAW.parse().unwrap();
    forbid_token_lookup(&server).await;
    mock_registry_quote(&server, usdc).await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Mainnet);
    let info = core
        .detect_token_info_with_api(&api, token())
        .await
        .unwrap();
    assert_eq!(info.version, SdkVersion::V2);
    assert_eq!(info.quote_token, usdc);
    assert_eq!(
        core.detect_version_with_api(&api, token()).await.unwrap(),
        SdkVersion::V2
    );
}

/// The registry wins over an API that (wrongly or stale) says V1.
#[tokio::test]
async fn api_v1_but_registry_entry_is_v2() {
    let server = MockServer::start().await;
    let usdc: Address = USDC_RAW.parse().unwrap();
    mock_token(&server, 200, token_body("V1")).await;
    mock_registry_quote(&server, usdc).await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Mainnet);
    let info = core
        .detect_token_info_with_api(&api, token())
        .await
        .unwrap();
    assert_eq!(info.version, SdkVersion::V2);
    assert_eq!(info.quote_token, usdc);
}

#[tokio::test]
async fn unregistered_v1_token_quotes_wmon() {
    let server = MockServer::start().await;
    mock_token(&server, 200, token_body("V1")).await;
    mock_registry_quote(&server, Address::ZERO).await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Mainnet);
    let info = core
        .detect_token_info_with_api(&api, token())
        .await
        .unwrap();
    assert_eq!(info.version, SdkVersion::V1);
    assert_eq!(info.quote_token, wmon());
}

#[tokio::test]
async fn api_404_is_none_with_zero_quote() {
    let server = MockServer::start().await;
    mock_token(
        &server,
        404,
        json!({ "error": format!("Token not found: {TOKEN_RAW}") }),
    )
    .await;
    mock_registry_quote(&server, Address::ZERO).await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Mainnet);
    let info = core
        .detect_token_info_with_api(&api, token())
        .await
        .unwrap();
    assert_eq!(info.version, SdkVersion::None);
    assert_eq!(info.quote_token, Address::ZERO);
}

/// A 404 that is not the api-server's `{"error": ..}` body (wrong base URL,
/// proxy page) must not be read as "unknown token".
#[tokio::test]
async fn non_api_404_is_err() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/token/{:?}", token())))
        .respond_with(ResponseTemplate::new(404).set_body_string("<html>Not Found</html>"))
        .mount(&server)
        .await;
    mock_registry_quote(&server, Address::ZERO).await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Mainnet);
    assert!(core
        .detect_token_info_with_api(&api, token())
        .await
        .is_err());
}

#[tokio::test]
async fn other_api_error_is_err() {
    let server = MockServer::start().await;
    mock_token(&server, 500, json!({ "error": "Internal server error" })).await;
    mock_registry_quote(&server, Address::ZERO).await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Mainnet);
    assert!(core
        .detect_token_info_with_api(&api, token())
        .await
        .is_err());
    assert!(core.detect_version_with_api(&api, token()).await.is_err());
}

#[tokio::test]
async fn api_v2_without_registry_entry_is_err() {
    let server = MockServer::start().await;
    mock_token(&server, 200, token_body("V2")).await;
    mock_registry_quote(&server, Address::ZERO).await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Mainnet);
    assert!(core
        .detect_token_info_with_api(&api, token())
        .await
        .is_err());
}

#[tokio::test]
async fn network_mismatch_is_err_without_http() {
    let server = MockServer::start().await;
    forbid_token_lookup(&server).await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let core = core_for(&server, Network::Mainnet);
    let api = api_for(&server, Network::Testnet);
    assert!(core
        .detect_token_info_with_api(&api, token())
        .await
        .is_err());
}
