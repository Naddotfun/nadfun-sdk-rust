//! Smoke test: two SDK clients can coexist in one process pointing at
//! different networks — previously impossible due to the global `set_network`
//! lock.
//!
//! No RPC, no API calls — just construction + accessor checks.

use nadfun_sdk::{ApiClient, Network};

#[test]
fn api_clients_on_different_networks_coexist() {
    let mainnet = ApiClient::new(Network::Mainnet);
    let testnet = ApiClient::new(Network::Testnet);
    assert_eq!(mainnet.network(), Network::Mainnet);
    assert_eq!(testnet.network(), Network::Testnet);
}

#[test]
fn constants_lookups_use_passed_network_not_global() {
    // The two networks must resolve to different addresses for at least the
    // API server URL — proves the helper reads the argument, not a global.
    assert_ne!(
        nadfun_sdk::constants::get_api_server_url(Network::Mainnet),
        nadfun_sdk::constants::get_api_server_url(Network::Testnet),
    );
    assert_ne!(
        nadfun_sdk::constants::get_wmon(Network::Mainnet),
        nadfun_sdk::constants::get_wmon(Network::Testnet),
    );
}

#[test]
fn v2_helpers_return_correct_network() {
    // v2 is deployed on both mainnet and testnet; addresses should differ.
    // Getters return `&str` directly (no `Option`) — v2 is always present.
    let mn = nadfun_sdk::constants::get_nadfun_router_v2(Network::Mainnet);
    let tn = nadfun_sdk::constants::get_nadfun_router_v2(Network::Testnet);
    assert_ne!(mn, tn);
}
