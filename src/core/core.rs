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
    api::ApiClient,
    constants::*,
    contracts::{
        BondingCurveRouter, BondingCurveV2, DexRouter, Lens, NadFunFactory, NadFunRouter,
        ProtocolManagerV2, TokenInfoLens, TokenRegistryV2,
    },
    core::nonce::accepted_sync_hash,
    core::v1::CoreV1,
    core::v2::CoreV2,
    types::*,
    version::{SdkVersion, TokenInfo},
};
use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use std::{sync::Arc, time::Duration};

/// Settings for [`Core::wait_for_receipt`].
///
/// The default polls every 350 ms for up to 120 seconds and does not wait for finality.
#[derive(Debug, Clone, Copy)]
pub struct ReceiptWaitOptions {
    /// Delay between receipt or finality checks. Must be non-zero.
    pub poll_interval: Duration,
    /// One deadline shared by receipt polling and optional finality polling. Must be non-zero.
    pub timeout: Duration,
    /// Wait until the receipt's block is at or below the chain's finalized block, then re-fetch it.
    pub finalized: bool,
}

impl Default for ReceiptWaitOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(350),
            timeout: Duration::from_secs(120),
            finalized: false,
        }
    }
}

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
/// per-token registry + `TokenInfoLens`. Every network has a lens address
/// configured (the mainnet one is not a lens; see
/// `Core::detect_token_info_with_api`), so `Core::new` resolves it at
/// construction rather than carrying a fallback branch.
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
    /// Its signing provider adds a generated access list to unsigned sends and
    /// estimates when the RPC supports `eth_createAccessList`.
    ///
    /// Wires both v1 and v2 contract bindings for `network`. Errors if the
    /// network does not have v2 configured (currently impossible — both
    /// `Network::Mainnet` and `Network::Testnet` ship with v2).
    pub async fn new(rpc_url: String, private_key: String, network: Network) -> Result<Self> {
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();

        let provider = Arc::new(crate::core::nonce::signing_provider(&rpc_url, signer)?);

        Self::with_provider(provider, wallet_address, network)
    }

    /// Construct a `Core` from an existing provider + wallet address.
    ///
    /// Use this when you want to share an RPC provider (and nonce state)
    /// across multiple `Core` instances — for example, two `Core`s pointing
    /// at different networks but sharing a connection pool. Caller is
    /// responsible for ensuring the provider's signer matches
    /// `wallet_address`.
    ///
    /// The provider is used as-is, so the caller owns its nonce handling.
    /// [`Core::new`] builds a provider with SDK nonce handling (one shared state
    /// per chain + account, sends serialized per account, no nonce taken by a
    /// gas-estimation revert or a node rejection); a provider from alloy's
    /// `ProviderBuilder::new().wallet(..)` does not have it. For this
    /// constructor, access-list generation and gas estimation are controlled
    /// by the supplied provider.
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
    ///
    /// **Mainnet:** the shipped `TOKEN_INFO_LENS` address is not a
    /// `TokenInfoLens` (another contract lives there), so this errors there;
    /// use [`Self::detect_token_info_with_api`].
    pub async fn detect_version(&self, token: Address) -> Result<SdkVersion> {
        Ok(self.detect_token_info(token).await?.version)
    }

    /// Batch version detection — one `TokenInfoLens` RPC classifies the
    /// whole list. Order matches the input.
    ///
    /// **Mainnet:** the shipped `TOKEN_INFO_LENS` address is not a
    /// `TokenInfoLens` (another contract lives there), so this errors there;
    /// use [`Self::detect_token_info_with_api`].
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
    ///
    /// **Mainnet:** the shipped `TOKEN_INFO_LENS` address is not a
    /// `TokenInfoLens` (another contract lives there), so this errors there;
    /// use [`Self::detect_token_info_with_api`].
    pub async fn detect_token_info(&self, token: Address) -> Result<TokenInfo> {
        self.v2.token_info_lens.get_token_info(token).await
    }

    /// Batch [`Self::detect_token_info`] — one RPC call for the whole list,
    /// order preserved. Empty input returns an empty vec without an RPC.
    ///
    /// **Mainnet:** the shipped `TOKEN_INFO_LENS` address is not a
    /// `TokenInfoLens` (another contract lives there), so this errors there;
    /// use [`Self::detect_token_info_with_api`].
    pub async fn detect_token_infos(&self, tokens: Vec<Address>) -> Result<Vec<TokenInfo>> {
        self.v2.token_info_lens.get_token_infos(tokens).await
    }

    /// Classify a token from the v2 `TokenRegistry` plus the api-server
    /// (`GET /token/{token}`) instead of the `TokenInfoLens`. Use this on
    /// mainnet, where the shipped `TOKEN_INFO_LENS` address holds a different
    /// contract and lens calls revert.
    ///
    /// The chain is read first: a registry entry means V2 with that quote
    /// token, whatever the API says. Only an unregistered token is looked up
    /// in the API: V1 -> the network's WMON; the api-server's 404 error body
    /// -> `SdkVersion::None` with `Address::ZERO` (lens semantics). An API that
    /// claims V2 for an unregistered token, any other API error (including a
    /// 404 without the api-server error body), or an `api` bound to a
    /// different network than this `Core` is an error.
    pub async fn detect_token_info_with_api(
        &self,
        api: &ApiClient,
        token: Address,
    ) -> Result<TokenInfo> {
        if api.network() != self.network {
            anyhow::bail!(
                "detect_token_info_with_api: ApiClient is bound to {:?} but Core is on {:?}",
                api.network(),
                self.network,
            );
        }
        // Chain first: a v2 registry entry is authoritative (covers tokens the
        // API has not indexed yet and an API that misreports the version).
        let quote = self.v2.token_registry.get_quote_token(token).await?;
        if quote != Address::ZERO {
            return Ok(TokenInfo {
                version: SdkVersion::V2,
                quote_token: quote,
            });
        }
        let Some(api_info) = api.get_token_opt(token).await? else {
            return Ok(TokenInfo {
                version: SdkVersion::None,
                quote_token: Address::ZERO,
            });
        };
        let quote_token = match api_info.version {
            SdkVersion::V1 => crate::constants::get_wmon(self.network).parse()?,
            SdkVersion::V2 => anyhow::bail!(
                "detect_token_info_with_api: API reports {token} as V2 but the v2 \
                 TokenRegistry has no entry for it"
            ),
            SdkVersion::None => Address::ZERO,
        };
        Ok(TokenInfo {
            version: api_info.version,
            quote_token,
        })
    }

    /// Version-only [`Self::detect_token_info_with_api`].
    pub async fn detect_version_with_api(
        &self,
        api: &ApiClient,
        token: Address,
    ) -> Result<SdkVersion> {
        Ok(self.detect_token_info_with_api(api, token).await?.version)
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

    /// Wait for `tx_hash` to be mined, verify that it succeeded, and optionally wait for finality.
    ///
    /// A reverted transaction returns an error containing its hash and block. With
    /// `opts.finalized`, this compares the receipt block against the RPC's `finalized` block and
    /// re-fetches the receipt before returning, so a displaced receipt is not reported as final.
    /// One deadline covers both polling phases. Receipt RPC errors fail immediately;
    /// `get_receipt` remains a single lookup.
    pub async fn wait_for_receipt(
        &self,
        tx_hash: B256,
        opts: ReceiptWaitOptions,
    ) -> Result<TransactionResult> {
        if opts.poll_interval.is_zero() {
            anyhow::bail!("receipt poll interval must be non-zero");
        }
        if opts.timeout.is_zero() {
            anyhow::bail!("receipt wait timeout must be non-zero");
        }

        let deadline = tokio::time::Instant::now()
            .checked_add(opts.timeout)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "receipt wait timeout {0:?} is too large for transaction {tx_hash}",
                    opts.timeout
                )
            })?;
        let receipt = wait_for_receipt_until(
            &self.provider,
            tx_hash,
            opts.poll_interval,
            deadline,
            opts.timeout,
            "receipt",
        )
        .await?;
        ensure_successful_receipt(tx_hash, &receipt)?;

        if !opts.finalized {
            return Ok(transaction_result(&receipt));
        }

        let receipt_block = receipt.block_number.ok_or_else(|| {
            anyhow::anyhow!("receipt for transaction {tx_hash} has no block number to finalize")
        })?;
        loop {
            let finalized_block = tokio::time::timeout_at(
                deadline,
                self.provider.get_block_number_by_id(BlockId::finalized()),
            )
            .await
            .map_err(|_| receipt_timeout(tx_hash, opts.timeout, "finality"))?
            .with_context(|| format!("failed to query finalized block for transaction {tx_hash}"))?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "finalized block RPC returned no block number for transaction {tx_hash}"
                )
            })?;

            if finalized_block >= receipt_block {
                break;
            }

            sleep_for_poll_interval(deadline, opts.poll_interval).await;
        }

        let finalized_receipt =
            tokio::time::timeout_at(deadline, self.provider.get_transaction_receipt(tx_hash))
                .await
                .map_err(|_| receipt_timeout(tx_hash, opts.timeout, "finality receipt re-check"))?
                .with_context(|| format!("failed to re-check finalized receipt {tx_hash}"))?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "receipt for transaction {tx_hash} disappeared during finality check"
                    )
                })?;

        if finalized_receipt.transaction_hash != tx_hash
            || finalized_receipt.transaction_hash != receipt.transaction_hash
            || finalized_receipt.block_number != receipt.block_number
            || finalized_receipt.block_hash != receipt.block_hash
        {
            anyhow::bail!(
                "receipt for transaction {tx_hash} changed during finality check: \
                 before tx {} block {:?} hash {:?}, after tx {} block {:?} hash {:?}",
                receipt.transaction_hash,
                receipt.block_number,
                receipt.block_hash,
                finalized_receipt.transaction_hash,
                finalized_receipt.block_number,
                finalized_receipt.block_hash,
            );
        }
        ensure_successful_receipt(tx_hash, &finalized_receipt)?;

        Ok(transaction_result(&finalized_receipt))
    }

    /// Submit `tx` with the provider's synchronous-send method and return its mined receipt.
    ///
    /// The receipt confirms inclusion and execution status, but is not necessarily final; pass its
    /// hash to [`Core::wait_for_receipt`] with
    /// `ReceiptWaitOptions { finalized: true, ..Default::default() }` when finality is required.
    /// Only a JSON-RPC code 4 error with a 32-byte hash in `data` (either a bare string or
    /// `{ "hash": "0x…" }`) is treated as an accepted sync send. It polls for a receipt without
    /// resubmitting and continues through transient receipt RPC errors. The SDK falls back to one
    /// ordinary send only when the sync method returns JSON-RPC `-32601`, then polls for a receipt
    /// for up to 120 seconds. The sync RPC may itself take up to 120 seconds; receipt polling after
    /// a fallback or accepted-hash response can add another 120 seconds.
    ///
    /// The `-32601` fallback signs and sends the request again. With a provider supplied through
    /// [`Core::with_provider`], a cached nonce manager can make that fallback stall; the SDK's
    /// [`Core::new`] provider uses `NonceGuard` instead. A gateway that reports `-32601` after it
    /// executed the sync request is not detected, so the fallback may resubmit the transaction.
    ///
    /// # Errors
    ///
    /// Returns RPC errors, reverted receipt errors, hash-mismatch errors, and receipt timeout
    /// errors. Sync errors other than `-32601` and pre-submission validation errors (including
    /// code 5) contain `outcome unknown; do not resend blindly` and any known hash. A timeout while
    /// polling a transaction already known to be submitted starts with the stable prefix
    /// `transaction submitted; outcome unknown` and includes the hash and last receipt RPC error.
    /// A fallback ordinary-send failure without a JSON-RPC error response also marks its outcome
    /// unknown.
    pub async fn send_transaction_sync(&self, tx: TransactionRequest) -> Result<TransactionResult> {
        let fallback_tx = tx.clone();
        let receipt = match self.provider.send_transaction_sync(tx).await {
            Ok(receipt) => receipt,
            Err(error) if error.error_code() == Some(-32601) => {
                let pending = match self.provider.send_transaction(fallback_tx).await {
                    Ok(pending) => pending,
                    Err(error) => {
                        let hash = error
                            .tx_hash_data()
                            .map(|hash| format!(" (transaction {hash})"))
                            .unwrap_or_default();
                        let outcome = if error.as_error_resp().is_none() {
                            "; outcome unknown; do not resend blindly"
                        } else {
                            ""
                        };
                        return Err(anyhow::anyhow!(
                            "sync send method was not found; ordinary send failed{outcome}{hash}: {error}"
                        ));
                    }
                };
                let hash = *pending.tx_hash();
                let options = ReceiptWaitOptions::default();
                let receipt = wait_for_submitted_receipt(
                    &self.provider,
                    hash,
                    options.poll_interval,
                    options.timeout,
                )
                .await?;
                ensure_successful_receipt(hash, &receipt)?;
                return Ok(transaction_result(&receipt));
            }
            Err(error) => {
                if let Some(hash) = accepted_sync_hash(&error) {
                    let options = ReceiptWaitOptions::default();
                    let receipt = wait_for_submitted_receipt(
                        &self.provider,
                        hash,
                        options.poll_interval,
                        options.timeout,
                    )
                    .await?;
                    ensure_successful_receipt(hash, &receipt)?;
                    return Ok(transaction_result(&receipt));
                }

                let hash = error
                    .tx_hash_data()
                    .map(|hash| format!(" (transaction {hash})"))
                    .unwrap_or_default();
                // Monad's code 5 is TransactionUnready from validate_and_decode_tx, before submission.
                let pre_submission = error
                    .as_error_resp()
                    .is_some_and(|payload| matches!(payload.code, 5 | -32600 | -32602));
                let outcome = if pre_submission {
                    String::new()
                } else {
                    "; outcome unknown; do not resend blindly".to_owned()
                };
                return Err(anyhow::anyhow!(
                    "sync transaction send failed{outcome}{hash}: {error}"
                ));
            }
        };

        let hash = receipt.transaction_hash;
        ensure_successful_receipt(hash, &receipt)?;
        Ok(transaction_result(&receipt))
    }

    /// Check that the wallet can cover a native `value` plus its maximum gas cost.
    ///
    /// Pass the same `value`, `gas_limit`, and fee cap used by the send that follows. For EIP-1559,
    /// use `max_fee_per_gas`; for `LegacyWithPrice`, use `gas_price`. The check reads the latest
    /// balance and does not reserve funds, so another transaction or provider-filled gas/fee values
    /// can change the balance or send cost before submission. If gas or fees are automatic, first
    /// choose a conservative explicit cap. No extra balance reserve is applied.
    pub async fn check_native_send_balance(
        &self,
        value: U256,
        gas_limit: u64,
        fee_cap_per_gas: u128,
    ) -> Result<()> {
        let gas_cost = U256::from(gas_limit).checked_mul(U256::from(fee_cap_per_gas));
        let required = gas_cost.and_then(|cost| value.checked_add(cost));
        let balance = self
            .provider
            .get_balance(self.wallet_address)
            .latest()
            .await
            .with_context(|| match required {
                Some(required) => format!(
                    "checking native send balance: required {required} wei, available balance unavailable"
                ),
                None => format!(
                    "checking native send balance: required amount overflows U256 for value {value} wei, \
                     gas limit {gas_limit}, and fee cap {fee_cap_per_gas} wei per gas; \
                     available balance unavailable"
                ),
            })?;

        let Some(gas_cost) = gas_cost else {
            anyhow::bail!(
                "native send balance requirement overflows U256 for value {value} wei, \
                 gas limit {gas_limit}, and fee cap {fee_cap_per_gas} wei per gas; \
                 available balance is {balance} wei"
            );
        };
        let Some(required) = required else {
            anyhow::bail!(
                "native send balance requirement overflows U256 for value {value} wei plus \
                 gas cost {gas_cost} wei; available balance is {balance} wei"
            );
        };
        if balance < required {
            anyhow::bail!(
                "insufficient native balance for send: required {required} wei \
                 (value {value} wei + gas cost {gas_cost} wei), available {balance} wei"
            );
        }

        Ok(())
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

/// Build v2 contract bindings for `network`. Errors only if a v2 address
/// constant fails to parse. v2 is deployed on every supported `Network`,
/// so the address helpers return `&str` directly (no `None` arm).
fn build_v2_contracts(provider: &Arc<DynProvider>, network: Network) -> Result<V2Contracts> {
    let router_s = get_nadfun_router_v2(network);
    let factory_s = get_nadfun_factory_v2(network);
    let bc_s = get_bonding_curve_v2(network);
    let reg_s = get_token_registry_v2(network);

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
    let lens_s = get_token_info_lens(network);
    let lens_addr: Address = lens_s
        .parse()
        .with_context(|| format!("invalid TokenInfoLens address {lens_s:?} for {network:?}"))?;
    let token_info_lens = TokenInfoLens::new(lens_addr, provider.clone());

    let pm_s = get_protocol_manager_v2(network);
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
    poll_interval: Duration,
    max_wait: Duration,
) -> Result<alloy::rpc::types::TransactionReceipt> {
    let deadline = tokio::time::Instant::now()
        .checked_add(max_wait)
        .ok_or_else(|| anyhow::anyhow!("receipt wait timeout {max_wait:?} is too large"))?;
    wait_for_receipt_until(
        provider,
        tx_hash,
        poll_interval,
        deadline,
        max_wait,
        "receipt",
    )
    .await
}

async fn wait_for_receipt_until(
    provider: &Arc<DynProvider>,
    tx_hash: B256,
    poll_interval: Duration,
    deadline: tokio::time::Instant,
    timeout: Duration,
    phase: &str,
) -> Result<alloy::rpc::types::TransactionReceipt> {
    loop {
        let receipt = tokio::time::timeout_at(deadline, provider.get_transaction_receipt(tx_hash))
            .await
            .map_err(|_| receipt_timeout(tx_hash, timeout, phase))?
            .with_context(|| format!("failed to get receipt {tx_hash}"))?;
        match receipt {
            Some(receipt) => {
                ensure_receipt_hash(tx_hash, &receipt)?;
                return Ok(receipt);
            }
            None => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(receipt_timeout(tx_hash, timeout, phase));
                }
                sleep_for_poll_interval(deadline, poll_interval).await;
            }
        }
    }
}

