# Unified `Core` + `0.4.0` Refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Single `Core` instance that handles v1 and v2 tokens (trade, create, stream) with `Network` as an instance field and Codex P1/P2/P3 findings resolved, shipping as `0.4.0`.

**Architecture:**
- Remove the process-global `set_network` / `get_current_network` lock. Every entry point (`Core`, `ApiClient`, `CurveStream`, `DexStream`, `CurveIndexer`, `DexIndexer`, `PoolDiscovery`) takes a `Network` and stores it. `constants::get_*` helpers become free functions taking `&Network`.
- Collapse `CoreV2` into `Core`. `Core` owns both v1 (BondingCurveRouter + DexRouter + Lens) and v2 (NadFunRouter + NadFunFactory + BondingCurveV2 + TokenRegistryV2) bindings. v1/v2 dispatch on `buy`/`sell`/`get_amount_out` is driven by `TokenRegistryV2::is_registered(token)` with an in-memory cache. v2-only methods (exact-out, permit, vault create) are exposed as separate methods (no auto-dispatch).
- All Codex P1 findings close inside this branch; P2/P3 close where cheap, others get an explicit defer note in the PR.

**Tech Stack:** Rust 2021 · `alloy 1.0.24` · `tokio` · `anyhow`/`thiserror` · `serde` · `wiremock` (tests).

**Goal definition** (must all be green before declaring done):

1. `CoreV2` removed; single `Core` handles v1/v2 with auto-dispatch + explicit `*_v2` methods.
2. `set_network`/`get_current_network` removed from public API; `Network` is an instance field on every entry point.
3. Codex P1 8 items closed (defer reason logged inline if any item is intentionally deferred).
4. `cargo build --all-targets` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo fmt --all --check` + `cargo test --all-targets` all pass.
5. Testnet live smoke (`v2_smoke`, unified scenario) passes against `dev-node.nadapp.net`.
6. `Cargo.toml` at `0.4.0`. `CHANGELOG.md`/`MIGRATION.md`/`README.md`/`llms.txt` reflect new surface.
7. `/codex review` second pass returns 0 P1 findings.

**Branch:** `refactor/unified-core` (off `v2`, commit `1239cf0`). Target: `mainnet` via release PR. **STOP before merging — user gate.**

---

## File Structure

**Removed:**
- `src/core/v2/mod.rs` — collapsed into `src/core/v1/core.rs` (renamed to `src/core/core.rs`).
- `src/core/v2/` directory.
- Public `set_network` / `get_current_network` / `CURRENT_NETWORK` static.

**Created:**
- `MIGRATION.md` (repo root) — `0.3.x → 0.4.0` migration guide.

**Modified (high-level):**
- `src/constants.rs` — `get_*()` family takes `network: Network` arg; `set_network`/`get_current_network`/`CURRENT_NETWORK` deleted.
- `src/core/mod.rs` — single `Core` re-export; `CoreV2` removed.
- `src/core/v1/core.rs` → `src/core/core.rs` — absorbs all v2 surface as additional methods/fields.
- `src/api/mod.rs` — `ApiClient::new(network)` / `from_env(network)`; stores `Network`.
- `src/stream/v1/curve/stream.rs`, `src/stream/v1/curve/indexer.rs`, `src/stream/v1/dex/stream.rs`, `src/stream/v1/dex/indexer.rs` — constructors take `Network`.
- `src/stream/v2/curve/stream.rs`, `src/stream/v2/curve/indexer.rs`, `src/stream/v2/dex/stream.rs`, `src/stream/v2/dex/indexer.rs` — same.
- `src/contracts/v1/dex_factory.rs` — `PoolDiscovery::new(provider, network)`, drop `WMON` flat import (P1 #5).
- `src/contracts/mod.rs` — `get_pool_addresses_for_tokens(provider, tokens, network)`.
- `src/types/mod.rs` — explicit re-exports instead of `pub use v1::*; pub use v2::*;` (P1 #7).
- `src/types/v1/creator.rs` — `ApiTokenInfo.version` null-tolerant deserializer (P1 #6).
- `src/types/v2/events.rs`, `src/stream/v2/dex/events.rs` — switch to `log.log_decode::<Event>()` (P1 #1).
- `src/types/v2/params.rs` — `V2CreateTokenParams.payment` Native variant uses `buy_quote_amount` instead of duplicate `value` (P1 #4); `V2BuyWithNativeParams.value` folded into params (P2 #10); `VaultType::Custom` `#[serde(other)]` (P3 #17).
- `src/lib.rs` + `src/lib.rs::prelude` — explicit re-exports, drop `set_network`/`get_current_network`/`CoreV2`.
- `examples/v2/*`, `examples/unified_dispatch.rs`, `examples/core/*`, `examples/stream/*` — migrate to single `Core` + new constructor signatures.
- `Cargo.toml` — version `0.3.12 → 0.4.0`.
- `CHANGELOG.md` — `[0.4.0]` section with breaking changes.
- `README.md` + `llms.txt` — Quick Start + API surface rewrite.

**New tests:**
- `tests/network_instance.rs` — Core/ApiClient/streams can coexist in one process with different `Network`s.
- `tests/unified_core_dispatch.rs` — v1 token routed to v1 path, v2 token routed to v2 path on same `Core`.
- `tests/v2_events_decode.rs` — extend to assert indexed fields decode correctly (e.g. `token`, `buyer`).

---

## Phase A — Foundation: drop global `set_network`, thread `Network` through entry points

### Task A1: Convert `constants::get_*` to take `&Network`

**Files:**
- Modify: `src/constants.rs`

- [ ] **Step 1: Change signatures.** Replace each `pub fn get_X() -> &'static str` that calls `get_current_network()` with `pub fn get_X(network: Network) -> &'static str`. Same for the `Option<&'static str>` v2 helpers.

Concretely, rewrite the bottom half of `src/constants.rs` (lines 277–504) so each helper takes `network: Network` and matches directly. Example:

```rust
pub fn get_dex_factory(network: Network) -> &'static str {
    match network {
        Network::Mainnet => addresses::mainnet::v1::DEX_FACTORY,
        Network::Testnet => addresses::testnet::v1::DEX_FACTORY,
    }
}
```

Apply to every `get_*` (v1 + v2). Keep the v2 helpers returning `Option<&'static str>`.

- [ ] **Step 2: Delete `set_network`/`get_current_network`/`CURRENT_NETWORK` static.** Lines 47–69 of `src/constants.rs` come out entirely. Remove the `use std::sync::RwLock;` import that becomes unused.

- [ ] **Step 3: Delete the legacy `pub use mainnet::*;` re-export at the bottom of `addresses` module.** Lines 261 in `src/constants.rs` (`pub use mainnet::*;` inside `pub mod addresses`). Callers must now explicitly pick the network — no implicit mainnet default.

- [ ] **Step 4: Update the module-level doctest.** Rewrite lines 18–33 (the `## Usage` doctest) to show the new instance-style API (without `set_network`).

- [ ] **Step 5: `cargo build` — expect callsite failures.**

```bash
cargo build --lib 2>&1 | tee /tmp/build-a1.log
```

Expected: build fails with `get_dex_factory()` etc. missing arguments at every callsite. We fix those in subsequent tasks. **Do not commit yet** — A1 isn't independently green; commit at the end of Phase A.

---

### Task A2: Update internal callsites of `get_*` helpers

