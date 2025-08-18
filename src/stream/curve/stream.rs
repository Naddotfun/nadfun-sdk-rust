use crate::constants::BONDING_CURVE;
use crate::types::{BondingCurveEvent, EventType, decode_bonding_curve_event};

use alloy::{
    primitives::{Address, B256},
    providers::{DynProvider, Provider, ProviderBuilder, WsConnect},
    rpc::types::Filter,
};
use anyhow::Result;
use futures_util::{Stream, StreamExt};
use std::{collections::HashSet, pin::Pin, sync::Arc};

/// Bonding curve event stream with simplified implementation
pub struct CurveStream {
    provider: Arc<DynProvider>,
    event_types: Option<Vec<EventType>>,
    token_filter: Option<HashSet<Address>>,
}

impl CurveStream {
    /// Create a WebSocket-based event stream
    pub async fn new(rpc_url: String) -> Result<CurveStream> {
        let ws = WsConnect::new(rpc_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        let dyn_provider = Arc::new(DynProvider::new(provider));

        Ok(CurveStream {
            provider: dyn_provider,
            event_types: None,
            token_filter: None,
        })
    }

    /// Subscribe to specific event types (network-level filtering)
    pub fn subscribe_events(mut self, event_types: Vec<EventType>) -> Self {
        self.event_types = Some(event_types);
        self
    }

    /// Filter by specific tokens (client-level filtering)
    pub fn filter_tokens(mut self, tokens: Vec<Address>) -> Self {
        self.token_filter = Some(tokens.into_iter().collect());
        self
    }

    /// Create subscription and return raw stream - no transformations!
    pub async fn subscribe(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<BondingCurveEvent>> + Send>>> {
        let bonding_curve_address: Address = BONDING_CURVE
            .parse()
            .expect("Invalid bonding curve address");
        let event_types = self
            .event_types
            .as_ref()
            .map(|v| v.clone())
            .unwrap_or_else(|| {
                vec![
                    EventType::Create,
                    EventType::Buy,
                    EventType::Sell,
                    EventType::Sync,
                    EventType::Lock,
                    EventType::Listed,
                ]
            });

        let signatures: Vec<B256> = event_types.iter().map(|et| et.signature()).collect();

        // Filter for bonding curve address only
        let filter = Filter::new()
            .address(bonding_curve_address)
            .event_signature(signatures);

        let sub = self.provider.subscribe_logs(&filter).await?;
        let token_filter = self.token_filter.clone();

        let stream = sub
            .into_stream()
            .map(move |log| {
                decode_bonding_curve_event(log).and_then(|event| {
                    // Apply client-side token filtering if specified
                    if let Some(ref allowed_tokens) = token_filter {
                        if !allowed_tokens.contains(&event.token()) {
                            return Err(anyhow::anyhow!("Token not in filter"));
                        }
                    }
                    Ok(event)
                })
            })
            .filter_map(|result| async move {
                match result {
                    Ok(event) => Some(Ok(event)),
                    Err(_) => None, // Skip filtered events
                }
            });

        Ok(Box::pin(stream))
    }

    /// Get token filter for manual filtering by caller
    pub fn get_token_filter(&self) -> Option<&HashSet<Address>> {
        self.token_filter.as_ref()
    }
}