async fn wait_for_submitted_receipt(
    provider: &Arc<DynProvider>,
    tx_hash: B256,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<alloy::rpc::types::TransactionReceipt> {
    let deadline = tokio::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "transaction submitted; outcome unknown: receipt deadline overflow for {tx_hash}"
            )
        })?;
    let mut last_rpc_error = None;

    loop {
        match tokio::time::timeout_at(deadline, provider.get_transaction_receipt(tx_hash)).await {
            Err(_) => {
                let last_rpc_error = last_rpc_error.or_else(|| {
                    Some("eth_getTransactionReceipt timed out at the shared deadline".to_owned())
                });
                return Err(submitted_receipt_timeout(tx_hash, timeout, last_rpc_error));
            }
            Ok(Err(error)) => {
                last_rpc_error = Some(format!("eth_getTransactionReceipt: {error:#}"));
            }
            Ok(Ok(Some(receipt))) => {
                ensure_receipt_hash(tx_hash, &receipt)?;
                return Ok(receipt);
            }
            Ok(Ok(None)) => {}
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(submitted_receipt_timeout(tx_hash, timeout, last_rpc_error));
        }
        sleep_for_poll_interval(deadline, poll_interval).await;
    }
}

async fn sleep_for_poll_interval(deadline: tokio::time::Instant, poll_interval: Duration) {
    tokio::time::sleep(
        poll_interval.min(deadline.saturating_duration_since(tokio::time::Instant::now())),
    )
    .await;
}

