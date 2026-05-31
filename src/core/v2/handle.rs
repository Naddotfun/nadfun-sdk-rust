//! `CoreV2` — v2 namespace handle. Borrows `&Core`; methods delegate to the
//! v2 contract bindings. Construct via [`crate::Core::v2`].

use crate::contracts::v2::{NadFunPair, PairReserves};
use crate::core::core::wait_for_receipt;
use crate::core::Core;
use crate::{
    api::ApiClient,
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
            V2CreatePayment::Native { quote_token } => {
                // Native create funds a native-quoted token: the on-chain
                // `createWithNative` requires `quoteToken` to be a
                // native-equivalent the router honors (its `wrappedNative`
                // or a configured LvMON-style minter token) and
                // `msg.value >= deployFee(quoteToken) + buyQuoteAmount`.
                //
                // The SDK ships no baked allowlist and does not auto-resolve
                // the native quote token — the caller supplies it explicitly
                // (resolve via `constants::quote_tokens(network)` or
                // `core.v2().wrapped_native()`).
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
    // v2: curve / config / registry views (passthrough parity)
    // ========================================================================

    /// Whether the v2 bonding-curve protocol is halted (no trading allowed).
    pub async fn is_halted(self) -> Result<bool> {
        self.core.v2.bonding_curve.is_halted().await
    }

    /// Anti-sniping penalty (basis points) for a buy on `token` at the current
    /// block. Driven by `block.number - createdAtBlock` against the
    /// ProtocolManager's penalty table.
    pub async fn get_sniping_penalty(self, token: Address) -> Result<U256> {
        self.core.v2.bonding_curve.get_sniping_penalty(token).await
    }

    /// Quote token configured for `token`'s curve (WMON, LvMON, or an ERC-20).
    pub async fn quote_token(self, token: Address) -> Result<Address> {
        self.core.v2.bonding_curve.get_quote_token(token).await
    }

    /// Full on-chain bonding-curve state for `token`. See [`V2Curve`].
    pub async fn get_curve(self, token: Address) -> Result<V2Curve> {
        self.core.v2.bonding_curve.get_curve(token).await
    }

    /// Per-quote-token protocol config (genesis curve params + fees +
    /// graduation economics). See [`V2QuoteConfig`].
    pub async fn quote_config(self, quote_token: Address) -> Result<V2QuoteConfig> {
        self.core.v2.protocol_manager.get_config(quote_token).await
    }

    /// DexType discriminator registered for `token` (the post-graduation DEX).
    pub async fn get_dex_type(self, token: Address) -> Result<u8> {
        self.core.v2.token_registry.get_dex_type(token).await
    }

    /// Whether `token` is registered in the v2 `TokenRegistry`.
    pub async fn is_registered(self, token: Address) -> Result<bool> {
        self.core.v2.token_registry.is_registered(token).await
    }

    // ========================================================================
    // v2: computed helpers (v1 Lens parity; v2 has no on-chain equivalent)
    //
    // Pure curve math reproducing the on-chain BondingCurve arithmetic — v2
    // exposes no getProgress / availableBuyTokens / getInitialBuyAmountOut, so
    // these compute client-side from get_curve / quote_config. See
    // `crate::core::v2::calc` and the live drift tests.
    // ========================================================================

    /// Bonding-curve progress in basis points (0–10000 = 0–100%), computed from
    /// the live curve. Graduated tokens read 10000. v1 parity for
    /// [`crate::CoreV1::get_progress`].
    pub async fn get_progress(self, token: Address) -> Result<U256> {
        let curve = self.core.v2.bonding_curve.get_curve(token).await?;
        Ok(crate::core::v2::calc::progress_bps(&curve))
    }

    /// Tokens still buyable on the curve before graduation, and the quote-token
    /// amount required to buy them all.
    ///
    /// Returns `(available_tokens, required_quote)`. `available_tokens =
    /// virtual_token_reserve − min_token_reserve`; `required_quote` is the
    /// on-chain bonding-curve quote ([`Self::get_bonding_curve_amount_in`]) for
    /// that output, and round-trips exactly at this boundary: feeding it back
    /// through [`Self::get_bonding_curve_amount_out`] returns `available_tokens`
    /// with no rounding gap, since the curve's `getAmountIn`/`getAmountOut`
    /// converge at the `min_token_reserve` edge. A graduated token returns
    /// `(0, 0)`. v1 parity for [`crate::CoreV1::available_buy_tokens`].
    pub async fn available_buy_tokens(self, token: Address) -> Result<(U256, U256)> {
        let curve = self.core.v2.bonding_curve.get_curve(token).await?;
        let available = crate::core::v2::calc::available_buy_tokens(&curve);
        if available.is_zero() {
            return Ok((U256::ZERO, U256::ZERO));
        }
        let required_quote = self
            .core
            .v2
            .router
            .get_bonding_curve_amount_in(token, available, true)
            .await?;
        Ok((available, required_quote))
    }

    /// Tokens received for the create-time initial buy of `amount_in` quote, for
    /// a token quoted in `quote_token` and created with `creator_fee_rate` (bps).
    ///
    /// Returns the EXACT output the on-chain `BondingCurve._initialBuy` mints —
    /// computed from the quote token's genesis [`V2QuoteConfig`] (the curve a
    /// freshly created token inherits) plus the token's per-token creator fee.
    /// The create-time buy is anti-sniping EXEMPT, so this reproduces it to the
    /// wei: it deducts the combined protocol + creator fee (one ceil `mulDivUp`)
    /// then applies the constant-product / supply-cap math.
    ///
    /// `creator_fee_rate` is the value passed in `V2CreateTokenParams` (or read
    /// from [`crate::V2Curve::creator_fee_rate`]); it is a per-token parameter,
    /// not part of the genesis config, so the caller must supply it.
    ///
    /// NOTE: unlike v1's parameterless [`crate::CoreV1::get_initial_buy_amount_out`]
    /// (all v1 tokens share one genesis curve), v2 genesis params differ per
    /// quote token, so this takes `quote_token`.
    ///
    /// This models only the create-time buy. A later buy on the same curve still
    /// differs by design: it carries the time-decaying anti-sniping penalty that
    /// the create-time buy is exempt from.
    pub async fn get_initial_buy_amount_out(
        self,
        quote_token: Address,
        amount_in: U256,
        creator_fee_rate: u16,
    ) -> Result<U256> {
        let config = self
            .core
            .v2
            .protocol_manager
            .get_config(quote_token)
            .await?;
        crate::core::v2::calc::initial_buy_amount_out(&config, amount_in, creator_fee_rate)
    }

    /// Whether `token`'s graduated DEX pair is locked.
    ///
    /// CAVEAT: this is the **post-graduation `NadFunPair` lock**, semantically
    /// different from v1 [`crate::CoreV1::is_locked`] (a bonding-curve lock).
    /// Errors before graduation: the registry assigns a pair at creation time,
    /// so a non-zero pair does NOT imply graduation — this gates on the curve's
    /// `graduated` flag (a pre-graduation pair has no meaningful lock state).
    pub async fn is_locked(self, token: Address) -> Result<bool> {
        let pair = self.resolve_graduated_pair(token, "is_locked").await?;
        NadFunPair::new(pair, self.core.provider.clone())
            .is_locked()
            .await
    }

    /// Reserves of `token`'s graduated DEX pair. Errors before graduation
    /// (gates on the curve's `graduated` flag, not merely a non-zero pair).
    ///
    /// `reserve0` / `reserve1` follow the pair's `token0` / `token1` ordering,
    /// which is the address-sorted order of `(token, quote_token)` — NOT
    /// necessarily token-then-quote. To map a reserve to a side, compare
    /// against [`Self::quote_token`] or call `pair.token0()` via the
    /// [`Self::router`]-style escape hatch.
    pub async fn get_reserves(self, token: Address) -> Result<PairReserves> {
        let pair = self.resolve_graduated_pair(token, "get_reserves").await?;
        NadFunPair::new(pair, self.core.provider.clone())
            .get_reserves()
            .await
    }

    /// Resolve the DEX pair address for a token that has actually graduated.
    ///
    /// Gates on graduation because the registry assigns a pair at token creation
    /// (so `pair != ZERO` is true pre-graduation too); only a graduated token
    /// has a live pair with meaningful lock/reserve state.
    ///
    /// Uses a single `get_curve` call — it returns both `graduated` and `pair`,
    /// so we avoid a separate `is_graduated` + `get_pair` round-trip (Codex P2).
    /// For an unregistered token `get_curve` returns a zeroed curve
    /// (`graduated == false`), which falls through to the not-graduated error.
    async fn resolve_graduated_pair(self, token: Address, op: &str) -> Result<Address> {
        let curve = self.core.v2.bonding_curve.get_curve(token).await?;
        if !curve.graduated {
            return Err(anyhow::anyhow!(
                "{op}: token {token} has not graduated (no live v2 DEX pair)"
            ));
        }
        if curve.pair == Address::ZERO {
            return Err(anyhow::anyhow!(
                "{op}: token {token} reports graduated but has no pair"
            ));
        }
        Ok(curve.pair)
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
