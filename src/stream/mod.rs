//! Event streaming and indexing module
//!
//! This module is organized by functionality:
//! - `curve`: Bonding curve event streaming and indexing
//! - `dex`: DEX (Uniswap V3) event streaming and indexing
//!
//! Both support real-time streaming and historical indexing with
//! 2-stage filtering capabilities.

pub mod curve;
pub mod dex;

// Re-export main functionality
pub use curve::{CurveIndexer, CurveStream};
pub use dex::{UniswapSwapIndexer, UniswapSwapStream};

// Re-export types from the types module
pub use crate::types::{
    BondingCurveEvent,
    BuyEvent,
    CreateEvent,
    // Bonding curve types
    EventType,
    ListedEvent,

    LockEvent,
    PoolMetadata,
    SellEvent,
    // Uniswap types
    SwapEvent,
    SyncEvent,
    decode_bonding_curve_event,
    decode_swap_event,
};

/// Usage Examples:
///
/// ```rust
/// use nadfun_sdk::stream::{CurveStream, CurveIndexer, UniswapSwapIndexer, EventType};
/// use alloy::primitives::Address;
/// use alloy::providers::ProviderBuilder;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let ws_url = "wss://eth.merkle.io".to_string();
///     let http_url = "https://eth.merkle.io".to_string();
///     let bonding_curve_address: Address = "0x...".parse()?;
///     let my_tokens = vec!["0x...".parse()?];
///     
///     // Bonding curve streaming
///     let curve_stream = CurveStream::new(ws_url).await?;
///     
///     // Bonding curve indexing
///     let provider = Arc::new(ProviderBuilder::new().connect_http(http_url.parse()?));
///     let curve_indexer = CurveIndexer::new(provider.clone(), bonding_curve_address);
///     
///     // DEX (Uniswap) indexing  
///     let dex_indexer = UniswapSwapIndexer::from_tokens(provider, my_tokens).await?;
///     let swap_events = dex_indexer.fetch_events(18_000_000, 18_010_000).await?;
///     
///     Ok(())
/// }
/// ```

#[cfg(test)]
mod tests {
    use crate::types::*;
    use alloy::{
        primitives::{Address, B256, U256},
        sol_types::SolEvent,
    };

    #[test]
    fn test_event_type_signatures() {
        assert_eq!(
            EventType::Create.signature(),
            bonding_curve::IBondingCurve::CurveCreate::SIGNATURE_HASH
        );
        assert_eq!(
            EventType::Buy.signature(),
            bonding_curve::IBondingCurve::CurveBuy::SIGNATURE_HASH
        );
        assert_eq!(
            EventType::Sell.signature(),
            bonding_curve::IBondingCurve::CurveSell::SIGNATURE_HASH
        );
    }

    #[test]
    fn test_bonding_curve_event_methods() {
        let create_event = CreateEvent {
            creator: Address::ZERO,
            token: Address::ZERO,
            pool: Address::ZERO,
            name: "Test".to_string(),
            symbol: "TEST".to_string(),
            token_uri: "".to_string(),
            virtual_mon: U256::from(1000000),
            virtual_token: U256::from(1000000),
            target_token_amount: U256::from(1000000),
            block_number: 100,
            transaction_hash: B256::ZERO,
            transaction_index: 0,
            log_index: 0,
        };

        let event = BondingCurveEvent::Create(create_event);
        assert_eq!(event.block_number(), 100);
        assert_eq!(event.token(), Address::ZERO);
        assert_eq!(event.event_type(), EventType::Create);
    }
}
