//! Loopback JSON-RPC checks for receipt polling and finalized receipt validation.

use alloy::{primitives::B256, signers::local::PrivateKeySigner};
use nadfun_sdk::{Core, Network, ReceiptWaitOptions};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use wiremock::{matchers::method, Mock, MockServer, Request, ResponseTemplate};

const TX_HASH: B256 = B256::repeat_byte(0xaa);
const OTHER_TX_HASH: B256 = B256::repeat_byte(0xcc);
const BLOCK_HASH: B256 = B256::repeat_byte(0xbb);
const OTHER_BLOCK_HASH: B256 = B256::repeat_byte(0xdd);

struct RpcState {
    requests: Vec<Value>,
    receipts: VecDeque<Value>,
    finalized_blocks: VecDeque<Value>,
    receipt_delay: Duration,
    finalized_delay: Duration,
}

struct RpcFixture {
    server: MockServer,
    state: Arc<Mutex<RpcState>>,
}

impl RpcFixture {
    async fn start(
        receipts: impl IntoIterator<Item = Value>,
        finalized_blocks: impl IntoIterator<Item = Value>,
    ) -> Self {
        Self::start_with_delays(receipts, finalized_blocks, Duration::ZERO, Duration::ZERO).await
    }

    async fn start_with_delays(
        receipts: impl IntoIterator<Item = Value>,
        finalized_blocks: impl IntoIterator<Item = Value>,
        receipt_delay: Duration,
        finalized_delay: Duration,
    ) -> Self {
        let server = MockServer::builder().start().await;
        let state = Arc::new(Mutex::new(RpcState {
            requests: Vec::new(),
            receipts: receipts.into_iter().collect(),
            finalized_blocks: finalized_blocks.into_iter().collect(),
            receipt_delay,
            finalized_delay,
        }));
        let responder_state = state.clone();

        Mock::given(method("POST"))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                let rpc_method = body["method"].as_str().unwrap_or_default();
                let id = body["id"].clone();
                let (reply, delay) = {
                    let mut state = responder_state.lock().unwrap();
                    state.requests.push(body.clone());
                    match rpc_method {
                        "eth_getTransactionReceipt" => (
                            state.receipts.pop_front().unwrap_or_else(|| json!({"result": null})),
                            state.receipt_delay,
                        ),
                        "eth_getBlockByNumber" => (
                            state
                                .finalized_blocks
                                .pop_front()
                                .unwrap_or_else(|| json!({"result": null})),
                            state.finalized_delay,
                        ),
                        other => (
                            json!({
                                "error": { "code": -32601, "message": format!("unexpected method: {other}") }
                            }),
                            Duration::ZERO,
                        ),
                    }
                };

                let mut response = json!({"jsonrpc": "2.0", "id": id});
                if let Some(result) = reply.get("result") {
                    response["result"] = result.clone();
                } else {
                    response["error"] = reply["error"].clone();
                }
                let template = ResponseTemplate::new(200).set_body_json(response);
                if delay.is_zero() {
                    template
                } else {
                    template.set_delay(delay)
                }
            })
            .mount(&server)
            .await;

        Self { server, state }
    }

    fn methods(&self) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .requests
            .iter()
            .map(|request| request["method"].as_str().unwrap().to_owned())
            .collect()
    }

    fn requests(&self) -> Vec<Value> {
        self.state.lock().unwrap().requests.clone()
    }
}

async fn core(fixture: &RpcFixture) -> Core {
    let signer = PrivateKeySigner::random();
    let private_key = format!("0x{}", hex::encode(signer.to_bytes()));
    Core::new(fixture.server.uri(), private_key, Network::Testnet)
        .await
        .unwrap()
}

fn opts(finalized: bool) -> ReceiptWaitOptions {
    ReceiptWaitOptions {
        poll_interval: Duration::from_millis(1),
        timeout: Duration::from_secs(2),
        finalized,
    }
}

fn receipt(hash: B256, block: u64, block_hash: B256, status: bool) -> Value {
    json!({
        "type": "0x2",
        "status": if status { "0x1" } else { "0x0" },
        "cumulativeGasUsed": "0x5208",
        "logs": [],
        "logsBloom": format!("0x{}", "00".repeat(256)),
        "transactionHash": hash,
        "transactionIndex": "0x0",
        "blockHash": block_hash,
        "blockNumber": format!("0x{block:x}"),
        "gasUsed": "0x5208",
        "effectiveGasPrice": "0x1",
        "from": "0x1111111111111111111111111111111111111111",
        "to": "0x2222222222222222222222222222222222222222",
        "contractAddress": null
    })
}

