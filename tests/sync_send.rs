//! Loopback JSON-RPC checks for Core's synchronous transaction submission.

use alloy::{
    consensus::{TxEnvelope, Typed2718},
    eips::eip2718::Decodable2718,
    network::TransactionBuilder,
    primitives::{Address, B256},
    rpc::types::{AccessList, TransactionRequest},
    signers::local::PrivateKeySigner,
};
use nadfun_sdk::{Core, Network};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use wiremock::{matchers::method, Mock, MockServer, Request, ResponseTemplate};

const SYNC_HASH: B256 = B256::repeat_byte(0xaa);
const ACCEPTED_HASH: B256 = B256::repeat_byte(0xbb);
const ORDINARY_HASH: B256 = B256::repeat_byte(0xcc);

#[derive(Clone, Copy)]
enum SyncReply {
    Receipt,
    Reverted,
    MethodNotFound,
    AcceptedHash,
    AcceptedBareString,
    CodeFourWithoutHash,
    OldTimeoutTextWrongCode,
    AmbiguousTransport,
    FallbackTransport,
    OtherHash,
}

struct RpcState {
    requests: Vec<Value>,
    sync_reply: SyncReply,
    sync_count: usize,
    ordinary_count: usize,
    receipts: VecDeque<Value>,
}

struct RpcFixture {
    server: MockServer,
    state: Arc<Mutex<RpcState>>,
}

