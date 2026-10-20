//! Loopback JSON-RPC checks for nonce classification on sync sends.

use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::eip2718::Decodable2718,
    network::TransactionBuilder,
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::{AccessList, TransactionRequest},
    signers::local::PrivateKeySigner,
};
use nadfun_sdk::{Core, Network};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use wiremock::{matchers::method, Mock, MockServer, Request, ResponseTemplate};

#[derive(Clone, Copy)]
enum FirstSend {
    AcceptedHash,
    AcceptedBareString,
    MethodNotFound,
    CodeFiveRejection,
    CodeFourWithoutHash,
    OldTimeoutTextWrongCode,
    AmbiguousTransport,
    OtherHash,
}

struct RpcState {
    requests: Vec<Value>,
    first_send: FirstSend,
    send_count: usize,
}

struct RpcFixture {
    server: MockServer,
    state: Arc<Mutex<RpcState>>,
}

impl RpcFixture {
    async fn start(first_send: FirstSend) -> Self {
        let server = MockServer::builder().start().await;
        let state = Arc::new(Mutex::new(RpcState {
            requests: Vec::new(),
            first_send,
            send_count: 0,
        }));
        let responder_state = state.clone();

        Mock::given(method("POST"))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                let rpc_method = body["method"].as_str().unwrap_or_default();
                let id = body["id"].clone();
                let (first_send, send_number) = {
                    let mut state = responder_state.lock().unwrap();
                    state.requests.push(body.clone());
                    if rpc_method == "eth_sendRawTransactionSync" {
                        state.send_count += 1;
                    }
                    (state.first_send, state.send_count)
                };

                if rpc_method == "eth_sendRawTransactionSync"
                    && send_number == 1
                    && matches!(first_send, FirstSend::AmbiguousTransport)
                {
                    return ResponseTemplate::new(500)
                        .set_body_string("sync send response was lost");
                }

                let mut response = json!({ "jsonrpc": "2.0", "id": id });
                match rpc_method {
                    "eth_chainId" => response["result"] = json!("0x279f"),
                    "eth_getTransactionCount" => response["result"] = json!("0x0"),
                    "eth_sendRawTransactionSync" if send_number == 1 => {
                        response["error"] = match first_send {
                            FirstSend::AcceptedHash => json!({
                                "code": 4,
                                "message": "Transaction receipt not available within 2000ms timeout",
                                "data": { "hash": format!("0x{}", "ab".repeat(32)) }
                            }),
                            FirstSend::AcceptedBareString => json!({
                                "code": 4,
                                "message": "Transaction added but not processed in time",
                                "data": format!("0x{}", "ab".repeat(32))
                            }),
                            FirstSend::MethodNotFound => json!({
                                "code": -32601,
                                "message": "Method not found"
                            }),
                            FirstSend::CodeFiveRejection => json!({
                                "code": 5,
                                "message": "The transaction is not ready to be processed"
                            }),
                            FirstSend::CodeFourWithoutHash => json!({
                                "code": 4,
                                "message": "Transaction receipt not available within 2000ms timeout",
                                "data": {}
                            }),
                            FirstSend::OldTimeoutTextWrongCode => json!({
                                "code": -32000,
                                "message": "Transaction added but not processed in time",
                                "data": format!("0x{}", "ab".repeat(32))
                            }),
                            FirstSend::AmbiguousTransport => unreachable!(),
                            FirstSend::OtherHash => json!({
                                "code": -32000,
                                "message": "invalid transaction",
                                "data": format!("0x{}", "cd".repeat(32))
                            }),
                        };
                    }
                    "eth_sendRawTransactionSync" => {
                        response["result"] = receipt();
                    }
                    other => {
                        response["error"] = json!({
                            "code": -32601,
                            "message": format!("unexpected method: {other}")
                        });
                    }
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

    fn signed_nonces(&self) -> Vec<u64> {
        self.requests()
            .into_iter()
            .filter(|request| request["method"] == "eth_sendRawTransactionSync")
            .map(|request| {
                let raw = request["params"][0].as_str().unwrap();
                let raw = hex::decode(raw.strip_prefix("0x").unwrap()).unwrap();
                let mut raw = raw.as_slice();
                let tx = TxEnvelope::decode_2718(&mut raw).unwrap();
                assert!(raw.is_empty(), "signed transaction has trailing bytes");
                tx.nonce()
            })
            .collect()
    }
}

async fn signed_core(fixture: &RpcFixture) -> Core {
    let signer = PrivateKeySigner::random();
    let private_key = format!("0x{}", hex::encode(signer.to_bytes()));
    Core::new(fixture.server.uri(), private_key, Network::Testnet)
        .await
        .unwrap()
}

fn request(nonce: Option<u64>) -> TransactionRequest {
    let mut request = TransactionRequest::default()
        .with_chain_id(10_143)
        .with_gas_limit(21_000)
        .with_max_fee_per_gas(1)
        .with_max_priority_fee_per_gas(1)
        .with_to(Address::repeat_byte(0x42));
    request.access_list = Some(AccessList::default());
    if let Some(nonce) = nonce {
        request.set_nonce(nonce);
    }
    request
}

async fn send_error_then_success(
    fixture: &RpcFixture,
    core: &Core,
    first_nonce: Option<u64>,
) -> Vec<u64> {
    assert!(core
        .provider()
        .send_transaction_sync(request(first_nonce))
        .await
        .is_err());
    core.provider()
        .send_transaction_sync(request(None))
        .await
        .unwrap();
    fixture.signed_nonces()
}

fn receipt() -> Value {
    json!({
        "type": "0x2",
        "status": "0x1",
        "cumulativeGasUsed": "0x5208",
        "logs": [],
        "logsBloom": format!("0x{}", "00".repeat(256)),
        "transactionHash": B256::repeat_byte(0xaa),
        "transactionIndex": "0x0",
        "blockHash": B256::repeat_byte(0xbb),
        "blockNumber": "0x1",
        "gasUsed": "0x5208",
        "effectiveGasPrice": "0x1",
        "from": Address::repeat_byte(0x11),
        "to": Address::repeat_byte(0x42),
        "contractAddress": null
    })
}

#[tokio::test]
async fn recognized_sync_timeout_hash_advances_auto_nonce() {
    let fixture = RpcFixture::start(FirstSend::AcceptedHash).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 1]);
}