fn finalized_block(number: u64) -> Value {
    json!({
        "result": {
            "hash": format!("0x{}", "ee".repeat(32)),
            "parentHash": format!("0x{}", "00".repeat(32)),
            "sha3Uncles": format!("0x{}", "00".repeat(32)),
            "miner": "0x1111111111111111111111111111111111111111",
            "stateRoot": format!("0x{}", "00".repeat(32)),
            "transactionsRoot": format!("0x{}", "00".repeat(32)),
            "receiptsRoot": format!("0x{}", "00".repeat(32)),
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "difficulty": "0x0",
            "number": format!("0x{number:x}"),
            "gasLimit": "0x100000",
            "gasUsed": "0x0",
            "timestamp": "0x1",
            "extraData": "0x",
            "mixHash": format!("0x{}", "00".repeat(32)),
            "nonce": "0x0000000000000000",
            "transactions": [],
            "uncles": []
        }
    })
}

#[tokio::test]
async fn wait_for_receipt_polls_null_then_returns_receipt_in_request_order() {
    let fixture = RpcFixture::start(
        [
            json!({"result": null}),
            json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)}),
        ],
        [],
    )
    .await;
    let core = core(&fixture).await;

    let result = core.wait_for_receipt(TX_HASH, opts(false)).await.unwrap();

    assert!(result.status);
    assert_eq!(result.transaction_hash, TX_HASH);
    assert_eq!(result.block_number, Some(5));
    assert_eq!(
        fixture.methods(),
        ["eth_getTransactionReceipt", "eth_getTransactionReceipt"]
    );
}

#[tokio::test]
async fn wait_for_receipt_fails_fast_with_context_on_receipt_rpc_error() {
    let fixture = RpcFixture::start(
        [json!({"error": {"code": -32000, "message": "receipt backend unavailable"}})],
        [],
    )
    .await;
    let core = core(&fixture).await;

    let error = core
        .wait_for_receipt(TX_HASH, opts(false))
        .await
        .unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("failed to get receipt"), "{message}");
    assert!(message.contains("receipt backend unavailable"), "{message}");
    assert_eq!(fixture.methods(), ["eth_getTransactionReceipt"]);
}

#[tokio::test]
async fn wait_for_receipt_rejects_receipt_with_different_transaction_hash() {
    let fixture = RpcFixture::start(
        [json!({"result": receipt(OTHER_TX_HASH, 5, BLOCK_HASH, true)})],
        [],
    )
    .await;
    let core = core(&fixture).await;

    let error = core
        .wait_for_receipt(TX_HASH, opts(false))
        .await
        .unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains(&TX_HASH.to_string()), "{message}");
    assert!(message.contains(&OTHER_TX_HASH.to_string()), "{message}");
    assert_eq!(fixture.methods(), ["eth_getTransactionReceipt"]);
}

#[tokio::test]
async fn wait_for_receipt_rejects_reverted_receipt_with_hash_and_block() {
    let fixture = RpcFixture::start(
        [json!({"result": receipt(TX_HASH, 17, BLOCK_HASH, false)})],
        [],
    )
    .await;
    let core = core(&fixture).await;

    let error = core
        .wait_for_receipt(TX_HASH, opts(false))
        .await
        .unwrap_err();
    let message = error.to_string();

    assert!(message.contains(&TX_HASH.to_string()), "{message}");
    assert!(message.contains("17"), "{message}");
    assert!(message.contains("revert"), "{message}");
    assert_eq!(fixture.methods(), ["eth_getTransactionReceipt"]);
}

#[tokio::test]
async fn wait_for_receipt_rejects_zero_interval_and_timeout_without_rpc() {
    let fixture = RpcFixture::start([], []).await;
    let core = core(&fixture).await;

    let mut zero_interval = opts(false);
    zero_interval.poll_interval = Duration::ZERO;
    assert!(core.wait_for_receipt(TX_HASH, zero_interval).await.is_err());

    let mut zero_timeout = opts(false);
    zero_timeout.timeout = Duration::ZERO;
    assert!(core.wait_for_receipt(TX_HASH, zero_timeout).await.is_err());
    assert!(fixture.methods().is_empty());
}

#[tokio::test]
async fn wait_for_receipt_times_out_with_hash_and_receipt_phase() {
    let fixture = RpcFixture::start([], []).await;
    let core = core(&fixture).await;
    let mut options = opts(false);
    options.poll_interval = Duration::from_millis(1);
    options.timeout = Duration::from_millis(8);

    let error = core.wait_for_receipt(TX_HASH, options).await.unwrap_err();
    let message = error.to_string();

    assert!(message.contains(&TX_HASH.to_string()), "{message}");
    assert!(message.contains("receipt"), "{message}");
}

