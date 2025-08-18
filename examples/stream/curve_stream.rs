//! Bonding curve real-time streaming example
//!
//! Demonstrates 3 different streaming scenarios:
//! 1. All bonding curve events (all event types, all tokens)
//! 2. Specific event types only
//! 3. Specific tokens only
//!
//! ## Usage
//!
//! ```bash
//! # Scenario 1: All events
//! cargo run --example curve_stream -- --ws-url wss://your-ws-url
//!
//! # Scenario 2: Specific events only (Buy/Sell)
//! cargo run --example curve_stream -- --ws-url wss://your-ws-url --events Buy,Sell
//!
//! # Scenario 3: Specific tokens only
//! cargo run --example curve_stream -- --ws-url wss://your-ws-url --tokens 0xToken1,0xToken2
//!
//! # Combined: Specific events AND tokens
//! cargo run --example curve_stream -- --ws-url wss://your-ws-url --events Buy,Sell --tokens 0xToken1
//! ```

use anyhow::Result;
use futures_util::{pin_mut, StreamExt};
use nadfun_sdk::stream::CurveStream;
use nadfun_sdk::types::{BondingCurveEvent, EventType};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    // Parse command line arguments for filtering
    let mut event_filter: Option<Vec<EventType>> = None;
    let mut token_filter: Option<Vec<alloy::primitives::Address>> = None;

    // Parse events if provided
    if let Ok(events_env) = std::env::var("EVENTS") {
        event_filter = Some(parse_event_types(&events_env)?);
    }

    // Parse tokens if provided  
    if !config.tokens.is_empty() {
        token_filter = Some(
            config
                .tokens
                .iter()
                .filter_map(|addr| addr.parse().ok())
                .collect(),
        );
    }

    // Determine scenario
    match (&event_filter, &token_filter) {
        (None, None) => {
            println!("🌟 SCENARIO 1: All bonding curve events (all types, all tokens)");
            run_all_events_scenario(&config.ws_url).await?;
        }
        (Some(events), None) => {
            println!("🎯 SCENARIO 2: Specific event types only: {:?}", events);
            run_specific_events_scenario(&config.ws_url, events.clone()).await?;
        }
        (None, Some(tokens)) => {
            println!("🏷️ SCENARIO 3: Specific tokens only: {} tokens", tokens.len());
            run_specific_tokens_scenario(&config.ws_url, tokens.clone()).await?;
        }
        (Some(events), Some(tokens)) => {
            println!(
                "🎯🏷️ COMBINED: Specific events {:?} AND {} tokens",
                events,
                tokens.len()
            );
            run_combined_scenario(&config.ws_url, events.clone(), tokens.clone()).await?;
        }
    }

    Ok(())
}

/// Scenario 1: All bonding curve events
async fn run_all_events_scenario(ws_url: &str) -> Result<()> {
    println!("📡 Creating CurveStream for all events...");
    
    let curve_stream = CurveStream::new(ws_url.to_string()).await?;
    let stream = curve_stream.subscribe().await?;
    pin_mut!(stream);

    println!("🔴 Listening for ALL bonding curve events...");

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                handle_event(&event, "ALL");
            }
            Err(e) => {
                println!("⚠️ Error processing event: {}", e);
            }
        }
    }

    Ok(())
}

/// Scenario 2: Specific event types only
async fn run_specific_events_scenario(ws_url: &str, event_types: Vec<EventType>) -> Result<()> {
    println!("📡 Creating CurveStream for specific events...");
    
    let curve_stream = CurveStream::new(ws_url.to_string())
        .await?
        .subscribe_events(event_types.clone());
    
    let stream = curve_stream.subscribe().await?;
    pin_mut!(stream);

    println!("🎯 Listening for specific events: {:?}", event_types);

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                handle_event(&event, "FILTERED_EVENTS");
            }
            Err(e) => {
                println!("⚠️ Error processing event: {}", e);
            }
        }
    }

    Ok(())
}

/// Scenario 3: Specific tokens only
async fn run_specific_tokens_scenario(
    ws_url: &str,
    monitored_tokens: Vec<alloy::primitives::Address>,
) -> Result<()> {
    println!("📡 Creating CurveStream for specific tokens...");
    
    let curve_stream = CurveStream::new(ws_url.to_string())
        .await?
        .filter_tokens(monitored_tokens.clone());
    
    let stream = curve_stream.subscribe().await?;
    pin_mut!(stream);

    println!("🏷️ Listening for {} specific tokens", monitored_tokens.len());
    for (i, token) in monitored_tokens.iter().enumerate() {
        println!("   {}. {}", i + 1, token);
    }

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                handle_event(&event, "FILTERED_TOKENS");
            }
            Err(e) => {
                println!("⚠️ Error processing event: {}", e);
            }
        }
    }

    Ok(())
}

/// Combined scenario: Specific events AND tokens
async fn run_combined_scenario(
    ws_url: &str,
    event_types: Vec<EventType>,
    monitored_tokens: Vec<alloy::primitives::Address>,
) -> Result<()> {
    println!("📡 Creating CurveStream for specific events AND tokens...");
    
    let curve_stream = CurveStream::new(ws_url.to_string())
        .await?
        .subscribe_events(event_types.clone())
        .filter_tokens(monitored_tokens.clone());
    
    let stream = curve_stream.subscribe().await?;
    pin_mut!(stream);

    println!("🎯🏷️ Listening for {:?} events on {} tokens", event_types, monitored_tokens.len());

    while let Some(event_result) = stream.next().await {
        match event_result {
            Ok(event) => {
                handle_event(&event, "COMBINED_FILTER");
            }
            Err(e) => {
                println!("⚠️ Error processing event: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_event(event: &BondingCurveEvent, scenario: &str) {
    println!(
        "🎉 [{}] {:?} event for token {} | Block: {} | TxIndex: {}",
        scenario,
        event.event_type(),
        event.token(),
        event.block_number(),
        event.transaction_index()
    );

    match event.event_type() {
        EventType::Create => {
            println!("   ✨ New token created!");
        }
        EventType::Buy => {
            println!("   💰 Buy transaction detected");
        }
        EventType::Sell => {
            println!("   💸 Sell transaction detected");
        }
        EventType::Sync => {
            println!("   🔄 Sync event");
        }
        EventType::Lock => {
            println!("   🔒 Lock event");
        }
        EventType::Listed => {
            println!("   🚀 Token listed on DEX!");
        }
    }
}

fn parse_event_types(events_str: &str) -> Result<Vec<EventType>> {
    events_str
        .split(',')
        .map(|s| match s.trim() {
            "Create" => Ok(EventType::Create),
            "Buy" => Ok(EventType::Buy),
            "Sell" => Ok(EventType::Sell),
            "Sync" => Ok(EventType::Sync),
            "Lock" => Ok(EventType::Lock),
            "Listed" => Ok(EventType::Listed),
            _ => Err(anyhow::anyhow!("Unknown event type: {}", s)),
        })
        .collect()
}