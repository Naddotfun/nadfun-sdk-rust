//! NadFun v2 pair factory binding.
//!
//! Wraps `NadFunFactory.sol` which deploys deterministic CREATE2 pairs and
//! exposes pair-lookup view methods used by the SDK for pool discovery.

use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol!(
    #[sol(rpc)]
    INadFunFactory,
    "abi/v2/NadFunFactory.json"
);

pub struct NadFunFactory<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> NadFunFactory<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Look up the deterministic pair address for a (tokenA, tokenB) pair, or
    /// `Address::ZERO` if no pair has been deployed.
    pub async fn get_pair(&self, token_a: Address, token_b: Address) -> Result<Address> {
        let contract = INadFunFactory::new(self.address, self.provider.as_ref());
        let pair = contract.getPair(token_a, token_b).call().await?;
        Ok(pair)
    }

    /// Number of pairs deployed by the factory.
    pub async fn all_pairs_length(&self) -> Result<U256> {
        let contract = INadFunFactory::new(self.address, self.provider.as_ref());
        let len = contract.allPairsLength().call().await?;
        Ok(len)
    }

    /// Pair address at index (0-indexed).
    pub async fn pair_at(&self, index: U256) -> Result<Address> {
        let contract = INadFunFactory::new(self.address, self.provider.as_ref());
        let pair = contract.allPairs(index).call().await?;
        Ok(pair)
    }

    /// Address of the pair implementation used as the proxy implementation.
    pub async fn implementation(&self) -> Result<Address> {
        let contract = INadFunFactory::new(self.address, self.provider.as_ref());
        let impl_addr = contract.implementation().call().await?;
        Ok(impl_addr)
    }

    /// Address of the fee collector all pairs report into.
    pub async fn fee_collector(&self) -> Result<Address> {
        let contract = INadFunFactory::new(self.address, self.provider.as_ref());
        let collector = contract.feeCollector().call().await?;
        Ok(collector)
    }

    /// Address of the protocol manager.
    pub async fn protocol_manager(&self) -> Result<Address> {
        let contract = INadFunFactory::new(self.address, self.provider.as_ref());
        let manager = contract.protocolManager().call().await?;
        Ok(manager)
    }
}
