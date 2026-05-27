//! Subscribe to NadFunPair swap events for a token's pair.

use alloy::primitives::Address;
use anyhow::Result;
use futures_util::{pin_mut, StreamExt};
use nadfun_sdk::{stream::v2::NadFunSwapStream, CoreV2};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    // We need a wallet to construct CoreV2; ws URL needs to also work for
    // RPC (use the same endpoint or set RPC_URL separately).
    let dummy_key =
        "0x0000000000000000000000000000000000000000000000000000000000000001".to_string();

    nadfun_sdk::set_network(config.network);

    // Resolve pair addresses for the requested tokens via CoreV2's
    // TokenRegistry handle. Falls back to direct factory lookup if needed.
    let core = CoreV2::new(config.rpc_url, dummy_key, config.network).await?;
    let tokens: Vec<Address> = config
        .tokens
        .iter()
        .filter_map(|t| t.parse().ok())
        .collect();
    if tokens.is_empty() {
        anyhow::bail!("--tokens or TOKENS env required");
    }

    let mut pairs = Vec::new();
    for token in &tokens {
        let pool = core.pool_address(*token).await?;
        if pool != Address::ZERO {
            pairs.push(pool);
            println!("token {} -> pair {}", token, pool);
        } else {
            eprintln!("token {} not registered on v2 — skipping", token);
        }
    }
    if pairs.is_empty() {
        anyhow::bail!("no v2 pairs found for the requested tokens");
    }

    let stream = NadFunSwapStream::new(config.ws_url, pairs).await?;
    println!("listening for NadFunPair swaps…");
    let s = stream.subscribe().await?;
    pin_mut!(s);

    while let Some(item) = s.next().await {
        match item {
            Ok(swap) => println!(
                "swap pair={} sender={} to={} 0in={} 1in={} 0out={} 1out={} block={}",
                swap.pair_address,
                swap.sender,
                swap.to,
                swap.amount0_in,
                swap.amount1_in,
                swap.amount0_out,
                swap.amount1_out,
                swap.block_number
            ),
            Err(e) => eprintln!("decode error: {}", e),
        }
    }
    Ok(())
}
