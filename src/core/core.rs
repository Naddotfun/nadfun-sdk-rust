//! Unified `Core` — one entry point for v1 + v2 bonding-curve trading,
//! token creation, and pool discovery on Nad.fun.
//!
//! A single `Core` instance binds to a `Network` and wires both the v1
//! (`BondingCurveRouter` + `DexRouter` + `Lens`) and v2 (`NadFunRouter` +
//! `NadFunFactory` + `BondingCurveV2` + `TokenRegistryV2`) contract
//! surfaces. v1 trades use `buy` / `sell` and the auto-routing `Lens`
//! quote; v2 trades use the explicit `*_v2` methods (different params
//! shape, so auto-dispatch on `buy` / `sell` would erase information).
//!
//! Use `Core::detect_version(token)` to ask the on-chain
//! `TokenRegistryV2::is_registered` probe (with in-process caching)
//! whether a token is v1 or v2, then dispatch to the right surface.

use crate::{
    api::ApiClient,
    constants::*,
    contracts::{
        BondingCurveRouter, BondingCurveV2, CreatorClient, DexRouter, Lens, NadFunFactory,
        NadFunRouter, TokenRegistryV2,
    },
    core::v1::gas::{estimate_gas, GasEstimationParams},
    types::{v2::events::IBondingCurveV2Events, *},
    version::SdkVersion,
};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol_types::SolEvent,
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

/// Unified high-level SDK client. Handles v1 + v2 trading, token creation,
/// quote routing, and creator reward claims from a single instance.
pub struct Core {
    // v1
    bonding_curve_router: BondingCurveRouter<DynProvider>,
    dex_router: DexRouter<DynProvider>,
    lens: Lens<DynProvider>,
    // v2 — None when v2 isn't deployed on this network (the v2-only methods
    // will return a clear error in that case).
    v2: Option<V2Bindings>,
    // shared
    provider: Arc<DynProvider>,
    wallet_address: Address,
    network: Network,
    // Per-token version cache. Populated by `detect_version` so subsequent
    // dispatch calls don't re-hit the chain.
    version_cache: Arc<RwLock<HashMap<Address, SdkVersion>>>,
}

/// v2 contract bindings — bundled together so the optional v2 surface is a
/// single `Option<_>` on `Core` instead of four parallel options.
struct V2Bindings {
    router: NadFunRouter<DynProvider>,
    factory: NadFunFactory<DynProvider>,
    bonding_curve: BondingCurveV2<DynProvider>,
    token_registry: TokenRegistryV2<DynProvider>,
}

impl Core {
    /// Create a new `Core` from RPC URL + private key + network.
    ///
    /// Wires both v1 and v2 contract bindings for `network`. If v2 isn't
    /// deployed on this network, v2-only methods return a clear error and
    /// `detect_version` always returns `SdkVersion::V1`.
    pub async fn new(rpc_url: String, private_key: String, network: Network) -> Result<Self> {
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();

        let wallet = EthereumWallet::from(signer);
        let url = rpc_url.parse()?;
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);
        let dyn_provider = Arc::new(DynProvider::new(provider));

