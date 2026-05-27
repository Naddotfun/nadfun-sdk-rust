//! High-level SDK client for NadFun contract v2.
//!
//! v2 uses a single `NadFunRouter` for token creation, quotes, native +
//! ERC-20 quote-token trading, permit flows, and exact-output swaps. The
//! supporting `NadFunFactory`, `BondingCurveV2`, and `TokenRegistryV2`
//! bindings are wired alongside it for pool discovery, raw curve quoting,
//! and on-chain v2 token detection.
//!
//! v1 callers use [`crate::Core`] — completely independent.

use crate::{
    api::ApiClient,
    constants::{
        get_bonding_curve_v2, get_nadfun_factory_v2, get_nadfun_router_v2, get_token_registry_v2,
        set_network, Network,
    },
    contracts::{BondingCurveV2, NadFunFactory, NadFunRouter, TokenRegistryV2},
    types::*,
};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use std::sync::Arc;

/// High-level SDK client for NadFun contract v2.
pub struct CoreV2 {
    router: NadFunRouter<DynProvider>,
    factory: NadFunFactory<DynProvider>,
    bonding_curve: BondingCurveV2<DynProvider>,
    token_registry: TokenRegistryV2<DynProvider>,
    provider: Arc<DynProvider>,
    wallet_address: Address,
    network: Network,
}

impl CoreV2 {
    /// Construct a `CoreV2` from RPC URL + private key + network.
    ///
    /// Sets the global network configuration (`set_network`) and resolves
    /// all v2 contract addresses for that network. Returns an error if the
    /// network does not have v2 configured (see [`get_nadfun_router_v2`]).
    pub async fn new(rpc_url: String, private_key: String, network: Network) -> Result<Self> {
        set_network(network);

        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();
        let wallet = EthereumWallet::from(signer);
        let url = rpc_url.parse()?;
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);
        let dyn_provider = Arc::new(DynProvider::new(provider));