**Files (every file that calls a `get_*` helper):**
- Modify: `src/core/v1/core.rs`
- Modify: `src/core/v2/mod.rs`
- Modify: `src/api/mod.rs` (`get_api_server_url`)
- Modify: `src/contracts/v1/dex_factory.rs` (`get_dex_factory` + drop `WMON` flat re-export — P1 #5 lands here)
- Modify: any other file the `cargo build` output flags.

- [ ] **Step 1: Run the build to enumerate callsites.**

```bash
cargo build --lib 2>&1 | grep "^error" | head -50
```

- [ ] **Step 2: For each callsite, thread the local `network` field.** Pattern:

```rust
// Before
let addr: Address = get_dex_router().parse()?;

// After (inside method on a struct that has `network: Network` field)
let addr: Address = get_dex_router(self.network).parse()?;
```

In free functions / constructors that don't yet have a `network`, accept it as a new parameter (covered in later tasks for each module).

- [ ] **Step 3: For `src/contracts/v1/dex_factory.rs`:**
  - Delete `pub use crate::constants::{DEFAULT_FEE_TIER, WMON};` (line 14).
  - Add a `network: Network` field to `PoolDiscovery`. Change `PoolDiscovery::new` to `pub fn new(provider: Arc<P>, network: Network) -> Result<Self>`. Use `get_dex_factory(network)`.
  - Replace `WMON.parse()?` (lines 35, 65) with `get_wmon(network).parse()?` using `self.network`.
  - Update `get_pool_addresses_for_tokens(provider, tokens)` → `get_pool_addresses_for_tokens(provider, tokens, network)` and pass `network` into `PoolDiscovery::new`.

- [ ] **Step 4: `cargo build --lib`** — expect the lib to compile clean. Test/example failures are addressed later.

- [ ] **Step 5: No commit yet** — phase commit at end.

---

### Task A3: Thread `Network` through `ApiClient`

**Files:**
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Add field + change constructors.**

```rust
pub struct ApiClient {
    // ...existing fields...
    network: Network,
    base_url: String,
}

impl ApiClient {
    pub fn new(network: Network) -> Self {
        Self::with_base_url(get_api_server_url(network).to_string(), network)
    }

    pub fn with_base_url(base_url: String, network: Network) -> Self {
        Self {
            // ...
            base_url,
            network,
        }
    }

    pub fn network(&self) -> Network {
        self.network
    }
}
```

Drop the existing zero-arg `new()`. Migrate any internal `get_api_server_url()` callsite to use `self.network` or the explicit `base_url`.

- [ ] **Step 2: `from_env` accepts network too.** If `ApiClient::from_env()` exists, change to `from_env(network: Network)`. Bail out cleanly if env keys missing.

- [ ] **Step 3: `cargo build --lib`** clean.

---

### Task A4: Thread `Network` through streams + indexers + helpers

**Files:**
- Modify: `src/stream/v1/curve/stream.rs` — `CurveStream::new(rpc_url, network)`. Store `network` field.
- Modify: `src/stream/v1/curve/indexer.rs` — `CurveIndexer::new(provider, network)`. Store `network`.
- Modify: `src/stream/v1/dex/stream.rs` — `DexStream::new(rpc_url, pool_addresses, network)`. Store `network`.
- Modify: `src/stream/v1/dex/indexer.rs` — `DexIndexer::new(rpc_url, pool_addresses, network)`. Store `network`.
- Modify: `src/stream/v2/curve/stream.rs`, `src/stream/v2/curve/indexer.rs`, `src/stream/v2/dex/stream.rs`, `src/stream/v2/dex/indexer.rs` — same pattern.

- [ ] **Step 1: For each constructor, add `network: Network` as the last positional argument.** Where the body calls a `get_*` helper, switch to `get_*(self.network)`. Where addresses are already known (e.g. user-supplied `pool_addresses`), just store the network for downstream methods that may need it (e.g. block-range queries that hit `get_api_server_url`).

- [ ] **Step 2: `cargo build --lib`** clean.

---

### Task A5: Update `src/lib.rs` to explicit re-exports + drop globals

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Drop `set_network`/`get_current_network` from `pub use`** (lines 99–102) and from `prelude` (line 143).

- [ ] **Step 2: Replace `pub use types::*;`** (line 110) with an explicit re-export list, dropping the alloy `sol!` internal types that cause glob-export collisions (P1 #7). List the user-facing types:

```rust
pub use types::{
    // v1
    ApiCreatorInfo, ApiTokenInfo, ApiMarketInfo,
    BondingCurveEvent, BuyEvent, SellEvent, SyncEvent, GraduateEvent, CreateEvent,
    BuyParams, SellParams, SellPermitParams, GasPricing, TransactionResult,
    Router, // v1 router enum
    SwapEvent, PoolMetadata, // dex
    CreatorClaimableRewards, CreatorReward, // creator
    // v2
    V2BondingCurveEvent, V2BuyEvent as V2BuyBcEvent, V2SellEvent as V2SellBcEvent,
    V2CreateEvent, V2GraduateEvent, V2SyncEvent, V2SnipingPenaltyEvent,
    V2BuyParams, V2BuyWithNativeParams, V2BuyWithPermitParams,
    V2SellParams, V2SellToNativeParams, V2SellWithPermitParams, V2SellToNativeWithPermitParams,
    V2ExactOutBuyParams, V2ExactOutBuyWithNativeParams, V2ExactOutSellParams, V2ExactOutSellToNativeParams,
    V2CreateParams, V2CreateWithNativeParams, V2CreateTokenParams, V2CreatePayment, V2PrepareCreationParams, V2PreparedCreation,
    V2TokenCreationResult, V2GasEstimationParams,
    V2DexType, V2VaultAllocation, VaultType,
    V2DexSwapEvent, // dex stream
};
```

If any name clashes between v1 and v2, rename with `as` like the `V2BuyBcEvent` example above. Run the build and iterate until it compiles.

- [ ] **Step 3: Mirror the explicit list in `mod prelude`** (around line 125–151).

- [ ] **Step 4: Same in `src/types/mod.rs`** — replace `pub use v1::*; pub use v2::*;` with explicit lists for the user-facing types only. Keep internal types (alloy `sol!`-generated like `IBondingCurve`) `pub(crate)`.

- [ ] **Step 5: `cargo build --lib`** clean.

---

### Task A6: Update tests and examples to new constructor signatures

**Files (mechanical edits — each call to `ApiClient::new()` / stream `new(...)` / `PoolDiscovery::new(...)` etc. needs a `network` arg):**
- Modify: `tests/api_v2_endpoints.rs`
- Modify: `tests/core_v2_api.rs`
- Modify: `tests/v2_events_decode.rs`
- Modify: `examples/common/*.rs`, `examples/core/*.rs`, `examples/create/*.rs`, `examples/creator/*.rs`, `examples/stream/*.rs`, `examples/token/*.rs`, `examples/v2/*.rs`, `examples/unified_dispatch.rs`

- [ ] **Step 1: `cargo build --tests --examples`** to enumerate failures.

```bash
cargo build --tests --examples 2>&1 | grep "^error" | sort -u | head -50
```

- [ ] **Step 2: For each test/example, pass `Network::Testnet` (tests) or read from env (`std::env::var("NAD_NETWORK").as_deref() == Ok("mainnet")`) (examples).** Tests should be deterministic — hardcode `Network::Testnet` unless the test specifically targets mainnet. Add a small helper in `examples/common/` if duplication grows:

```rust
// examples/common/network.rs
use nadfun_sdk::Network;
pub fn env_network() -> Network {
    match std::env::var("NAD_NETWORK").as_deref() {
        Ok("mainnet") => Network::Mainnet,
        _ => Network::Testnet,
    }
}
```

- [ ] **Step 3: `cargo build --all-targets`** clean.

- [ ] **Step 4: `cargo test --lib`** — make sure existing lib unit tests still pass. Integration tests with on-chain calls may still need updates from later phases; that's fine.

---

### Task A7: Add a `network_instance` test that proves no global state

**Files:**
- Create: `tests/network_instance.rs`

- [ ] **Step 1: Write the test (failing without our changes from Phase A).**

```rust
//! Smoke test: two SDK clients can coexist in one process pointing at
//! different networks — previously impossible due to the global `set_network`
//! lock.

use nadfun_sdk::{ApiClient, Network};

#[test]
fn api_clients_on_different_networks_coexist() {
    let mainnet = ApiClient::new(Network::Mainnet);
    let testnet = ApiClient::new(Network::Testnet);
    assert_eq!(mainnet.network(), Network::Mainnet);
    assert_eq!(testnet.network(), Network::Testnet);
    // Base URLs must differ — proving each instance reads from its own network.
    assert_ne!(
        nadfun_sdk::constants::get_api_server_url(mainnet.network()),
        nadfun_sdk::constants::get_api_server_url(testnet.network()),
    );
}
```

- [ ] **Step 2: Run the test.**

```bash
cargo test --test network_instance -- --nocapture
```

Expected: pass.

---

### Task A8: Commit Phase A

- [ ] **Step 1: `cargo fmt --all`** and `cargo clippy --lib --all-features -- -D warnings` (lib only; the clippy gate over all targets lands in Phase F task F1).

- [ ] **Step 2: Stage and commit:**

```bash
git add -A
git commit -m "refactor(network): remove set_network global; thread Network through entry points

- constants::get_*() helpers now take Network arg
- ApiClient::new(network) / from_env(network)
- CurveStream/DexStream/CurveIndexer/DexIndexer/PoolDiscovery store Network
- Drop pub use mainnet::* implicit default; explicit network selection
- lib.rs + types/mod.rs use explicit re-exports (P1 #7)
- dex_factory drops WMON flat import (P1 #5)

BREAKING: ApiClient::new() / stream constructors / PoolDiscovery::new() / get_pool_addresses_for_tokens now require a Network argument. set_network / get_current_network removed from public API."
```

---

## Phase B — Unified `Core`: collapse `CoreV2` into `Core`

### Task B1: Move `core::v1::core::Core` to `src/core/core.rs` and absorb v2 fields

**Files:**
- Create: `src/core/core.rs` (new home for unified `Core`)
- Modify: `src/core/mod.rs`
- Delete: `src/core/v1/core.rs`
- Modify: `src/core/v1/mod.rs` (keep gas + utils only; drop `Core` re-export)
- Modify: `src/core/v2/mod.rs` → migrate methods into `Core`, then delete file
- Delete: `src/core/v2/` directory once empty

- [ ] **Step 1: Create `src/core/core.rs`** by moving content from `src/core/v1/core.rs`, expanding the struct:

```rust
//! Unified `Core` — one entry point for v1 + v2 bonding-curve trading,
//! token creation, and pool discovery. Routes per-token based on
//! `TokenRegistryV2::isRegistered` with an in-memory cache.

use crate::{
    api::ApiClient,
    constants::*,
    contracts::{
        BondingCurveRouter, BondingCurveV2, CreatorClient, DexRouter, Lens,
        NadFunFactory, NadFunRouter, TokenRegistryV2,
    },
    core::v1::gas::{estimate_gas, GasEstimationParams},
    types::*,
};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

pub struct Core {
    // v1
    bonding_curve_router: BondingCurveRouter<DynProvider>,
    dex_router: DexRouter<DynProvider>,
    lens: Lens<DynProvider>,
    // v2 (None when v2 not deployed on this network — falls back to v1-only behavior)
    v2: Option<V2Bindings>,
    // shared
    provider: Arc<DynProvider>,
    wallet_address: Address,
    network: Network,
    // Per-token version cache: v2 if TokenRegistryV2::isRegistered, v1 otherwise.
    version_cache: Arc<RwLock<HashMap<Address, SdkVersion>>>,
}

struct V2Bindings {
    router: NadFunRouter<DynProvider>,
    factory: NadFunFactory<DynProvider>,
    bonding_curve: BondingCurveV2<DynProvider>,
    token_registry: TokenRegistryV2<DynProvider>,
}
```

- [ ] **Step 2: Constructors.** Single `Core::new(rpc_url, private_key, network)` and `Core::with_provider(provider, wallet_address, network)` (the latter merges `Core` and `CoreV2`'s `with_provider`). Both attempt to wire v2 bindings; if `get_nadfun_router_v2(network)` returns `None`, the `v2` field is `None` and v2-only methods return a clear error.

```rust
impl Core {
    pub async fn new(rpc_url: String, private_key: String, network: Network) -> Result<Self> {
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();
        let wallet = EthereumWallet::from(signer);
        let url = rpc_url.parse()?;
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);
        let dyn_provider = Arc::new(DynProvider::new(provider));
        Self::with_provider(dyn_provider, wallet_address, network)
    }

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
}

/// Returns `Some` when v2 is configured for the network, `None` otherwise.
/// Never errors — v2 absence is a normal state on networks without a v2 deployment.
fn build_v2_bindings(provider: &Arc<DynProvider>, network: Network) -> Result<Option<V2Bindings>> {
    let Some(router_s) = get_nadfun_router_v2(network) else { return Ok(None); };
    let Some(factory_s) = get_nadfun_factory_v2(network) else { return Ok(None); };
    let Some(bc_s) = get_bonding_curve_v2(network) else { return Ok(None); };
    let Some(reg_s) = get_token_registry_v2(network) else { return Ok(None); };

    Ok(Some(V2Bindings {
        router: NadFunRouter::new(router_s.parse()?, provider.clone()),
        factory: NadFunFactory::new(factory_s.parse()?, provider.clone()),
        bonding_curve: BondingCurveV2::new(bc_s.parse()?, provider.clone()),
        token_registry: TokenRegistryV2::new(reg_s.parse()?, provider.clone()),
    }))
}
```

- [ ] **Step 3: Move all v1 methods unchanged** (lines 71–end of the old `src/core/v1/core.rs`) into the new `Core` impl. Auto-routing `get_amount_out`/`get_amount_in`/`buy`/`sell`/`sell_permit`/`get_receipt` stay as the v1 surface (they hit `Lens` and the v1 routers).

- [ ] **Step 4: Add v2 surface as new methods on `Core`.** Each existing `CoreV2` method becomes a `*_v2` method on `Core` (we keep them as opt-in / explicit calls for now — auto-dispatch lands in Task B3). Helper:

```rust
fn v2(&self) -> Result<&V2Bindings> {
    self.v2.as_ref().ok_or_else(|| anyhow::anyhow!(
        "NadFun contract v2 is not deployed on {:?}", self.network
    ))
}
```

Then port each `CoreV2::X` method as `Core::X_v2`:

```rust
pub async fn create_v2(&self, params: V2CreateParams) -> Result<B256> {
    self.v2()?.router.create(params).await
}

pub async fn create_with_native_v2(&self, params: V2CreateWithNativeParams) -> Result<B256> {
    self.v2()?.router.create_with_native(params).await
}

pub async fn create_token_v2(
    &self,
    params: V2CreateTokenParams,
    api: &ApiClient,
) -> Result<V2TokenCreationResult> { /* existing CoreV2::create_token body, using self.v2()? */ }

pub async fn buy_v2(&self, params: V2BuyParams) -> Result<B256> {
    self.v2()?.router.buy(params).await
}
// ... same for buy_with_native_v2, buy_with_permit_v2, sell_v2, sell_to_native_v2,
//     sell_with_permit_v2, sell_to_native_with_permit_v2, exact_out_buy_v2,
//     exact_out_buy_with_native_v2, exact_out_sell_v2, exact_out_sell_to_native_v2.

pub async fn quote_v2(&self, token: Address, amount_in: U256, is_buy: bool) -> Result<U256> {
    self.v2()?.router.get_amount_out(token, amount_in, is_buy).await
}
// ... quote_in_v2, quote_bonding_curve_v2, quote_bonding_curve_in_v2,
//     quote_dex_v2, quote_dex_in_v2.

pub async fn is_graduated_v2(&self, token: Address) -> Result<bool> {
    self.v2()?.router.is_graduated(token).await
}

pub async fn pool_address_v2(&self, token: Address) -> Result<Address> {
    self.v2()?.token_registry.get_pair(token).await
}

pub async fn wrapped_native_v2(&self) -> Result<Address> {
    self.v2()?.router.wrapped_native().await
}

pub async fn estimate_gas_v2(&self, params: V2GasEstimationParams) -> Result<u64> {
    self.v2()?.router.estimate_gas(params, self.wallet_address).await
}
```

- [ ] **Step 5: Keep escape-hatch accessors** for advanced users (was on `CoreV2`):

```rust
pub fn router_v2(&self) -> Result<&NadFunRouter<DynProvider>> { Ok(&self.v2()?.router) }
pub fn factory_v2(&self) -> Result<&NadFunFactory<DynProvider>> { Ok(&self.v2()?.factory) }
pub fn bonding_curve_v2(&self) -> Result<&BondingCurveV2<DynProvider>> { Ok(&self.v2()?.bonding_curve) }
pub fn token_registry_v2(&self) -> Result<&TokenRegistryV2<DynProvider>> { Ok(&self.v2()?.token_registry) }
pub fn provider(&self) -> &Arc<DynProvider> { &self.provider }
pub fn wallet_address(&self) -> Address { self.wallet_address }
pub fn network(&self) -> Network { self.network }
```

- [ ] **Step 6: Wire `src/core/mod.rs`.** Replace lines 67–78 with:

```rust
pub mod core;
pub mod v1; // keep for gas, utils

pub use crate::types::Router;
pub use core::Core;
pub use v1::{
    estimate_buy_gas, estimate_gas, estimate_sell_gas, estimate_sell_permit_gas,
    GasEstimationParams, SlippageUtils,
};
```

- [ ] **Step 7: Update `src/core/v1/mod.rs`** to drop `pub mod core;` and `pub use core::Core;` (keep `gas` and `utils` only).

- [ ] **Step 8: Delete `src/core/v1/core.rs` and `src/core/v2/mod.rs`.** `rm` them (or `git rm`).

```bash
git rm src/core/v1/core.rs src/core/v2/mod.rs
```

If the `src/core/v2/` directory has no other files, remove the directory.

- [ ] **Step 9: Update `src/lib.rs`** — drop `CoreV2` from the `pub use core::{...}` line (101) and from `prelude` (130). Drop `set_network`/`get_current_network` if not already done in A5.

- [ ] **Step 10: `cargo build --lib`** clean.

---

### Task B2: Auto-dispatch test (failing first — TDD)

**Files:**
- Create: `tests/unified_core_dispatch.rs`

- [ ] **Step 1: Write the failing test.**

```rust
//! v1 and v2 tokens routed correctly from a single Core instance.
//!
//! Skipped unless TESTNET_PRIVATE_KEY + TESTNET_RPC_URL are set. Hits a
//! known-graduated v1 token and a known v2 token on dev-node.nadapp.net.

use nadfun_sdk::{Core, Network, SdkVersion};
use alloy::primitives::Address;

fn env_or_skip(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

#[tokio::test(flavor = "multi_thread")]
async fn dispatches_v1_and_v2_from_one_core() {
    let Some(rpc) = env_or_skip("TESTNET_RPC_URL") else { eprintln!("skip: TESTNET_RPC_URL not set"); return; };
    let Some(key) = env_or_skip("TESTNET_PRIVATE_KEY") else { eprintln!("skip: TESTNET_PRIVATE_KEY not set"); return; };

    let core = Core::new(rpc, key, Network::Testnet).await.unwrap();

    // Pick a v1 token from the testnet (any non-graduated v1 token works).
    let v1_token: Address = "0x0000000000000000000000000000000000000001".parse().unwrap();
    // Pick a v2 token (one of the 97 deployed pairs — TODO fill in real address).
    let v2_token: Address = "0x0000000000000000000000000000000000000002".parse().unwrap();

    assert_eq!(core.detect_version(v1_token).await.unwrap(), SdkVersion::V1);
    assert_eq!(core.detect_version(v2_token).await.unwrap(), SdkVersion::V2);
}
```

(The actual token addresses are placeholders; Task B4 fills them after listing testnet tokens.)

- [ ] **Step 2: Run the test.**

```bash
TESTNET_RPC_URL= TESTNET_PRIVATE_KEY= cargo test --test unified_core_dispatch -- --nocapture
```

Expected: skip with "TESTNET_RPC_URL not set" — confirms the test compiles and the new API surface (`Core::detect_version`) doesn't exist yet. Build should still fail at `core.detect_version(...)`.

---

### Task B3: Implement `Core::detect_version` + auto-dispatch on `buy`/`sell`/`get_amount_out`

**Files:**
- Modify: `src/core/core.rs`

- [ ] **Step 1: Add `detect_version`.**

```rust
impl Core {
    /// Detect whether `token` is a v2 token (registered in TokenRegistryV2)
    /// or v1. Result is cached in-process. Returns `SdkVersion::V1` if v2
    /// bindings aren't available on this network.
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
}
```

- [ ] **Step 2: Run B2 test against testnet to confirm.** (After B4 fills real addresses.)

---

### Task B4: Fill real testnet token addresses in B2 test

**Files:**
- Modify: `tests/unified_core_dispatch.rs`

- [ ] **Step 1: List a v2 token from testnet `NadFunFactory::allPairs`.** Use the existing `examples/v2/testnet_smoke.rs` to print a pair → token mapping, or query `factory.allPairs(0)` then `pair.token()` directly.

```bash
cargo run --example testnet_smoke -- --print-tokens 2>&1 | head -20
```

(If the example doesn't have `--print-tokens` mode, add a one-off bin that does it; remove before commit.)

- [ ] **Step 2: Pick one known v1 token + one v2 token.** Hardcode their addresses in the test. Add a comment `// from testnet_smoke output 2026-05-28`.

- [ ] **Step 3: Run the test against testnet.**

```bash
TESTNET_RPC_URL=https://dev-node.nadapp.net/ \
  TESTNET_PRIVATE_KEY=$NAD_TESTNET_KEY \
  cargo test --test unified_core_dispatch -- --nocapture
```

Expected: pass.

---

### Task B5: Commit Phase B

- [ ] **Step 1: `cargo fmt --all`** + `cargo clippy --lib --all-targets -- -D warnings`.

- [ ] **Step 2: Commit.**

```bash
git add -A
git commit -m "refactor(core): collapse CoreV2 into Core with auto-version dispatch

- Single Core handles v1 + v2 tokens. v2 bindings are Option<_> for
  networks without v2 deployed.
- Core::detect_version(token) probes TokenRegistryV2::isRegistered and
  caches in-process.
- v2-only methods exposed as *_v2 (create_v2, buy_v2, exact_out_buy_v2, ...)
- Escape hatches: router_v2(), factory_v2(), token_registry_v2(), ...
- Tests: tests/unified_core_dispatch.rs proves a single Core dispatches
  v1 + v2 tokens correctly on testnet.

BREAKING: CoreV2 removed. Migrate to Core (see MIGRATION.md)."
```

---

## Phase C — Codex P1 remaining: event decoder, create_token receipt, payment validation, ApiTokenInfo null

### Task C1: P1 #1 — v2 event decoder uses `log.log_decode::<Event>()`

**Files:**
- Modify: `src/types/v2/events.rs:209-...`
- Modify: `src/stream/v2/dex/events.rs:91-...`
- Modify: `tests/v2_events_decode.rs`

- [ ] **Step 1: Add a failing test with indexed fields populated.**

In `tests/v2_events_decode.rs`, add:

```rust
#[test]
fn create_event_decodes_indexed_fields() {
    use alloy::primitives::{address, Address, Log as PrimLog, LogData, B256, U256};
    use alloy::sol_types::SolEvent;
    use nadfun_sdk::types::v2::events::{decode_v2_bonding_curve_event, V2BondingCurveEvent};

    let creator = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let token = address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let pair = address!("cccccccccccccccccccccccccccccccccccccccc");
    let quote_token = address!("dddddddddddddddddddddddddddddddddddddddd");
    // ... build a PrimLog using IBondingCurveV2Events::Create::SIGNATURE_HASH
    //     in topic0 and the three indexed addresses in topic1/topic2/topic3,
    //     with non-indexed fields (name/symbol/tokenURI/reserves) ABI-encoded
    //     in the data slot.

    // Convert PrimLog -> alloy::rpc::types::Log via .into() or struct literal.
    // Decode.
    let log = make_create_log(creator, token, pair, quote_token, "NAME", "SYM", "ipfs://x");
    let evt = decode_v2_bonding_curve_event(log).unwrap();
    match evt {
        V2BondingCurveEvent::Create(e) => {
            assert_eq!(e.creator, creator, "creator (indexed) must decode from topics");
            assert_eq!(e.token, token, "token (indexed) must decode from topics");
            assert_eq!(e.pair, pair, "pair (indexed) must decode from topics");
            // non-indexed
            assert_eq!(e.name, "NAME");
            assert_eq!(e.symbol, "SYM");
        }
        _ => panic!("expected Create event"),
    }
}
```

Run: expect failure (indexed fields decode to zero with current `decode_log_data`).

- [ ] **Step 2: Rewrite the decoder body** in `src/types/v2/events.rs:209-...` to use `log.log_decode::<EventStruct>()`:

```rust
pub fn decode_v2_bonding_curve_event(log: Log) -> Result<V2BondingCurveEvent> {
    let topic0 = log
        .topic0()
        .copied()
        .ok_or_else(|| anyhow!("log has no topics"))?;
    let block_number = log.block_number.unwrap_or_default();
    let transaction_hash = log.transaction_hash.unwrap_or_default();
    let transaction_index = log.transaction_index.unwrap_or_default();
    let log_index = log.log_index.unwrap_or_default();

    if topic0 == IBondingCurveV2Events::Create::SIGNATURE_HASH {
        let decoded = log.log_decode::<IBondingCurveV2Events::Create>()
            .map_err(|e| anyhow!("decode Create: {e}"))?;
        let e = decoded.inner.data;
        Ok(V2BondingCurveEvent::Create(V2CreateEvent { creator: e.creator, token: e.token, pair: e.pair, quote_token: e.quoteToken, name: e.name, symbol: e.symbol, token_uri: e.tokenURI, virtual_quote_reserve: e.virtualQuoteReserve, virtual_token_reserve: e.virtualTokenReserve, min_token_reserve: e.minTokenReserve, block_number, transaction_hash, transaction_index, log_index }))
    } else if topic0 == IBondingCurveV2Events::Buy::SIGNATURE_HASH {
        // same pattern
    } /* ...remaining variants... */
}
```

- [ ] **Step 3: Same change for `src/stream/v2/dex/events.rs:91`.**

- [ ] **Step 4: Run the test.**

```bash
cargo test --test v2_events_decode -- --nocapture
```

Expected: pass.

- [ ] **Step 5: Commit.**

```bash
git add -A
git commit -m "fix(v2/events): decode indexed fields via log.log_decode (Codex P1 #1)

decode_log_data() only reads the data segment; indexed fields like token,
buyer, seller, creator land in topics and were silently zeroed.

Switch both bonding-curve and dex event decoders to log.log_decode::<Event>(),
add tests asserting indexed addresses decode from topics."
```

---

### Task C2: P1 #3 — `create_token_v2` verifies receipt + predicted address

**Files:**
- Modify: `src/core/core.rs` (the migrated `create_token_v2` method)

- [ ] **Step 1: Write a test (skipped unless on testnet).**

```rust
// In tests/core_v2_api.rs or a new tests/v2_create_token_verify.rs

#[tokio::test(flavor = "multi_thread")]
async fn create_token_verifies_predicted_address_against_receipt() {
    // Skip unless TESTNET_PRIVATE_KEY + NAD_API_KEY are set.
    // After create_token succeeds, assert that
    //   TokenRegistryV2::getPair(result.token_address) != Address::ZERO,
    // and the BondingCurve "Create" event in the receipt matches token_address.
}
```

- [ ] **Step 2: Add the verification in `create_token_v2`.**

After `tx_hash = ...`:

```rust
// Wait for receipt and verify the predicted address matches the on-chain
// Create event (defense against salt-server / contract drift).
let receipt = self.provider
    .get_transaction_receipt(tx_hash)
    .await?
    .ok_or_else(|| anyhow::anyhow!("create_token: receipt not found for {tx_hash}"))?;

if !receipt.status() {
    return Err(anyhow::anyhow!("create_token: transaction reverted ({tx_hash})"));
}

// Find Create event in logs and assert token matches.
let create_sig = IBondingCurveV2Events::Create::SIGNATURE_HASH;
let create_log = receipt.logs().iter().find(|l| l.topic0() == Some(&create_sig));

match create_log {
    Some(log) => {
        let alloy_log: alloy::rpc::types::Log = log.clone();
        let evt = alloy_log.log_decode::<IBondingCurveV2Events::Create>()
            .map_err(|e| anyhow::anyhow!("decode Create: {e}"))?;
        let on_chain_token = evt.inner.data.token;
        if on_chain_token != prepared.token_address {
            return Err(anyhow::anyhow!(
                "create_token: predicted token {} does not match on-chain {}",
                prepared.token_address, on_chain_token
            ));
        }
    }
    None => {
        // Fall back to TokenRegistry probe — Create event might be on a different contract.
        let pair = self.v2()?.token_registry.get_pair(prepared.token_address).await?;
        if pair == Address::ZERO {
            return Err(anyhow::anyhow!(
                "create_token: predicted token {} is not registered on-chain after tx {}",
                prepared.token_address, tx_hash
            ));
        }
    }
}
```

- [ ] **Step 3: Run the test against testnet (or mark `#[ignore]` if testnet key absent).**

- [ ] **Step 4: Commit.**

```bash
git commit -am "fix(v2/create): verify on-chain Create event matches predicted address (Codex P1 #3)"
```

---

### Task C3: P1 #4 — `V2CreateTokenParams.payment` Native uses `buy_quote_amount`, not duplicate `value`

**Files:**
- Modify: `src/types/v2/params.rs:248` area + `V2CreatePayment` enum
- Modify: `src/core/core.rs` (the `create_token_v2` body using `payment`)
- Modify: `examples/v2/*.rs`

- [ ] **Step 1: Inspect the current `V2CreatePayment` enum.** Open `src/types/v2/params.rs` and locate `V2CreatePayment`. If `Native { value: U256 }` exists, change to:

```rust
#[derive(Debug, Clone)]
pub enum V2CreatePayment {
    /// Initial buy funded with native MON (msg.value). Amount taken from
    /// V2CreateTokenParams.buy_quote_amount; no separate value field.
    Native,
    /// Initial buy funded with an ERC-20 quote token (pre-approved).
    Erc20 { quote_token: Address },
}
```

- [ ] **Step 2: Update `create_token_v2`** to derive `native_value` from `params.buy_quote_amount`:

```rust
let tx_hash = match params.payment {
    V2CreatePayment::Native => {
        let on_chain = V2CreateWithNativeParams {
            // ...
            buy_quote_amount: params.buy_quote_amount,
            native_value: params.buy_quote_amount,
            // ...
        };
        self.v2()?.router.create_with_native(on_chain).await?
    }
    // Erc20 unchanged
};
```

- [ ] **Step 3: Update all examples** that construct `V2CreatePayment::Native { value: ... }` to `V2CreatePayment::Native` (drop the `value`).

- [ ] **Step 4: `cargo build --all-targets`** clean. Run create-token tests.

- [ ] **Step 5: Commit.**

```bash
git commit -am "fix(v2/types): drop V2CreatePayment::Native.value; derive from buy_quote_amount (Codex P1 #4)"
```

---

### Task C4: P1 #6 — `ApiTokenInfo.version` null-tolerant deserializer

**Files:**
- Modify: `src/types/v1/creator.rs:102-105`

- [ ] **Step 1: Add a wrapper deserializer.**

```rust
// At the bottom of the file, near `deserialize_string_default_empty`:
fn deserialize_sdk_version_null_default<'de, D>(de: D) -> Result<crate::version::SdkVersion, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<crate::version::SdkVersion>::deserialize(de)?.unwrap_or_default())
}
```

- [ ] **Step 2: Replace the field attribute:**

```rust
// before
#[serde(default)]
pub version: crate::version::SdkVersion,

// after
#[serde(default, deserialize_with = "deserialize_sdk_version_null_default")]
pub version: crate::version::SdkVersion,
```

- [ ] **Step 3: Add a unit test in the same file (or `tests/api_v2_endpoints.rs`).**

```rust
#[test]
fn token_info_handles_null_version() {
    let raw = r#"{ "token_id": "0x0000000000000000000000000000000000000001",
                   "name": "x", "symbol": "x", "decimals": 18,
                   "description": null, "image_uri": "", "twitter": "", "telegram": "", "website": "",
                   "version": null }"#;
    let info: super::ApiTokenInfo = serde_json::from_str(raw).unwrap();
    assert_eq!(info.version, crate::version::SdkVersion::V1); // default
}

#[test]
fn token_info_handles_missing_version() {
    let raw = r#"{ "token_id": "0x...", ... }"#; // no version key at all
    let info: super::ApiTokenInfo = serde_json::from_str(raw).unwrap();
    assert_eq!(info.version, crate::version::SdkVersion::V1);
}
```

- [ ] **Step 4: Run tests.**

```bash
cargo test --lib creator
```

- [ ] **Step 5: Commit.**

```bash
git commit -am "fix(api): ApiTokenInfo.version tolerates null (Codex P1 #6)"
```

---

### Task C5: P1 #8 — clippy `-D warnings` gate passes

**Files:** whatever clippy flags.

- [ ] **Step 1: Run clippy.**

```bash
cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tee /tmp/clippy.log
```

- [ ] **Step 2: Fix unused imports** (especially in `src/contracts/v2/mod.rs` re-exports). Drop dead re-exports.

- [ ] **Step 3: Fix `too_many_arguments`** by wrapping into a params struct where it appears (likely router methods). If genuinely unavoidable, scope with `#[allow(clippy::too_many_arguments)]` on the specific function with a one-line comment explaining why.

- [ ] **Step 4: Fix missing doc comments** on public items, or add scoped `#[allow(missing_docs)]`.

- [ ] **Step 5: Re-run** until exit 0.

- [ ] **Step 6: Commit.**

```bash
git commit -am "chore: clippy --all-targets --all-features -- -D warnings clean (Codex P1 #8)"
```

---

## Phase D — Codex P2 fixes

### Task D1: P2 #9 — `estimate_gas` with `from=ZERO` errors

**Files:**
- Modify: wherever `estimate_gas` accepts a from address — likely `src/core/v1/gas.rs` (`GasEstimationParams`) and the v2 path inside `NadFunRouter::estimate_gas`.

- [ ] **Step 1: Audit.** Search:

```bash
grep -rn "Address::ZERO\|address!(\"0\"\\) *)" src/core src/contracts
```

- [ ] **Step 2: At every `estimate_gas` entry point, reject `Address::ZERO`** for `from`. Return `Err(anyhow!("estimate_gas: from address must not be zero"))`.

- [ ] **Step 3: Add a unit test** that constructs a params with `Address::ZERO` and asserts the error.

- [ ] **Step 4: Commit.**

```bash
git commit -am "fix(gas): reject Address::ZERO as from in estimate_gas (Codex P2 #9)"
```

---

### Task D2: P2 #10 — Fold `value` into `V2BuyWithNativeParams`

**Files:**
- Modify: `src/types/v2/params.rs` — add `value: U256` to `V2BuyWithNativeParams`.
- Modify: `src/contracts/v2/router.rs` — `buy_with_native(params)` reads `params.value`.
- Modify: `src/core/core.rs` — `buy_with_native_v2(params)` signature drops the trailing `value`.
- Modify: examples/tests that pass `value` separately.

- [ ] **Step 1: Edit struct.**

```rust
pub struct V2BuyWithNativeParams {
    // ...existing fields...
    /// Native MON sent with the call (`msg.value`). Must match the router's
    /// expected input amount for an exact-in buy.
    pub value: U256,
}
```

- [ ] **Step 2: Edit router method.** `pub async fn buy_with_native(&self, params: V2BuyWithNativeParams) -> Result<B256>` reads `params.value`.

- [ ] **Step 3: Update `Core::buy_with_native_v2`** to single-arg.

- [ ] **Step 4: Build + fix callers.**

- [ ] **Step 5: Commit.**

```bash
git commit -am "refactor(v2/params): fold value into V2BuyWithNativeParams (Codex P2 #10)"
```

---

### Task D3: P2 #11 — `discover_pools_unified` uses `TokenRegistryV2::getPair`

**Files:**
- Likely `src/contracts/v2/mod.rs` or wherever unified pool discovery lives.

- [ ] **Step 1: Locate.**

```bash
grep -rn "discover_pools_unified\|discover_pools" src/
```

- [ ] **Step 2: Replace WMON-pair guessing** with a direct `TokenRegistryV2::get_pair(token)` call for v2 tokens, and the existing `PoolDiscovery::get_pool_for_token` for v1.

- [ ] **Step 3: Test.** Add an integration test against testnet that asserts the unified discovery returns the same pair as `TokenRegistryV2`.

- [ ] **Step 4: Commit.**

```bash
git commit -am "fix(pool-discovery): use TokenRegistryV2::getPair for v2 (Codex P2 #11)"
```

---

### Task D4: P2 #15 — `V2PreparedCreation` carries normalized name/symbol

**Files:**
- Modify: `src/types/v2/params.rs` — add `name: String`, `symbol: String` to `V2PreparedCreation`.
- Modify: `src/api/mod.rs` — `prepare_token_creation_v2` returns server-normalized name/symbol.
- Modify: `src/core/core.rs::create_token_v2` — use `prepared.name`/`prepared.symbol` in the on-chain call instead of `params.name`/`params.symbol`.

- [ ] **Step 1: Audit prepared shape.**

```bash
grep -rn "V2PreparedCreation" src/
```

- [ ] **Step 2: Add fields + thread through.** Default-fall to `params.name`/`params.symbol` if the server doesn't return them (back-compat).

- [ ] **Step 3: Commit.**

```bash
git commit -am "fix(v2/create): use server-normalized name/symbol from V2PreparedCreation (Codex P2 #15)"
```

---

## Phase E — Codex P3 fixes

### Task E1: P3 #17 — `VaultType::Custom` `#[serde(other)]`

**Files:**
- Modify: `src/types/v2/params.rs` — `VaultType` enum.

- [ ] **Step 1: Add the variant + attr.**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultType {
    Burn,
    Lp,
    CreatorFee,
    Gift,
    #[serde(other)]
    Custom,
}
```

(Adjust based on the existing variants; the key point is `#[serde(other)]` so unknown vault types deserialize to `Custom` instead of erroring.)

- [ ] **Step 2: Unit test** with an unknown JSON value:

```rust
#[test]
fn unknown_vault_type_falls_back_to_custom() {
    let v: VaultType = serde_json::from_str("\"future_vault_type\"").unwrap();
    assert_eq!(v, VaultType::Custom);
}
```

- [ ] **Step 3: Commit.**

```bash
git commit -am "fix(v2/vault): VaultType::Custom catches unknown variants (Codex P3 #17)"
```

---

### Task E2: P3 #18 — `NadFunSwapStream::new` rejects empty `pairs`

**Files:**
- Modify: `src/stream/v2/dex/stream.rs:25`

- [ ] **Step 1: Add guard.**

```rust
pub async fn new(ws_url: String, pairs: Vec<Address>, network: Network) -> Result<NadFunSwapStream> {
    if pairs.is_empty() {
        return Err(anyhow::anyhow!(
            "NadFunSwapStream: at least one pair address is required; use NadFunSwapStream::all_pairs(...) if you intend to subscribe to every pair"
        ));
    }
    // ...
}
```

- [ ] **Step 2: Optional `all_pairs` constructor**: only add if needed for current usage. If not used, skip — the error is the docs.

- [ ] **Step 3: Test** in `tests/api_v2_endpoints.rs` or a new file.

```rust
#[tokio::test]
async fn nadfun_swap_stream_rejects_empty_pairs() {
    let err = NadFunSwapStream::new("ws://ignored".into(), vec![], Network::Testnet).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("at least one pair"), "unexpected: {msg}");
}
```

- [ ] **Step 4: Commit.**

```bash
git commit -am "fix(stream/v2): reject empty pairs in NadFunSwapStream (Codex P3 #18)"
```

---

## Phase F — Release prep: version, CHANGELOG, MIGRATION, README, llms.txt, examples

### Task F1: Pre-release verification gate

- [ ] **Step 1: `cargo fmt --all --check`** — fix anything still unformatted.
- [ ] **Step 2: `cargo clippy --all-targets --all-features -- -D warnings`** — clean.
- [ ] **Step 3: `cargo build --all-targets`** — clean.
- [ ] **Step 4: `cargo test --all-targets`** — all green.

If any of these fail, fix and re-run from the failing step. **Do not proceed to F2 until F1 is fully green.**

---

### Task F2: Bump version to `0.4.0`

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Edit version.**

```toml
[package]
name = "nadfun_sdk"
version = "0.4.0"
```

- [ ] **Step 2: `cargo build`** to refresh `Cargo.lock`.

- [ ] **Step 3: Commit.**

```bash
git commit -am "chore: bump version to 0.4.0"
```

---

### Task F3: Update `CHANGELOG.md` with `[0.4.0]` section

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Move `[Unreleased]` entries** to a new `## [0.4.0] - 2026-05-28` section and add the unified-Core-specific entries:

```markdown
## [0.4.0] - 2026-05-28

### Breaking changes
- `CoreV2` removed. `Core` now handles v1 and v2 tokens — use `*_v2` methods for v2-only operations.
- `set_network` / `get_current_network` removed. `Network` is now an instance field on every entry point.
- `ApiClient::new()` → `ApiClient::new(network: Network)`.
- `ApiClient::from_env()` → `ApiClient::from_env(network: Network)`.
- `CurveStream::new(url)` → `CurveStream::new(url, network)`. Same shape for `DexStream`, `CurveIndexer`, `DexIndexer`.
- `PoolDiscovery::new(provider)` → `PoolDiscovery::new(provider, network)`.
- `get_pool_addresses_for_tokens(provider, tokens)` → `get_pool_addresses_for_tokens(provider, tokens, network)`.
- `constants::get_*()` helpers now take a `Network` argument.
- `V2CreatePayment::Native { value }` → `V2CreatePayment::Native` (uses `params.buy_quote_amount`).
- `V2BuyWithNativeParams.value` is now a struct field; `Core::buy_with_native_v2` is single-arg.
- `types/mod.rs` and `lib.rs` switched from `pub use *` glob to explicit re-exports — internal alloy `sol!` types are no longer publicly visible.

### Added
- `Core::detect_version(token)` — v2 / v1 detection with in-process cache.
- `Core::create_v2`, `create_with_native_v2`, `create_token_v2`, `buy_v2`, `buy_with_native_v2`, `buy_with_permit_v2`, `sell_v2`, `sell_to_native_v2`, `sell_with_permit_v2`, `sell_to_native_with_permit_v2`, `exact_out_buy_v2`, `exact_out_buy_with_native_v2`, `exact_out_sell_v2`, `exact_out_sell_to_native_v2`, `quote_v2`, `is_graduated_v2`, `pool_address_v2`, `wrapped_native_v2`, `estimate_gas_v2`, plus binding escape hatches (`router_v2()`, `token_registry_v2()`, …).
- `MIGRATION.md` covering 0.3.x → 0.4.0.

### Fixed
- v2 event decoder reads indexed fields from topics (Codex P1 #1).
- `Core::create_token_v2` verifies the predicted address against the on-chain `Create` event (Codex P1 #3).
- `ApiTokenInfo.version` deserialization tolerates `null` (Codex P1 #6).
- `PoolDiscovery::get_pools_for_tokens` uses the network-correct WMON (Codex P1 #5).
- `estimate_gas` rejects `Address::ZERO` as `from` (Codex P2 #9).
- `VaultType::Custom` catches unknown vault types (Codex P3 #17).
- `NadFunSwapStream::new` rejects empty pair lists (Codex P3 #18).
```

- [ ] **Step 2: Commit.**

```bash
git commit -am "docs: CHANGELOG [0.4.0] release notes"
```

---

### Task F4: Write `MIGRATION.md`

**Files:**
- Create: `MIGRATION.md`

- [ ] **Step 1: Write the migration doc.**

```markdown
# Migrating to `nadfun_sdk` 0.4.0

0.4.0 collapses `CoreV2` into a unified `Core` and removes the process-global
`set_network` configuration. Every entry point now takes a `Network` argument.

## 1. Replace `CoreV2` with `Core`

```rust
// Before (0.3.x)
let core_v1 = Core::new(rpc.clone(), key.clone(), Network::Mainnet).await?;
let core_v2 = CoreV2::new(rpc, key, Network::Mainnet).await?;

// After (0.4.0)
let core = Core::new(rpc, key, Network::Mainnet).await?;
// core.buy(...) / core.sell(...) for v1 (auto-routed via Lens)
// core.buy_v2(...) / core.create_token_v2(...) for v2-only paths
// core.detect_version(token) when you need to dispatch yourself
```

## 2. Drop `set_network`

```rust
// Before
set_network(Network::Mainnet);
let core = Core::new(rpc, key).await?;     // implicit network

// After
let core = Core::new(rpc, key, Network::Mainnet).await?;
```

## 3. ApiClient + streams + helpers take `Network`

```rust
// Before
let api = ApiClient::new();
let curve = CurveStream::new(ws_url).await?;
let pools = PoolDiscovery::new(provider)?;
let addrs = get_pool_addresses_for_tokens(provider, tokens).await?;

// After
let api = ApiClient::new(Network::Mainnet);
let curve = CurveStream::new(ws_url, Network::Mainnet).await?;
let pools = PoolDiscovery::new(provider, Network::Mainnet)?;
let addrs = get_pool_addresses_for_tokens(provider, tokens, Network::Mainnet).await?;
```

## 4. `V2CreatePayment::Native` no longer carries a `value`

The native amount comes from `V2CreateTokenParams.buy_quote_amount`.

```rust
// Before
V2CreatePayment::Native { value: parse_ether("0.1")? }

// After
V2CreatePayment::Native // and set params.buy_quote_amount = parse_ether("0.1")?
```

## 5. `V2BuyWithNativeParams.value` is now a struct field

```rust
// Before
core.buy_with_native(params, value).await?;

// After
core.buy_with_native_v2(V2BuyWithNativeParams { /* ... */, value }).await?;
```

## 6. Glob imports — explicit re-exports

If you wrote `use nadfun_sdk::types::*`, this still works but some internal
alloy `sol!`-generated types (`IBondingCurve`, etc.) are no longer re-exported
publicly. Use the explicit `nadfun_sdk::contracts::*` (if exposed) for those.
```

- [ ] **Step 2: Commit.**

```bash
git add MIGRATION.md
git commit -m "docs: add MIGRATION.md for 0.4.0"
```

---

### Task F5: Update `README.md` Quick Start

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Rewrite the Quick Start section** to show single `Core` with both v1 + v2 trading.
- [ ] **Step 2: Update all code blocks** that reference `set_network`, `CoreV2`, or zero-arg `ApiClient::new()`.
- [ ] **Step 3: `cargo build --examples`** — README snippets used as `rust,ignore` shouldn't break, but cross-check any `rust` doctest.
- [ ] **Step 4: Commit.**

```bash
git commit -am "docs(readme): rewrite Quick Start for unified Core + 0.4.0"
```

---

### Task F6: Regenerate `llms.txt`

**Files:**
- Modify: `llms.txt`

- [ ] **Step 1: Re-emit the API surface summary** to reflect the new `Core` + `Core::*_v2` methods + new constructors. Walk every `pub fn`/`pub async fn` re-exported from `src/lib.rs` and `prelude` and group by entry point.
- [ ] **Step 2: Drop `CoreV2`, `set_network`, `get_current_network` sections** (they no longer exist).
- [ ] **Step 3: Add a top-line note** that 0.4.0 unified Core.
- [ ] **Step 4: Commit.**

```bash
git commit -am "docs(llms): regenerate API summary for unified Core 0.4.0"
```

---

### Task F7: Clean up `examples/`

**Files:**
- Modify: `examples/v2/*.rs`, `examples/core/*.rs`, `examples/stream/*.rs`
- Decide: keep or delete `examples/unified_dispatch.rs`

- [ ] **Step 1: Walk every example** and switch to single-`Core` + `Network` args. Already partly done in A6 for the build to succeed; this task polishes idiom and removes any `CoreV2` references in comments / docstrings.

- [ ] **Step 2: `examples/unified_dispatch.rs`** — if it demonstrated using both `Core` and `CoreV2` in one process, simplify to demonstrating `core.detect_version(...)` + `core.buy(...)`/`core.buy_v2(...)`. Otherwise delete it as obsolete.

- [ ] **Step 3: `cargo build --examples`** clean.

- [ ] **Step 4: Commit.**

```bash
git commit -am "refactor(examples): unify on single Core + Network instance"
```

---

### Task F8: Update `branches/refactor/unified-core.md` Changes section

**Files:**
- Modify: `branches/refactor/unified-core.md`

- [ ] **Step 1: Replace the placeholder `Changes` list** with the actual change log gathered from commits on this branch. Use:

```bash
git log v2..HEAD --oneline --no-decorate
```

…and copy each commit subject as a bullet under `## Changes`.

- [ ] **Step 2: Commit.**

```bash
git commit -am "docs(branch): update Changes section for unified-core"
```

---

## Phase G — Verification + PR prep

### Task G1: Final verification gate

- [ ] **Step 1: `cargo fmt --all --check`** — pass.
- [ ] **Step 2: `cargo clippy --all-targets --all-features -- -D warnings`** — pass.
- [ ] **Step 3: `cargo build --all-targets`** — pass.
- [ ] **Step 4: `cargo test --all-targets`** — all green (excluding `#[ignore]`d testnet-required tests).
- [ ] **Step 5: `cargo doc --no-deps`** — no broken intra-doc links.

If any step fails: stop, fix, restart from Step 1.

---

### Task G2: Testnet live smoke

**Files:** N/A (read-only checks).

- [ ] **Step 1: Set up env.**

```bash
export TESTNET_RPC_URL=https://dev-node.nadapp.net/
export TESTNET_WS_URL=wss://dev-node.nadapp.net/
export TESTNET_PRIVATE_KEY="<your testnet key>"
export NAD_API_KEY="<dev api key>"
```

- [ ] **Step 2: Run testnet smokes.**

```bash
cargo run --example testnet_smoke
cargo run --example v2_smoke || true
cargo test --test unified_core_dispatch -- --nocapture
```

Expected: all complete without panics. `factory.allPairs > 0`, version detection identifies real testnet tokens correctly.

- [ ] **Step 3: Document the run** in the PR description (commit hash + endpoint + counts).

---

### Task G3: `/codex review` second pass

- [ ] **Step 1: Run.**

```
/codex review
```

Expected: 0 P1 findings. Some P2/P3 may remain; record them in the PR description and decide which (if any) to fix in this branch vs. defer.

- [ ] **Step 2: Apply AUTO-FIX items immediately. For ASK items, confirm with user before applying.** Include the review-induced commits in the same push before opening the PR.

---

### Task G4: Open PR `refactor/unified-core → mainnet` and **STOP**

- [ ] **Step 1: Push the branch.**

```bash
git push -u origin refactor/unified-core
```

- [ ] **Step 2: Compose the PR body** using the CHANGELOG `[0.4.0]` section + Codex review summary + testnet smoke evidence. Use `gh pr create` per global git-workflow.md.

- [ ] **Step 3: Open the PR (target `mainnet`).**

- [ ] **Step 4: STOP.** Per goal directive, do not merge. Surface the PR URL and a one-line summary of what's left for the user (e.g. mainnet v2 address confirmation).

---

## Self-Review

**Spec coverage:**
- Goal definition #1 (CoreV2 → Core) → Phase B.
- Goal definition #2 (set_network removal) → Phase A.
- Goal definition #3 (Codex P1) → Phase A (#5, #7, #8), Phase C (#1, #3, #4, #6).
- Goal definition #4 (build/clippy/fmt/test gates) → Tasks A8, C5, F1, G1.
- Goal definition #5 (testnet smoke) → Task G2.
- Goal definition #6 (0.4.0 + CHANGELOG + MIGRATION + README + llms.txt) → Phase F.
- Goal definition #7 (`/codex review` 0 P1) → Task G3.
- Stop-before-merge → Task G4 Step 4.

**Placeholder scan:** Each task has concrete file paths and code snippets where code is needed. Mechanical bulk-edits (Task A2, A6) reference the build error log to drive iteration rather than enumerate every callsite by hand — acceptable because the pattern is uniform and `cargo build` is authoritative.

**Type consistency:** `Core::detect_version` (Task B3) returns `SdkVersion` (defined in `src/version.rs`, already re-exported from `lib.rs`). `V2CreatePayment::Native` (Task C3) used in `create_token_v2` (Task B1 Step 4 and re-edited in Task C3) — same name. `V2PreparedCreation` fields (Task D4) match server response — flagged to verify against `ApiClient::prepare_token_creation_v2` at implementation time.

**Known gap (acknowledged):** Testnet token addresses in Task B4 are filled at execution time from `factory.allPairs` — placeholder addresses in B2 will be replaced before the test runs against live testnet.

---

## Execution

After saving the plan, two options:

1. **Subagent-Driven** (recommended) — fresh subagent per task, review between tasks.
2. **Inline Execution** — execute in this session with executing-plans, batch with checkpoints.

User picks.
