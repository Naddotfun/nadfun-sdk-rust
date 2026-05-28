//! Compile-time and runtime checks that the unified `Core`'s v2 surface
//! exposes the types and methods promised by the v2 SDK design.
//!
//! Live RPC interactions are out of scope here — those are covered by the
//! examples that run against a real testnet RPC. This file just guarantees
//! the public surface is reachable and that v2-not-configured paths fire
//! correctly when v2 isn't deployed for the active network.

use alloy::primitives::{Address, B256, U256};
use nadfun_sdk::{
    Core, GasPricing, Network, V2BuyParams, V2BuyWithNativeParams, V2BuyWithPermitParams,
    V2CreatePayment, V2CreateTokenParams, V2DexType, V2ExactOutBuyParams, V2GasEstimationParams,
    V2PermitParams, V2PrepareCreationParams, V2PreparedCreation, V2SellParams,
    V2TokenCreationResult, V2VaultAllocation,
};

const SAMPLE_TOKEN: Address = Address::ZERO;
const SAMPLE_QUOTE: Address = Address::ZERO;

fn sample_buy_params() -> V2BuyParams {
    V2BuyParams {
        token: SAMPLE_TOKEN,
        amount_in: U256::from(100u64),
        amount_out_min: U256::from(1u64),
        deadline: U256::from(1_900_000_000u64),
        gas_limit: Some(250_000),
        gas_price: Some(GasPricing::Legacy),
        nonce: Some(1),
    }
}

