//! `TokenInfoLens` binding — stateless view contract that classifies a
//! token as `V1` / `V2` / `None` *and* returns its on-chain `quoteToken`
//! by probing the v1 and v2 `TokenRegistry` in a single call.
//!
//! Solidity source:
//! ```solidity
//! contract TokenInfoLens {
//!     enum Version { None, V1, V2 }
//!     struct TokenInfo { Version version; address quoteToken; }
//!     function getTokenInfo(address token) external view returns (TokenInfo);
//!     function getTokenInfos(address[] calldata tokens) external view returns (TokenInfo[] memory);
//! }
//! ```
//!
//! Used by [`crate::Core::detect_token_info`] / [`crate::Core::detect_token_infos`]
//! (full info) and [`crate::Core::detect_version`] / [`crate::Core::detect_versions`]
//! (version only). One RPC call covers both registries; the explicit `None`
//! variant catches arbitrary ERC-20 addresses that aren't Nad.fun tokens.
//!
//! V1 tokens always quote against the wrapped native (WMON), which the Lens
//! returns as the `quoteToken`; V2 tokens return whatever quote token their
//! registry entry records (WMON / LvMON / USDT / ...).

use crate::version::{SdkVersion, TokenInfo};
use alloy::{primitives::Address, providers::Provider, sol};
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface ITokenInfoLens {
        struct TokenInfo {
            uint8 version;
            address quoteToken;
        }
        function getTokenInfo(address token) external view returns (TokenInfo memory);
        function getTokenInfos(address[] calldata tokens) external view returns (TokenInfo[] memory);
    }
}

/// Solidity enum value for `Version::None`.
const VERSION_NONE: u8 = 0;
/// Solidity enum value for `Version::V1`.
const VERSION_V1: u8 = 1;
/// Solidity enum value for `Version::V2`.
const VERSION_V2: u8 = 2;

/// SDK wrapper around the `TokenInfoLens` view contract.
pub struct TokenInfoLens<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> TokenInfoLens<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Classify a single token (version + quote token). One RPC call.
    pub async fn get_token_info(&self, token: Address) -> Result<TokenInfo> {
        let contract = ITokenInfoLens::new(self.address, self.provider.as_ref());
        let raw = contract.getTokenInfo(token).call().await?;
        decode_token_info(raw.version, raw.quoteToken, token)
    }

    /// Classify many tokens in a single RPC call. Order is preserved.
    pub async fn get_token_infos(&self, tokens: Vec<Address>) -> Result<Vec<TokenInfo>> {
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let contract = ITokenInfoLens::new(self.address, self.provider.as_ref());
        let raws = contract.getTokenInfos(tokens.clone()).call().await?;
        if raws.len() != tokens.len() {
            return Err(anyhow::anyhow!(
                "TokenInfoLens::getTokenInfos returned {} entries for {} tokens",
                raws.len(),
                tokens.len(),
            ));
        }
        raws.into_iter()
            .zip(tokens.iter())
            .map(|(raw, t)| decode_token_info(raw.version, raw.quoteToken, *t))
            .collect()
    }
}

fn decode_token_info(raw_version: u8, quote_token: Address, token: Address) -> Result<TokenInfo> {
    let version = match raw_version {
        VERSION_NONE => SdkVersion::None,
        VERSION_V1 => SdkVersion::V1,
        VERSION_V2 => SdkVersion::V2,
        _ => {
            return Err(anyhow::anyhow!(
                "TokenInfoLens returned unknown version {raw_version} for token {token}; \
                 SDK only knows None=0 / V1=1 / V2=2 — upgrade nadfun_sdk"
            ))
        }
    };
    Ok(TokenInfo {
        version,
        quote_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    const QUOTE: Address = address!("Be3fa50514D9617ce645a02B34F595541AF02b6b");

    #[test]
    fn decodes_known_versions_and_passes_quote_through() {
        let none = decode_token_info(VERSION_NONE, Address::ZERO, Address::ZERO).unwrap();
        assert_eq!(none.version, SdkVersion::None);
        assert_eq!(none.quote_token, Address::ZERO);

        let v1 = decode_token_info(VERSION_V1, QUOTE, Address::ZERO).unwrap();
        assert_eq!(v1.version, SdkVersion::V1);
        assert_eq!(v1.quote_token, QUOTE);

        let v2 = decode_token_info(VERSION_V2, QUOTE, Address::ZERO).unwrap();
        assert_eq!(v2.version, SdkVersion::V2);
        assert_eq!(v2.quote_token, QUOTE);
    }

    #[test]
    fn rejects_unknown_version() {
        let err = decode_token_info(3, QUOTE, Address::ZERO).unwrap_err();
        assert!(err.to_string().contains("unknown version 3"));
    }
}
