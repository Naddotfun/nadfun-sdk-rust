use alloy::primitives::{Address, U256};
use nadfun_sdk::{CoreV2, Network, V2BuyWithNativeParams};

#[test]
fn core_v2_public_api_types_are_available() {
    let token = Address::repeat_byte(0x11);

    let params = V2BuyWithNativeParams {
        token,
        amount_out_min: U256::from(1),
        deadline: U256::from(1_900_000_000u64),
        gas_limit: Some(250_000),
        gas_price: None,
        nonce: Some(7),
    };

    assert_eq!(params.token, token);
    assert_eq!(params.amount_out_min, U256::from(1));
    assert_eq!(params.gas_limit, Some(250_000));
    assert_eq!(params.nonce, Some(7));

    let _network = Network::Testnet;
    let _constructor = CoreV2::new;
}