#[tokio::test]
async fn finalized_wait_uses_finalized_height_then_refetches_receipt() {
    let fixture = RpcFixture::start(
        [
            json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)}),
            json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)}),
        ],
        [finalized_block(4), finalized_block(5)],
    )
    .await;
    let core = core(&fixture).await;

    let result = core.wait_for_receipt(TX_HASH, opts(true)).await.unwrap();

    assert_eq!(result.transaction_hash, TX_HASH);
    assert_eq!(result.block_number, Some(5));
    assert_eq!(
        fixture.methods(),
        [
            "eth_getTransactionReceipt",
            "eth_getHeaderByNumber",
            "eth_getBlockByNumber",
            "eth_getHeaderByNumber",
            "eth_getBlockByNumber",
            "eth_getTransactionReceipt"
        ]
    );
    let requests = fixture.requests();
    assert_eq!(requests[2]["params"][0], "finalized");
    assert_eq!(requests[4]["params"][0], "finalized");
}

#[tokio::test]
async fn finalized_wait_errors_if_receipt_disappears_after_finality() {
    let fixture = RpcFixture::start(
        [
            json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)}),
            json!({"result": null}),
        ],
        [finalized_block(5)],
    )
    .await;
    let core = core(&fixture).await;

    let error = core
        .wait_for_receipt(TX_HASH, opts(true))
        .await
        .unwrap_err();
    let message = error.to_string();

    assert!(message.contains("disappear"), "{message}");
    assert!(message.contains(&TX_HASH.to_string()), "{message}");
    assert_eq!(
        fixture.methods(),
        [
            "eth_getTransactionReceipt",
            "eth_getHeaderByNumber",
            "eth_getBlockByNumber",
            "eth_getTransactionReceipt"
        ]
    );
}

#[tokio::test]
async fn finalized_wait_errors_if_receipt_hash_or_inclusion_changes() {
    for replacement in [
        receipt(OTHER_TX_HASH, 5, BLOCK_HASH, true),
        receipt(TX_HASH, 6, BLOCK_HASH, true),
        receipt(TX_HASH, 5, OTHER_BLOCK_HASH, true),
    ] {
        let fixture = RpcFixture::start(
            [
                json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)}),
                json!({"result": replacement}),
            ],
            [finalized_block(5)],
        )
        .await;
        let core = core(&fixture).await;

        let error = core
            .wait_for_receipt(TX_HASH, opts(true))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("changed"), "{error:#}");
    }
}

#[tokio::test]
async fn finalized_wait_rejects_recheck_for_different_requested_transaction() {
    let fixture = RpcFixture::start(
        [
            json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)}),
            json!({"result": receipt(OTHER_TX_HASH, 5, BLOCK_HASH, true)}),
        ],
        [finalized_block(5)],
    )
    .await;
    let core = core(&fixture).await;

    let error = core
        .wait_for_receipt(TX_HASH, opts(true))
        .await
        .unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains(&TX_HASH.to_string()), "{message}");
    assert!(message.contains(&OTHER_TX_HASH.to_string()), "{message}");
    assert_eq!(
        fixture.methods(),
        [
            "eth_getTransactionReceipt",
            "eth_getHeaderByNumber",
            "eth_getBlockByNumber",
            "eth_getTransactionReceipt"
        ]
    );
}

#[tokio::test]
async fn finalized_rpc_error_is_returned_instead_of_success() {
    let fixture = RpcFixture::start(
        [json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)})],
        [json!({"error": {"code": -32601, "message": "finalized tag unsupported"}})],
    )
    .await;
    let core = core(&fixture).await;

    let error = core
        .wait_for_receipt(TX_HASH, opts(true))
        .await
        .unwrap_err();
    let message = format!("{error:#}");

    assert!(message.contains("finalized"), "{message}");
    assert!(message.contains("unsupported"), "{message}");
    assert_eq!(
        fixture.methods(),
        [
            "eth_getTransactionReceipt",
            "eth_getHeaderByNumber",
            "eth_getBlockByNumber"
        ]
    );
}

#[tokio::test]
async fn one_deadline_covers_initial_receipt_and_finality_rpc() {
    let fixture = RpcFixture::start_with_delays(
        [json!({"result": receipt(TX_HASH, 5, BLOCK_HASH, true)})],
        [finalized_block(5)],
        Duration::from_millis(300),
        Duration::from_millis(300),
    )
    .await;
    let core = core(&fixture).await;
    let mut options = opts(true);
    options.timeout = Duration::from_millis(500);
    let error = core.wait_for_receipt(TX_HASH, options).await.unwrap_err();

    assert!(error.to_string().contains("finality"), "{error:#}");
    assert_eq!(
        fixture.methods(),
        [
            "eth_getTransactionReceipt",
            "eth_getHeaderByNumber",
            "eth_getBlockByNumber"
        ]
    );
}