impl RpcFixture {
    async fn start(sync_reply: SyncReply, receipts: impl IntoIterator<Item = Value>) -> Self {
        let server = MockServer::builder().start().await;
        let state = Arc::new(Mutex::new(RpcState {
            requests: Vec::new(),
            sync_reply,
            sync_count: 0,
            ordinary_count: 0,
            receipts: receipts.into_iter().collect(),
        }));
        let responder_state = state.clone();

        Mock::given(method("POST"))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                let rpc_method = body["method"].as_str().unwrap_or_default();
                let id = body["id"].clone();
                let (sync_reply, send_number, receipt_reply) = {
                    let mut state = responder_state.lock().unwrap();
                    state.requests.push(body.clone());
                    let send_number = match rpc_method {
                        "eth_sendRawTransactionSync" => {
                            state.sync_count += 1;
                            state.sync_count
                        }
                        "eth_sendRawTransaction" => {
                            state.ordinary_count += 1;
                            state.ordinary_count
                        }
                        _ => 0,
                    };
                    let receipt_reply = if rpc_method == "eth_getTransactionReceipt" {
                        Some(
                            state
                                .receipts
                                .pop_front()
                                .unwrap_or_else(|| json!({"result": null})),
                        )
                    } else {
                        None
                    };
                    (state.sync_reply, send_number, receipt_reply)
                };

                if rpc_method == "eth_sendRawTransactionSync"
                    && send_number == 1
                    && matches!(sync_reply, SyncReply::AmbiguousTransport)
                {
                    return ResponseTemplate::new(500)
                        .set_body_string("sync send response was lost");
                }
                if rpc_method == "eth_sendRawTransaction"
                    && send_number == 1
                    && matches!(sync_reply, SyncReply::FallbackTransport)
                {
                    return ResponseTemplate::new(500)
                        .set_body_string("ordinary send response was lost");
                }

                let mut response = json!({"jsonrpc": "2.0", "id": id});
                match rpc_method {
                    "eth_chainId" => response["result"] = json!("0x279f"),
                    "eth_getTransactionCount" => response["result"] = json!("0x0"),
                    "eth_sendRawTransactionSync" if send_number == 1 => match sync_reply {
                        SyncReply::Receipt => {
                            response["result"] = receipt(SYNC_HASH, 5, true);
                        }
                        SyncReply::Reverted => {
                            response["result"] = receipt(SYNC_HASH, 6, false);
                        }
                        SyncReply::MethodNotFound => {
                            response["error"] = json!({"code": -32601, "message": "Method not found"});
                        }
                        SyncReply::FallbackTransport => {
                            response["error"] = json!({"code": -32601, "message": "Method not found"});
                        }
                        SyncReply::AcceptedHash => {
                            response["error"] = json!({
                                "code": 4,
                                "message": "Transaction receipt not available within 2000ms timeout",
                                "data": { "hash": ACCEPTED_HASH }
                            });
                        }
                        SyncReply::AcceptedBareString => {
                            response["error"] = json!({
                                "code": 4,
                                "message": "Transaction added but not processed in time",
                                "data": ACCEPTED_HASH
                            });
                        }
                        SyncReply::CodeFourWithoutHash => {
                            response["error"] = json!({
                                "code": 4,
                                "message": "Transaction receipt not available within 2000ms timeout",
                                "data": {}
                            });
                        }
                        SyncReply::OldTimeoutTextWrongCode => {
                            response["error"] = json!({
                                "code": -32000,
                                "message": "Transaction added but not processed in time",
                                "data": { "hash": SYNC_HASH }
                            });
                        }
                        SyncReply::AmbiguousTransport => unreachable!(),
                        SyncReply::OtherHash => {
                            response["error"] = json!({
                                "code": -32000,
                                "message": "invalid transaction",
                                "data": SYNC_HASH
                            });
                        }
                    },
                    "eth_sendRawTransactionSync" => {
                        response["error"] = json!({"code": -32601, "message": "unexpected second sync send"});
                    }
                    "eth_sendRawTransaction" => response["result"] = json!(ORDINARY_HASH),
                    "eth_getTransactionReceipt" => {
                        let reply = receipt_reply.unwrap();
                        if let Some(result) = reply.get("result") {
                            response["result"] = result.clone();
                        } else {
                            response["error"] = reply["error"].clone();
                        }
                    }
                    other => {
                        response["error"] = json!({"code": -32601, "message": format!("unexpected method: {other}")});
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

    fn methods(&self) -> Vec<String> {
        self.requests()
            .iter()
            .map(|request| request["method"].as_str().unwrap().to_owned())
            .collect()
    }

    fn send_count(&self, method: &str) -> usize {
        self.methods()
            .iter()
            .filter(|candidate| candidate.as_str() == method)
            .count()
    }
}

async fn core(fixture: &RpcFixture) -> Core {
    let signer = PrivateKeySigner::random();
    let private_key = format!("0x{}", hex::encode(signer.to_bytes()));
    Core::new(fixture.server.uri(), private_key, Network::Testnet)
        .await
        .unwrap()
}

fn request() -> TransactionRequest {
    let mut tx = TransactionRequest::default()
        .with_chain_id(10_143)
        .with_gas_limit(21_000)
        .with_max_fee_per_gas(2)
        .with_max_priority_fee_per_gas(1)
        .with_to(Address::repeat_byte(0x42));
    tx.access_list = Some(AccessList::default());
    tx
}

fn receipt(hash: B256, block: u64, status: bool) -> Value {
    json!({
        "type": "0x2",
        "status": if status { "0x1" } else { "0x0" },
        "cumulativeGasUsed": "0x5208",
        "logs": [],
        "logsBloom": format!("0x{}", "00".repeat(256)),
        "transactionHash": hash,
        "transactionIndex": "0x0",
        "blockHash": B256::repeat_byte(0xdd),
        "blockNumber": format!("0x{block:x}"),
        "gasUsed": "0x5208",
        "effectiveGasPrice": "0x1",
        "from": Address::repeat_byte(0x11),
        "to": Address::repeat_byte(0x42),
        "contractAddress": null
    })
}

#[tokio::test]
async fn sync_send_returns_successful_type_two_receipt_from_one_sync_send() {
    let fixture = RpcFixture::start(SyncReply::Receipt, []).await;
    let core = core(&fixture).await;

    let result = core.send_transaction_sync(request()).await.unwrap();

    assert!(result.status);
    assert_eq!(result.transaction_hash, SYNC_HASH);
    assert_eq!(result.block_number, Some(5));
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    let sync_request = fixture
        .requests()
        .into_iter()
        .find(|request| request["method"] == "eth_sendRawTransactionSync")
        .unwrap();
    let raw = sync_request["params"][0].as_str().unwrap();
    let bytes = hex::decode(raw.strip_prefix("0x").unwrap()).unwrap();
    let mut bytes = bytes.as_slice();
    let signed = TxEnvelope::decode_2718(&mut bytes).unwrap();
    assert!(bytes.is_empty());
    assert_eq!(signed.ty(), 2);
}

#[tokio::test]
async fn method_not_found_falls_back_to_exactly_one_ordinary_send_and_waits() {
    let fixture = RpcFixture::start(
        SyncReply::MethodNotFound,
        [
            json!({"result": null}),
            json!({"result": receipt(ORDINARY_HASH, 7, true)}),
        ],
    )
    .await;
    let core = core(&fixture).await;

    let result = core.send_transaction_sync(request()).await.unwrap();

    assert!(result.status);
    assert_eq!(result.transaction_hash, ORDINARY_HASH);
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 1);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 2);
    let methods = fixture.methods();
    let sync_index = methods
        .iter()
        .position(|method| method == "eth_sendRawTransactionSync")
        .unwrap();
    let ordinary_index = methods
        .iter()
        .position(|method| method == "eth_sendRawTransaction")
        .unwrap();
    assert!(sync_index < ordinary_index);
}

#[tokio::test]
async fn accepted_hash_error_waits_for_receipt_without_resubmitting() {
    let fixture = RpcFixture::start(
        SyncReply::AcceptedHash,
        [
            json!({"result": null}),
            json!({"result": receipt(ACCEPTED_HASH, 8, true)}),
        ],
    )
    .await;
    let core = core(&fixture).await;

    let result = core.send_transaction_sync(request()).await.unwrap();

    assert_eq!(result.transaction_hash, ACCEPTED_HASH);
    assert_eq!(result.block_number, Some(8));
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 2);
    let methods = fixture.methods();
    let sync_index = methods
        .iter()
        .position(|method| method == "eth_sendRawTransactionSync")
        .unwrap();
    assert!(methods[sync_index + 1..]
        .iter()
        .all(|method| method == "eth_getTransactionReceipt"));
}

