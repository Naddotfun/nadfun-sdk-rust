//! CreatorTreasury contract client for claiming creator rewards

use alloy::{
    primitives::{Address, B256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

use crate::types::{CreatorBatchClaimParams, CreatorClaimParams, GasPricing};

// Load ABI from JSON file
sol!(
    #[sol(rpc)]
    ICreatorTreasury,
    "abi/ICreatorTreasury.json"
);

/// CreatorTreasury client for claiming creator rewards
pub struct CreatorClient<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> CreatorClient<P> {
    /// Create a new CreatorClient
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Claim reward for a single token
    ///
    /// Uses the Merkle proof from the API to verify and claim rewards.
    /// The claimed wMON is automatically converted to native MON.
    pub async fn claim(&self, params: CreatorClaimParams) -> Result<B256> {
        let contract = ICreatorTreasury::new(self.address, self.provider.as_ref());

        let tokens = vec![params.token];
        let amounts = vec![params.amount];
        let proofs = vec![params.merkle_proof];

        let mut tx = contract.claim(tokens, amounts, proofs);

        if let Some(gas_limit) = params.gas_limit {
            tx = tx.gas(gas_limit);
        }

        if let Some(gas_price) = params.gas_price {
            tx = match gas_price {
                GasPricing::Legacy => tx,
                GasPricing::LegacyWithPrice { gas_price } => tx.gas_price(gas_price),
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => tx
                    .max_fee_per_gas(max_fee_per_gas)
                    .max_priority_fee_per_gas(max_priority_fee_per_gas),
            };
        }

        if let Some(nonce) = params.nonce {
            tx = tx.nonce(nonce);
        }

        let pending = tx.send().await?;
        Ok(*pending.tx_hash())
    }

    /// Claim rewards for multiple tokens in a single transaction
    ///
    /// More gas efficient than calling claim() multiple times.
    pub async fn claim_batch(&self, params: CreatorBatchClaimParams) -> Result<B256> {
        let contract = ICreatorTreasury::new(self.address, self.provider.as_ref());

        let mut tx = contract.claim(params.tokens, params.amounts, params.merkle_proofs);

        if let Some(gas_limit) = params.gas_limit {
            tx = tx.gas(gas_limit);
        }

        if let Some(gas_price) = params.gas_price {
            tx = match gas_price {
                GasPricing::Legacy => tx,
                GasPricing::LegacyWithPrice { gas_price } => tx.gas_price(gas_price),
                GasPricing::Eip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                } => tx
                    .max_fee_per_gas(max_fee_per_gas)
                    .max_priority_fee_per_gas(max_priority_fee_per_gas),
            };
        }

        if let Some(nonce) = params.nonce {
            tx = tx.nonce(nonce);
        }

        let pending = tx.send().await?;
        Ok(*pending.tx_hash())
    }

    /// Get the wMON token address used by the treasury
    pub async fn get_wmon(&self) -> Result<Address> {
        let contract = ICreatorTreasury::new(self.address, self.provider.as_ref());
        let result = contract.wMon().call().await?;
        Ok(result)
    }

    /// Get the CreatorManager contract address
    pub async fn get_creator_manager(&self) -> Result<Address> {
        let contract = ICreatorTreasury::new(self.address, self.provider.as_ref());
        let result = contract.creatorManager().call().await?;
        Ok(result)
    }
}
