//! Bonding curve historical indexing example
//!
//! Shows how to:
//! 1. Create curve indexer with HTTP provider
//! 2. Fetch bonding curve events from specific block ranges  
//! 3. Filter by event types (Create, Buy, Sell) and tokens
//! 4. Handle large data sets with batching
//!
//! ## Usage
//!
//! ```bash
//! # Use environment variables
//! export RPC_URL="https://your-rpc-url"
//! cargo run --example curve_indexer
//!
//! # Or use CLI arguments
//! cargo run --example curve_indexer -- --rpc-url https://your-rpc-url
//! ```

use alloy::providers::{Provider, ProviderBuilder};
use anyhow::Result;
use nadfun_sdk::stream::{CurveIndexer, EventType};
use std::sync::Arc;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    println!("📈 Historical Event Fetching");

    // 1. Create HTTP provider and indexer
    let provider = Arc::new(ProviderBuilder::new().connect_http(config.rpc_url.parse()?));
    let indexer = CurveIndexer::new(provider.clone());

    // 2. Fetch events from specific block range (wider range to find events)
    let current_block = provider.get_block_number().await?; // Get recent blocks
    let events = indexer
        .fetch_events(
            current_block - 100, // from_block (wider range)
            current_block,       // to_block
            vec![EventType::Create, EventType::Buy, EventType::Sell], // event types
            None,                // token_filter (None = all tokens)
        )
        .await?;

    println!("✅ Found {} events", events.len());

    // 4. Filter by specific tokens if provided
    if !config.tokens.is_empty() {
        let specific_tokens: Vec<alloy::primitives::Address> = config
            .tokens
            .iter()
            .filter_map(|addr| addr.parse().ok())
            .collect();

        let filtered_events = indexer
            .fetch_events(
                current_block - 100, // Wider range for filtering too
                current_block,
                vec![EventType::Buy, EventType::Sell],
                Some(specific_tokens.clone()), // Only these tokens
            )
            .await?;

        println!(
            "\n🎯 Filtered events for specific tokens: {}",
            filtered_events.len()
        );
        println!("Target tokens: {:?}", specific_tokens);

        for event in filtered_events.iter().take(10) {
            println!(
                "🎯 {:?} | Token: {} | Block: {}",
                event.event_type(),
                event.token(),
                event.block_number()
            );
        }
    } else {
        println!("\n🎯 No specific tokens provided for filtering");
    }

    // 5. Test fetch_all_events with automatic batching
    println!("\n🔄 Testing fetch_all_events with automatic batching...");
    let batch_size = 100; // 20 blocks per batch
    let start_block = current_block - 1000;

    let all_events = indexer
        .fetch_all_events(
            start_block,
            batch_size,
            vec![EventType::Create, EventType::Buy, EventType::Sell],
            None, // No token filter for this test
        )
        .await?;

    println!("📦 Batch processing results:");
    println!("   Start block: {}", start_block);
    println!("   End block: {}", current_block);
    println!("   Batch size: {} blocks", batch_size);
    println!("   Total events found: {}", all_events.len());

    if !all_events.is_empty() {
        println!("\n📊 Events from batch processing (showing first 10):");
        for event in all_events.iter().take(10) {
            println!(
                "🔄 {:?} | Token: {} | Block: {}",
                event.event_type(),
                event.token(),
                event.block_number()
            );
        }

        // Show unique tokens found in batch processing
        let mut batch_unique_tokens = std::collections::HashSet::new();
        for event in all_events.iter() {
            batch_unique_tokens.insert(event.token());
        }

        println!("\n📊 Unique tokens found in batch processing:");
        for token in batch_unique_tokens.iter() {
            println!("  {}", token);
        }

        // Event type breakdown
        let mut create_count = 0;
        let mut buy_count = 0;
        let mut sell_count = 0;

        for event in all_events.iter() {
            match event.event_type() {
                EventType::Create => create_count += 1,
                EventType::Buy => buy_count += 1,
                EventType::Sell => sell_count += 1,
                EventType::Sync | EventType::Lock | EventType::Listed => {
                    // Other event types - not counted in main breakdown
                }
            }
        }

        println!("\n📈 Event type breakdown:");
        println!("  Create: {} events", create_count);
        println!("  Buy: {} events", buy_count);
        println!("  Sell: {} events", sell_count);
    }

    println!("\n📦 Historical data example completed successfully!");

    Ok(())
}
