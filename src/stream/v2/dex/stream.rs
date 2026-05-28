use super::events::{decode_nadfun_swap_event, nadfun_swap_signature, NadFunSwapEvent};

use crate::constants::Network;
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
/// of interest before constructing the stream. Bound to a `Network` for
/// downstream callers that need the context.
pub struct NadFunSwapStream {
    provider: Arc<DynProvider>,
    pairs: Vec<Address>,
    network: Network,
}

impl NadFunSwapStream {
    /// Connect over WebSocket and bind to a set of pair addresses on
    /// `network`.
    ///
    /// `pairs` MUST be non-empty — an empty filter would let `eth_subscribe`
    /// fan out across every contract on chain, and the subscription would
    /// just discard everything client-side. Returns an error so the misuse
    /// surfaces immediately instead of silently going quiet (Codex P3 #18).
    pub async fn new(
        ws_url: String,
        pairs: Vec<Address>,
        network: Network,
    ) -> Result<NadFunSwapStream> {
        if pairs.is_empty() {
            return Err(anyhow::anyhow!(
                "NadFunSwapStream::new: at least one pair address is required"
            ));
        }
        let ws = WsConnect::new(ws_url);
        let provider = ProviderBuilder::new().connect_ws(ws).await?;
        Ok(NadFunSwapStream {
            provider: Arc::new(DynProvider::new(provider)),
            pairs,
            network,
        })
    }

    /// Network this stream is bound to.
    pub fn network(&self) -> Network {
        self.network
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
