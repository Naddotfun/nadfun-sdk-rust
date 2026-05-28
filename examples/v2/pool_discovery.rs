//! Discover pools for a list of tokens across both v1 (Capricorn CL) and v2
//! (NadFun) surfaces.

use alloy::primitives::Address;
use alloy::providers::{DynProvider, ProviderBuilder};
use anyhow::Result;
use nadfun_sdk::stream::v2::discover_pools_unified;
use std::sync::Arc;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();
    let network = config.network;

    let provider = ProviderBuilder::new().connect_http(config.rpc_url.parse()?);
    let provider = Arc::new(DynProvider::new(provider));

    let tokens: Vec<Address> = config
        .tokens
        .iter()
        .filter_map(|t| t.parse().ok())
        .collect();
    if tokens.is_empty() {
        anyhow::bail!("--tokens or TOKENS env required");
    }

    let pools = discover_pools_unified(provider, tokens.clone(), network).await?;
    println!(
        "found {} pool(s) across {} token(s)",
        pools.len(),
        tokens.len()
    );
    for p in pools {
        println!(
            "  token={} pool={} surface={:?}",
            p.token, p.pool, p.surface
        );
    }
    Ok(())
}
