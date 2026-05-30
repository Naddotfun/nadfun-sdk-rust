//! `CoreV1` — v1 namespace handle. Borrows `&Core`; methods delegate to the
//! v1 contract bindings. Construct via [`crate::Core::v1`].

use crate::core::Core;
use crate::core::v1::{estimate_gas as free_estimate_gas, GasEstimationParams};
use crate::{
    api::ApiClient,
    constants::get_creator_treasury,
    contracts::{BondingCurveRouter, CreatorClient, DexRouter, Lens},
    types::*,
};
use alloy::{
    primitives::{Address, B256, U256},
    providers::DynProvider,
};
use anyhow::Result;

/// v1 trading/query namespace handle. Zero-cost `Copy` wrapper over `&Core`.
#[derive(Clone, Copy)]
pub struct CoreV1<'a> {
    pub(crate) core: &'a Core,
}

impl<'a> CoreV1<'a> {
    // ========================================================================
    // v1: auto-routing (lens) + buy/sell
    // ========================================================================

    /// Get amount out for a v1 trade, with auto-routed router selection
    /// (bonding curve vs Capricorn CL DEX) via Lens.
    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<(Router, U256)> {
        let (router_address, amount_out) = self
            .core
            .v1
            .lens
            .get_amount_out(token, amount_in, is_buy)
            .await?;

        let router = if router_address == self.core.v1.dex_router.address {
            Router::Dex(router_address)
        } else if router_address == self.core.v1.bonding_curve_router.address {
            Router::BondingCurve(router_address)
        } else {
            return Err(anyhow::anyhow!(
                "Unknown router address: {}",
                router_address
            ));
        };

        Ok((router, amount_out))
    }

    /// Inverse: how much `amount_in` produces `amount_out`. Returns the
    /// router that owns the position.
    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<(Router, U256)> {
        let (router_address, amount_in) = self
            .core
            .v1
            .lens
            .get_amount_in(token, amount_out, is_buy)
            .await?;

        let router = if router_address == self.core.v1.dex_router.address {
            Router::Dex(router_address)
        } else if router_address == self.core.v1.bonding_curve_router.address {
            Router::BondingCurve(router_address)
        } else {
            return Err(anyhow::anyhow!(
                "Unknown router address: {}",
                router_address
            ));
        };

        Ok((router, amount_in))
    }

