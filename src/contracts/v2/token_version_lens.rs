//! `TokenVersionLens` binding — stateless view contract that classifies
//! a token as `V1` / `V2` / `None` by simultaneously probing the v1
//! `TokenRegistry` and the v2 `TokenRegistry`.
//!
//! Solidity source:
//! ```solidity
//! contract TokenVersionLens {
//!     enum Version { None, V1, V2 }
//!     function getVersion(address token) external view returns (Version);
//!     function getVersions(address[] calldata tokens) external view returns (Version[] memory);
//! }
//! ```
//!
//! Use this from [`crate::Core::detect_version`] / [`crate::Core::detect_versions`]
//! to dispatch user code into the right v1/v2 surface. One RPC call
//! covers both registries, and the explicit `None` variant catches
//! arbitrary ERC-20 addresses that aren't Nad.fun-deployed tokens.

use crate::version::SdkVersion;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface ITokenVersionLens {
        function getVersion(address token) external view returns (uint8);
        function getVersions(address[] calldata tokens) external view returns (uint8[] memory);
    }
}

/// Solidity enum value for `Version::None`.
const VERSION_NONE: u8 = 0;
/// Solidity enum value for `Version::V1`.
const VERSION_V1: u8 = 1;
/// Solidity enum value for `Version::V2`.
const VERSION_V2: u8 = 2;

/// SDK wrapper around the `TokenVersionLens` view contract.
pub struct TokenVersionLens<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> TokenVersionLens<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Classify a single token's version. One RPC call covers both v1
    /// and v2 registry lookups on chain.
    pub async fn get_version(&self, token: Address) -> Result<SdkVersion> {
        let contract = ITokenVersionLens::new(self.address, self.provider.as_ref());
        let raw = contract.getVersion(token).call().await?;
        decode_version(raw, token)
    }

    /// Classify many tokens in a single RPC call. Order is preserved.
    pub async fn get_versions(&self, tokens: Vec<Address>) -> Result<Vec<SdkVersion>> {
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let contract = ITokenVersionLens::new(self.address, self.provider.as_ref());
        let raws = contract.getVersions(tokens.clone()).call().await?;
        if raws.len() != tokens.len() {
            return Err(anyhow::anyhow!(
                "TokenVersionLens::getVersions returned {} entries for {} tokens",
                raws.len(),
                tokens.len(),
            ));
        }
        raws.into_iter()
            .zip(tokens.iter())
            .map(|(raw, t)| decode_version(raw, *t))
            .collect()
    }
}

fn decode_version(raw: u8, token: Address) -> Result<SdkVersion> {
    match raw {
        VERSION_NONE => Ok(SdkVersion::None),
        VERSION_V1 => Ok(SdkVersion::V1),
        VERSION_V2 => Ok(SdkVersion::V2),
        _ => Err(anyhow::anyhow!(
            "TokenVersionLens returned unknown version {raw} for token {token}; \
             SDK only knows None=0 / V1=1 / V2=2 — upgrade nadfun_sdk"
        )),
    }
}

// Allow U256 import to be unused if alloy ever stops needing it transitively.
#[allow(dead_code)]
const _: Option<U256> = None;
