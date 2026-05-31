//! v2 read-only smoke test — verifies every wired v2 contract responds on
//! the active network. No private key, no tx; only `eth_call`.
//!
//! Usage:
//!   cargo run --example v2_smoke -- --rpc-url https://dev-node.nadapp.net/ --network testnet

use alloy::primitives::Address;
use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use anyhow::Result;
use nadfun_sdk::{
    constants::{
        get_burn_vault_v2, get_creator_fee_vault_v2, get_fee_to_v2, get_gift_vault_v2,
        get_lp_vault_v2, get_nad_swap_adapter_v2, get_nadfun_factory_v2, get_nadfun_pair_impl_v2,
        get_nadfun_router_v2, get_protocol_manager_v2, get_token_impl_v2, get_token_registry_v2,
    },
    Core, Network,
};
use std::sync::Arc;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();
    let net: Network = config.network;

    println!("\n=== Static address inventory ===");
    macro_rules! list {
        ($($name:literal => $getter:expr,)+) => {{
            $(
                let v = $getter;
                println!("  {:<22} {}", $name, v.unwrap_or("(none)"));
            )+
        }};
    }
    list! {
        "nadfun_router"        => get_nadfun_router_v2(net),
        "nadfun_factory"       => get_nadfun_factory_v2(net),
        "nadfun_pair_impl"     => get_nadfun_pair_impl_v2(net),
        "nad_swap_adapter"     => get_nad_swap_adapter_v2(net),
        "token_registry"       => get_token_registry_v2(net),
        "token_impl"           => get_token_impl_v2(net),
        "protocol_manager"     => get_protocol_manager_v2(net),
        "burn_vault"           => get_burn_vault_v2(net),
        "lp_vault"             => get_lp_vault_v2(net),
        "creator_fee_vault"    => get_creator_fee_vault_v2(net),
        "gift_vault"           => get_gift_vault_v2(net),
        "fee_to"               => get_fee_to_v2(net),
    };

    println!("\n=== Live RPC view-method probe ===");
    let provider = Arc::new(DynProvider::new(
        ProviderBuilder::new().connect_http(config.rpc_url.parse()?),
    ));
    println!(
        "  block_number           {}",
        provider.get_block_number().await?
    );

    // Read-only Core — wallet_address is ignored for view calls.
    let core = Core::with_provider(provider.clone(), Address::ZERO, config.network)?;
    let core_v2 = core.v2();
    let registry = core_v2.token_registry();

    match core.v2().wrapped_native().await {
        Ok(w) => println!("  router.wrappedNative   {}", w),
        Err(e) => println!("  router.wrappedNative   ERR: {}", e),
    }

    match core.v2().factory().all_pairs_length().await {
        Ok(n) => println!("  factory.allPairs       {} pair(s)", n),
        Err(e) => println!("  factory.allPairs       ERR: {}", e),
    }

    match core.v2().factory().implementation().await {
        Ok(i) => println!("  factory.impl           {}", i),
        Err(e) => println!("  factory.impl           ERR: {}", e),
    }

    match core.v2().factory().fee_collector().await {
        Ok(c) => println!("  factory.feeCollector   {}", c),
        Err(e) => println!("  factory.feeCollector   ERR: {}", e),
    }

    match core.v2().bonding_curve().is_halted().await {
        Ok(h) => println!("  bondingCurve.isHalted  {}", h),
        Err(e) => println!("  bondingCurve.isHalted  ERR: {}", e),
    }

    match core.v2().bonding_curve().version().await {
        Ok(v) => println!("  bondingCurve.VERSION   {}", v),
        Err(e) => println!("  bondingCurve.VERSION   ERR: {}", e),
    }

    // Should be false but call must succeed regardless.
    match registry.is_registered(Address::ZERO).await {
        Ok(r) => println!("  registry.isReg(0x0)    {}", r),
        Err(e) => println!("  registry.isReg(0x0)    ERR: {}", e),
    }

    // Optional per-token probes if caller passes --token or --tokens.
    let probe: Vec<Address> = config
        .token
        .iter()
        .chain(config.tokens.iter())
        .filter_map(|s| s.parse().ok())
        .collect();
    if !probe.is_empty() {
        println!("\n=== Per-token registry probe ===");
        for token in probe {
            let reg = registry.is_registered(token).await.unwrap_or(false);
            let pair = registry.get_pair(token).await.unwrap_or(Address::ZERO);
            println!("  {} registered={} pair={}", token, reg, pair);
        }
    }

    println!("\n✅ smoke complete");
    Ok(())
}
