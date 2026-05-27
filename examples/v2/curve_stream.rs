//! Subscribe to v2 BondingCurve events in real time.

use alloy::primitives::Address;
use anyhow::Result;
use futures_util::{pin_mut, StreamExt};
use nadfun_sdk::stream::v2::CurveStreamV2;
use nadfun_sdk::V2EventType;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    nadfun_sdk::set_network(config.network);

    let mut stream = CurveStreamV2::new(config.ws_url.clone()).await?;

    // Allow event-type filtering via EVENTS env var (Buy,Sell,Sync,...).
    if let Ok(events_env) = std::env::var("EVENTS") {
        let parsed: Vec<V2EventType> = events_env
            .split(',')
            .filter_map(|s| match s.trim() {
                "Create" => Some(V2EventType::Create),
                "Buy" => Some(V2EventType::Buy),
                "Sell" => Some(V2EventType::Sell),
                "Sync" => Some(V2EventType::Sync),
                "Graduate" => Some(V2EventType::Graduate),
                "SnipingPenalty" => Some(V2EventType::SnipingPenalty),
                _ => None,
            })
            .collect();
        if !parsed.is_empty() {
            stream = stream.subscribe_events(parsed);
        }
    }

    // Optional client-side token filter.
    if !config.tokens.is_empty() {
        let toks: Vec<Address> = config
            .tokens
            .iter()
            .filter_map(|t| t.parse().ok())
            .collect();
        stream = stream.filter_tokens(toks);
    }

    println!("listening for v2 BondingCurve events…");
    let s = stream.subscribe().await?;
    pin_mut!(s);

    while let Some(item) = s.next().await {
        match item {
            Ok(event) => println!(
                "{:?} token={} block={} log_index={}",
                event.event_type(),
                event.token(),
                event.block_number(),
                event.log_index()
            ),
            Err(e) => eprintln!("decode error: {}", e),
        }
    }
    Ok(())
}
