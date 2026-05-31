# v2 Namespace Refactor — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move every flat `Core::*` v1/v2 method behind `core.v1()` / `core.v2()` borrowed namespace handles and drop the 29 `_v2` suffixes — a pure restructure with **no new methods and no behavior change**.

**Architecture:** Add two zero-cost `Copy` handle types `CoreV1<'a>` / `CoreV2<'a>` that each wrap `&Core`. `Core` keeps its internal `V1Contracts`/`V2Contracts` fields (visibility widened to `pub(crate)` so sibling handle modules can reach them); method bodies move verbatim into the handles with `self.X` → `self.core.X`. Cross-cutting methods (`detect_*`, `get_receipt`, accessors) stay on `Core`.

**Tech Stack:** Rust 2021, alloy 1.0.24, tokio (multi-thread), anyhow at boundaries.

**Spec:** `docs/superpowers/specs/2026-05-30-v2-namespace-refactor-design.md`
**Branch:** `feat/v2-namespace` off `v2`. **Scope:** Phase 1 only (Phases 2–3 are separate plans authored at their kickoff).

---

## File map

- **Modify** `src/core/core.rs` — widen field visibility to `pub(crate)`; make `wait_for_receipt` `pub(crate)`; add `v1()`/`v2()` accessors; **delete** all flat v1/v2 trade/quote/create/reward/escape methods (they relocate). Keep `new`, `with_provider`, `detect_*`, `get_receipt`, `provider()`, `wallet_address()`, `network()`, builders, `wait_for_receipt`.
- **Create** `src/core/v1/handle.rs` — `CoreV1<'a>` + all v1 handle methods.
- **Create** `src/core/v2/mod.rs` + `src/core/v2/handle.rs` — `CoreV2<'a>` + all v2 handle methods.
- **Modify** `src/core/mod.rs` — `pub mod v2;`; re-export `CoreV1`, `CoreV2`.
- **Modify** `src/core/v1/mod.rs` — `pub mod handle;`; re-export `CoreV1`.
- **Modify** `src/lib.rs` — add `CoreV1`, `CoreV2` to crate-root `pub use` and `prelude`.
- **Modify** tests: `tests/core_v2_api.rs`, `tests/unified_core_dispatch.rs`, `tests/lifecycle_smoke.rs`, `tests/network_instance.rs`.
- **Modify** examples: `examples/core/*`, `examples/create/create_token.rs`, `examples/creator/claim_reward.rs`, `examples/v2/*`, `examples/unified_dispatch.rs`.
- **Modify** docs: `CHANGELOG.md`, `llms.txt`, `README.md`, `examples/EXAMPLES.md`.

## Method → destination (the move map)

`core.v1()` (drop nothing, move from `Core`): `get_amount_out`, `get_amount_in`, `buy`, `sell`, `sell_permit`, `available_buy_tokens`, `is_locked`, `is_graduated`, `get_initial_buy_amount_out`, `get_deploy_fee`, `get_progress`, `estimate_gas`, `create_token`, `claim_creator_reward`, `claim_creator_rewards_batch`, `bonding_curve_router()`, `dex_router()`, `lens()`.

`core.v2()` (drop `_v2`, move from `Core`): `create`, `create_with_native`, `create_token`, `buy`, `buy_with_native`, `buy_with_permit`, `sell`, `sell_to_native`, `sell_with_permit`, `sell_to_native_with_permit`, `exact_out_buy`, `exact_out_buy_with_native`, `exact_out_sell`, `exact_out_sell_to_native`, `get_amount_out`, `get_amount_in`, `get_bonding_curve_amount_out`, `get_bonding_curve_amount_in`, `get_dex_amount_out`, `get_dex_amount_in`, `is_graduated`, `pool_address`, `wrapped_native`, `deploy_fee`, `estimate_gas`, `router()`, `factory()`, `bonding_curve()`, `token_registry()`, `token_info_lens()`.

Stays on `Core`: `new`, `with_provider`, `v1()`, `v2()`, `detect_version`, `detect_versions`, `detect_token_info`, `detect_token_infos`, `get_receipt`, `provider()`, `wallet_address()`, `network()`.

---

## Task 1: Branch + widen internal visibility + accessors + handle scaffolds

**Files:**
- Modify: `src/core/core.rs` (struct fields, `wait_for_receipt`, add accessors)
- Create: `src/core/v1/handle.rs`, `src/core/v2/mod.rs`, `src/core/v2/handle.rs`
- Modify: `src/core/mod.rs`, `src/core/v1/mod.rs`, `src/lib.rs`

