//! Wiremock-driven integration tests for `ApiClient` v2 surface.
//!
//! These tests assert wire-level shape (paths, request bodies, response
//! parsing) without hitting the live `api.nadapp.net` endpoint.

use alloy::primitives::Address;
use nadfun_sdk::{ApiClient, SaltParams, SdkVersion, VaultType};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SAMPLE_TOKEN_RAW: &str = "0xabcdef0123456789abcdef0123456789abcdef01";
const SAMPLE_CREATOR_RAW: &str = "0x1234567890abcdef1234567890abcdef12345678";

fn client_for(server: &MockServer) -> ApiClient {
    ApiClient::new().with_api_url(server.uri())
}

/// Render an `Address` the same way `ApiClient` does for URL composition
/// (`format!("{:?}", addr)`), so wiremock path matchers line up with the
/// outgoing HTTP request.
fn token_path(addr: Address) -> String {
    format!("{:?}", addr)
}

fn parse_address(s: &str) -> Address {
    s.parse().expect("valid hex address")
}

/// `get_token` parses the v2 token-info response, surfaces the `version`
/// discriminator, and propagates optional v2-only fields.
#[tokio::test]
async fn get_token_parses_v2_response() {
    let server = MockServer::start().await;

    let token = parse_address(SAMPLE_TOKEN_RAW);
    Mock::given(method("GET"))
        .and(path(format!("/token/{}", token_path(token))))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "token_id": SAMPLE_TOKEN_RAW,
            "name": "TestToken",
            "symbol": "TT",
            "image_uri": "ipfs://image",
            "description": "A test token",
            "is_graduated": false,
            "is_nsfw": false,
            "twitter": null,
            "telegram": null,
            "website": null,
            "created_at": 1_700_000_000_u64,
            "creator": {
                "account_id": SAMPLE_CREATOR_RAW,
                "nickname": "alice",
                "bio": null,
                "image_uri": null
            },
            "is_cto": false,
            "version": "V2"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let api = client_for(&server);
    let token = SAMPLE_TOKEN_RAW.parse().unwrap();
    let info = api.get_token(token).await.expect("get_token");

    assert_eq!(info.token_id, SAMPLE_TOKEN_RAW);
    assert_eq!(info.name, "TestToken");
    assert_eq!(info.version, SdkVersion::V2);
    assert!(info.creator.is_some());
    assert_eq!(info.creator.as_ref().unwrap().account_id, SAMPLE_CREATOR_RAW);
    // Null-tolerated string fields default to empty.
    assert_eq!(info.twitter, "");
    assert_eq!(info.telegram, "");
    assert_eq!(info.website, "");
    // `token_address` returns a real Address.
    assert_eq!(info.token_address().unwrap(), token);
}

/// `get_token` falls back to `SdkVersion::V1` when the `version` field is
/// absent — backward-compat behavior for legacy v1 responses.
#[tokio::test]
async fn get_token_defaults_to_v1_when_version_missing() {
    let server = MockServer::start().await;

    let token = parse_address(SAMPLE_TOKEN_RAW);
    Mock::given(method("GET"))
        .and(path(format!("/token/{}", token_path(token))))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "token_id": SAMPLE_TOKEN_RAW,
            "name": "V1Token",
            "symbol": "V1T",
            "image_uri": "ipfs://image",
            "is_graduated": true
        })))
        .mount(&server)
        .await;

    let api = client_for(&server);
    let info = api.get_token(SAMPLE_TOKEN_RAW.parse().unwrap()).await.unwrap();
    assert_eq!(info.version, SdkVersion::V1);
    assert!(info.is_graduated);
    assert!(info.creator.is_none());
}

