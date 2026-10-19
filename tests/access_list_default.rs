//! Loopback JSON-RPC checks for access-list generation in SDK signing providers.

use alloy::{
    consensus::{transaction::SignerRecoverable, Transaction, TxEnvelope, Typed2718},
    eips::eip2718::Decodable2718,
    network::TransactionBuilder,
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::{AccessList, AccessListItem, TransactionRequest},
    signers::local::PrivateKeySigner,
};
use nadfun_sdk::{
    Core, GasEstimationParams, GasPricing, Network, Router, V2ExactOutBuyParams,
    V2GasEstimationParams,
};
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use wiremock::{matchers::method, Mock, MockServer, Request, ResponseTemplate};

const CHAIN_ID: u64 = 143;
const GAS_ESTIMATE: u64 = 50_000;
const DESTINATION: Address = Address::repeat_byte(0x42);
const WARM_EMPTY: Address = Address::repeat_byte(0x51);
const WARM_KEYED: Address = Address::repeat_byte(0x52);
const SENDER_KEY: B256 = B256::repeat_byte(0xa1);
const DESTINATION_KEY: B256 = B256::repeat_byte(0xb2);
const WARM_KEY: B256 = B256::repeat_byte(0xc3);

#[derive(Clone, Copy, Debug)]
enum AccessListReply {
    Generated,
    RevertResult,
    RpcFailure,
    Timeout,
    EmptyAfterFiltering,
    Oversized,
    TooCostly,
}

#[derive(Clone, Copy)]
enum EstimateReply {
    Success,
    Gas(u64),
    RpcFailure,
}

#[derive(Clone, Copy)]
enum SendReply {
    Accept,
    RejectFirst,
}

struct RpcState {
    requests: Vec<Value>,
    access_list_reply: AccessListReply,
    estimate_reply: EstimateReply,
    send_reply: SendReply,
    raw_send_count: usize,
}

struct RpcFixture {
    server: MockServer,
    state: Arc<Mutex<RpcState>>,
}

