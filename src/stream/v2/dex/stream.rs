use super::events::{decode_nadfun_swap_event, nadfun_swap_signature, NadFunSwapEvent};

use alloy::{
    primitives::Address,
    providers::{DynProvider, Provider, ProviderBuilder, WsConnect},
    rpc::types::Filter,
};
use anyhow::Result;
use futures_util::{Stream, StreamExt};
use std::{collections::HashSet, pin::Pin, sync::Arc};

/// Real-time NadFunPair `Swap` stream.
///
/// Subscribes to swap logs across an explicit set of pair addresses. Use
/// [`crate::contracts::NadFunFactory::get_pair`] or
/// [`crate::CoreV2::pool_address`] to resolve pair addresses for tokens
/// of interest before constructing the stream.
pub struct NadFunSwapStream {
    provider: Arc<DynProvider>,
    pairs: Vec<Address>,
}

impl NadFunSwapStream {
    /// Connect over WebSocket and bind to a set of pair addresses.
    pub async fn new(ws_url: String, pairs: Vec<Address>) -> Result<NadFunSwapStream> {
        let ws = WsConnect::new(ws_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        Ok(NadFunSwapStream {
            provider: Arc::new(DynProvider::new(provider)),
            pairs,
        })
    }

    /// Open the subscription. Yields decoded swap events for the bound pair
    /// set. Logs that fail to decode are silently dropped.
    pub async fn subscribe(
        &self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<NadFunSwapEvent>> + Send>>> {
        let filter = Filter::new()
            .address(self.pairs.clone())
            .event_signature(nadfun_swap_signature());

        let sub = self.provider.subscribe_logs(&filter).await?;
        let pair_set: HashSet<Address> = self.pairs.iter().copied().collect();

        let stream = sub
            .into_stream()
            .map(move |log| {
                decode_nadfun_swap_event(log).and_then(|event| {
                    // Defensive: subscription filter should already gate by
                    // address, but verify here in case the RPC returns extras.
                    if !pair_set.is_empty() && !pair_set.contains(&event.pair_address) {
                        return Err(anyhow::anyhow!("Pair not in filter"));
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

    pub fn pairs(&self) -> &[Address] {
        &self.pairs
    }
}
