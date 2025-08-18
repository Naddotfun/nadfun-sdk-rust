//! DEX historical indexing example
//!
//! Shows how to:
//! 1. Discover pools for tokens automatically
//! 2. Fetch historical DEX swap events from specific pools
//! 3. Use UniswapSwapIndexer for batch processing
//!
//! ## Usage
//!
//! ```bash
//! # Use environment variables
//! export RPC_URL="https://your-rpc-url"
//! export TOKENS="0xToken1,0xToken2"
//! cargo run --example dex_indexer
//!
//! # Or use CLI arguments
//! cargo run --example dex_indexer -- --rpc-url https://your-rpc-url --tokens 0xToken1,0xToken2
//! ```

use alloy::providers::{Provider, ProviderBuilder};
use anyhow::Result;
use nadfun_sdk::stream::UniswapSwapIndexer;
use std::sync::Arc;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    println!("💱 DEX Historical Indexing");

    // Get current block for dynamic range
    let provider = Arc::new(ProviderBuilder::new().connect_http(config.rpc_url.parse()?));
    let current_block = provider.get_block_number().await?;

    // Tokens we want to monitor DEX activity for
    let tokens: Vec<alloy::primitives::Address> = if !config.tokens.is_empty() {
        config
            .tokens
            .iter()
            .filter_map(|addr| addr.parse().ok())
            .collect()
    } else {
        // Use example tokens if none provided
        vec![
            "0x25eD53F8A8Ead909F82e6F41F0480BE8ceF24589".parse()?,
            "0xAfE99b588f3A3bA093AeA65D995B3eD97c45Af82".parse()?,
        ]
    };

    println!("Monitoring {} tokens", tokens.len());

    // Discover pools for the specified tokens
    let indexer =
        UniswapSwapIndexer::discover_pools_for_tokens(config.rpc_url.clone(), tokens.clone())
            .await?;

    println!("Pool addresses discovered and monitored:");
    for (i, pool) in indexer.pool_addresses().iter().enumerate() {
        println!("  {}. {}", i + 1, pool);
    }

    // Fetch recent swap events from discovered pools
    let events = indexer
        .fetch_events(current_block - 100, current_block)
        .await?;

    println!("\n✅ Found {} swap events", events.len());

    // Display first few events
    for event in events.iter().take(10) {
        println!(
            "💱 Swap | Pool: {} | Block: {}",
            event.pool_address, event.block_number
        );
        println!("   Amount0: {} | Amount1: {}", event.amount0, event.amount1);
    }

    if events.is_empty() {
        println!("💡 No swap events found in recent blocks for the discovered pools");
        println!("   Try with different tokens or check if pools have recent activity");
    }

    // Test fetch_all_events with automatic batching
    println!("\n🔄 Testing fetch_all_events with automatic batching...");
    let batch_size = 20; // 20 blocks per batch
    let start_block = current_block - 5000;

    let all_events = indexer.fetch_all_events(start_block, batch_size).await?;

    println!("📦 Batch processing results:");
    println!("   Start block: {}", start_block);
    println!("   End block: {}", current_block);
    println!("   Batch size: {} blocks", batch_size);
    println!("   Total events found: {}", all_events.len());

    if !all_events.is_empty() {
        println!("\n📊 Sample events from batch processing:");
        for event in all_events.iter().take(3) {
            println!(
                "💱 Batch Event | Pool: {} | Block: {}",
                event.pool_address, event.block_number
            );
        }
    }

    Ok(())
}