#[tokio::test]
async fn eip_7966_bare_string_hash_advances_auto_nonce() {
    let fixture = RpcFixture::start(FirstSend::AcceptedBareString).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 1]);
}

#[tokio::test]
async fn method_not_found_leaves_sync_nonce_reusable() {
    let fixture = RpcFixture::start(FirstSend::MethodNotFound).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 0]);
}

#[tokio::test]
async fn monad_code_five_rejection_leaves_sync_nonce_reusable() {
    let fixture = RpcFixture::start(FirstSend::CodeFiveRejection).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 0]);
}

#[tokio::test]
async fn code_four_without_hash_leaves_sync_nonce_reusable() {
    let fixture = RpcFixture::start(FirstSend::CodeFourWithoutHash).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 0]);
}

#[tokio::test]
async fn old_timeout_text_on_non_four_error_does_not_advance_nonce() {
    let fixture = RpcFixture::start(FirstSend::OldTimeoutTextWrongCode).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 0]);
}

#[tokio::test]
async fn ambiguous_sync_transport_error_reuses_nonce() {
    let fixture = RpcFixture::start(FirstSend::AmbiguousTransport).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 0]);
}

#[tokio::test]
async fn unrecognized_hash_bearing_sync_error_reuses_nonce() {
    let fixture = RpcFixture::start(FirstSend::OtherHash).await;
    let core = signed_core(&fixture).await;

    assert_eq!(send_error_then_success(&fixture, &core, None).await, [0, 0]);
}

#[tokio::test]
async fn recognized_sync_timeout_hash_advances_fixed_nonce_state() {
    let fixture = RpcFixture::start(FirstSend::AcceptedHash).await;
    let core = signed_core(&fixture).await;

    assert_eq!(
        send_error_then_success(&fixture, &core, Some(7)).await,
        [7, 8]
    );
}