#[tokio::test]
async fn accepted_hash_rejects_receipt_with_different_transaction_hash() {
    let fixture = RpcFixture::start(
        SyncReply::AcceptedHash,
        [json!({"result": receipt(ORDINARY_HASH, 8, true)})],
    )
    .await;
    let core = core(&fixture).await;

    let error = core.send_transaction_sync(request()).await.unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("mismatched transaction hash"), "{message}");
    assert!(message.contains(&ACCEPTED_HASH.to_string()), "{message}");
    assert!(message.contains(&ORDINARY_HASH.to_string()), "{message}");
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 1);
}

#[tokio::test]
async fn accepted_bare_string_eip_7966_hash_waits_without_resubmitting() {
    let fixture = RpcFixture::start(
        SyncReply::AcceptedBareString,
        [json!({"result": receipt(ACCEPTED_HASH, 8, true)})],
    )
    .await;
    let core = core(&fixture).await;

    let result = core.send_transaction_sync(request()).await.unwrap();

    assert_eq!(result.transaction_hash, ACCEPTED_HASH);
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
}

#[tokio::test]
async fn code_four_without_hash_is_an_unknown_outcome_without_resubmitting() {
    let fixture = RpcFixture::start(SyncReply::CodeFourWithoutHash, []).await;
    let core = core(&fixture).await;

    let error = core.send_transaction_sync(request()).await.unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("outcome unknown"), "{message}");
    assert!(message.contains("do not resend blindly"), "{message}");
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 0);
}

#[tokio::test]
async fn old_timeout_text_on_non_four_error_is_not_treated_as_accepted() {
    let fixture = RpcFixture::start(
        SyncReply::OldTimeoutTextWrongCode,
        [json!({"result": receipt(SYNC_HASH, 8, true)})],
    )
    .await;
    let core = core(&fixture).await;

    let error = core.send_transaction_sync(request()).await.unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("outcome unknown"), "{message}");
    assert!(message.contains("do not resend blindly"), "{message}");
    assert!(message.contains(&SYNC_HASH.to_string()), "{message}");
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 0);
}

