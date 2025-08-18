//! Pool discovery example
//!
//! Shows how to:
//! 1. Find Uniswap V3 pool addresses for tokens using UniswapSwapIndexer
//! 2. Auto-discover pools paired with WMON
//!
//! ## Usage
//!
//! ```bash
//! # Using environment variables
//! export RPC_URL="https://your-rpc-url"
//! export TOKENS="0xToken1,0xToken2"  # Multiple tokens
//! export TOKEN="0xTokenAddress"      # Single token
//! cargo run --example pool_discovery
//!
//! # Using command line arguments
//! cargo run --example pool_discovery -- --rpc-url https://your-rpc-url --tokens 0xToken1,0xToken2
//! cargo run --example pool_discovery -- --rpc-url https://your-rpc-url --token 0xTokenAddress
//! ```

use alloy::primitives::Address;
use anyhow::Result;
use nadfun_sdk::stream::UniswapSwapIndexer;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    // Token addresses - can be provided via CLI or use examples
    let tokens: Vec<alloy::primitives::Address> = if !config.tokens.is_empty() {
        config.tokens.iter()
            .filter_map(|addr| addr.parse().ok())
            .collect()
    } else if let Some(token_address) = config.token {
        vec![token_address.parse()?]
    } else {
        // Use example token addresses if none provided
        vec![
            "0x1234567890123456789012345678901234567890".parse()?,
            "0x2345678901234567890123456789012345678901".parse()?,
            "0x3456789012345678901234567890123456789012".parse()?,
        ]
    };

    println!("🔍 Pool Discovery Example");
    println!("Tokens to search: {}", tokens.len());

    // Method 1: Discover all pools at once
    discover_all_pools(&config.rpc_url, &tokens).await?;

    // Method 2: Discover pools one by one
    discover_individual_pools(&config.rpc_url, &tokens).await?;

    Ok(())
}

async fn discover_all_pools(rpc_url: &str, tokens: &[Address]) -> Result<()> {
    println!("\n📦 Auto Pool Discovery");

    // UniswapSwapIndexer automatically finds pools for tokens
    let indexer =
        UniswapSwapIndexer::discover_pools_for_tokens(rpc_url.to_string(), tokens.to_vec()).await?;

    println!("Found pools:");
    for (i, pool) in indexer.pool_addresses().iter().enumerate() {
        println!("  {}. Pool: {}", i + 1, pool);
    }

    Ok(())
}

async fn discover_individual_pools(rpc_url: &str, tokens: &[Address]) -> Result<()> {
    println!("\n🔧 Individual Pool Discovery");

    // Create indexer for each token individually
    for (i, &token) in tokens.iter().enumerate() {
        match UniswapSwapIndexer::discover_pool_for_token(rpc_url.to_string(), token).await {
            Ok(indexer) => {
                let pools = indexer.pool_addresses();
                if pools.is_empty() {
                    println!("  {}. Token {} -> No pool found", i + 1, token);
                } else {
                    println!("  {}. Token {} -> Pool {}", i + 1, token, pools[0]);
                }
            }
            Err(_) => {
                println!("  {}. Token {} -> Error finding pool", i + 1, token);
            }
        }
    }

    Ok(())
}
