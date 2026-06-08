//! NadFun v2 TokenRegistry binding.
//!
//! TokenRegistry stores per-token metadata (pair, quote token, DEX adapter
//! type) for every v2 token deployed through the router. The SDK uses it for
//! pool discovery and as the on-chain v1-vs-v2 version probe — v2 tokens are
//! registered here, v1 tokens are not.

use alloy::{primitives::Address, providers::Provider, sol};
use anyhow::Result;
use std::sync::Arc;

sol!(
    #[sol(rpc)]
    ITokenRegistryV2,
    "abi/v2/TokenRegistry.json"
);

/// Registry metadata for a v2 token.
#[derive(Debug, Clone, Copy)]
pub struct TokenRegistryInfo {
    pub pair: Address,
    pub quote_token: Address,
    pub dex_type: u8,
}

pub struct TokenRegistryV2<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> TokenRegistryV2<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Whether this token has been registered with the v2 system.
    ///
    /// Returns `true` for v2 tokens and `false` for v1 tokens (or unknown
    /// addresses). This is the on-chain hook the SDK uses to detect token
    /// version without needing the API.
    pub async fn is_registered(&self, token: Address) -> Result<bool> {
        let contract = ITokenRegistryV2::new(self.address, self.provider.as_ref());
        let registered = contract.isRegistered(token).call().await?;
        Ok(registered)
    }

    /// Full registry record for a v2 token: pair, quote token, dex type.
    pub async fn get_token_info(&self, token: Address) -> Result<TokenRegistryInfo> {
        let contract = ITokenRegistryV2::new(self.address, self.provider.as_ref());
        let info = contract.getTokenInfo(token).call().await?;
        Ok(TokenRegistryInfo {
            pair: info.pair,
            quote_token: info.quoteToken,
            dex_type: info.dexType,
        })
    }

    /// Pair address for a v2 token. Returns `Address::ZERO` for unregistered tokens.
    pub async fn get_pair(&self, token: Address) -> Result<Address> {
        let contract = ITokenRegistryV2::new(self.address, self.provider.as_ref());
        let pair = contract.getPair(token).call().await?;
        Ok(pair)
    }

    /// Quote token configured for the token.
    pub async fn get_quote_token(&self, token: Address) -> Result<Address> {
        let contract = ITokenRegistryV2::new(self.address, self.provider.as_ref());
        let quote = contract.getQuoteToken(token).call().await?;
        Ok(quote)
    }

    /// DEX adapter type for the token (matches the `DexType` enum on-chain:
    /// NadFun = 0, UniswapV3 = 1 (future), UniswapV4 = 2 (future)).
    pub async fn get_dex_type(&self, token: Address) -> Result<u8> {
        let contract = ITokenRegistryV2::new(self.address, self.provider.as_ref());
        let dex_type = contract.getDexType(token).call().await?;
        Ok(dex_type)
    }

    /// Adapter contract address for a given DEX type.
    pub async fn get_adapter(&self, dex_type: u8) -> Result<Address> {
        let contract = ITokenRegistryV2::new(self.address, self.provider.as_ref());
        let adapter = contract.getAdapter(dex_type).call().await?;
        Ok(adapter)
    }
}