impl RpcFixture {
    async fn start(
        access_list_reply: AccessListReply,
        estimate_reply: EstimateReply,
        send_reply: SendReply,
    ) -> Self {
        let server = MockServer::start().await;
        let state = Arc::new(Mutex::new(RpcState {
            requests: Vec::new(),
            access_list_reply,
            estimate_reply,
            send_reply,
            raw_send_count: 0,
        }));
        let responder_state = state.clone();

        Mock::given(method("POST"))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                let rpc_method = body["method"].as_str().unwrap_or_default();
                let id = body["id"].clone();
                let (access_list_reply, estimate_reply, send_reply, send_number) = {
                    let mut state = responder_state.lock().unwrap();
                    state.requests.push(body.clone());
                    if rpc_method == "eth_sendRawTransaction" {
                        state.raw_send_count += 1;
                    }
                    (
                        state.access_list_reply,
                        state.estimate_reply,
                        state.send_reply,
                        state.raw_send_count,
                    )
                };

                if rpc_method == "eth_createAccessList"
                    && matches!(access_list_reply, AccessListReply::Timeout)
                {
                    let result = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "accessList": generated_list(&body["params"][0]),
                            "gasUsed": "0xc350"
                        }
                    });
                    return ResponseTemplate::new(200)
                        .set_delay(Duration::from_millis(5_100))
                        .set_body_json(result);
                }

                let result = match rpc_method {
                    "eth_chainId" => json!({ "result": "0x8f" }),
                    "eth_getTransactionCount" => json!({ "result": "0x0" }),
                    "eth_createAccessList" => match access_list_reply {
                        AccessListReply::Generated => json!({
                            "result": {
                                "accessList": generated_list(&body["params"][0]),
                                "gasUsed": "0xc350"
                            }
                        }),
                        AccessListReply::EmptyAfterFiltering => json!({
                            "result": {
                                "accessList": [
                                    { "address": body["params"][0]["from"], "storageKeys": [] },
                                    { "address": body["params"][0]["to"], "storageKeys": [] }
                                ],
                                "gasUsed": "0x5208"
                            }
                        }),
                        AccessListReply::TooCostly => json!({
                            "result": {
                                "accessList": generated_list(&body["params"][0]),
                                "gasUsed": "0x3bc4"
                            }
                        }),
                        AccessListReply::Oversized => json!({
                            "result": {
                                "accessList": [{
                                    "address": WARM_EMPTY,
                                    "storageKeys": vec![format!("{WARM_KEY}"); 1024]
                                }],
                                "gasUsed": "0xc350"
                            }
                        }),
                        AccessListReply::RevertResult => json!({
                            "result": {
                                "accessList": [],
                                "gasUsed": "0x0",
                                "error": "execution reverted"
                            }
                        }),
                        AccessListReply::RpcFailure => json!({
                            "error": { "code": -32000, "message": "access-list RPC unavailable" }
                        }),
                        AccessListReply::Timeout => unreachable!("handled by delayed response"),
                    },
                    "eth_estimateGas" => match estimate_reply {
                        EstimateReply::Success => json!({ "result": "0xc350" }),
                        EstimateReply::Gas(gas) => json!({ "result": format!("0x{gas:x}") }),
                        EstimateReply::RpcFailure => json!({
                            "error": { "code": -32000, "message": "original estimate failure" }
                        }),
                    },
                    "eth_sendRawTransaction" => {
                        if matches!(send_reply, SendReply::RejectFirst) && send_number == 1 {
                            json!({
                                "error": { "code": -32000, "message": "nonce too low" }
                            })
                        } else {
                            json!({ "result": format!("0x{}", "ab".repeat(32)) })
                        }
                    }
                    other => json!({
                        "error": { "code": -32601, "message": format!("unexpected method: {other}") }
                    }),
                };

                let mut response = json!({ "jsonrpc": "2.0", "id": id });
                if let Some(value) = result.get("result") {
                    response["result"] = value.clone();
                } else {
                    response["error"] = result["error"].clone();
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

async fn signed_core(fixture: &RpcFixture) -> (Core, Address) {
    let signer = PrivateKeySigner::random();
    let address = signer.address();
    let private_key = format!("0x{}", hex::encode(signer.to_bytes()));
    let core = Core::new(fixture.server.uri(), private_key, Network::Mainnet)
        .await
        .unwrap();
    assert_eq!(core.wallet_address(), address);
    (core, address)
}

fn generated_list(tx: &Value) -> Value {
    let from = tx["from"]
        .as_str()
        .unwrap_or("0x0000000000000000000000000000000000000000");
    let to = tx["to"]
        .as_str()
        .unwrap_or("0x0000000000000000000000000000000000000000");
    json!([
        { "address": from, "storageKeys": [] },
        { "address": to, "storageKeys": [] },
        { "address": from, "storageKeys": [SENDER_KEY] },
        { "address": to, "storageKeys": [DESTINATION_KEY] },
        { "address": WARM_EMPTY, "storageKeys": [] },
        { "address": WARM_KEYED, "storageKeys": [WARM_KEY] }
    ])
}

fn expected_filtered_list(sender: Address, destination: Address) -> AccessList {
    AccessList(vec![
        AccessListItem {
            address: sender,
            storage_keys: vec![SENDER_KEY],
        },
        AccessListItem {
            address: destination,
            storage_keys: vec![DESTINATION_KEY],
        },
        AccessListItem {
            address: WARM_EMPTY,
            storage_keys: vec![],
        },
        AccessListItem {
            address: WARM_KEYED,
            storage_keys: vec![WARM_KEY],
        },
    ])
}

fn simple_request(destination: Address) -> TransactionRequest {
    TransactionRequest::default()
        .with_chain_id(CHAIN_ID)
        .with_to(destination)
        .with_gas_price(3)
        .with_input(vec![0x01, 0x02, 0x03])
}

fn method_positions(methods: &[String], wanted: &str) -> Vec<usize> {
    methods
        .iter()
        .enumerate()
        .filter_map(|(index, method)| (method == wanted).then_some(index))
        .collect()
}

fn signed_transaction(fixture: &RpcFixture) -> TxEnvelope {
    let request = fixture
        .requests()
        .into_iter()
        .find(|request| request["method"] == "eth_sendRawTransaction")
        .expect("mock send did not receive a signed transaction");
    let raw = request["params"][0].as_str().unwrap();
    let raw = hex::decode(raw.strip_prefix("0x").unwrap()).unwrap();
    let mut raw = raw.as_slice();
    let tx = TxEnvelope::decode_2718(&mut raw).unwrap();
    assert!(
        raw.is_empty(),
        "signed transaction decoder left trailing bytes"
    );
    tx
}

fn write_report(fixture: &RpcFixture, sender: Address) {
    let requests = fixture.requests();
    let methods: Vec<_> = requests
        .iter()
        .map(|request| request["method"].as_str().unwrap())
        .collect();
    let create_index = methods
        .iter()
        .position(|method| *method == "eth_createAccessList");
    let estimate_request = requests
        .iter()
        .find(|request| request["method"] == "eth_estimateGas");
    let tx = signed_transaction(fixture);
    let recovered_sender = tx.recover_signer().ok();
    let access_summary: Vec<_> = tx
        .access_list()
        .into_iter()
        .flat_map(|list| list.iter())
        .map(|item| {
            let address_role = if item.address == sender {
                "sender"
            } else if Some(item.address) == tx.to() {
                "destination"
            } else if item.address == WARM_EMPTY {
                "warm-empty"
            } else {
                "warm-keyed"
            };
            json!({
                "address_role": address_role,
                "storage_key_count": item.storage_keys.len()
            })
        })
        .collect();
    let report = json!({
        "schema_version": 1,
        "rpc_order": methods,
        "create_access_list_index": create_index,
        "estimate_contains_access_list": estimate_request
            .map(|request| !request["params"][0]["accessList"].is_null())
            .unwrap_or(false),
        "signed_transaction": {
            "type": tx.ty(),
            "gas_limit": tx.gas_limit(),
            "nonce": tx.nonce(),
            "signer_matches_expected": recovered_sender == Some(sender),
            "access_list": access_summary
        }
    });
    std::fs::create_dir_all("target").unwrap();
    std::fs::write(
        "target/access-list-default-report.json",
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn auto_send_uses_access_list_before_gas_estimate() {
    let fixture = RpcFixture::start(
        AccessListReply::Generated,
        EstimateReply::Success,
        SendReply::Accept,
    )
    .await;
    let (core, sender) = signed_core(&fixture).await;
    let _ = core
        .provider()
        .send_transaction(simple_request(DESTINATION))
        .await
        .unwrap();

    let requests = fixture.requests();
    let methods = fixture.methods();
    let create = method_positions(&methods, "eth_createAccessList")
        .first()
        .copied();
    let estimate = method_positions(&methods, "eth_estimateGas")
        .first()
        .copied();
    let estimate_request = requests
        .iter()
        .find(|request| request["method"] == "eth_estimateGas");
    let expected = expected_filtered_list(sender, DESTINATION);
    let signed = signed_transaction(&fixture);
    write_report(&fixture, sender);

    assert!(create.is_some(), "send did not call eth_createAccessList");
    assert!(estimate.is_some(), "send did not call eth_estimateGas");
    assert!(
        create.unwrap() < estimate.unwrap(),
        "access-list probe must precede gas estimate"
    );
    let probe = &requests[create.unwrap()]["params"][0];
    assert_eq!(
        probe["from"].as_str().unwrap().parse::<Address>().unwrap(),
        sender
    );
    assert!(probe["gas"].is_null(), "probe must omit the caller gas cap");
    let estimate_request = estimate_request.expect("missing gas estimate request");
    let estimate_list: AccessList =
        serde_json::from_value(estimate_request["params"][0]["accessList"].clone()).unwrap();
    assert_eq!(
        estimate_list, expected,
        "estimate did not use filtered access list"
    );
    assert_eq!(
        signed.ty(),
        1,
        "legacy gas pricing plus an access list must sign EIP-2930"
    );
    assert_eq!(
        signed.gas_limit(),
        GAS_ESTIMATE,
        "signed gas must use list-aware estimate"
    );
    assert_eq!(signed.access_list(), Some(&expected));
    assert_eq!(signed.recover_signer().unwrap(), sender);
}

#[tokio::test]
async fn public_estimates_use_access_list() {
    let fixture = RpcFixture::start(
        AccessListReply::Generated,
        EstimateReply::Success,
        SendReply::Accept,
    )
    .await;
    let (core, sender) = signed_core(&fixture).await;
    let v1_estimate = core
        .v1()
        .estimate_gas(
            &Router::BondingCurve(Address::repeat_byte(0x61)),
            GasEstimationParams::Sell {
                token: Address::repeat_byte(0x62),
                amount_in: U256::from(10),
                amount_out_min: U256::from(1),
                to: sender,
                deadline: U256::from(1_900_000_000u64),
            },
        )
        .await
        .unwrap();
    assert_eq!(v1_estimate, GAS_ESTIMATE);

    let mut v2_params = V2ExactOutBuyParams {
        token: Address::repeat_byte(0x63),
        to: sender,
        amount_out: U256::from(7),
        amount_in_max: U256::from(11),
        deadline: U256::from(1_900_000_000u64),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    let v2_estimate = core
        .v2()
        .estimate_gas(V2GasEstimationParams::ExactOutBuy(v2_params.clone()))
        .await
        .unwrap();
    assert_eq!(v2_estimate, GAS_ESTIMATE);
    v2_params.gas_limit = Some(v2_estimate);
    v2_params.gas_price = Some(GasPricing::LegacyWithPrice { gas_price: 3 });
    core.v2().exact_out_buy(v2_params).await.unwrap();

    let requests = fixture.requests();
    let estimates: Vec<_> = requests
        .iter()
        .filter(|request| request["method"] == "eth_estimateGas")
        .collect();
    assert_eq!(
        estimates.len(),
        2,
        "explicit gas limit should skip a third estimate"
    );
    assert!(
        estimates
            .iter()
            .all(|request| !request["params"][0]["accessList"].is_null()),
        "both public estimates must include generated access lists"
    );
    let access_calls = requests
        .iter()
        .filter(|request| request["method"] == "eth_createAccessList")
        .count();
    assert_eq!(
        access_calls, 3,
        "each public estimate and the send should build once"
    );
    let signed = signed_transaction(&fixture);
    assert_eq!(
        signed.gas_limit(),
        v2_estimate,
        "caller estimate was changed on send"
    );
    assert_eq!(
        signed.access_list(),
        Some(&expected_filtered_list(sender, signed.to().unwrap()))
    );
}

#[tokio::test]
async fn warm_entries_keep_storage_keys() {
    let fixture = RpcFixture::start(
        AccessListReply::Generated,
        EstimateReply::Success,
        SendReply::Accept,
    )
    .await;
    let (core, sender) = signed_core(&fixture).await;
    let estimate = core
        .provider()
        .estimate_gas(
            TransactionRequest::default()
                .with_chain_id(CHAIN_ID)
                .with_to(DESTINATION),
        )
        .await
        .unwrap();
    assert_eq!(estimate, GAS_ESTIMATE);

    let requests = fixture.requests();
    let probe = requests
        .iter()
        .find(|request| request["method"] == "eth_createAccessList")
        .expect("direct estimate did not create an access list");
    assert_eq!(
        probe["params"][0]["from"]
            .as_str()
            .unwrap()
            .parse::<Address>()
            .unwrap(),
        sender
    );
    let estimate = requests
        .iter()
        .find(|request| request["method"] == "eth_estimateGas")
        .unwrap();
    let actual: AccessList =
        serde_json::from_value(estimate["params"][0]["accessList"].clone()).unwrap();
    assert_eq!(actual, expected_filtered_list(sender, DESTINATION));
}

#[tokio::test]
async fn empty_filtered_list_is_not_attached() {
    let fixture = RpcFixture::start(
        AccessListReply::EmptyAfterFiltering,
        EstimateReply::Success,
        SendReply::Accept,
    )
    .await;
    let (core, _) = signed_core(&fixture).await;
    core.provider()
        .estimate_gas(TransactionRequest::default().with_to(DESTINATION))
        .await
        .unwrap();
    let _ = core
        .provider()
        .send_transaction(simple_request(DESTINATION).with_gas_limit(30_000))
        .await
        .unwrap();

    let requests = fixture.requests();
    let estimate = requests
        .iter()
        .find(|request| request["method"] == "eth_estimateGas")
        .expect("missing estimate request");
    assert!(estimate["params"][0]["accessList"].is_null());
    assert!(
        signed_transaction(&fixture).access_list().is_none(),
        "a filtered empty list must not affect the signed transaction"
    );
}

#[tokio::test]
async fn access_list_failure_preserves_estimate_error() {
    for (index, reply) in [AccessListReply::RevertResult, AccessListReply::RpcFailure]
        .into_iter()
        .enumerate()
    {
        let fixture = RpcFixture::start(reply, EstimateReply::RpcFailure, SendReply::Accept).await;
        let (core, _) = signed_core(&fixture).await;
        let result = core
            .provider()
            .send_transaction(simple_request(Address::repeat_byte(0x71 + index as u8)))
            .await;
        let error = result.expect_err("failed original estimate unexpectedly sent");
        assert!(
            format!("{error:#}").contains("original estimate failure"),
            "fallback must surface the original estimate error: {error:#}"
        );

        let requests = fixture.requests();
        assert!(requests
            .iter()
            .any(|request| request["method"] == "eth_createAccessList"));
        let estimate = requests
            .iter()
            .find(|request| request["method"] == "eth_estimateGas")
            .expect("fallback did not use the original estimate path");
        assert!(estimate["params"][0]["accessList"].is_null());
        assert!(
            !requests.iter().any(|request| {
                matches!(
                    request["method"].as_str(),
                    Some("eth_getTransactionCount" | "eth_sendRawTransaction")
                )
            }),
            "a failed estimate must not take a nonce or send"
        );
    }
}

#[tokio::test]
async fn explicit_gas_limit_is_checked_after_failed_access_list_probe() {
    for reply in [AccessListReply::RpcFailure, AccessListReply::RevertResult] {
        let fixture = RpcFixture::start(reply, EstimateReply::Gas(70_000), SendReply::Accept).await;
        let (core, _) = signed_core(&fixture).await;
        let result = core
            .provider()
            .send_transaction(simple_request(DESTINATION).with_gas_limit(60_000))
            .await;
        assert!(
            result.is_err(),
            "unsafe explicit gas limit was sent: {reply:?}"
        );

        let requests = fixture.requests();
        let estimate = requests
            .iter()
            .find(|request| request["method"] == "eth_estimateGas")
            .expect("failed probe must trigger a list-less gas estimate");
        assert!(estimate["params"][0]["accessList"].is_null());
        assert!(estimate["params"][0]["gas"].is_null());
        assert!(
            !requests.iter().any(|request| {
                matches!(
                    request["method"].as_str(),
                    Some("eth_getTransactionCount" | "eth_sendRawTransaction")
                )
            }),
            "an over-limit estimate must not take a nonce or send"
        );
    }

    let fixture = RpcFixture::start(
        AccessListReply::RpcFailure,
        EstimateReply::Gas(50_000),
        SendReply::Accept,
    )
    .await;
    let (core, _) = signed_core(&fixture).await;
    let _ = core
        .provider()
        .send_transaction(simple_request(DESTINATION).with_gas_limit(60_000))
        .await
        .expect("a gas limit that covers the list-less estimate should send");
    let requests = fixture.requests();
    assert!(requests
        .iter()
        .any(|request| request["method"] == "eth_estimateGas"));
    assert!(requests
        .iter()
        .any(|request| request["method"] == "eth_sendRawTransaction"));
    assert!(signed_transaction(&fixture).access_list().is_none());
}

#[tokio::test]
async fn oversized_or_intrinsically_expensive_lists_are_discarded() {
    for reply in [AccessListReply::TooCostly, AccessListReply::Oversized] {
        let fixture = RpcFixture::start(reply, EstimateReply::Success, SendReply::Accept).await;
        let (core, _) = signed_core(&fixture).await;

        core.provider()
            .estimate_gas(TransactionRequest::default().with_to(DESTINATION))
            .await
            .unwrap();
        let _ = core
            .provider()
            .send_transaction(simple_request(DESTINATION))
            .await
            .unwrap();

        let requests = fixture.requests();
        let estimates: Vec<_> = requests
            .iter()
            .filter(|request| request["method"] == "eth_estimateGas")
            .collect();
        assert_eq!(estimates.len(), 2);
        assert!(
            estimates
                .iter()
                .all(|request| request["params"][0]["accessList"].is_null()),
            "rejected RPC list must be omitted from estimate and send filling: {reply:?}"
        );
    }
}

#[tokio::test]
async fn explicit_gas_limit_is_checked_when_generated_list_is_rejected() {
    for reply in [AccessListReply::Oversized, AccessListReply::TooCostly] {
        let fixture = RpcFixture::start(reply, EstimateReply::Gas(70_000), SendReply::Accept).await;
        let (core, _) = signed_core(&fixture).await;
        assert!(core
            .provider()
            .send_transaction(simple_request(DESTINATION).with_gas_limit(60_000))
            .await
            .is_err());

        let requests = fixture.requests();
        let estimate = requests
            .iter()
            .find(|request| request["method"] == "eth_estimateGas")
            .expect("rejected list must trigger a list-less estimate");
        assert!(estimate["params"][0]["accessList"].is_null());
        assert!(!requests.iter().any(|request| {
            matches!(
                request["method"].as_str(),
                Some("eth_getTransactionCount" | "eth_sendRawTransaction")
            )
        }));
    }
}

#[tokio::test]
async fn access_list_probe_uses_a_short_timeout() {
    let fixture = RpcFixture::start(
        AccessListReply::Timeout,
        EstimateReply::Gas(50_000),
        SendReply::Accept,
    )
    .await;
    let (core, _) = signed_core(&fixture).await;
    let _ = core
        .provider()
        .send_transaction(simple_request(DESTINATION).with_gas_limit(60_000))
        .await
        .unwrap();

    assert!(
        fixture
            .requests()
            .iter()
            .any(|request| request["method"] == "eth_estimateGas"),
        "a delayed access-list response must time out and use list-less estimation"
    );
}

#[tokio::test]
async fn estimate_access_list_probe_forwards_block_and_skips_overrides() {
    let fixture = RpcFixture::start(
        AccessListReply::Generated,
        EstimateReply::Success,
        SendReply::Accept,
    )
    .await;
    let (core, _) = signed_core(&fixture).await;
    core.provider()
        .estimate_gas(TransactionRequest::default().with_to(DESTINATION))
        .block(alloy::eips::BlockId::latest())
        .await
        .unwrap();

    let access_list_call = fixture
        .requests()
        .into_iter()
        .find(|request| request["method"] == "eth_createAccessList")
        .expect("estimate did not probe for an access list");
    assert_eq!(access_list_call["params"][1], "latest");

    core.provider()
        .estimate_gas(TransactionRequest::default().with_to(DESTINATION))
        .overrides(alloy::rpc::types::state::StateOverride::default())
        .await
        .unwrap();
    assert_eq!(
        fixture
            .requests()
            .iter()
            .filter(|request| request["method"] == "eth_createAccessList")
            .count(),
        1,
        "state overrides cannot be represented by eth_createAccessList"
    );
}

#[tokio::test]
async fn caller_list_and_gas_limit_are_respected() {
    let fixture = RpcFixture::start(
        AccessListReply::Generated,
        EstimateReply::Success,
        SendReply::Accept,
    )
    .await;
    let (core, sender) = signed_core(&fixture).await;
    let caller_list = AccessList(vec![AccessListItem {
        address: Address::repeat_byte(0x81),
        storage_keys: vec![B256::repeat_byte(0xd4)],
    }]);
    let _ = core
        .provider()
        .send_transaction(
            simple_request(Address::repeat_byte(0x82))
                .with_access_list(caller_list.clone())
                .with_gas_limit(60_000),
        )
        .await
        .unwrap();

    let _ = core
        .provider()
        .send_transaction(simple_request(Address::repeat_byte(0x83)).with_gas_limit(70_000))
        .await
        .unwrap();

    let requests = fixture.requests();
    let creates = requests
        .iter()
        .filter(|request| request["method"] == "eth_createAccessList")
        .count();
    assert_eq!(
        creates, 1,
        "a supplied list wins; an absent list is generated once"
    );
    let signed: Vec<_> = requests
        .iter()
        .filter(|request| request["method"] == "eth_sendRawTransaction")
        .map(|request| {
            let raw = request["params"][0].as_str().unwrap();
            let raw = hex::decode(raw.strip_prefix("0x").unwrap()).unwrap();
            let mut raw = raw.as_slice();
            TxEnvelope::decode_2718(&mut raw).unwrap()
        })
        .collect();
    assert_eq!(signed.len(), 2);
    assert_eq!(signed[0].gas_limit(), 60_000);
    assert_eq!(signed[0].access_list(), Some(&caller_list));
    assert_eq!(signed[1].gas_limit(), 70_000);
    assert_eq!(
        signed[1].access_list(),
        Some(&expected_filtered_list(sender, Address::repeat_byte(0x83)))
    );
    assert!(!requests
        .iter()
        .any(|request| request["method"] == "eth_estimateGas"));
}

#[tokio::test]
async fn nonce_retry_builds_once_per_send_attempt() {
    let fixture = RpcFixture::start(
        AccessListReply::Generated,
        EstimateReply::Success,
        SendReply::RejectFirst,
    )
    .await;
    let (core, _) = signed_core(&fixture).await;
    let request = simple_request(Address::repeat_byte(0x91));
    assert!(core
        .provider()
        .send_transaction(request.clone())
        .await
        .is_err());
    let _ = core.provider().send_transaction(request).await.unwrap();

    let methods = fixture.methods();
    let creates = method_positions(&methods, "eth_createAccessList");
    let estimates = method_positions(&methods, "eth_estimateGas");
    let counts = method_positions(&methods, "eth_getTransactionCount");
    let sends = method_positions(&methods, "eth_sendRawTransaction");
    assert_eq!(creates.len(), 2, "one list is built for each send attempt");
    assert_eq!(estimates.len(), 2);
    assert_eq!(sends.len(), 2);
    assert!(creates[0] < estimates[0] && estimates[0] < counts[0] && counts[0] < sends[0]);
    assert!(creates[1] < estimates[1] && estimates[1] < counts[2] && counts[2] < sends[1]);
}
