//! `CoreV2` — v2 namespace handle. Borrows `&Core`; methods delegate to the
//! v2 contract bindings. Construct via [`crate::Core::v2`].

use crate::core::core::wait_for_receipt;
use crate::core::Core;
use crate::{
    api::ApiClient,
    constants::*,
    contracts::{BondingCurveV2, NadFunFactory, NadFunRouter, TokenInfoLens, TokenRegistryV2},
    types::{v2::events::IBondingCurveV2Events, *},
};
use alloy::{
    primitives::{Address, B256, U256},
    providers::DynProvider,
    sol_types::SolEvent,
};
use anyhow::{Context, Result};
use std::time::Duration;

/// v2 trading/query namespace handle. Zero-cost `Copy` wrapper over `&Core`.
///
/// Methods take `self` by value (the handle is `Copy`) so a future or an
/// escape-hatch reference can outlive the temporary returned by
/// [`crate::Core::v2`] — e.g. `let fut = core.v2().get_amount_out(..);` then
/// `fut.await`, or `let router = core.v2().router();` used later. Reference-
/// returning escape hatches yield `&'a _` tied to the underlying `&Core`.
#[derive(Clone, Copy)]
pub struct CoreV2<'a> {
    pub(crate) core: &'a Core,
}

impl<'a> CoreV2<'a> {
    // ========================================================================
    // v2: token creation
    // ========================================================================

    /// Low-level v2 create with ERC-20 quote token + pre-approved initial
    /// buy. Use [`Self::create_token`] for the full orchestrated flow.
    pub async fn create(self, params: V2CreateParams) -> Result<B256> {
        self.core.v2.router.create(params).await
    }

    /// Low-level v2 create funded by native MON (`msg.value`).
    pub async fn create_with_native(self, params: V2CreateWithNativeParams) -> Result<B256> {
        self.core.v2.router.create_with_native(params).await
    }