- [ ] **Step 1: Create the branch**

```bash
cd /Users/gyu/project/nads-pump/rust-sdk
git checkout v2 && git pull --ff-only 2>/dev/null; git checkout -b feat/v2-namespace
```

- [ ] **Step 2: Widen `Core` field + helper-struct visibility in `src/core/core.rs`**

Change the `Core` struct and the two contract structs so sibling handle modules can read them. Replace the existing definitions (around `core.rs:46-74`) with:

```rust
pub struct Core {
    pub(crate) v1: V1Contracts,
    pub(crate) v2: V2Contracts,
    pub(crate) provider: Arc<DynProvider>,
    pub(crate) wallet_address: Address,
    pub(crate) network: Network,
}

pub(crate) struct V1Contracts {
    pub(crate) bonding_curve_router: BondingCurveRouter<DynProvider>,
    pub(crate) dex_router: DexRouter<DynProvider>,
    pub(crate) lens: Lens<DynProvider>,
}

pub(crate) struct V2Contracts {
    pub(crate) router: NadFunRouter<DynProvider>,
    pub(crate) factory: NadFunFactory<DynProvider>,
    pub(crate) bonding_curve: BondingCurveV2<DynProvider>,
    pub(crate) token_registry: TokenRegistryV2<DynProvider>,
    pub(crate) token_info_lens: TokenInfoLens<DynProvider>,
    pub(crate) protocol_manager: ProtocolManagerV2<DynProvider>,
}
```

(Keep the doc comments. Only the `struct`/field visibility lines change.)

- [ ] **Step 3: Make `wait_for_receipt` reachable from handle modules**

In `src/core/core.rs`, change the free fn signature (around `core.rs:882`) from `async fn wait_for_receipt(` to:

```rust
pub(crate) async fn wait_for_receipt(
```

- [ ] **Step 4: Add the `v1()` / `v2()` accessors to `impl Core`**

In `src/core/core.rs`, add to `impl Core` (place right after `with_provider`), and add the imports at the top (`use crate::core::v1::CoreV1; use crate::core::v2::CoreV2;`):

```rust
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
```

- [ ] **Step 5: Create empty handle scaffolds**

Create `src/core/v1/handle.rs` (use the re-exported `crate::core::Core` path — guaranteed by `src/core/mod.rs`):

```rust
//! `CoreV1` — v1 namespace handle. Borrows `&Core`; methods delegate to the
//! v1 contract bindings. Construct via [`crate::Core::v1`].

use crate::core::Core;

/// v1 trading/query namespace handle. Zero-cost `Copy` wrapper over `&Core`.
#[derive(Clone, Copy)]
pub struct CoreV1<'a> {
    pub(crate) core: &'a Core,
}

impl<'a> CoreV1<'a> {
    // methods added in Task 3
}
```

Create `src/core/v2/mod.rs`:

```rust
//! Core v2 namespace handle module.

pub mod handle;
pub use handle::CoreV2;
```

Create `src/core/v2/handle.rs`:

```rust
//! `CoreV2` — v2 namespace handle. Borrows `&Core`; methods delegate to the
//! v2 contract bindings. Construct via [`crate::Core::v2`].

use crate::core::Core;

/// v2 trading/query namespace handle. Zero-cost `Copy` wrapper over `&Core`.
#[derive(Clone, Copy)]
pub struct CoreV2<'a> {
    pub(crate) core: &'a Core,
}

impl<'a> CoreV2<'a> {
    // methods added in Task 2
}
```

- [ ] **Step 6: Wire modules + re-exports**

In `src/core/mod.rs` add `pub mod v2;` next to the existing `pub mod v1;`, and add to the re-export block:

```rust
pub use v1::CoreV1;
pub use v2::CoreV2;
```

In `src/core/v1/mod.rs` add `pub mod handle;` and `pub use handle::CoreV1;`.

In `src/lib.rs`, extend the core re-export line and the prelude to include the handles:

```rust
// crate root (was: pub use core::{estimate_gas, Core, GasEstimationParams, Router, SlippageUtils};)
pub use core::{estimate_gas, Core, CoreV1, CoreV2, GasEstimationParams, Router, SlippageUtils};
```