/// `get_token_vaults` parses the v2 vault response with mixed vault types.
#[tokio::test]
async fn get_token_vaults_parses_mixed_vaults() {
    let server = MockServer::start().await;

    let token = parse_address(SAMPLE_TOKEN_RAW);
    Mock::given(method("GET"))
        .and(path(format!("/vault/{}", token_path(token))))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "token_id": SAMPLE_TOKEN_RAW,
            "quote_id": "0xWMON0000000000000000000000000000000000",
            "total_quote_amount": "1000",
            "total_quote_amount_usd": "1500.00",
            "vaults": [
                {
                    "vault_id": "0xBurn0000000000000000000000000000000000",
                    "bps": 1000,
                    "name": "Buyback & Burn",
                    "active": true,
                    "quote_amount": "100",
                    "quote_amount_usd": "150.00",
                    "last_executed_at": 1_700_000_001_u64,
                    "vault_type": "BURN",
                    "stats": { "tokens_burned": "42" }
                },
                {
                    "vault_id": "0xCreator00000000000000000000000000000000",
                    "bps": 2000,
                    "name": "Creator Fee",
                    "active": true,
                    "quote_amount": "200",
                    "quote_amount_usd": "300.00",
                    "vault_type": "CREATOR_FEE",
                    "stats": { "current_balance": "200" }
                }
            ]
        })))
        .mount(&server)
        .await;

    let api = client_for(&server);
    let state = api
        .get_token_vaults(SAMPLE_TOKEN_RAW.parse().unwrap())
        .await
        .expect("get_token_vaults");

    assert_eq!(state.vaults.len(), 2);
    assert_eq!(state.vaults[0].vault_type, VaultType::Burn);
    assert_eq!(state.vaults[1].vault_type, VaultType::CreatorFee);
    assert_eq!(state.vaults[0].bps, 1000);
    assert_eq!(state.vaults[1].bps, 2000);
}

/// `SaltParams` with `version: None` does NOT include the field on the wire —
/// preserving wire-level backward compat for v1 callers.
#[tokio::test]
async fn salt_params_v1_omits_version_field() {
    let p = SaltParams {
        creator: SAMPLE_CREATOR_RAW.to_string(),
        metadata_uri: "ipfs://meta".into(),
        name: "X".into(),
        symbol: "Y".into(),
        version: None,
    };
    let body = serde_json::to_value(&p).unwrap();
    assert!(body.get("version").is_none(), "version must be absent on v1 wire: {}", body);
}

/// `SaltParams` with `version: Some(V2)` serializes `"version": "V2"`.
#[tokio::test]
async fn salt_params_v2_serializes_version_uppercase() {
    let p = SaltParams {
        creator: SAMPLE_CREATOR_RAW.to_string(),
        metadata_uri: "ipfs://meta".into(),
        name: "X".into(),
        symbol: "Y".into(),
        version: Some(SdkVersion::V2),
    };
    let body = serde_json::to_value(&p).unwrap();
    assert_eq!(body.get("version").and_then(|v| v.as_str()), Some("V2"));
}

/// `post_salt` round-trips through wiremock with the V2 version field set,
/// confirming both the request body shape and the response parsing.
#[tokio::test]
async fn post_salt_sends_version_v2_and_parses_response() {
    let server = MockServer::start().await;

    let expected_body = json!({
        "creator": SAMPLE_CREATOR_RAW,
        "metadata_uri": "ipfs://meta",
        "name": "Test",
        "symbol": "TST",
        "version": "V2"
    });

    Mock::given(method("POST"))
        .and(path("/agent/salt"))
        .and(body_json(&expected_body))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "salt": "0x0000000000000000000000000000000000000000000000000000000000000001",
            "address": SAMPLE_TOKEN_RAW
        })))
        .expect(1)
        .mount(&server)
        .await;

    let api = client_for(&server);
    let response = api
        .post_salt(SaltParams {
            creator: SAMPLE_CREATOR_RAW.to_string(),
            metadata_uri: "ipfs://meta".into(),
            name: "Test".into(),
            symbol: "TST".into(),
            version: Some(SdkVersion::V2),
        })
        .await
        .expect("post_salt");

    assert_eq!(response.address, SAMPLE_TOKEN_RAW);
    assert!(response.salt.starts_with("0x"));
}