    /// v1 buy. Pair with [`Self::get_amount_out`] to get the correct
    /// `router`. Returns the submitted tx hash.
    pub async fn buy(&self, params: BuyParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.core.v1.dex_router.buy(params).await,
            Router::BondingCurve(_) => self.core.v1.bonding_curve_router.buy(params).await,
        }
    }

    /// v1 sell. Pair with [`Self::get_amount_out`] for the router.
    pub async fn sell(&self, params: SellParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.core.v1.dex_router.sell(params).await,
            Router::BondingCurve(_) => self.core.v1.bonding_curve_router.sell(params).await,
        }
    }

    /// v1 sell with caller-provided EIP-2612 permit signature.
    pub async fn sell_permit(&self, params: SellPermitParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.core.v1.dex_router.sell_permit(params).await,
            Router::BondingCurve(_) => self.core.v1.bonding_curve_router.sell_permit(params).await,
        }
    }

    // ========================================================================
    // v1: Lens queries
    // ========================================================================

    /// Get available buy tokens and required MON amount (Lens helper).
    pub async fn available_buy_tokens(&self, token: Address) -> Result<(U256, U256)> {
        self.core.v1.lens.available_buy_tokens(token).await
    }

    /// Check if v1 token is locked.
    pub async fn is_locked(&self, token: Address) -> Result<bool> {
        self.core.v1.lens.is_locked(token).await
    }

    /// Check if v1 token has graduated from bonding curve to DEX.
    pub async fn is_graduated(&self, token: Address) -> Result<bool> {
        self.core.v1.lens.is_graduated(token).await
    }

    /// Calculate how many tokens an initial buy of `amount_in` MON produces
    /// at token-creation time (v1 only).
    pub async fn get_initial_buy_amount_out(&self, amount_in: U256) -> Result<U256> {
        self.core.v1.lens.get_initial_buy_amount_out(amount_in).await
    }

    /// Get v1 deploy fee for token creation.
    pub async fn get_deploy_fee(&self) -> Result<U256> {
        self.core.v1.bonding_curve_router.get_deploy_fee().await
    }

    /// Get bonding curve progress in basis points (0–10000 = 0–100%).
    pub async fn get_progress(&self, token: Address) -> Result<U256> {
        self.core.v1.lens.get_progress(token).await
    }

    /// Estimate gas for a v1 trading operation.
    pub async fn estimate_gas(&self, router: &Router, params: GasEstimationParams) -> Result<u64> {
        free_estimate_gas(self.core.provider.clone(), router, params).await
    }

    // ========================================================================
    // v1: token creation
    // ========================================================================

    /// v1 end-to-end token creation flow: image upload + metadata + salt
    /// mining + on-chain create transaction.
    pub async fn create_token(
        &self,
        params: CreateTokenParams,
        api_client: &ApiClient,
    ) -> Result<TokenCreationResult> {
        // Mirror the v2 guard: the salt server mines CREATE2 against the
        // v1 contract addresses of `api_client.network()`, so a mismatch
        // with the network this Core submits to would predict the wrong
        // token address. Fail fast.
        if api_client.network() != self.core.network {
            return Err(anyhow::anyhow!(
                "create_token: ApiClient is bound to {:?} but Core is on {:?}",
                api_client.network(),
                self.core.network,
            ));
        }

        let (metadata_uri, image_uri, salt, token_address_str, is_nsfw) =
            api_client.prepare_token_creation(&params).await?;
        let token_address: Address = token_address_str.parse()?;

        let deploy_fee = self.get_deploy_fee().await?;
        let total_value = params.value + deploy_fee;

        let tx_hash = self
            .core
            .v1
            .bonding_curve_router
            .create(
                params.name.clone(),
                params.symbol.clone(),
                metadata_uri.clone(),
                params.amount_out,
                salt,
                params.action_id,
                total_value,
                None,
                None,
                None,
            )
            .await?;

        Ok(TokenCreationResult {
            token_address,
            metadata_uri,
            image_uri,
            salt: format!("0x{}", hex::encode(salt)),
            transaction_hash: tx_hash,
            is_nsfw,
        })
    }

    // ========================================================================
    // v1: creator rewards
    // ========================================================================

    /// Claim creator reward for a single v1 token.
    pub async fn claim_creator_reward(&self, params: CreatorClaimParams) -> Result<B256> {
        let treasury_address: Address = get_creator_treasury(self.core.network).parse()?;
        let creator = CreatorClient::new(treasury_address, self.core.provider.clone());
        creator.claim(params).await
    }

    /// Batch-claim creator rewards across multiple v1 tokens in one tx.
    pub async fn claim_creator_rewards_batch(
        &self,
        params: CreatorBatchClaimParams,
    ) -> Result<B256> {
        let treasury_address: Address = get_creator_treasury(self.core.network).parse()?;
        let creator = CreatorClient::new(treasury_address, self.core.provider.clone());
        creator.claim_batch(params).await
    }

    // ========================================================================
    // Escape hatches: direct access to underlying v1 contract bindings.
    // ========================================================================

    pub fn bonding_curve_router(&self) -> &BondingCurveRouter<DynProvider> {
        &self.core.v1.bonding_curve_router
    }

    pub fn dex_router(&self) -> &DexRouter<DynProvider> {
        &self.core.v1.dex_router
    }

    pub fn lens(&self) -> &Lens<DynProvider> {
        &self.core.v1.lens
    }
}
