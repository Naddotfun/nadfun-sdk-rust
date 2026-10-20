//! Loopback JSON-RPC checks for the opt-in native send budget check.

use alloy::{primitives::U256, signers::local::PrivateKeySigner};
use nadfun_sdk::{Core, Network};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use wiremock::{matchers::method, Mock, MockServer, Request, ResponseTemplate};

struct RpcState {
    requests: Vec<Value>,
    balance_reply: Value,
}

struct RpcFixture {
    server: MockServer,
    state: Arc<Mutex<RpcState>>,
}

impl RpcFixture {
    async fn start(balance_reply: Value) -> Self {
        let server = MockServer::builder().start().await;
        let state = Arc::new(Mutex::new(RpcState {
            requests: Vec::new(),
            balance_reply,
        }));
        let responder_state = state.clone();

        Mock::given(method("POST"))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                let id = body["id"].clone();
                let rpc_method = body["method"].as_str().unwrap_or_default();
                let reply = {
                    let mut state = responder_state.lock().unwrap();
                    state.requests.push(body.clone());
                    if rpc_method == "eth_getBalance" {
                        state.balance_reply.clone()
                    } else {
                        json!({"error": {"code": -32601, "message": format!("unexpected method: {rpc_method}")}})
                    }
                };
                let mut response = json!({"jsonrpc": "2.0", "id": id});
                if let Some(result) = reply.get("result") {
                    response["result"] = result.clone();
                } else {
                    response["error"] = reply["error"].clone();
                }
                ResponseTemplate::new(200).set_body_json(response)
            })
            .mount(&server)
            .await;

        Self { server, state }
    }

    fn requests(&self) -> Vec<Value> {
        self.state.lock().unwrap().requests.clone()
    }

    fn methods(&self) -> Vec<String> {
        self.requests()
            .iter()
            .map(|request| request["method"].as_str().unwrap().to_owned())
            .collect()
    }
}

async fn core(fixture: &RpcFixture) -> (Core, alloy::primitives::Address) {
    let signer = PrivateKeySigner::random();
    let address = signer.address();
    let private_key = format!("0x{}", hex::encode(signer.to_bytes()));
    let core = Core::new(fixture.server.uri(), private_key, Network::Testnet)
        .await
        .unwrap();
    (core, address)
}

fn balance(value: U256) -> Value {
    json!({"result": format!("0x{value:x}")})
}

#[tokio::test]
async fn native_balance_precheck_rejects_shortfall_and_only_reads_latest_balance() {
    let fixture = RpcFixture::start(balance(U256::from(15))).await;
    let (core, wallet) = core(&fixture).await;

    let error = core
        .check_native_send_balance(U256::from(10), 3, 2)
        .await
        .unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("16"), "{message}");
    assert!(message.contains("15"), "{message}");
    assert_eq!(fixture.methods(), ["eth_getBalance"]);
    let request = &fixture.requests()[0];
    assert!(request["params"][0]
        .as_str()
        .unwrap()
        .eq_ignore_ascii_case(&wallet.to_string()));
    assert_eq!(request["params"][1], "latest");
}

#[tokio::test]
async fn native_balance_precheck_allows_exact_threshold_and_above() {
    for available in [16, 17] {
        let fixture = RpcFixture::start(balance(U256::from(available))).await;
        let (core, _) = core(&fixture).await;

        core.check_native_send_balance(U256::from(10), 3, 2)
            .await
            .unwrap();

        assert_eq!(fixture.methods(), ["eth_getBalance"]);
    }
}

#[tokio::test]
async fn native_balance_precheck_reports_checked_arithmetic_overflow_and_available_balance() {
    let fixture = RpcFixture::start(balance(U256::from(100))).await;
    let (core, _) = core(&fixture).await;

    let error = core
        .check_native_send_balance(U256::MAX, 1, 1)
        .await
        .unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("overflow"), "{message}");
    assert!(message.contains(&U256::MAX.to_string()), "{message}");
    assert!(message.contains("100"), "{message}");
    assert_eq!(fixture.methods(), ["eth_getBalance"]);
}

#[tokio::test]
async fn native_balance_precheck_adds_context_to_balance_rpc_error() {
    let fixture = RpcFixture::start(json!({
        "error": {"code": -32000, "message": "balance endpoint unavailable"}
    }))
    .await;
    let (core, _) = core(&fixture).await;

    let error = core
        .check_native_send_balance(U256::from(10), 3, 2)
        .await
        .unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("16"), "{message}");
    assert!(message.contains("available"), "{message}");
    assert!(
        message.contains("balance endpoint unavailable"),
        "{message}"
    );
    assert_eq!(fixture.methods(), ["eth_getBalance"]);
}

#[tokio::test]
async fn failed_precheck_never_reads_nonce_or_submits_a_transaction() {
    let fixture = RpcFixture::start(balance(U256::from(15))).await;
    let (core, _) = core(&fixture).await;

    assert!(core
        .check_native_send_balance(U256::from(10), 3, 2)
        .await
        .is_err());

    let methods = fixture.methods();
    assert!(methods.contains(&"eth_getBalance".to_owned()));
    assert!(!methods.contains(&"eth_getTransactionCount".to_owned()));
    assert!(!methods.contains(&"eth_sendRawTransaction".to_owned()));
    assert!(!methods.contains(&"eth_sendRawTransactionSync".to_owned()));
}