        Self::with_provider(dyn_provider, wallet_address, network)
    }

    /// Construct a `CoreV2` from an existing provider + wallet address.
    ///
    /// Use this when you want to share the underlying RPC provider (and thus
    /// nonce state) between [`crate::Core`] and `CoreV2` in a process that
    /// trades both v1 and v2 tokens. Caller is responsible for ensuring the
    /// provider's signer matches `wallet_address`.
    pub fn with_provider(
        provider: Arc<DynProvider>,
        wallet_address: Address,
        network: Network,
    ) -> Result<Self> {
        set_network(network);

        let router_address = parse_v2_addr(
            get_nadfun_router_v2(),
            network,
            "NadFunRouter",
        )?;
        let factory_address = parse_v2_addr(
            get_nadfun_factory_v2(),
            network,
            "NadFunFactory",
        )?;
        let bonding_curve_address = parse_v2_addr(
            get_bonding_curve_v2(),
            network,
            "BondingCurveV2",
        )?;
        let token_registry_address = parse_v2_addr(
            get_token_registry_v2(),
            network,
            "TokenRegistryV2",
        )?;

        let router = NadFunRouter::new(router_address, provider.clone());
        let factory = NadFunFactory::new(factory_address, provider.clone());
        let bonding_curve = BondingCurveV2::new(bonding_curve_address, provider.clone());
        let token_registry = TokenRegistryV2::new(token_registry_address, provider.clone());

        Ok(Self {
            router,
            factory,
            bonding_curve,
            token_registry,
            provider,
            wallet_address,
            network,
        })
    }

    // === Token creation ===

    /// Low-level: deploy a v2 token with an ERC-20 quote token + initial buy.
    /// Caller must have already approved the router for `buy_quote_amount`
    /// of `quote_token`. Use [`CoreV2::create_token`] for the high-level
    /// flow that orchestrates off-chain metadata + salt mining.
    pub async fn create(&self, params: V2CreateParams) -> Result<B256> {
        self.router.create(params).await
    }

    /// Low-level: deploy a v2 token funded by native MON (`msg.value`).
    pub async fn create_with_native(&self, params: V2CreateWithNativeParams) -> Result<B256> {
        self.router.create_with_native(params).await
    }

    /// High-level: end-to-end v2 token creation.
    ///
    /// Orchestrates the full flow:
    ///   1. Off-chain via `ApiClient`: upload image to IPFS, post metadata,
    ///      and mine a CREATE2 salt with `version: "V2"`.
    ///   2. On-chain: dispatch to `NadFunRouter::create` (when `payment` is
    ///      [`V2CreatePayment::Erc20`]) or `NadFunRouter::createWithNative`
    ///      (when `payment` is [`V2CreatePayment::Native`]).
    ///
    /// Returns the predicted token address (matches the salt-server output)
    /// along with the metadata + image URIs, the salt, the transaction
    /// hash, and the NSFW flag from server detection.
    ///
    /// Requires that the caller has approved the router for `buy_quote_amount`
    /// of `quote_token` when using `V2CreatePayment::Erc20`.
    pub async fn create_token(
        &self,
        params: V2CreateTokenParams,
        api: &ApiClient,
    ) -> Result<V2TokenCreationResult> {
        // Off-chain: image + metadata + salt mining.
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

        // On-chain: dispatch by payment mode. We thread the prepared
        // metadata_uri (not the raw image_uri) into the contract's tokenURI
        // field; the prepared name/symbol may differ from the user's input
        // if the server normalized them.
        let tx_hash = match params.payment {
            V2CreatePayment::Native { value } => {
                let on_chain = V2CreateWithNativeParams {
                    name: params.name.clone(),
                    symbol: params.symbol.clone(),
                    token_uri: prepared.metadata_uri.clone(),
                    creator_fee_rate: params.creator_fee_rate,
                    vaults: params.vaults.clone(),
                    salt: prepared.salt,
                    dex_type: params.dex_type,
                    buy_quote_amount: params.buy_quote_amount,
                    native_value: value,
                    deadline: params.deadline,
                    gas_limit: params.gas_limit,
                    gas_price: params.gas_price.clone(),
                    nonce: params.nonce,
                };
                self.router.create_with_native(on_chain).await?
            }
            V2CreatePayment::Erc20 { quote_token } => {
                let on_chain = V2CreateParams {
                    name: params.name.clone(),
                    symbol: params.symbol.clone(),
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
                self.router.create(on_chain).await?
            }
        };

        Ok(V2TokenCreationResult {
            token_address: prepared.token_address,
            metadata_uri: prepared.metadata_uri,
            image_uri: prepared.image_uri,
            salt: prepared.salt,
            transaction_hash: tx_hash,
            is_nsfw: prepared.is_nsfw,
        })
    }

    // === Trading: exact-in ===

    pub async fn buy(&self, params: V2BuyParams) -> Result<B256> {
        self.router.buy(params).await
    }

    pub async fn buy_with_native(
        &self,
        params: V2BuyWithNativeParams,
        value: U256,
    ) -> Result<B256> {
        self.router.buy_with_native(params, value).await
    }

    pub async fn buy_with_permit(&self, params: V2BuyWithPermitParams) -> Result<B256> {
        self.router.buy_with_permit(params).await
    }

    pub async fn sell(&self, params: V2SellParams) -> Result<B256> {
        self.router.sell(params).await
    }

    pub async fn sell_to_native(&self, params: V2SellToNativeParams) -> Result<B256> {
        self.router.sell_to_native(params).await
    }

    pub async fn sell_with_permit(&self, params: V2SellWithPermitParams) -> Result<B256> {
        self.router.sell_with_permit(params).await
    }

    pub async fn sell_to_native_with_permit(
        &self,
        params: V2SellToNativeWithPermitParams,
    ) -> Result<B256> {
        self.router.sell_to_native_with_permit(params).await
    }

    // === Trading: exact-out ===

    pub async fn exact_out_buy(&self, params: V2ExactOutBuyParams) -> Result<B256> {
        self.router.exact_out_buy(params).await
    }

    pub async fn exact_out_buy_with_native(
        &self,
        params: V2ExactOutBuyWithNativeParams,
    ) -> Result<B256> {
        self.router.exact_out_buy_with_native(params).await
    }

    pub async fn exact_out_sell(&self, params: V2ExactOutSellParams) -> Result<B256> {
        self.router.exact_out_sell(params).await
    }

    pub async fn exact_out_sell_to_native(
        &self,
        params: V2ExactOutSellToNativeParams,
    ) -> Result<B256> {
        self.router.exact_out_sell_to_native(params).await
    }

    // === Quotes ===

    /// Quote auto-routed (bonding curve pre-graduation, DEX post-graduation).
    pub async fn quote(&self, token: Address, amount_in: U256, is_buy: bool) -> Result<U256> {
        self.router.get_amount_out(token, amount_in, is_buy).await
    }

    /// Inverse quote — how much input you need to receive `amount_out`.
    pub async fn quote_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router.get_amount_in(token, amount_out, is_buy).await
    }

    /// Quote forced through the bonding curve (errors if graduated).
    pub async fn quote_bonding_curve(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router
            .get_bonding_curve_amount_out(token, amount_in, is_buy)
            .await
    }

    /// Inverse bonding-curve quote.
    pub async fn quote_bonding_curve_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router
            .get_bonding_curve_amount_in(token, amount_out, is_buy)
            .await
    }

    /// Quote forced through the DEX (errors if not yet graduated).
    pub async fn quote_dex(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router.get_dex_amount_out(token, amount_in, is_buy).await
    }

    /// Inverse DEX quote.
    pub async fn quote_dex_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router.get_dex_amount_in(token, amount_out, is_buy).await
    }

    // === Token / pool queries ===

    /// Whether the token has graduated from the bonding curve to the DEX.
    pub async fn is_graduated(&self, token: Address) -> Result<bool> {
        self.router.is_graduated(token).await
    }

    /// NadFunPair address for a v2 token (via `TokenRegistry::getPair`).
    /// Returns `Address::ZERO` if the token is not registered (e.g. it's a v1
    /// token or was never deployed through the v2 router).
    pub async fn pool_address(&self, token: Address) -> Result<Address> {
        self.token_registry.get_pair(token).await
    }

    /// Wrapped native (WMON) address known to the router.
    pub async fn wrapped_native(&self) -> Result<Address> {
        self.router.wrapped_native().await
    }

    // === Gas estimation ===

    /// Estimate gas for any v2 trade or creation operation. Uses `self.wallet_address`
    /// as the `from` address so allowance / balance checks succeed.
    pub async fn estimate_gas(&self, params: V2GasEstimationParams) -> Result<u64> {
        self.router
            .estimate_gas(params, self.wallet_address)
            .await
    }

    // === Receipts ===

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

    // === Escape hatches: direct access to the underlying alloy contract bindings ===

    /// `NadFunRouter` binding — entry point for all trading/creation methods.
    pub fn router(&self) -> &NadFunRouter<DynProvider> {
        &self.router
    }

    /// `NadFunFactory` binding — pair lookup + pair count + protocol manager view.
    pub fn factory(&self) -> &NadFunFactory<DynProvider> {
        &self.factory
    }

    /// `BondingCurveV2` binding — raw curve quoting + sniping penalty view.
    pub fn bonding_curve(&self) -> &BondingCurveV2<DynProvider> {
        &self.bonding_curve
    }

    /// `TokenRegistryV2` binding — pair lookup, quote-token lookup, v2 detection.
    pub fn token_registry(&self) -> &TokenRegistryV2<DynProvider> {
        &self.token_registry
    }

    /// Shared alloy provider (RPC).
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

/// Parse a v2 contract address with a clear error when v2 is not configured
/// for the current network.
fn parse_v2_addr(addr: Option<&'static str>, network: Network, name: &str) -> Result<Address> {
    let s = addr.ok_or_else(|| {
        anyhow::anyhow!(
            "NadFun contract v2 ({}) is not configured for {:?}",
            name,
            network
        )
    })?;
    s.parse()
        .with_context(|| format!("invalid {} address {:?} for {:?}", name, s, network))
}