    /// High-level v2 end-to-end token creation: off-chain (image + metadata
    /// + salt mining) then on-chain create. Dispatches to
    ///   - `NadFunRouter::create` for `V2CreatePayment::Erc20`, or
    ///   - `createWithNative` for `V2CreatePayment::Native`.
    pub async fn create_token(
        self,
        params: V2CreateTokenParams,
        api: &ApiClient,
    ) -> Result<V2TokenCreationResult> {
        let v2 = &self.core.v2;

        // The salt server computes CREATE2 against the v2 BondingCurve +
        // Token implementation of `api.network()`. If that disagrees with
        // the network this Core submits to, the predicted token address
        // is wrong and the create reverts (or worse, deploys to a wrong
        // address). Fail fast here instead of letting the on-chain step
        // catch it after funds have been committed.
        if api.network() != self.core.network {
            return Err(anyhow::anyhow!(
                "create_token: ApiClient is bound to {:?} but Core is on {:?}; \
                 v2 contract addresses differ per network — construct ApiClient with the same network",
                api.network(),
                self.core.network,
            ));
        }

        // The on-chain v2 create has no `creator` parameter — `msg.sender`
        // becomes the creator. If `params.creator_address` differs from
        // the wallet that signs the tx, the salt server mines a CREATE2
        // address for a different creator than the curve will deploy, and
        // we'd only catch it during receipt verification (after funds are
        // committed).
        if params.creator_address != self.core.wallet_address {
            return Err(anyhow::anyhow!(
                "create_token: params.creator_address ({}) must match Core's \
                 signing wallet ({}); the v2 router uses msg.sender as the creator",
                params.creator_address,
                self.core.wallet_address,
            ));
        }

        let prepared = api
            .prepare_token_creation_v2(&V2PrepareCreationParams {
                name: params.name.clone(),
                symbol: params.symbol.clone(),
                description: params.description.clone(),
                image_uri: params.image_uri.clone(),
                website: params.website.clone(),
                twitter: params.twitter.clone(),
                telegram: params.telegram.clone(),
                creator_address: params.creator_address,
            })
            .await?;

        // Use server-normalized name/symbol from the salt response — the
        // CREATE2 hash was computed against these, so the on-chain call
        // must use them too (Codex P2 #15).
        let on_chain_name = prepared.name.clone();
        let on_chain_symbol = prepared.symbol.clone();

        let tx_hash = match params.payment {
            V2CreatePayment::Native => {
                // Native create funds a WMON-quoted token: the on-chain
                // `createWithNative` requires `quoteToken == wrappedNative`
                // and `msg.value >= deployFee(quoteToken) + buyQuoteAmount`.
                // Resolve both so the caller doesn't have to.
                let quote_token: Address = get_wmon(self.core.network)
                    .parse()
                    .with_context(|| format!("invalid WMON address for {:?}", self.core.network))?;
                let deploy_fee = v2.protocol_manager.deploy_fee(quote_token).await?;
                let native_value = deploy_fee + params.buy_quote_amount;
                let on_chain = V2CreateWithNativeParams {
                    name: on_chain_name,
                    symbol: on_chain_symbol,
                    token_uri: prepared.metadata_uri.clone(),
                    quote_token,
                    creator_fee_rate: params.creator_fee_rate,
                    vaults: params.vaults.clone(),
                    salt: prepared.salt,
                    dex_type: params.dex_type,
                    buy_quote_amount: params.buy_quote_amount,
                    // msg.value = deployFee + buyQuoteAmount (drawn from
                    // buy_quote_amount; the deploy fee is added on top).
                    native_value,
                    deadline: params.deadline,
                    gas_limit: params.gas_limit,
                    gas_price: params.gas_price.clone(),
                    nonce: params.nonce,
                };
                v2.router.create_with_native(on_chain).await?
            }
            V2CreatePayment::Erc20 { quote_token } => {
                let on_chain = V2CreateParams {
                    name: on_chain_name,
                    symbol: on_chain_symbol,
                    token_uri: prepared.metadata_uri.clone(),
                    quote_token,
                    creator_fee_rate: params.creator_fee_rate,
                    vaults: params.vaults.clone(),
                    salt: prepared.salt,
                    dex_type: params.dex_type,
                    buy_quote_amount: params.buy_quote_amount,
                    deadline: params.deadline,
                    gas_limit: params.gas_limit,
                    gas_price: params.gas_price.clone(),
                    nonce: params.nonce,
                };
                v2.router.create(on_chain).await?
            }
        };

        // Wait for the transaction to land, then verify the on-chain
        // Create event matches the predicted token address. Defends
        // against salt-server / contract drift where the API and the live
        // curve disagree on CREATE2 inputs (Codex P1 #3).
        //
        // `provider.get_transaction_receipt` returns `None` until the tx
        // is mined; polling here turned the previous immediate call into
        // a "receipt not found" error on healthy RPCs.
        let receipt = wait_for_receipt(&self.core.provider, tx_hash, Duration::from_secs(120))
            .await
            .with_context(|| format!("create_token: waiting for receipt of {tx_hash}"))?;
        if !receipt.status() {
            return Err(anyhow::anyhow!(
                "create_token: transaction reverted ({tx_hash})"
            ));
        }

        let create_sig = IBondingCurveV2Events::Create::SIGNATURE_HASH;
        let create_log_opt = receipt
            .logs()
            .iter()
            .find(|l| l.topic0() == Some(&create_sig));

        if let Some(rpc_log) = create_log_opt {
            let decoded = IBondingCurveV2Events::Create::decode_log(&rpc_log.inner)
                .with_context(|| "create_token: failed to decode on-chain Create event")?;
            let on_chain_token = decoded.data.token;
            if on_chain_token != prepared.token_address {
                return Err(anyhow::anyhow!(
                    "create_token: predicted token {} does not match on-chain {} (tx {})",
                    prepared.token_address,
                    on_chain_token,
                    tx_hash
                ));
            }
        } else {
            // No Create log in the receipt — fall back to a TokenRegistry
            // probe, which proves the token was at least registered.
            let pair = v2.token_registry.get_pair(prepared.token_address).await?;
            if pair == Address::ZERO {
                return Err(anyhow::anyhow!(
                    "create_token: predicted token {} not registered on-chain after tx {}",
                    prepared.token_address,
                    tx_hash
                ));
            }
        }

        Ok(V2TokenCreationResult {
            token_address: prepared.token_address,
            metadata_uri: prepared.metadata_uri,
            image_uri: prepared.image_uri,
            salt: prepared.salt,
            transaction_hash: tx_hash,
            is_nsfw: prepared.is_nsfw,
        })
    }

    // ========================================================================
    // v2: trading (exact-in)
    // ========================================================================

    pub async fn buy(self, params: V2BuyParams) -> Result<B256> {
        self.core.v2.router.buy(params).await
    }

    pub async fn buy_with_native(self, params: V2BuyWithNativeParams) -> Result<B256> {
        self.core.v2.router.buy_with_native(params).await
    }

    pub async fn buy_with_permit(self, params: V2BuyWithPermitParams) -> Result<B256> {
        self.core.v2.router.buy_with_permit(params).await
    }

    pub async fn sell(self, params: V2SellParams) -> Result<B256> {
        self.core.v2.router.sell(params).await
    }

    pub async fn sell_to_native(self, params: V2SellToNativeParams) -> Result<B256> {
        self.core.v2.router.sell_to_native(params).await
    }

    pub async fn sell_with_permit(self, params: V2SellWithPermitParams) -> Result<B256> {
        self.core.v2.router.sell_with_permit(params).await
    }

    pub async fn sell_to_native_with_permit(
        self,
        params: V2SellToNativeWithPermitParams,
    ) -> Result<B256> {
        self.core.v2.router.sell_to_native_with_permit(params).await
    }

    // ========================================================================
    // v2: trading (exact-out)
    // ========================================================================

    pub async fn exact_out_buy(self, params: V2ExactOutBuyParams) -> Result<B256> {
        self.core.v2.router.exact_out_buy(params).await
    }

