//! Unified `Core` — one entry point for v1 + v2 bonding-curve trading,
//! token creation, and pool discovery on Nad.fun.
//!
//! A single `Core` instance binds to a `Network` and wires both the v1
//! (`BondingCurveRouter` + `DexRouter` + `Lens`) and v2 (`NadFunRouter` +
//! `NadFunFactory` + `BondingCurveV2` + `TokenRegistryV2`) contract
//! surfaces. v1 trades are accessed via `core.v1()` ([`CoreV1`] handle,
//! e.g. `core.v1().buy(...)`); v2 trades via `core.v2()` ([`CoreV2`]
//! handle, e.g. `core.v2().buy(...)`; the `_v2` suffix is dropped).
//!
//! Use `Core::detect_version(token)` (or `detect_token_info` for the quote
//! token too) to classify a token via the on-chain `TokenInfoLens` in one
//! RPC call, then dispatch to the right v1/v2 surface. Stateless — no
//! caching; cache results yourself if you need to.

use crate::{
    constants::*,
    contracts::{
        BondingCurveRouter, BondingCurveV2, DexRouter, Lens, NadFunFactory, NadFunRouter,
        ProtocolManagerV2, TokenInfoLens, TokenRegistryV2,
    },
    core::v1::CoreV1,
    core::v2::CoreV2,
    types::*,
    version::{SdkVersion, TokenInfo},
};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use std::{sync::Arc, time::Duration};

/// Unified high-level SDK client. Handles v1 + v2 trading, token creation,
/// quote routing, and creator reward claims from a single instance.
///
/// Internally splits contract bindings into per-version structs
/// ([`V1Contracts`], [`V2Contracts`]) so the v1 / v2 surface is visibly
/// separated in the type. Both are always wired — every supported
/// `Network` ships with both deployments. If a future network skips one,
/// `Core::new` will fail loudly during address resolution instead of
/// carrying a dead branch.
pub struct Core {
    pub(crate) v1: V1Contracts,
    pub(crate) v2: V2Contracts,
    pub(crate) provider: Arc<DynProvider>,
    pub(crate) wallet_address: Address,
    pub(crate) network: Network,
}

/// v1 contract bindings — bonding-curve router, DEX (Capricorn CL) router,
/// and the Lens used for auto-routing quotes.
pub(crate) struct V1Contracts {
    pub(crate) bonding_curve_router: BondingCurveRouter<DynProvider>,
    pub(crate) dex_router: DexRouter<DynProvider>,
    pub(crate) lens: Lens<DynProvider>,
}

/// v2 contract bindings — NadFunRouter + factory + bonding curve +
/// per-token registry + `TokenInfoLens`. The Lens is required: it's deployed
/// on every supported network, so `Core::new` resolves it at construction
/// (failing loudly if a future network ships without it) rather than
/// carrying a fallback branch.
pub(crate) struct V2Contracts {
    pub(crate) router: NadFunRouter<DynProvider>,
    pub(crate) factory: NadFunFactory<DynProvider>,
    pub(crate) bonding_curve: BondingCurveV2<DynProvider>,
    pub(crate) token_registry: TokenRegistryV2<DynProvider>,
    pub(crate) token_info_lens: TokenInfoLens<DynProvider>,
    pub(crate) protocol_manager: ProtocolManagerV2<DynProvider>,
}

impl Core {
    /// Create a new `Core` from RPC URL + private key + network.
    ///
    /// Wires both v1 and v2 contract bindings for `network`. Errors if the
    /// network does not have v2 configured (currently impossible — both
    /// `Network::Mainnet` and `Network::Testnet` ship with v2).
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
        let v1 = build_v1_contracts(&provider, network)?;
        let v2 = build_v2_contracts(&provider, network)?;

