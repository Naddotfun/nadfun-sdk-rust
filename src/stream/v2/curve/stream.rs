use crate::constants::{get_bonding_curve_v2, Network};
use crate::types::{decode_v2_bonding_curve_event, V2BondingCurveEvent, V2EventType};

use alloy::{
    primitives::{Address, B256},
    providers::{DynProvider, Provider, ProviderBuilder, WsConnect},
    rpc::types::Filter,
};
use anyhow::Result;
use futures_util::{Stream, StreamExt};
use std::{collections::HashSet, pin::Pin, sync::Arc};

/// v2 BondingCurve event stream.
///
/// Bound to a `Network` so the v2 BondingCurve address is resolved without
/// touching any global state. Use [`Self::subscribe_events`] for
/// network-level filtering and [`Self::filter_tokens`] for client-side
/// per-token filtering.
pub struct CurveStreamV2 {
    provider: Arc<DynProvider>,
    event_types: Option<Vec<V2EventType>>,
    token_filter: Option<HashSet<Address>>,
    network: Network,
}

impl CurveStreamV2 {
    /// Connect over WebSocket and prepare a stream for `network`.
    pub async fn new(ws_url: String, network: Network) -> Result<CurveStreamV2> {
        let ws = WsConnect::new(ws_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        Ok(CurveStreamV2 {
            provider: Arc::new(DynProvider::new(provider)),
            event_types: None,
            token_filter: None,
            network,
        })
    }

    /// Network this stream is bound to.
    pub fn network(&self) -> Network {
        self.network
    }

    /// Network-level event-type filter — translates to `topics[0]` filtering
    /// on `eth_subscribe`.
    pub fn subscribe_events(mut self, event_types: Vec<V2EventType>) -> Self {
        self.event_types = Some(event_types);
        self
    }

    /// Client-side per-token filter. Logs for other tokens are silently
    /// dropped from the stream.
    pub fn filter_tokens(mut self, tokens: Vec<Address>) -> Self {
        self.token_filter = Some(tokens.into_iter().collect());
        self
    }

    /// Open the subscription. Returns a stream of decoded events.
    pub async fn subscribe(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<V2BondingCurveEvent>> + Send>>> {
        let bonding_curve_address: Address = get_bonding_curve_v2(self.network)
            .ok_or_else(|| {
                anyhow::anyhow!("BondingCurveV2 is not configured for {:?}", self.network)
            })?
            .parse()?;
        let event_types = self.event_types.clone().unwrap_or_else(V2EventType::all);
        let signatures: Vec<B256> = event_types.iter().map(|et| et.signature()).collect();

        let filter = Filter::new()
            .address(bonding_curve_address)
            .event_signature(signatures);

        let sub = self.provider.subscribe_logs(&filter).await?;
        let token_filter = self.token_filter.clone();

        let stream = sub
            .into_stream()
            .map(move |log| {
                decode_v2_bonding_curve_event(log).and_then(|event| {
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
                    Err(_) => None,
                }
            });

        Ok(Box::pin(stream))
    }

    pub fn get_token_filter(&self) -> Option<&HashSet<Address>> {
        self.token_filter.as_ref()
    }
}