    pub async fn exact_out_buy_with_native(
        self,
        params: V2ExactOutBuyWithNativeParams,
    ) -> Result<B256> {
        self.core.v2.router.exact_out_buy_with_native(params).await
    }

    pub async fn exact_out_sell(self, params: V2ExactOutSellParams) -> Result<B256> {
        self.core.v2.router.exact_out_sell(params).await
    }

    pub async fn exact_out_sell_to_native(
        self,
        params: V2ExactOutSellToNativeParams,
    ) -> Result<B256> {
        self.core.v2.router.exact_out_sell_to_native(params).await
    }

    // ========================================================================
    // v2: quotes
    // ========================================================================

    /// Auto-routed v2 quote (bonding curve pre-graduation, DEX after).
    pub async fn get_amount_out(
        self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.core
            .v2
            .router
            .get_amount_out(token, amount_in, is_buy)
            .await
    }

    /// Auto-routed inverse v2 quote.
    pub async fn get_amount_in(
        self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.core
            .v2
            .router
            .get_amount_in(token, amount_out, is_buy)
            .await
    }

    /// v2 bonding-curve-only quote (errors if graduated).
    pub async fn get_bonding_curve_amount_out(
        self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.core
            .v2
            .router
            .get_bonding_curve_amount_out(token, amount_in, is_buy)
            .await
    }

    /// Inverse v2 bonding-curve-only quote.
    pub async fn get_bonding_curve_amount_in(
        self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.core
            .v2
            .router
            .get_bonding_curve_amount_in(token, amount_out, is_buy)
            .await
    }

    /// v2 DEX-only quote (errors if not graduated).
    pub async fn get_dex_amount_out(
        self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.core
            .v2
            .router
            .get_dex_amount_out(token, amount_in, is_buy)
            .await
    }

    /// Inverse v2 DEX-only quote.
    pub async fn get_dex_amount_in(
        self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.core
            .v2
            .router
            .get_dex_amount_in(token, amount_out, is_buy)
            .await
    }

    // ========================================================================
    // v2: token / pool queries
    // ========================================================================

    /// Whether the v2 token has graduated from bonding curve to DEX.
    pub async fn is_graduated(self, token: Address) -> Result<bool> {
        self.core.v2.router.is_graduated(token).await
    }

    /// NadFunPair address for a v2 token (via `TokenRegistry::getPair`).
    /// Returns `Address::ZERO` if the token isn't registered on v2.
    pub async fn pool_address(self, token: Address) -> Result<Address> {
        self.core.v2.token_registry.get_pair(token).await
    }

    /// Wrapped native (WMON) address known to the v2 router.
    pub async fn wrapped_native(self) -> Result<Address> {
        self.core.v2.router.wrapped_native().await
    }

    /// One-time v2 deploy fee for creating a token quoted in `quote_token`
    /// (denominated in the quote token). The on-chain create requires
    /// `msg.value >= deploy_fee + buy_quote_amount` for native funding;
    /// `create_token` adds it automatically.
    pub async fn deploy_fee(self, quote_token: Address) -> Result<U256> {
        self.core.v2.protocol_manager.deploy_fee(quote_token).await
    }

    /// Estimate gas for any v2 trade or create op. Uses
    /// `self.core.wallet_address` as the `from` so allowance / balance checks
    /// succeed. Errors when `self.core.wallet_address` is `Address::ZERO`
    /// (Codex P2 #9) — a read-only `Core` built via
    /// `Core::with_provider(_, Address::ZERO, _)` cannot estimate gas.
    pub async fn estimate_gas(self, params: V2GasEstimationParams) -> Result<u64> {
        if self.core.wallet_address == Address::ZERO {
            return Err(anyhow::anyhow!(
                "estimate_gas: wallet_address is Address::ZERO; \
                 construct Core with a real signer to estimate gas"
            ));
        }
        self.core
            .v2
            .router
            .estimate_gas(params, self.core.wallet_address)
            .await
    }

    // ========================================================================
    // Escape hatches: direct access to underlying v2 contract bindings.
    //
    // Return `&'a _` (tied to the borrowed `Core`, not to the temporary
    // handle) so `let router = core.v2().router();` keeps the reference valid
    // after the `core.v2()` temporary is dropped.
    // ========================================================================

    pub fn router(self) -> &'a NadFunRouter<DynProvider> {
        &self.core.v2.router
    }

    pub fn factory(self) -> &'a NadFunFactory<DynProvider> {
        &self.core.v2.factory
    }

    pub fn bonding_curve(self) -> &'a BondingCurveV2<DynProvider> {
        &self.core.v2.bonding_curve
    }

    pub fn token_registry(self) -> &'a TokenRegistryV2<DynProvider> {
        &self.core.v2.token_registry
    }

    /// `TokenInfoLens` binding for this `Core`'s network. Prefer
    /// [`crate::Core::detect_version`] / [`crate::Core::detect_token_info`];
    /// this is the raw escape hatch.
    pub fn token_info_lens(self) -> &'a TokenInfoLens<DynProvider> {
        &self.core.v2.token_info_lens
    }
}