        Ok(Self {
            v1,
            v2,
            provider,
            wallet_address,
            network,
        })
    }

    /// v1 namespace handle (bonding curve + Capricorn CL DEX). Zero-cost —
    /// borrows `&self`.
    pub fn v1(&self) -> CoreV1<'_> {
        CoreV1 { core: self }
    }

    /// v2 namespace handle (NadFunRouter + registry + vaults). Zero-cost —
    /// borrows `&self`.
    pub fn v2(&self) -> CoreV2<'_> {
        CoreV2 { core: self }
    }

    // ========================================================================
    // v1 + v2: dispatch primitive
    // ========================================================================

    /// Classify a token as v1 / v2 / not-registered. Stateless — each
    /// call hits the chain via the on-chain `TokenInfoLens` (one RPC
    /// covers both v1 and v2 registries and returns `None` for unknown
    /// tokens).
    ///
    /// Callers that issue many lookups for the same token should cache
    /// the result themselves — the SDK is intentionally stateless.
    pub async fn detect_version(&self, token: Address) -> Result<SdkVersion> {
        Ok(self.detect_token_info(token).await?.version)
    }

    /// Batch version detection — one `TokenInfoLens` RPC classifies the
    /// whole list. Order matches the input.
    pub async fn detect_versions(&self, tokens: Vec<Address>) -> Result<Vec<SdkVersion>> {
        Ok(self
            .detect_token_infos(tokens)
            .await?
            .into_iter()
            .map(|info| info.version)
            .collect())
    }

    /// Classify a token and resolve its on-chain `quote_token` in a single
    /// `TokenInfoLens` call. See [`TokenInfo`].
    ///
    /// The SDK does not pick a trade method from the quote token — that is
    /// the caller's decision (see the quote-routing matrix in `MIGRATION.md`).
    pub async fn detect_token_info(&self, token: Address) -> Result<TokenInfo> {
        self.v2.token_info_lens.get_token_info(token).await
    }

    /// Batch [`Self::detect_token_info`] — one RPC call for the whole list,
    /// order preserved. Empty input returns an empty vec without an RPC.
    pub async fn detect_token_infos(&self, tokens: Vec<Address>) -> Result<Vec<TokenInfo>> {
        self.v2.token_info_lens.get_token_infos(tokens).await
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
    // Escape hatches: direct access to underlying contract bindings.
    // v1 escape hatches are on CoreV1: use core.v1().bonding_curve_router() etc.
    // ========================================================================

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

/// Build v1 contract bindings for `network`. Parses the v1 addresses and
/// constructs the BondingCurveRouter, DexRouter, and Lens wrappers.
fn build_v1_contracts(provider: &Arc<DynProvider>, network: Network) -> Result<V1Contracts> {
    let lens_address: Address = get_lens_address(network).parse()?;
    let bonding_curve_router_address: Address = get_bonding_curve_router(network).parse()?;
    let dex_router_address: Address = get_dex_router(network).parse()?;
    let bonding_curve_address: Address = get_bonding_curve(network).parse()?;

    Ok(V1Contracts {
        bonding_curve_router: BondingCurveRouter::new(
            bonding_curve_router_address,
            bonding_curve_address,
            provider.clone(),
        ),
        dex_router: DexRouter::new(dex_router_address, provider.clone()),
        lens: Lens::new(lens_address, provider.clone()),
    })
}

/// Build v2 contract bindings for `network`. Errors if any v2 address
/// helper returns `None` (no v2 deployment) or fails to parse. Currently
/// every supported `Network` variant has v2 wired, so this only fires
/// for genuinely misconfigured deployments.
fn build_v2_contracts(provider: &Arc<DynProvider>, network: Network) -> Result<V2Contracts> {
    let router_s = get_nadfun_router_v2(network)
        .ok_or_else(|| anyhow::anyhow!("NadFunRouter v2 not configured for {network:?}"))?;
    let factory_s = get_nadfun_factory_v2(network)
        .ok_or_else(|| anyhow::anyhow!("NadFunFactory v2 not configured for {network:?}"))?;
    let bc_s = get_bonding_curve_v2(network)
        .ok_or_else(|| anyhow::anyhow!("BondingCurve v2 not configured for {network:?}"))?;
    let reg_s = get_token_registry_v2(network)
        .ok_or_else(|| anyhow::anyhow!("TokenRegistry v2 not configured for {network:?}"))?;

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

    // TokenInfoLens is required — it's deployed on every supported network.
    let lens_s = get_token_info_lens(network)
        .ok_or_else(|| anyhow::anyhow!("TokenInfoLens not configured for {network:?}"))?;
    let lens_addr: Address = lens_s
        .parse()
        .with_context(|| format!("invalid TokenInfoLens address {lens_s:?} for {network:?}"))?;
    let token_info_lens = TokenInfoLens::new(lens_addr, provider.clone());

    let pm_s = get_protocol_manager_v2(network)
        .ok_or_else(|| anyhow::anyhow!("ProtocolManager v2 not configured for {network:?}"))?;
    let pm_addr: Address = pm_s
        .parse()
        .with_context(|| format!("invalid ProtocolManager address {pm_s:?} for {network:?}"))?;
    let protocol_manager = ProtocolManagerV2::new(pm_addr, provider.clone());

    Ok(V2Contracts {
        router: NadFunRouter::new(router_addr, provider.clone()),
        factory: NadFunFactory::new(factory_addr, provider.clone()),
        bonding_curve: BondingCurveV2::new(bc_addr, provider.clone()),
        token_registry: TokenRegistryV2::new(reg_addr, provider.clone()),
        token_info_lens,
        protocol_manager,
    })
}

/// Poll for a transaction receipt until `tx_hash` lands or `max_wait`
/// elapses. Returns the receipt or an error on timeout / RPC failure.
///
/// `provider.get_transaction_receipt` returns `Ok(None)` while the tx is
/// still pending; without polling, that becomes a confusing "receipt not
/// found" right after a successful broadcast.
pub(crate) async fn wait_for_receipt(
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