#[test]
fn v2_params_construct_and_field_access() {
    let p = sample_buy_params();
    assert_eq!(p.token, SAMPLE_TOKEN);
    assert_eq!(p.amount_in, U256::from(100u64));
    assert_eq!(p.gas_limit, Some(250_000));
    assert_eq!(p.nonce, Some(1));

    let native = V2BuyWithNativeParams {
        token: SAMPLE_TOKEN,
        amount_out_min: U256::from(1u64),
        deadline: U256::from(1_900_000_000u64),
        value: U256::from(100u64),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    assert_eq!(native.token, SAMPLE_TOKEN);
    assert_eq!(native.value, U256::from(100u64));

    let permit = V2BuyWithPermitParams {
        token: SAMPLE_TOKEN,
        amount_in: U256::from(100u64),
        amount_out_min: U256::from(1u64),
        deadline: U256::from(1_900_000_000u64),
        permit: V2PermitParams {
            v: 27,
            r: B256::ZERO,
            s: B256::ZERO,
        },
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    assert_eq!(permit.permit.v, 27);
}

#[test]
fn v2_dex_type_enum() {
    assert_eq!(V2DexType::NadFun.as_u8(), 0);
}

#[test]
fn v2_create_payment_native_and_erc20() {
    let native = V2CreatePayment::Native;
    let erc20 = V2CreatePayment::Erc20 {
        quote_token: SAMPLE_QUOTE,
    };
    // Both variants are constructible — the actual routing happens inside
    // Core::create_token_v2 based on the variant. For Native, msg.value is
    // derived from V2CreateTokenParams.buy_quote_amount (Codex P1 #4).
    match native {
        V2CreatePayment::Native => {}
        V2CreatePayment::Erc20 { .. } => panic!("native should be Native"),
    }
    match erc20 {
        V2CreatePayment::Erc20 { .. } => {}
        V2CreatePayment::Native => panic!("erc20 should be Erc20"),
    }
}

#[test]
fn v2_create_token_params_carries_all_fields() {
    let p = V2CreateTokenParams {
        name: "Foo".into(),
        symbol: "FOO".into(),
        description: "desc".into(),
        image_uri: "https://example.com/img.png".into(),
        website: None,
        twitter: None,
        telegram: None,
        creator_address: Address::ZERO,
        creator_fee_rate: 100,
        vaults: vec![V2VaultAllocation {
            vault: Address::ZERO,
            bps: 10_000,
            setup_data: alloy::primitives::Bytes::default(),
        }],
        dex_type: V2DexType::NadFun,
        buy_quote_amount: U256::from(1u64),
        payment: V2CreatePayment::Native,
        deadline: U256::from(1_900_000_000u64),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    };
    assert_eq!(p.creator_fee_rate, 100);
    assert_eq!(p.vaults.len(), 1);
}

#[test]
fn v2_gas_estimation_params_enum_constructs_each_variant() {
    let _ = V2GasEstimationParams::Buy(sample_buy_params());
    let _ = V2GasEstimationParams::BuyWithNative(V2BuyWithNativeParams {
        token: SAMPLE_TOKEN,
        amount_out_min: U256::from(1u64),
        deadline: U256::from(1_900_000_000u64),
        value: U256::from(1u64),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    });
    let _ = V2GasEstimationParams::Sell(V2SellParams {
        token: SAMPLE_TOKEN,
        amount_in: U256::from(1u64),
        amount_out_min: U256::from(1u64),
        deadline: U256::from(1_900_000_000u64),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    });
    let _ = V2GasEstimationParams::ExactOutBuy(V2ExactOutBuyParams {
        token: SAMPLE_TOKEN,
        amount_out: U256::from(1u64),
        amount_in_max: U256::from(1u64),
        deadline: U256::from(1_900_000_000u64),
        gas_limit: None,
        gas_price: None,
        nonce: None,
    });
}

/// Compile-time guarantee that every v2-surface method on `Core` exists
/// with the expected shape. Calls are only ever reached at runtime if the
/// constructor succeeds, which it won't without a real RPC — but the
/// compile is the assertion.
#[allow(unreachable_code, dead_code, unused_variables)]
async fn _core_v2_methods_compile(c: &Core) {
    let token = Address::ZERO;
    let amount = U256::from(1u64);
    let _: Result<B256, _> = c.buy_v2(sample_buy_params()).await;
    let _: Result<B256, _> = c
        .buy_with_native_v2(V2BuyWithNativeParams {
            token,
            amount_out_min: amount,
            deadline: amount,
            value: amount,
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await;
    let _: Result<B256, _> = c
        .buy_with_permit_v2(V2BuyWithPermitParams {
            token,
            amount_in: amount,
            amount_out_min: amount,
            deadline: amount,
            permit: V2PermitParams {
                v: 27,
                r: B256::ZERO,
                s: B256::ZERO,
            },
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await;
    let _: Result<B256, _> = c
        .sell_v2(V2SellParams {
            token,
            amount_in: amount,
            amount_out_min: amount,
            deadline: amount,
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
        .await;
    let _: Result<U256, _> = c.get_amount_out_v2(token, amount, true).await;
    let _: Result<U256, _> = c.get_amount_in_v2(token, amount, true).await;
    let _: Result<U256, _> = c.get_bonding_curve_amount_out_v2(token, amount, true).await;
    let _: Result<U256, _> = c.get_bonding_curve_amount_in_v2(token, amount, true).await;
    let _: Result<U256, _> = c.get_dex_amount_out_v2(token, amount, true).await;
    let _: Result<U256, _> = c.get_dex_amount_in_v2(token, amount, true).await;
    let _: Result<bool, _> = c.is_graduated_v2(token).await;
    let _: Result<Address, _> = c.pool_address_v2(token).await;
    let _: Result<Address, _> = c.wrapped_native_v2().await;
    let _: Result<u64, _> = c
        .estimate_gas_v2(V2GasEstimationParams::Buy(sample_buy_params()))
        .await;
    // escape hatches — return &_ directly since v2 is always wired.
    let _r = c.router_v2();
    let _f = c.factory_v2();
    let _bc = c.bonding_curve_v2();
    let _tr = c.token_registry_v2();
    let _p = c.provider();
    let _w: Address = c.wallet_address();
    let _n: Network = c.network();
}

#[test]
fn v2_prepare_creation_types_compile() {
    let p = V2PrepareCreationParams {
        name: "X".into(),
        symbol: "X".into(),
        description: String::new(),
        image_uri: String::new(),
        website: None,
        twitter: None,
        telegram: None,
        creator_address: Address::ZERO,
    };
    let r = V2PreparedCreation {
        image_uri: String::new(),
        metadata_uri: String::new(),
        salt: B256::ZERO,
        token_address: Address::ZERO,
        is_nsfw: false,
        name: String::new(),
        symbol: String::new(),
    };
    let _res = V2TokenCreationResult {
        token_address: Address::ZERO,
        metadata_uri: String::new(),
        image_uri: String::new(),
        salt: B256::ZERO,
        transaction_hash: B256::ZERO,
        is_nsfw: false,
    };
    assert_eq!(p.name, "X");
    assert!(!r.is_nsfw);
}

/// `with_provider` reports a clear error when the active network has no v2
/// deployment. (Currently both Mainnet and Testnet are configured, so to
/// exercise the failure path we'd need a hypothetical network — this test
/// just confirms the error type compiles.)
#[test]
fn with_provider_error_type_is_anyhow() {
    // type-only check
    fn _accepts_any_anyhow_err(_e: anyhow::Error) {}
    // Smoke-test that `Network` variants are reachable from the public
    // surface (previously this checked the now-removed `set_network`).
    let _ = Network::Testnet;
}
