//! DEX real-time streaming example
//!
//! Demonstrates 3 different DEX streaming scenarios:
//! 1. Monitor specific pool addresses directly
//! 2. Auto-discover pools for specific tokens
//! 3. Monitor all swap events (using discovered pools)
//!
//! ## Usage
//!
//! ```bash
//! # Scenario 1: Monitor specific pool addresses
//! cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint --pools 0xPool1,0xPool2
//!
//! # Scenario 2: Auto-discover pools for tokens
//! cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint --tokens 0xToken1,0xToken2
//!
//! # Scenario 3: Monitor single token's pool
//! cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint --token 0xTokenAddress
//! ```

use anyhow::Result;
use futures_util::{pin_mut, StreamExt};
use nadfun_sdk::stream::UniswapSwapStream;
use nadfun_sdk::types::SwapEvent;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    // Parse command line arguments for different scenarios
    let mut pools_filter: Option<Vec<alloy::primitives::Address>> = None;
    let mut tokens_filter: Option<Vec<alloy::primitives::Address>> = None;
    let single_token = config.token.clone();

    // Parse pools if provided via environment variable
    if let Ok(pools_env) = std::env::var("POOLS") {
        pools_filter = Some(parse_addresses(&pools_env)?);
    }

    // Parse tokens from config
    if !config.tokens.is_empty() {
        tokens_filter = Some(
            config
                .tokens
                .iter()
                .filter_map(|addr| addr.parse().ok())
                .collect(),
        );
    }

    // Determine scenario based on arguments
    match (&pools_filter, &tokens_filter, &single_token) {
        (Some(pools), _, _) => {
            println!("🎯 SCENARIO 1: Monitoring specific pool addresses: {} pools", pools.len());
            run_specific_pools_scenario(&config.ws_url, pools.clone()).await?;
        }
        (None, Some(tokens), _) => {
            println!("🔍 SCENARIO 2: Auto-discovering pools for {} tokens", tokens.len());
            run_token_discovery_scenario(&config.ws_url, tokens.clone()).await?;
        }
        (None, None, Some(token)) => {
            if let Ok(token_address) = token.parse() {
                println!("🏷️ SCENARIO 3: Single token pool discovery");
                run_single_token_scenario(&config.ws_url, token_address).await?;
            } else {
                println!("❌ Invalid token address provided");
                return Ok(());
            }
        }
        _ => {
            println!("❌ Please provide either:");
            println!("   - Pool addresses: POOLS=0xPool1,0xPool2 cargo run --example dex_stream");
            println!("   - Token addresses: cargo run --example dex_stream -- --tokens 0xToken1,0xToken2");
            println!("   - Single token: cargo run --example dex_stream -- --token 0xTokenAddress");
            return Ok(());
        }
    }

    Ok(())
}

/// Scenario 1: Monitor specific pool addresses directly
async fn run_specific_pools_scenario(ws_url: &str, pool_addresses: Vec<alloy::primitives::Address>) -> Result<()> {
    println!("📡 Creating UniswapSwapStream for specific pools...");
    
    for (i, pool) in pool_addresses.iter().enumerate() {
        println!("   {}. Pool: {}", i + 1, pool);
    }
    
    let swap_stream = UniswapSwapStream::new(ws_url.to_string(), pool_addresses).await?;
    let stream = swap_stream.subscribe().await?;
    pin_mut!(stream);

    println!("🔴 Listening for DEX swap events in specified pools...");

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                handle_swap_event(&event, "SPECIFIC_POOLS");
            }
            Err(e) => {
                println!("⚠️ Error processing swap event: {}", e);
            }
        }
    }

    Ok(())
}

/// Scenario 2: Auto-discover pools for specific tokens
async fn run_token_discovery_scenario(ws_url: &str, token_addresses: Vec<alloy::primitives::Address>) -> Result<()> {
    println!("📡 Auto-discovering pools for tokens...");
    
    for (i, token) in token_addresses.iter().enumerate() {
        println!("   {}. Token: {}", i + 1, token);
    }
    
    let swap_stream = UniswapSwapStream::discover_pools_for_tokens(ws_url.to_string(), token_addresses).await?;
    let stream = swap_stream.subscribe().await?;
    pin_mut!(stream);

    println!("🔍 Listening for DEX swap events in discovered pools...");

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                handle_swap_event(&event, "TOKEN_DISCOVERY");
            }
            Err(e) => {
                println!("⚠️ Error processing swap event: {}", e);
            }
        }
    }

    Ok(())
}

/// Scenario 3: Single token pool discovery
async fn run_single_token_scenario(ws_url: &str, token_address: alloy::primitives::Address) -> Result<()> {
    println!("📡 Discovering pool for single token...");
    println!("   Token: {}", token_address);
    
    let swap_stream = UniswapSwapStream::discover_pool_for_token(ws_url.to_string(), token_address).await?;
    let stream = swap_stream.subscribe().await?;
    pin_mut!(stream);

    println!("🏷️ Listening for DEX swap events in discovered pool...");

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                handle_swap_event(&event, "SINGLE_TOKEN");
            }
            Err(e) => {
                println!("⚠️ Error processing swap event: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_swap_event(event: &SwapEvent, scenario: &str) {
    println!(
        "💱 [{}] Swap in pool {} | Block: {} | TxIndex: {}",
        scenario,
        event.pool_address,
        event.block_number,
        event.transaction_index
    );
    
    println!(
        "   💰 Amount0: {} | Amount1: {}",
        event.amount0, event.amount1
    );
    
    println!(
        "   👤 Sender: {} | Recipient: {}",
        event.sender, event.recipient
    );
    
    println!(
        "   📊 Liquidity: {} | Tick: {} | Price: {}",
        event.liquidity, event.tick, event.sqrt_price_x96
    );
    
    println!("   ─────────────────────────────────────");
}

fn parse_addresses(addrs_str: &str) -> Result<Vec<alloy::primitives::Address>> {
    addrs_str
        .split(',')
        .map(|s| s.trim().parse::<alloy::primitives::Address>().map_err(|e| anyhow::anyhow!("Invalid address {}: {}", s, e)))
        .collect()
}