        Self::with_provider(dyn_provider, wallet_address, network)
    }

    /// Construct a `Core` from an existing provider + wallet address.
    ///
    /// Use this when you want to share an RPC provider (and nonce state)
    /// across multiple `Core` instances — for example, two `Core`s pointing
    /// at different networks but sharing a connection pool. Caller is
    /// responsible for ensuring the provider's signer matches
    /// `wallet_address`.
    pub fn with_provider(
        provider: Arc<DynProvider>,
        wallet_address: Address,
        network: Network,
    ) -> Result<Self> {
        let lens_address: Address = get_lens_address(network).parse()?;
        let bonding_curve_router_address: Address = get_bonding_curve_router(network).parse()?;
        let dex_router_address: Address = get_dex_router(network).parse()?;
        let bonding_curve_address: Address = get_bonding_curve(network).parse()?;

        let bonding_curve_router = BondingCurveRouter::new(
            bonding_curve_router_address,
            bonding_curve_address,
            provider.clone(),
        );
        let dex_router = DexRouter::new(dex_router_address, provider.clone());
        let lens = Lens::new(lens_address, provider.clone());

        let v2 = build_v2_bindings(&provider, network)?;

        Ok(Self {
            bonding_curve_router,
            dex_router,
            lens,
            v2,
            provider,
            wallet_address,
            network,
            version_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Internal helper — returns the v2 bindings or a clear error if v2 is
    /// not configured on this network.
    fn v2(&self) -> Result<&V2Bindings> {
        self.v2.as_ref().ok_or_else(|| {
            anyhow::anyhow!("NadFun contract v2 is not deployed on {:?}", self.network)
        })
    }

    /// Whether v2 contracts are wired for this `Core`'s network.
    pub fn v2_available(&self) -> bool {
        self.v2.is_some()
    }

    // ========================================================================
    // v1 + v2: dispatch primitive
    // ========================================================================

    /// Detect whether `token` is a v2 token (registered in
    /// `TokenRegistryV2`) or a v1 token. Result is cached in-process so
    /// repeated calls don't re-hit the chain.
    ///
    /// Always returns `SdkVersion::V1` when v2 isn't configured on this
    /// network.
    pub async fn detect_version(&self, token: Address) -> Result<SdkVersion> {
        if let Ok(cache) = self.version_cache.read() {
            if let Some(v) = cache.get(&token) {
                return Ok(*v);
            }
        }

        let v = match self.v2.as_ref() {
            Some(v2) => {
                let pair = v2.token_registry.get_pair(token).await?;
                if pair == Address::ZERO {
                    SdkVersion::V1
                } else {
                    SdkVersion::V2
                }
            }
            None => SdkVersion::V1,
        };

        if let Ok(mut cache) = self.version_cache.write() {
            cache.insert(token, v);
        }
        Ok(v)
    }

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
        let (router_address, amount_out) =
            self.lens.get_amount_out(token, amount_in, is_buy).await?;

        let router = if router_address == self.dex_router.address {
            Router::Dex(router_address)
        } else if router_address == self.bonding_curve_router.address {
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
        let (router_address, amount_in) =
            self.lens.get_amount_in(token, amount_out, is_buy).await?;

        let router = if router_address == self.dex_router.address {
            Router::Dex(router_address)
        } else if router_address == self.bonding_curve_router.address {
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
            Router::Dex(_) => self.dex_router.buy(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.buy(params).await,
        }
    }

    /// v1 sell. Pair with [`Self::get_amount_out`] for the router.
    pub async fn sell(&self, params: SellParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.dex_router.sell(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.sell(params).await,
        }
    }

    /// v1 sell with caller-provided EIP-2612 permit signature.
    pub async fn sell_permit(&self, params: SellPermitParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.dex_router.sell_permit(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.sell_permit(params).await,
        }
    }

    /// Get transaction receipt for `tx_hash`. Works for both v1 and v2
    /// transactions — the receipt format is chain-level.
    pub async fn get_receipt(&self, tx_hash: B256) -> Result<TransactionResult> {
        let receipt = self
            .provider
            .get_transaction_receipt(tx_hash)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Transaction receipt not found"))?;

        Ok(TransactionResult {
            transaction_hash: receipt.transaction_hash,
            block_number: receipt.block_number,
            gas_used: Some(U256::from(receipt.gas_used)),
            status: receipt.status(),
            logs: receipt.logs().to_vec(),
        })
    }

    // ========================================================================
    // v1: Lens queries
    // ========================================================================

    /// Get available buy tokens and required MON amount (Lens helper).
    pub async fn available_buy_tokens(&self, token: Address) -> Result<(U256, U256)> {
        self.lens.available_buy_tokens(token).await
    }

    /// Check if v1 token is locked.
    pub async fn is_locked(&self, token: Address) -> Result<bool> {
        self.lens.is_locked(token).await
    }

    /// Check if v1 token has graduated from bonding curve to DEX.
    pub async fn is_graduated(&self, token: Address) -> Result<bool> {
        self.lens.is_graduated(token).await
    }

    /// Calculate how many tokens an initial buy of `amount_in` MON produces
    /// at token-creation time (v1 only).
    pub async fn get_initial_buy_amount_out(&self, amount_in: U256) -> Result<U256> {
        self.lens.get_initial_buy_amount_out(amount_in).await
    }

    /// Get v1 deploy fee for token creation.
    pub async fn get_deploy_fee(&self) -> Result<U256> {
        self.bonding_curve_router.get_deploy_fee().await
    }

    /// Get bonding curve progress in basis points (0–10000 = 0–100%).
    pub async fn get_progress(&self, token: Address) -> Result<U256> {
        self.lens.get_progress(token).await
    }

    /// Estimate gas for a v1 trading operation.
    pub async fn estimate_gas(&self, router: &Router, params: GasEstimationParams) -> Result<u64> {
        estimate_gas(self.provider.clone(), router, params).await
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
        if api_client.network() != self.network {
            return Err(anyhow::anyhow!(
                "create_token: ApiClient is bound to {:?} but Core is on {:?}",
                api_client.network(),
                self.network,
            ));
        }

        let (metadata_uri, image_uri, salt, token_address_str, is_nsfw) =
            api_client.prepare_token_creation(&params).await?;
        let token_address: Address = token_address_str.parse()?;

        let deploy_fee = self.get_deploy_fee().await?;
        let total_value = params.value + deploy_fee;

        let tx_hash = self
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
        let treasury_address: Address = get_creator_treasury(self.network).parse()?;
        let creator = CreatorClient::new(treasury_address, self.provider.clone());
        creator.claim(params).await
    }

    /// Batch-claim creator rewards across multiple v1 tokens in one tx.
    pub async fn claim_creator_rewards_batch(
        &self,
        params: CreatorBatchClaimParams,
    ) -> Result<B256> {
        let treasury_address: Address = get_creator_treasury(self.network).parse()?;
        let creator = CreatorClient::new(treasury_address, self.provider.clone());
        creator.claim_batch(params).await
    }

    // ========================================================================
    // v2: token creation
    // ========================================================================

    /// Low-level v2 create with ERC-20 quote token + pre-approved initial
    /// buy. Use [`Self::create_token_v2`] for the full orchestrated flow.
    pub async fn create_v2(&self, params: V2CreateParams) -> Result<B256> {
        self.v2()?.router.create(params).await
    }

    /// Low-level v2 create funded by native MON (`msg.value`).
    pub async fn create_with_native_v2(&self, params: V2CreateWithNativeParams) -> Result<B256> {
        self.v2()?.router.create_with_native(params).await
    }

    /// High-level v2 end-to-end token creation: off-chain (image + metadata
    /// + salt mining) then on-chain create. Dispatches to
    ///   - `NadFunRouter::create` for `V2CreatePayment::Erc20`, or
    ///   - `createWithNative` for `V2CreatePayment::Native`.
    pub async fn create_token_v2(
        &self,
        params: V2CreateTokenParams,
        api: &ApiClient,
    ) -> Result<V2TokenCreationResult> {
        let v2 = self.v2()?;

        // The salt server computes CREATE2 against the v2 BondingCurve +
        // Token implementation of `api.network()`. If that disagrees with
        // the network this Core submits to, the predicted token address
        // is wrong and the create reverts (or worse, deploys to a wrong
        // address). Fail fast here instead of letting the on-chain step
        // catch it after funds have been committed.
        if api.network() != self.network {
            return Err(anyhow::anyhow!(
                "create_token_v2: ApiClient is bound to {:?} but Core is on {:?}; \
                 v2 contract addresses differ per network — construct ApiClient with the same network",
                api.network(),
                self.network,
            ));
        }

        // The on-chain v2 create has no `creator` parameter — `msg.sender`
        // becomes the creator. If `params.creator_address` differs from
        // the wallet that signs the tx, the salt server mines a CREATE2
        // address for a different creator than the curve will deploy, and
        // we'd only catch it during receipt verification (after funds are
        // committed).
        if params.creator_address != self.wallet_address {
            return Err(anyhow::anyhow!(
                "create_token_v2: params.creator_address ({}) must match Core's \
                 signing wallet ({}); the v2 router uses msg.sender as the creator",
                params.creator_address,
                self.wallet_address,
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
                let on_chain = V2CreateWithNativeParams {
                    name: on_chain_name,
                    symbol: on_chain_symbol,
                    token_uri: prepared.metadata_uri.clone(),
                    creator_fee_rate: params.creator_fee_rate,
                    vaults: params.vaults.clone(),
                    salt: prepared.salt,
                    dex_type: params.dex_type,
                    buy_quote_amount: params.buy_quote_amount,
                    // msg.value is always equal to buy_quote_amount for
                    // native payments — no separate knob (Codex P1 #4).
                    native_value: params.buy_quote_amount,
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
        let receipt = wait_for_receipt(&self.provider, tx_hash, Duration::from_secs(120))
            .await
            .with_context(|| format!("create_token_v2: waiting for receipt of {tx_hash}"))?;
        if !receipt.status() {
            return Err(anyhow::anyhow!(
                "create_token_v2: transaction reverted ({tx_hash})"
            ));
        }

        let create_sig = IBondingCurveV2Events::Create::SIGNATURE_HASH;
        let create_log_opt = receipt
            .logs()
            .iter()
            .find(|l| l.topic0() == Some(&create_sig));

        if let Some(rpc_log) = create_log_opt {
            let decoded = IBondingCurveV2Events::Create::decode_log(&rpc_log.inner)
                .with_context(|| "create_token_v2: failed to decode on-chain Create event")?;
            let on_chain_token = decoded.data.token;
            if on_chain_token != prepared.token_address {
                return Err(anyhow::anyhow!(
                    "create_token_v2: predicted token {} does not match on-chain {} (tx {})",
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
                    "create_token_v2: predicted token {} not registered on-chain after tx {}",
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

    pub async fn buy_v2(&self, params: V2BuyParams) -> Result<B256> {
        self.v2()?.router.buy(params).await
    }

    pub async fn buy_with_native_v2(&self, params: V2BuyWithNativeParams) -> Result<B256> {
        self.v2()?.router.buy_with_native(params).await
    }

    pub async fn buy_with_permit_v2(&self, params: V2BuyWithPermitParams) -> Result<B256> {
        self.v2()?.router.buy_with_permit(params).await
    }

    pub async fn sell_v2(&self, params: V2SellParams) -> Result<B256> {
        self.v2()?.router.sell(params).await
    }

    pub async fn sell_to_native_v2(&self, params: V2SellToNativeParams) -> Result<B256> {
        self.v2()?.router.sell_to_native(params).await
    }

    pub async fn sell_with_permit_v2(&self, params: V2SellWithPermitParams) -> Result<B256> {
        self.v2()?.router.sell_with_permit(params).await
    }

    pub async fn sell_to_native_with_permit_v2(
        &self,
        params: V2SellToNativeWithPermitParams,
    ) -> Result<B256> {
        self.v2()?.router.sell_to_native_with_permit(params).await
    }

    // ========================================================================
    // v2: trading (exact-out)
    // ========================================================================

    pub async fn exact_out_buy_v2(&self, params: V2ExactOutBuyParams) -> Result<B256> {
        self.v2()?.router.exact_out_buy(params).await
    }

    pub async fn exact_out_buy_with_native_v2(
        &self,
        params: V2ExactOutBuyWithNativeParams,
    ) -> Result<B256> {
        self.v2()?.router.exact_out_buy_with_native(params).await
    }

    pub async fn exact_out_sell_v2(&self, params: V2ExactOutSellParams) -> Result<B256> {
        self.v2()?.router.exact_out_sell(params).await
    }

    pub async fn exact_out_sell_to_native_v2(
        &self,
        params: V2ExactOutSellToNativeParams,
    ) -> Result<B256> {
        self.v2()?.router.exact_out_sell_to_native(params).await
    }

    // ========================================================================
    // v2: quotes
    // ========================================================================

    /// Auto-routed v2 quote (bonding curve pre-graduation, DEX after).
    pub async fn quote_v2(&self, token: Address, amount_in: U256, is_buy: bool) -> Result<U256> {
        self.v2()?
            .router
            .get_amount_out(token, amount_in, is_buy)
            .await
    }

    /// Auto-routed inverse v2 quote.
    pub async fn quote_in_v2(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.v2()?
            .router
            .get_amount_in(token, amount_out, is_buy)
            .await
    }

    /// v2 bonding-curve-only quote (errors if graduated).
    pub async fn quote_bonding_curve_v2(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.v2()?
            .router
            .get_bonding_curve_amount_out(token, amount_in, is_buy)
            .await
    }

    /// Inverse v2 bonding-curve-only quote.
    pub async fn quote_bonding_curve_in_v2(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.v2()?
            .router
            .get_bonding_curve_amount_in(token, amount_out, is_buy)
            .await
    }

    /// v2 DEX-only quote (errors if not graduated).
    pub async fn quote_dex_v2(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.v2()?
            .router
            .get_dex_amount_out(token, amount_in, is_buy)
            .await
    }

    /// Inverse v2 DEX-only quote.
    pub async fn quote_dex_in_v2(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.v2()?
            .router
            .get_dex_amount_in(token, amount_out, is_buy)
            .await
    }

    // ========================================================================
    // v2: token / pool queries
    // ========================================================================

    /// Whether the v2 token has graduated from bonding curve to DEX.
    pub async fn is_graduated_v2(&self, token: Address) -> Result<bool> {
        self.v2()?.router.is_graduated(token).await
    }

    /// NadFunPair address for a v2 token (via `TokenRegistry::getPair`).
    /// Returns `Address::ZERO` if the token isn't registered on v2.
    pub async fn pool_address_v2(&self, token: Address) -> Result<Address> {
        self.v2()?.token_registry.get_pair(token).await
    }

    /// Wrapped native (WMON) address known to the v2 router.
    pub async fn wrapped_native_v2(&self) -> Result<Address> {
        self.v2()?.router.wrapped_native().await
    }

    /// Estimate gas for any v2 trade or create op. Uses
    /// `self.wallet_address` as the `from` so allowance / balance checks
    /// succeed. Errors when `self.wallet_address` is `Address::ZERO`
    /// (Codex P2 #9) — a read-only `Core` built via
    /// `Core::with_provider(_, Address::ZERO, _)` cannot estimate gas.
    pub async fn estimate_gas_v2(&self, params: V2GasEstimationParams) -> Result<u64> {
        if self.wallet_address == Address::ZERO {
            return Err(anyhow::anyhow!(
                "estimate_gas_v2: wallet_address is Address::ZERO; \
                 construct Core with a real signer to estimate gas"
            ));
        }
        self.v2()?
            .router
            .estimate_gas(params, self.wallet_address)
            .await
    }

    // ========================================================================
    // Escape hatches: direct access to underlying contract bindings.
    // ========================================================================

    pub fn bonding_curve_router(&self) -> &BondingCurveRouter<DynProvider> {
        &self.bonding_curve_router
    }

    pub fn dex_router(&self) -> &DexRouter<DynProvider> {
        &self.dex_router
    }

    pub fn lens(&self) -> &Lens<DynProvider> {
        &self.lens
    }

    pub fn router_v2(&self) -> Result<&NadFunRouter<DynProvider>> {
        Ok(&self.v2()?.router)
    }

    pub fn factory_v2(&self) -> Result<&NadFunFactory<DynProvider>> {
        Ok(&self.v2()?.factory)
    }

    pub fn bonding_curve_v2(&self) -> Result<&BondingCurveV2<DynProvider>> {
        Ok(&self.v2()?.bonding_curve)
    }

    pub fn token_registry_v2(&self) -> Result<&TokenRegistryV2<DynProvider>> {
        Ok(&self.v2()?.token_registry)
    }

    pub fn provider(&self) -> &Arc<DynProvider> {
        &self.provider
    }

    pub fn wallet_address(&self) -> Address {
        self.wallet_address
    }

    pub fn network(&self) -> Network {
        self.network
    }
}

/// Build v2 bindings for `network` if v2 is configured there. Returns
/// `Ok(None)` (NOT an error) when v2 isn't deployed — that's a normal
/// state on networks without a v2 release.
fn build_v2_bindings(provider: &Arc<DynProvider>, network: Network) -> Result<Option<V2Bindings>> {
    let (Some(router_s), Some(factory_s), Some(bc_s), Some(reg_s)) = (
        get_nadfun_router_v2(network),
        get_nadfun_factory_v2(network),
        get_bonding_curve_v2(network),
        get_token_registry_v2(network),
    ) else {
        return Ok(None);
    };

    let router_addr: Address = router_s
        .parse()
        .with_context(|| format!("invalid NadFunRouter address {router_s:?} for {network:?}"))?;
    let factory_addr: Address = factory_s
        .parse()
        .with_context(|| format!("invalid NadFunFactory address {factory_s:?} for {network:?}"))?;
    let bc_addr: Address = bc_s
        .parse()
        .with_context(|| format!("invalid BondingCurveV2 address {bc_s:?} for {network:?}"))?;
    let reg_addr: Address = reg_s
        .parse()
        .with_context(|| format!("invalid TokenRegistryV2 address {reg_s:?} for {network:?}"))?;

    Ok(Some(V2Bindings {
        router: NadFunRouter::new(router_addr, provider.clone()),
        factory: NadFunFactory::new(factory_addr, provider.clone()),
        bonding_curve: BondingCurveV2::new(bc_addr, provider.clone()),
        token_registry: TokenRegistryV2::new(reg_addr, provider.clone()),
    }))
}

/// Poll for a transaction receipt until `tx_hash` lands or `max_wait`
/// elapses. Returns the receipt or an error on timeout / RPC failure.
///
/// `provider.get_transaction_receipt` returns `Ok(None)` while the tx is
/// still pending; without polling, that becomes a confusing "receipt not
/// found" right after a successful broadcast.
async fn wait_for_receipt(
    provider: &Arc<DynProvider>,
    tx_hash: B256,
    max_wait: Duration,
) -> Result<alloy::rpc::types::TransactionReceipt> {
    let poll_interval = Duration::from_millis(500);
    let deadline = tokio::time::Instant::now() + max_wait;
    loop {
        match provider.get_transaction_receipt(tx_hash).await? {
            Some(receipt) => return Ok(receipt),
            None => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(anyhow::anyhow!(
                        "timed out after {:?} waiting for receipt {tx_hash}",
                        max_wait,
                    ));
                }
                tokio::time::sleep(poll_interval).await;
            }
        }
    }
}
