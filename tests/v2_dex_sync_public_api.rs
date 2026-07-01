//! Locks the public path of the v2 DEX `Sync` surface — this crate treats its
//! re-export surface as a stability contract, so the paths themselves are the
//! thing under test. Compiling this file proves the re-export chain resolves;
//! the decode assertions prove the public fn behaves.
use alloy::primitives::{address, keccak256, Bytes, LogData, B256, U256};
use alloy::rpc::types::Log;
use alloy::sol_types::SolValue;
use nadfun_sdk::stream::v2::decode_nadfun_sync_event;
use nadfun_sdk::stream::{NadFunSyncEvent, NadFunSyncStream};

#[test]
fn sync_surface_is_public_and_decodes() {
    // Reserves in the data slot; Sync has no indexed fields → topics = [sig].
    let sig: B256 = keccak256("Sync(uint112,uint112)");
    let pair = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let data: Bytes = (U256::from(5u64), U256::from(9u64)).abi_encode_params().into();
    let inner = alloy::primitives::Log {
        address: pair,
        data: LogData::new_unchecked(vec![sig], data),
    };
    let log = Log {
        inner,
        block_hash: None,
        block_number: Some(1),
        block_timestamp: None,
        transaction_hash: Some(B256::ZERO),
        transaction_index: Some(0),
        log_index: Some(0),
        removed: false,
    };

    let ev: NadFunSyncEvent = decode_nadfun_sync_event(log).expect("public decode");
    assert_eq!(ev.pair_address, pair);
    assert_eq!(ev.reserve0, 5);
    assert_eq!(ev.reserve1, 9);

    // NadFunSyncStream must be nameable through the public path (no live
    // connection needed to prove the type is exported).
    fn _assert_stream_is_public(_: Option<NadFunSyncStream>) {}
}
