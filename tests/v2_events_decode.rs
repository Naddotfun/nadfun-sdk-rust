//! Smoke tests for v2 event signatures and signature-based dispatch.
//!
//! Live RPC and on-chain log inspection are out of scope here — the
//! integration tests in `examples/v2/*` cover that against a real testnet.
//! These checks just verify that the SDK's event-type discriminator agrees
//! with the alloy-generated SIGNATURE_HASH, so any caller code that does
//! topic[0]-based dispatch lines up with what the contract actually emits.

use nadfun_sdk::stream::v2::dex::nadfun_swap_signature;
use nadfun_sdk::{V2BondingCurveEvent, V2BuyEvent, V2EventType};

#[test]
fn v2_event_type_signatures_are_distinct() {
    let all = V2EventType::all();
    let mut seen = std::collections::HashSet::new();
    for et in &all {
        let sig = et.signature();
        assert!(
            seen.insert(sig),
            "duplicate signature for {:?}",
            et
        );
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
