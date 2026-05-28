use alloy::{primitives::Address, providers::Provider, sol};
use anyhow::Result;
use std::sync::Arc;

// Capricorn CL Factory interface
sol! {
    #[sol(rpc)]
    ICapricornCLFactory,
    "abi/ICapricornCLFactory.json"
}

pub use crate::constants::DEFAULT_FEE_TIER;
use crate::constants::{get_dex_factory, get_wmon, Network};

/// Pool discovery helper for finding DEX pools.
///
/// Stores the target `Network` so all v1 lookups (DEX factory + WMON) resolve
/// to the right deployment without consulting any global state.
pub struct PoolDiscovery<P> {
    provider: Arc<P>,
    factory_address: Address,
    network: Network,
}

impl<P: Provider + Clone> PoolDiscovery<P> {
    /// Create a new pool discovery instance for `network`.
    pub fn new(provider: Arc<P>, network: Network) -> Result<Self> {
        let factory_address = get_dex_factory(network).parse()?;
        Ok(Self {
            provider,
            factory_address,
            network,
        })
    }

    /// Get pool address for a specific token paired with WMON.
    /// Uses the default fee tier (1%).
    pub async fn get_pool_for_token(&self, token: Address) -> Result<Option<Address>> {
        let wmon: Address = get_wmon(self.network).parse()?;
        self.get_pool(token, wmon, DEFAULT_FEE_TIER).await
    }

    /// Get pool address for a specific token pair and fee tier
    pub async fn get_pool(
        &self,
        token_a: Address,
        token_b: Address,
        fee: u32,
    ) -> Result<Option<Address>> {
        use alloy::primitives::Uint;
        let factory = ICapricornCLFactory::new(self.factory_address, &self.provider);

        let result = factory
            .getPool(token_a, token_b, Uint::from(fee))
            .call()
            .await?;

        let pool_address = Address::from(result.0);

        // Address::ZERO means pool doesn't exist
        if pool_address == Address::ZERO {
            Ok(None)
        } else {
            Ok(Some(pool_address))
        }
    }

    /// Get multiple pool addresses for multiple tokens paired with WMON.
    pub async fn get_pools_for_tokens(&self, tokens: Vec<Address>) -> Result<Vec<Address>> {
        let wmon_address: Address = get_wmon(self.network).parse()?;
        let mut pools = Vec::new();

        for token in tokens {
            if let Some(pool) = self.get_pool(token, wmon_address, DEFAULT_FEE_TIER).await? {
                pools.push(pool);
            }
        }

        Ok(pools)
    }

    /// Network this discovery instance targets.
    pub fn network(&self) -> Network {
        self.network
    }
}

/// Convenience function to get pool addresses for tokens paired with WMON
/// on the given network.
pub async fn get_pool_addresses_for_tokens(
    provider: Arc<impl Provider + Clone>,
    tokens: Vec<Address>,
    network: Network,
) -> Result<Vec<Address>> {
    let discovery = PoolDiscovery::new(provider, network)?;
    discovery.get_pools_for_tokens(tokens).await
}