```rust
// in `pub mod prelude` (mirror the crate-root change)
pub use crate::core::{estimate_gas, Core, CoreV1, CoreV2, GasEstimationParams, Router, SlippageUtils};
```

- [ ] **Step 7: Verify it compiles (flat methods still present + empty handles)**

Run: `cargo build`
Expected: PASS. `Core` still has all flat methods; `CoreV1`/`CoreV2` exist but empty; `core.v1()`/`core.v2()` resolve.

- [ ] **Step 8: Commit**

```bash
git add src/core src/lib.rs
git commit -m "refactor(core): scaffold CoreV1/CoreV2 namespace handles"
```

---

## Task 2: Move the v2 surface into `CoreV2` (drop `_v2`)

**Files:**
- Modify: `src/core/v2/handle.rs` (add methods), `src/core/core.rs` (delete flat `*_v2` methods)
- Test: `tests/core_v2_api.rs`

- [ ] **Step 1: Update the v2 surface test to the new form (RED)**

Replace the body of `tests/core_v2_api.rs`'s surface fn so it calls the handle. The whole point of this file is to fail compilation if a v2 method is missing/mis-typed. Rewrite the v2 call(s) using `core.v2()`; e.g. the `buy` assertion becomes:

```rust
        let _ = core.v2().buy(params).await;
```

Apply the same `core.X_v2(...)` → `core.v2().X(...)` rewrite to every v2 call in the file (preserve the existing param structs and imports).

- [ ] **Step 2: Run the test build to confirm RED**

Run: `cargo test --test core_v2_api --no-run`
Expected: FAIL — `no method named 'buy' found for struct 'CoreV2'` (and similar) because `CoreV2` is still empty.

- [ ] **Step 3: Add all v2 methods to `CoreV2` in `src/core/v2/handle.rs`**

Extend the `use` block and fill the `impl<'a> CoreV2<'a>` with the bodies below (each is the original `Core::*_v2` body with `self.v2` → `self.core.v2`, `self.provider` → `self.core.provider`, `self.wallet_address` → `self.core.wallet_address`, `self.network` → `self.core.network`, and `wait_for_receipt(` → `crate::core::core::wait_for_receipt(`). Suffix `_v2` dropped from every method name:

```rust
use crate::core::Core;
use crate::core::core::wait_for_receipt; // pub(crate) free fn (not re-exported)
use crate::{
    api::ApiClient,
    constants::*,
    contracts::{
        BondingCurveV2, NadFunFactory, NadFunRouter, TokenInfoLens, TokenRegistryV2,
    },
    types::{v2::events::IBondingCurveV2Events, *},
};
use alloy::{
    primitives::{Address, B256, U256},
    providers::DynProvider,
    sol_types::SolEvent,
};
use anyhow::{Context, Result};
use std::time::Duration;
```

Methods (copy each body verbatim from `core.rs`, applying the `self.core.` rewrite + name de-suffixing):

- `create`, `create_with_native`, `create_token` (the full `create_token_v2` body, incl. the `wait_for_receipt(&self.core.provider, …)` + Create-event verification).
- exact-in trades: `buy`, `buy_with_native`, `buy_with_permit`, `sell`, `sell_to_native`, `sell_with_permit`, `sell_to_native_with_permit`.
- exact-out trades: `exact_out_buy`, `exact_out_buy_with_native`, `exact_out_sell`, `exact_out_sell_to_native`.
- quotes: `get_amount_out`, `get_amount_in`, `get_bonding_curve_amount_out`, `get_bonding_curve_amount_in`, `get_dex_amount_out`, `get_dex_amount_in`.
- queries: `is_graduated`, `pool_address`, `wrapped_native`, `deploy_fee`, `estimate_gas` (keep the `Address::ZERO` guard, using `self.core.wallet_address`).
- escape hatches (return `&` into `self.core.v2`): `router`, `factory`, `bonding_curve`, `token_registry`, `token_info_lens`.

Example of the mechanical transform (do this for every method):

```rust
    // was Core::buy_v2
    pub async fn buy(&self, params: V2BuyParams) -> Result<B256> {
        self.core.v2.router.buy(params).await
    }

    // was Core::get_amount_out_v2
    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.core.v2.router.get_amount_out(token, amount_in, is_buy).await
    }

    // was Core::token_info_lens()
    pub fn token_info_lens(&self) -> &TokenInfoLens<DynProvider> {
        &self.core.v2.token_info_lens
    }
```

- [ ] **Step 4: Delete the flat `*_v2` methods from `Core` in `src/core/core.rs`**