#[tokio::test]
async fn ambiguous_transport_error_does_not_trigger_a_second_send() {
    let fixture = RpcFixture::start(SyncReply::AmbiguousTransport, []).await;
    let core = core(&fixture).await;

    let error = core.send_transaction_sync(request()).await.unwrap_err();

    assert!(error.to_string().contains("sync"), "{error:#}");
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 0);
}

#[tokio::test]
async fn fallback_ordinary_transport_error_marks_unknown_outcome() {
    let fixture = RpcFixture::start(SyncReply::FallbackTransport, []).await;
    let core = core(&fixture).await;

    let error = core.send_transaction_sync(request()).await.unwrap_err();
    let message = format!("{error:#}");

    assert!(
        message.contains("outcome unknown; do not resend blindly"),
        "{message}"
    );
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 1);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 0);
}

#[tokio::test]
async fn unrecognized_hash_error_is_returned_with_hash_without_resending() {
    let fixture = RpcFixture::start(SyncReply::OtherHash, []).await;
    let core = core(&fixture).await;

    let error = core.send_transaction_sync(request()).await.unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains(&SYNC_HASH.to_string()), "{message}");
    assert!(message.contains("outcome unknown"), "{message}");
    assert!(message.contains("do not resend blindly"), "{message}");
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 0);
}

#[tokio::test]
async fn accepted_sync_path_polls_through_transient_receipt_rpc_errors() {
    let fixture = RpcFixture::start(
        SyncReply::AcceptedHash,
        [
            json!({"error": {"code": -32000, "message": "temporary receipt lookup failure"}}),
            json!({"result": receipt(ACCEPTED_HASH, 8, true)}),
        ],
    )
    .await;
    let core = core(&fixture).await;

    let result = core.send_transaction_sync(request()).await.unwrap();

    assert_eq!(result.transaction_hash, ACCEPTED_HASH);
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 2);
}

#[tokio::test]
async fn fallback_path_polls_through_transient_receipt_rpc_errors() {
    let fixture = RpcFixture::start(
        SyncReply::MethodNotFound,
        [
            json!({"error": {"code": -32000, "message": "temporary receipt lookup failure"}}),
            json!({"result": receipt(ORDINARY_HASH, 7, true)}),
        ],
    )
    .await;
    let core = core(&fixture).await;

    let result = core.send_transaction_sync(request()).await.unwrap();

    assert_eq!(result.transaction_hash, ORDINARY_HASH);
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 1);
    assert_eq!(fixture.send_count("eth_getTransactionReceipt"), 2);
}

#[tokio::test]
async fn submitted_receipt_timeout_reports_prefix_hash_and_last_rpc_error() {
    let fixture = RpcFixture::start(
        SyncReply::AcceptedBareString,
        [json!({
            "error": {"code": -32000, "message": "temporary receipt lookup failure"}
        })],
    )
    .await;
    let core = core(&fixture).await;

    let send = tokio::spawn(async move { core.send_transaction_sync(request()).await });
    let deadline = Instant::now() + Duration::from_secs(5);
    while fixture.send_count("eth_getTransactionReceipt") < 2 {
        assert!(Instant::now() < deadline, "receipt polling did not start");
        tokio::task::yield_now().await;
    }
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(121)).await;

    let error = send.await.unwrap().unwrap_err();
    let message = format!("{error:#}");

    assert!(
        message.starts_with("transaction submitted; outcome unknown"),
        "{message}"
    );
    assert!(message.contains(&ACCEPTED_HASH.to_string()), "{message}");
    assert!(
        message.contains("temporary receipt lookup failure"),
        "{message}"
    );
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
    assert!(fixture.send_count("eth_getTransactionReceipt") > 1);
}

#[tokio::test]
async fn sync_send_rejects_reverted_receipt_with_hash_and_block() {
    let fixture = RpcFixture::start(SyncReply::Reverted, []).await;
    let core = core(&fixture).await;

    let error = core.send_transaction_sync(request()).await.unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains(&SYNC_HASH.to_string()), "{message}");
    assert!(message.contains("6"), "{message}");
    assert!(message.contains("revert"), "{message}");
    assert_eq!(fixture.send_count("eth_sendRawTransactionSync"), 1);
    assert_eq!(fixture.send_count("eth_sendRawTransaction"), 0);
}