fn receipt_timeout(tx_hash: B256, timeout: Duration, phase: &str) -> anyhow::Error {
    anyhow::anyhow!("timed out after {timeout:?} waiting for {phase} of transaction {tx_hash}")
}

fn ensure_successful_receipt(
    tx_hash: B256,
    receipt: &alloy::rpc::types::TransactionReceipt,
) -> Result<()> {
    if !receipt.status() {
        anyhow::bail!(
            "transaction {tx_hash} reverted in block {:?}",
            receipt.block_number
        );
    }
    Ok(())
}

fn ensure_receipt_hash(
    tx_hash: B256,
    receipt: &alloy::rpc::types::TransactionReceipt,
) -> Result<()> {
    if receipt.transaction_hash != tx_hash {
        anyhow::bail!(
            "receipt requested for transaction {tx_hash} has mismatched transaction hash {}",
            receipt.transaction_hash
        );
    }
    Ok(())
}

fn submitted_receipt_timeout(
    tx_hash: B256,
    timeout: Duration,
    last_rpc_error: Option<String>,
) -> anyhow::Error {
    let last_rpc_error =
        last_rpc_error.unwrap_or_else(|| "none (receipt was not available)".to_owned());
    anyhow::anyhow!(
        "transaction submitted; outcome unknown: timed out after {timeout:?} waiting for \
         receipt {tx_hash}; last RPC error: {last_rpc_error}"
    )
}

fn transaction_result(receipt: &alloy::rpc::types::TransactionReceipt) -> TransactionResult {
    TransactionResult {
        transaction_hash: receipt.transaction_hash,
        block_number: receipt.block_number,
        gas_used: Some(U256::from(receipt.gas_used)),
        status: receipt.status(),
        logs: receipt.logs().to_vec(),
    }
}