Remove every `pub async fn *_v2`/`pub fn *_v2`/`router_v2`/`factory_v2`/`bonding_curve_v2`/`token_registry_v2`/`token_info_lens` method body from `impl Core` (the `// v2:` sections, `core.rs:380-790` region). Read the file fresh to confirm current line ranges before deleting. Leave `detect_*`, `get_receipt`, accessors, and all v1 methods untouched for now.

- [ ] **Step 5: Build + run the v2 surface test (GREEN)**

Run: `cargo build && cargo test --test core_v2_api`
Expected: PASS. If `unused import` fires in `core.rs` after deletions, remove the now-unused imports there.

- [ ] **Step 6: Commit**

```bash
git add src/core tests/core_v2_api.rs
git commit -m "refactor(core): move v2 surface to core.v2() handle, drop _v2 suffix"
```

---

## Task 3: Move the v1 surface into `CoreV1`

**Files:**
- Modify: `src/core/v1/handle.rs` (add methods), `src/core/core.rs` (delete flat v1 methods)
- Test: `tests/unified_core_dispatch.rs`

- [ ] **Step 1: Update the dispatch test to the new form (RED)**

In `tests/unified_core_dispatch.rs`, rewrite the v1/v2 calls to handles. The current body uses `core.get_amount_out(...)` (v1) and `core.get_amount_out_v2(...)` (v2). New:

```rust
    // v1 path
    let _ = core.v1().get_amount_out(token, U256::ZERO, true).await;

    match core.detect_version(token).await {
        Ok(SdkVersion::V1) => {
            let _ = core.v1().get_amount_out(token, U256::ZERO, true).await;
        }
        Ok(SdkVersion::V2) => {
            let _ = core.v2().get_amount_out(token, U256::ZERO, true).await;
        }
        Ok(SdkVersion::None) => {}
        Err(_) => {}
    }
```

- [ ] **Step 2: Confirm RED**

Run: `cargo test --test unified_core_dispatch --no-run`
Expected: FAIL — `no method named 'get_amount_out' found for struct 'CoreV1'`.

- [ ] **Step 3: Add all v1 methods to `CoreV1` in `src/core/v1/handle.rs`**

Extend the `use` block and fill `impl<'a> CoreV1<'a>` with the v1 bodies from `core.rs` (`self.v1` → `self.core.v1`, `self.provider` → `self.core.provider`, `self.network` → `self.core.network`; internal calls like `self.get_deploy_fee()` stay `self.get_deploy_fee()` since they are now handle methods):

```rust
use crate::core::Core;
use crate::core::v1::{estimate_gas, GasEstimationParams}; // re-exported by src/core/v1/mod.rs
use crate::{
    api::ApiClient,
    constants::*,
    contracts::{BondingCurveRouter, CreatorClient, DexRouter, Lens},
    types::*,
};
use alloy::{
    primitives::{Address, B256, U256},
    providers::DynProvider,
};
use anyhow::Result;
```

Methods to add (verbatim bodies from `core.rs`, `self.core.` rewrite):
- `get_amount_out`, `get_amount_in` (keep the router-resolution `if/else` comparing against `self.core.v1.dex_router.address` / `self.core.v1.bonding_curve_router.address`).
- `buy`, `sell`, `sell_permit` (the `match router { Router::Dex(_) => self.core.v1.dex_router.…, … }`).
- `available_buy_tokens`, `is_locked`, `is_graduated`, `get_initial_buy_amount_out`, `get_deploy_fee`, `get_progress`.
- `estimate_gas` (body: `estimate_gas(self.core.provider.clone(), router, params).await`).
- `create_token` (full body incl. the `api_client.network() != self.core.network` guard; `self.get_deploy_fee()` call; `self.core.v1.bonding_curve_router.create(...)`).
- `claim_creator_reward`, `claim_creator_rewards_batch` (`get_creator_treasury(self.core.network)`, `CreatorClient::new(treasury, self.core.provider.clone())`).
- escape hatches: `bonding_curve_router`, `dex_router`, `lens` (return `&self.core.v1.…`).

- [ ] **Step 4: Delete the flat v1 methods from `Core` in `src/core/core.rs`**

Remove the moved v1 methods from `impl Core` (`core.rs:159-378` region + v1 escape hatches `757-767`). Read fresh for current line ranges. After this, `impl Core` contains only: `new`, `with_provider`, `v1`, `v2`, `detect_*`, `get_receipt`, `provider`, `wallet_address`, `network`. Remove now-unused imports in `core.rs` (e.g. `CreatorClient`, `BuyParams`, gas imports) — let clippy guide you.

