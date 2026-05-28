//! Smoke tests for v2 event signatures and signature-based dispatch.
//!
//! Live RPC and on-chain log inspection are out of scope here — the
//! integration tests in `examples/v2/*` cover that against a real testnet.
//! These checks just verify that the SDK's event-type discriminator agrees
//! with the alloy-generated SIGNATURE_HASH, so any caller code that does
//! topic[0]-based dispatch lines up with what the contract actually emits.

use alloy::primitives::{address, Address, Bytes, LogData, B256, U256};
use alloy::sol_types::{SolEvent, SolValue};
use nadfun_sdk::stream::v2::dex::nadfun_swap_signature;
use nadfun_sdk::types::v2::events::{decode_v2_bonding_curve_event, IBondingCurveV2Events};
use nadfun_sdk::{V2BondingCurveEvent, V2BuyEvent, V2EventType};

#[test]
fn v2_event_type_signatures_are_distinct() {
    let all = V2EventType::all();
    let mut seen = std::collections::HashSet::new();
    for et in &all {
        let sig = et.signature();
        assert!(seen.insert(sig), "duplicate signature for {:?}", et);
    }
    assert_eq!(all.len(), 6);
}

#[test]
fn nadfun_swap_signature_is_distinct_from_bonding_curve_events() {
    let swap_sig = nadfun_swap_signature();
    for et in V2EventType::all() {
        assert_ne!(
            swap_sig,
            et.signature(),
            "NadFunPair::Swap signature collides with v2 BondingCurve::{:?}",
            et
        );
    }
}

#[test]
fn v2_bonding_curve_event_enum_helpers() {
    let buy = V2BondingCurveEvent::Buy(V2BuyEvent {
        token: alloy::primitives::Address::ZERO,
        buyer: alloy::primitives::Address::ZERO,
        quote_in: alloy::primitives::U256::from(1u64),
        token_out: alloy::primitives::U256::from(2u64),
        block_number: 42,
        transaction_hash: alloy::primitives::B256::ZERO,
        transaction_index: 0,
        log_index: 7,
    });
    assert_eq!(buy.event_type(), V2EventType::Buy);
    assert_eq!(buy.block_number(), 42);
    assert_eq!(buy.log_index(), 7);
}

/// Helper: pad a 20-byte address into a 32-byte topic value.
fn addr_topic(addr: Address) -> B256 {
    let mut t = [0u8; 32];
    t[12..].copy_from_slice(addr.as_slice());
    B256::from(t)
}

/// Build a synthetic alloy RPC log for `Event` from indexed topics + data.
fn rpc_log<E: SolEvent>(
    pair_address: Address,
    topics: Vec<B256>,
    data: Bytes,
) -> alloy::rpc::types::Log {
    let log_data = LogData::new_unchecked(topics, data);
    let primitive: alloy::primitives::Log = alloy::primitives::Log {
        address: pair_address,
        data: log_data,
    };
    let _ = std::marker::PhantomData::<E>; // type only — guarantees a SolEvent constraint
    alloy::rpc::types::Log {
        inner: primitive,
        block_hash: None,
        block_number: Some(1),
        block_timestamp: None,
        transaction_hash: Some(B256::ZERO),
        transaction_index: Some(0),
        log_index: Some(0),
        removed: false,
    }
}

/// Codex P1 #1: `decode_log` reads indexed fields (token / creator / pair)
/// from topics. The pre-fix `decode_log_data(log.data())` would have
/// silently zeroed all three.
#[test]
fn create_event_decodes_indexed_fields_from_topics() {
    let creator = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let token = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let pair = address!("cccccccccccccccccccccccccccccccccccccccc");
    let quote_token = address!("dddddddddddddddddddddddddddddddddddddddd");
    let virtual_q = U256::from(1_000u64);
    let virtual_t = U256::from(2_000u64);
    let min_t = U256::from(50u64);

    // Non-indexed data: (quoteToken: address, name: string, symbol: string,
    // tokenURI: string, virtualQuoteReserve: U256, virtualTokenReserve: U256,
    // minTokenReserve: U256). The exact order matters — must match the ABI.
    let data: Bytes = (
        quote_token,
        "TestName".to_string(),
        "TST".to_string(),
        "ipfs://meta".to_string(),
        virtual_q,
        virtual_t,
        min_t,
    )
        .abi_encode_params()
        .into();

    let topics = vec![
        IBondingCurveV2Events::Create::SIGNATURE_HASH,
        addr_topic(creator),
        addr_topic(token),
        addr_topic(pair),
    ];

    let log = rpc_log::<IBondingCurveV2Events::Create>(
        address!("1111111111111111111111111111111111111111"),
        topics,
        data,
    );

    let evt = decode_v2_bonding_curve_event(log).expect("decode");
    let create = match evt {
        V2BondingCurveEvent::Create(c) => c,
        other => panic!("expected Create, got {:?}", other),
    };

    // Indexed — previously zeroed; now correct.
    assert_eq!(create.creator, creator, "creator from topics");
    assert_eq!(create.token, token, "token from topics");
    assert_eq!(create.pair, pair, "pair from topics");
    // Non-indexed — already worked, but verify nothing regressed.
    assert_eq!(create.quote_token, quote_token);
    assert_eq!(create.name, "TestName");
    assert_eq!(create.symbol, "TST");
    assert_eq!(create.token_uri, "ipfs://meta");
    assert_eq!(create.virtual_quote_reserve, virtual_q);
    assert_eq!(create.virtual_token_reserve, virtual_t);
    assert_eq!(create.min_token_reserve, min_t);
}