- [ ] **Step 5: Build + run dispatch test (GREEN)**

Run: `cargo build && cargo test --test unified_core_dispatch`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/core tests/unified_core_dispatch.rs
git commit -m "refactor(core): move v1 surface to core.v1() handle"
```

---

## Task 4: Update remaining tests

**Files:** `tests/lifecycle_smoke.rs`, `tests/network_instance.rs`

- [ ] **Step 1: Rewrite Core call sites to handle form**

Read each file. Apply the transform rule to every `core.<method>` call:
- v1 method (see move map) → `core.v1().<method>`
- `core.<method>_v2(...)` → `core.v2().<method>(...)`
- cross-cutting (`detect_version`, `detect_token_info`, `get_receipt`, `provider`, `wallet_address`, `network`) → unchanged.

`tests/lifecycle_smoke.rs` may be `#[ignore]`d live-RPC tests — keep the `#[ignore]` and existing assertions; only the call syntax changes.

- [ ] **Step 2: Build all test targets**

Run: `cargo test --no-run --all-targets`
Expected: PASS (compiles). `network_instance` / `core_v2_api` / `unified_core_dispatch` run green; ignored smokes compile.

- [ ] **Step 3: Commit**

```bash
git add tests/
git commit -m "test: update lifecycle_smoke + network_instance to namespace handles"
```

---

## Task 5: Update examples

**Files:** `examples/core/{buy,sell,sell_permit,gas_estimation}.rs`, `examples/create/create_token.rs`, `examples/creator/claim_reward.rs`, `examples/v2/{buy,sell,buy_erc20_quote,exact_out,create_token,curve_stream,dex_stream,pool_discovery,smoke}.rs`, `examples/unified_dispatch.rs`

Transform rule (same as tests): v1 method → `core.v1().m(...)`; `core.m_v2(...)` → `core.v2().m(...)`; cross-cutting unchanged. Stream/indexer examples that don't touch `Core` trade/quote methods need no change.

- [ ] **Step 1: Update the v1 trading examples**

`examples/core/buy.rs`, `sell.rs`, `sell_permit.rs`, `gas_estimation.rs`: e.g. `core.get_amount_out(...)` → `core.v1().get_amount_out(...)`, `core.buy(params, router)` → `core.v1().buy(params, router)`, `core.estimate_gas(&router, params)` → `core.v1().estimate_gas(&router, params)`.

- [ ] **Step 2: Update create + creator examples**

`examples/create/create_token.rs`: `core.create_token(params, &api)` → `core.v1().create_token(params, &api)`.
`examples/creator/claim_reward.rs`: `core.claim_creator_reward(...)` / `claim_creator_rewards_batch(...)` → `core.v1().…`.

- [ ] **Step 3: Update the v2 examples**

`examples/v2/*`: drop `_v2` and route through `core.v2()`, e.g. `core.buy_v2(p)` → `core.v2().buy(p)`, `core.get_amount_out_v2(...)` → `core.v2().get_amount_out(...)`, `core.create_token_v2(p, &api)` → `core.v2().create_token(p, &api)`. `examples/unified_dispatch.rs`: the `match detect_version` arms call `core.v1().…` / `core.v2().…`; `core.detect_version`/`detect_token_info` stay on `core`.

- [ ] **Step 4: Build all examples**

Run: `cargo build --examples`
Expected: PASS — all examples compile against the new surface.

- [ ] **Step 5: Commit**

```bash
git add examples/
git commit -m "docs(examples): migrate to core.v1()/core.v2() namespace handles"
```

---

## Task 6: Update docs (CHANGELOG, llms.txt, README, EXAMPLES.md)

**Files:** `CHANGELOG.md`, `llms.txt`, `README.md`, `examples/EXAMPLES.md`

- [ ] **Step 1: CHANGELOG `[Unreleased]` breaking entry**

Add under `## [Unreleased]` → `### Changed` (create the subsection if absent):

```markdown
- **BREAKING:** `Core` trading methods moved behind namespace handles
  `core.v1()` / `core.v2()`. The flat `Core::buy`, `Core::get_amount_out`, …
  and all `*_v2` methods are removed; call `core.v1().buy(...)` /
  `core.v2().buy(...)` instead (the `_v2` suffix is dropped). Cross-version
  methods (`detect_version`, `detect_token_info`, `get_receipt`) stay on `Core`.
  New `CoreV1` / `CoreV2` handle types are re-exported from the crate root and
  prelude.
```

- [ ] **Step 2: Regenerate the affected `llms.txt` snippets**

Update every `llms.txt` code snippet / signature that referenced a flat `Core::*` or `*_v2` method to the handle form (`core.v1().*` / `core.v2().*`), and add `CoreV1`, `CoreV2` to the documented re-export list. Grep first: `grep -n "_v2\|core\.\(buy\|sell\|get_amount\|create_token\|estimate_gas\|claim_creator\)" llms.txt`.

- [ ] **Step 3: Update README snippets**

Update the Quick Start and any v1/v2 dispatch snippet in `README.md` to the handle form (match the `core.rs` module-doc example: `core.v1().get_amount_out(...)`, `core.v2().get_amount_out(...)`). Grep: `grep -n "_v2\|core\.\(buy\|sell\|get_amount\|create_token\)" README.md`.

- [ ] **Step 4: Update `examples/EXAMPLES.md`**

Update any inline call snippets to the handle form.

- [ ] **Step 5: Commit**

```bash
git add CHANGELOG.md llms.txt README.md examples/EXAMPLES.md
git commit -m "docs: document core.v1()/core.v2() namespace API"
```

---

## Task 7: Full verification + branch concept doc

**Files:** `branches/feat-v2-namespace.md` (create)

- [ ] **Step 1: Run the full gate**

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --examples
```
Expected: all PASS, 0 clippy warnings. Fix any `unused import` / dead-code fallout in `core.rs` from the deletions.

- [ ] **Step 2: Confirm public surface diff is intentional**

Run: `grep -rn "_v2" src/lib.rs src/core/mod.rs` — expect no `_v2` method re-exports; only the (unchanged) constants helper `get_nadfun_router_v2` and address helpers may legitimately remain. Confirm `CoreV1`/`CoreV2` are exported from crate root + prelude.

- [ ] **Step 3: Write the branch concept doc**

Create `branches/feat-v2-namespace.md` with Purpose (1–3 lines), Changes (the 6 commits), Outcome (left for PR/merge), per the project's `branches/<branch>.md` convention.

- [ ] **Step 4: Commit**

```bash
git add branches/feat-v2-namespace.md
git commit -m "docs: branch concept doc for feat/v2-namespace"
```

- [ ] **Step 5: `/codex review` before opening the PR**

Per CLAUDE.md absolute rule: run `/codex review` (gstack), apply AUTO-FIX immediately, confirm ASK items with the user, include review commits in the same push, then open PR `feat/v2-namespace → v2`.

---

## Self-review

**Spec coverage:**
- Locked decision 1 (borrowed accessor handles) → Task 1 (scaffold), Tasks 2–3 (methods). ✅
- Decision 2 (cross-cutting on Core; rewards on v1) → move map + Task 3 (`claim_creator_reward` under v1; `detect_*`/`get_receipt` kept on Core). ✅
- Phase 1 = no new methods / no behavior change → Tasks 2–3 are verbatim body moves; Phases 2–3 explicitly out of this plan. ✅
- File structure (mirror v1/v2; shrink core.rs) → Task 1 file map + creates. ✅
- Breaking + CHANGELOG + llms.txt + README + examples in same effort → Tasks 5–6. ✅
- ~97 call sites (examples + 4 tests) → Tasks 4–5 (rule + file list). ✅
- `/codex review` per PR → Task 7. ✅

**Placeholder scan:** Call-site edits in Tasks 4–6 are rule-based with representative before/after (the repo's stated convention for mechanical refactors authored against fresh reads) — not vague TODOs. Core handle code (Tasks 1–3) is exact.

**Type consistency:** `CoreV1<'a>`/`CoreV2<'a>` with `pub(crate) core: &'a Core`; accessors return `CoreV1<'_>`/`CoreV2<'_>`; `wait_for_receipt` is `pub(crate)`; field visibility `pub(crate)` — consistent across Tasks 1–3. Handle method names match the move map (de-suffixed v2, unchanged v1).

**Note for executor (R-fresh):** Per the unified-core roadmap convention, confirm current line ranges in `core.rs` with a fresh read before each deletion in Tasks 2 & 3 — the body content is fixed but line numbers drift as you edit.
