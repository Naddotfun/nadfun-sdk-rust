# Unified-Core Refactor — Master Roadmap

> **For agentic workers:** This is a MASTER ROADMAP, not a leaf plan. Each phase/sub-system is implemented as its own bite-sized plan authored **at phase kickoff** via `superpowers:writing-plans` against the then-current code (refactor steps need exact before/after snippets, which must be read fresh), then executed via `superpowers:subagent-driven-development`. Steps tracked with `- [ ]`.

**Goal:** Remove ~1,500 lines of v1/v2 duplication accumulated during the unified-core merge, fix one security issue, and align deliberate-vs-accidental v1/v2 inconsistencies — without breaking the crates.io public API except in a single intentional breaking batch.

**Architecture:** Land a characterization-test safety net first (core.rs has zero tests today), then internal-only dedup (macros for contract tx-options + trade methods; generic `LogStream<E>`/`LogIndexer<E>` for streams; `parse_response` for api; `resolve_router`/gas helpers for core; `NetworkAddresses` struct for constants), then additive consistency fixes, then a single breaking-cleanup batch.

**Tech Stack:** Rust 2021, alloy 1.0.24 (pubsub/providers/signer-keystore), tokio (multi-thread), anyhow at boundaries, thiserror for matchable errors.

---

## Release context (verified 2026-05-30)

- Last git tag: **`v0.2.0`**. HEAD = `v0.2.0-79-gd689b6f` on `refactor/unified-core`.
- `Cargo.toml` = `0.4.0`; CHANGELOG has a **dated** `## [0.4.0] - 2026-05-28` section but **no `v0.4.0` tag** ⇒ 0.4.0 is **prepared but NOT released** (not on crates.io).
- Baseline is clean: `cargo clippy --all-targets --all-features` = **0 warnings, exit 0**.
- `src` ≈ 9,208 LOC; biggest files: `core/core.rs` (903), `api/mod.rs` (660), `contracts/v2/router.rs` (616), `constants.rs` (508), `types/v1/dex.rs` (460), `token/token.rs` (437).

### Release decision point (needs owner call — gates Phase 3 timing)

| Option | What | Trade-off |
|---|---|---|
| **A — fold into 0.4.0** | Land breaking cleanup (Phase 3) before the 0.4.0 tag | One breaking release, cleanest API. **But** grows the already review-pending 0.4.0 PR #2 → release risk/delay |
| **B — defer to 0.5.0** (roadmap default) | Ship 0.4.0 as-is; do Phases 1–3 in a 0.5.0 cycle | Lower release risk; two breaking jumps (0.2→0.4, 0.4→0.5). Phases 1–2 are non-breaking and can land on `mainnet` anytime |

**Default assumption below: Option B.** Phases 0–2 do not block 0.4.0. Phase 3 batches into 0.5.0 on a fresh `refactor/breaking-cleanup` branch. Revisit if owner picks A.

## Public-API stability contract

- Phases 0–2 touch **no** re-exported surface, or are **purely additive** (safe under pre-1.0 minor bump).
- Phase 3 is the **only** breaking batch → `/plan-eng-review` + RFC in the PR description + `CHANGELOG.md` entry + `llms.txt` regen + README/examples updated **in the same PR**.
- Re-export inventory to diff against (from `src/lib.rs`): `ApiClient, ALLOWED_IMAGE_TYPES, get_creator_manager, get_creator_treasury, get_nadfun_router_v2, Network, get_pool_addresses_for_tokens, CreatorClient, PoolDiscovery, estimate_gas, Core, GasEstimationParams, Router, SlippageUtils, BondingCurveEvent, CurveIndexer, CurveStream, DexIndexer, DexStream, EventType, PoolMetadata, SwapEvent, TokenHelper, SdkVersion, TokenInfo, types::*`.

## Out of scope (verified non-issues — do NOT "fix")

- **Empty `pool_addresses`/`pairs` → full-range swap scan is INTENDED for BOTH v1 and v2** (owner-confirmed 2026-05-30; v2 guards removed the same day in `src/stream/v2/dex/{indexer,stream}.rs`, CHANGELOG `[Unreleased]`). Do NOT re-add guards; v1/v2 empty-filter behavior is now unified. **DONE.**
- **Curve streams use a different filter model on purpose**: the token filter is `Option<Vec<Address>>` (`None` = all tokens), already consistent across v1/v2 (`src/stream/{v1,v2}/curve/stream.rs`). This is distinct from dex's `Vec<Address>` (empty = all) by design — do NOT "unify" curve onto the dex model. Only dex pair-filtering was in scope for the empty-input change.
- `contracts/v2/pair.rs` / `factory.rs` view methods unused by core/stream are **intentionally-reserved** advanced-caller surface (`#![allow(dead_code)]` already present). Not dead code.
- `contracts/v2/router.rs` length (616) is **cohesive** (one binding, one contract). Do not split by concern; only dedup send-vs-estimate param mapping (Task 1c).

## Cross-cutting verification (run after every phase)

```bash
cargo test --all-features                      # narrowest scope per task; full suite per phase
cargo clippy --all-targets --all-features -- -D warnings
cargo build --examples                         # if any public item moved
cargo fmt --all --check
```
Coverage target 80%+ on touched modules (`cargo llvm-cov`). Each phase updates `branches/refactor/unified-core.md` (Changes section) and `CHANGELOG.md [Unreleased]` where user-visible.

---

## Phase 0 — Safety net & security hotfix

**Why first:** `core.rs` has **zero** unit tests (violates the TDD absolute rule) and is the target of Phase 1 extractions. Refactors without a safety net are unverifiable. The API-key hotfix is a real security issue, independent of refactoring, and ships fast.

**Release note:** Phase 0 is non-breaking (tests are additive; the key fix is a behavior tightening). Safe on `mainnet` and inside 0.4.0 if desired.

### Task 0.1 — Characterization tests for `core.rs` pure logic
- **Files:** Test in `src/core/core.rs` (`#[cfg(test)] mod tests`) or `tests/core_pure.rs`.
- **Targets (pure, no RPC):** the router-resolution branch (`core.rs:171-188`, `199-216`), and the guard clauses — network mismatch in `create_token`/`create_token_v2` (`core.rs:317-323`, `412-434`), `creator_address` mismatch, `estimate_gas_v2` `Address::ZERO` guard (`core.rs:741-746`). Some guards may need the precondition split from the RPC call to be reachable without a provider — do that split minimally here.
- **Verification:** new tests pass; they must FAIL if the resolution mapping or a guard is altered (Rule 9 — encode intent).
- **Effort:** M · **API risk:** None.

### Task 0.2 — [SECURITY] Stop leaking `X-API-Key` to non-API hosts
- **Files:** `src/api/mod.rs:104-119` (`request`), check `:156` (`with_api_url`), `:182` (`upload_image_from_uri`).
- **Issue:** `request()` injects `X-API-Key` for any URL when a key is set, including absolute third-party URLs. Latent today (external fetch bypasses it) but the public `get/post` accept arbitrary URLs.
- **Approach (TDD):** only attach the key when the resolved target is `self.api_url` (relative-path branch); external/absolute URLs go key-less. Test: a `request()` to an absolute non-api URL carries no `X-API-Key`; a relative path does.
- **Verification:** new test + existing api tests green.
- **Effort:** S · **API risk:** Internal-only (behavior tightening). · **CHANGELOG:** Security entry.

### Task 0.3 — [DATA] `V2BondingCurveEvent::transaction_index()` + deterministic v2 sort
- **Files:** `src/types/v2/events.rs:173-204` (add accessor; field exists at `:38` for swap), consumers `src/stream/v2/curve/indexer.rs:74-76`, `src/stream/v2/dex/indexer.rs:57`.
- **Issue:** v1 sorts by `(block, tx_index, log_index)`; v2 sorts by `(block, log_index)` only because the accessor is missing. Real v1/v2 divergence and a prerequisite for the unified `sort_key` in Phase 1f.
- **Approach (TDD):** add `transaction_index()`, sort v2 indexers by the full triple. Test ordering within a block.
- **Effort:** S · **API risk:** Additive (accessor on a not-yet-root-exported type).

### Task 0.4 — [VERIFY] On-chain cross-check of v2 Lens / QUOTE constant
- **Files (read-only):** `src/contracts/v2/token_info_lens.rs:110` (`QUOTE` test const = testnet `LV_MON` `src/constants.rs:235`), v2 `TOKEN_INFO_LENS` (`:140` mainnet, `:241` testnet).
- **Action:** No code typo found (v1 `LENS_ADDRESS` ≠ v2 `TOKEN_INFO_LENS`, distinct contracts). But the `QUOTE` fixture coincides with testnet `LV_MON` — call deployed mainnet `getTokenInfo` (`0x40c1…34cB`) on a known token to confirm the quote-token mapping before any release. This is the memory-flagged "Lens confirmation awaiting" merge-blocker for PR #2.
- **Owner/me:** needs a live RPC; do via `examples/` smoke or a one-off. **Not a code change.**
- **Effort:** S · **API risk:** None.

**Phase 0 exit:** core.rs has a pure-logic safety net; key-leak closed; v2 sort deterministic; Lens mapping confirmed. Cross-cutting verification green.

---

## Phase 1 — Internal-only dedup (≈1,500 LOC removed, zero public-API change)

**Order:** 1a→1b→1c (contracts) independent of 1d (core, gated by 0.1) / 1e (api, gated by 0.2) / 1f (stream, gated by 0.3) / 1g (constants). Can parallelize across sub-systems via separate branches; each is its own writing-plans plan + subagent execution.

### Task 1a — Promote `apply_tx_options!` macro to shared, apply across v1
- **Files:** macro source `src/contracts/v2/router.rs:24-51` → relocate to `src/contracts/mod.rs` (`pub(crate)`); apply in `src/contracts/v1/bonding_curve.rs` (7 sites: `76-99, 117-140, 158-181, 204-227, 248-271, 290-313, 339-362`), `src/contracts/v1/dex.rs` (6 sites: `42-65, 84-107, 130-153, 174-197, 216-239, 265-288`), `src/contracts/v1/creator.rs` (2 sites: `45-64, 78-97`).
- **Note:** creator passes `params` by value (`gas_price` moved) — needs a by-ref arm or second macro arm.
- **Removes:** ~270 LOC. **Effort:** M · **API risk:** None (method bodies only).

### Task 1b — Collapse byte-identical v1 `DexRouter`/`BondingCurveRouter` trade methods
- **Files:** `src/contracts/v1/dex.rs:30-292` vs `src/contracts/v1/bonding_curve.rs:105-366` (6 methods identical modulo interface type name — verified). ABIs field-identical (`abi/IDexRouter.json`, `abi/IBondingCurveRouter.json`).
- **Approach:** `macro_rules! impl_v1_trade_methods!($Router, $Iface)` emitting `buy/sell/sell_permit/exact_out_buy/exact_out_sell/exact_out_sell_permit`. Keep both structs + `BondingCurveRouter`-only `create`/`get_deploy_fee`.
- **Removes:** ~260 LOC. **Effort:** M · **API risk:** Internal-only (`contracts::v1::*` not root-exported).

### Task 1c — Dedup vault mapping + send-vs-estimate params in v2 router
- **Files:** `src/contracts/v2/router.rs:71-79`, `100-108`, `426-434` (vault closure ×3); the per-method `INadFunRouter::*Params` builders shared between send methods and their `estimate_gas` arms (`:435-613`).
- **Approach:** `pub(crate) fn map_vaults(...)` (a `From` on the foreign `sol!` type needs a newtype, so use a free fn) + `fn to_buy_params(p)`/`to_sell_params(p)` reused by send + estimate.
- **Removes:** ~80–120 LOC, shrinks the 180-line estimate match. **Effort:** M · **API risk:** Internal-only.

### Task 1d — `resolve_router` + gas `run_estimate` helpers in core
- **Files:** `src/core/core.rs:171-188` & `199-216` → private `fn resolve_router(&self, addr) -> Result<Router>`; `src/core/v1/gas.rs:144-351` (6 arms across `estimate_buy_gas`/`estimate_sell_gas`/`estimate_sell_permit_gas`) → `async fn run_estimate(provider, router_addr, call_data, value: Option<U256>)`.
- **Guarded by:** Task 0.1 tests. Add a `resolve_router` unit test (unknown-address error path).
- **Removes:** ~18 (router) + bulk of ~200 (gas tail). **Effort:** M · **API risk:** None.

### Task 1e — `parse_response` helper in api (dedup + fail-loud fix)
- **Files:** `src/api/mod.rs` — 5 explicit error blocks (`:226, 287, 489, 508`) + 3 that silently `.json()` and drop server error bodies (`:267` `upload_image_bytes`, `:323` `post_salt`, `:555` `get_created_tokens`).
- **Approach:** `async fn parse_response<T: DeserializeOwned>(resp, ctx) -> Result<T>` doing status-check → `ApiErrorResponse` → typed parse with body in error; route all 8 sites through it.
- **Fixes:** Rule 12 fail-loud regression (3 endpoints). **Removes:** ~50 LOC. **Effort:** M · **API risk:** None.

### Task 1f — Generic `LogStream<E>` / `LogIndexer<E>` for stream (highest single LOC win)
- **Files:** all of `src/stream/v1/**` + `src/stream/v2/**`. `fetch_all_events` is the same batching loop in all 4 indexers (`v1/curve/indexer.rs:98-130`, `v2/curve/indexer.rs:83-112`, `v1/dex/indexer.rs:108-131`, `v2/dex/indexer.rs:63-87`); `subscribe()` near-identical across 4 streams.
- **Approach:** internal `pub(crate)` helpers over a tiny trait:
  ```rust
  trait LogDecode: Sized + Send {
      fn decode(log: alloy::rpc::types::Log) -> anyhow::Result<Self>;
      fn sort_key(&self) -> (u64, u64, u64); // (block, tx_index, log_index) — needs Task 0.3
  }
  async fn fetch_all<E, P, F, Fut>(provider, start, batch, fetch_range: F) -> Result<Vec<E>> ...
  fn decode_filter_stream<E: LogDecode>(sub, keep: impl Fn(&E)->bool) -> impl Stream<Item=Result<E>> ...
  ```
  Concrete types keep public names/constructors; per-version delta collapses to `(signatures, address getter, decoder, filter predicate)`. **Both v1 and v2 dex now treat empty input as "no filter" (unified 2026-05-30) — the generic predicate is uniform across versions; no per-version empty-handling delta to preserve.**
- **Gated by:** Task 0.3 (uniform sort key). **Removes:** ~450 LOC. **Effort:** M–L · **API risk:** Internal-only (public types/constructors unchanged).

### Task 1g — `NetworkAddresses` struct backing constants helpers
- **Files:** `src/constants.rs:55-145` & `148-246` (mainnet/testnet const blocks), `:267-504` (~33 `match network` helpers).
- **Approach:** `struct NetworkAddresses { ... v2: V2Addresses }`, `const MAINNET/TESTNET`, `fn addresses(network) -> &'static NetworkAddresses`. Keep every existing `get_*` as a thin wrapper (they are the public/prelude contract — `get_creator_manager`, `get_creator_treasury`, `get_nadfun_router_v2`). Preserve `Option` fields for testnet-only `get_lv_mon_v2`/`get_fee_to_v2` (`:478, 486`).
- **Removes:** ~200 LOC. **Effort:** M–L · **API risk:** None (wrappers kept).

**Phase 1 exit:** ~1,500 LOC of duplication gone; clippy `-D warnings` clean; no re-export diff; examples build.

---

## Phase 2 — Additive consistency (non-breaking)

### Task 2a — Symmetrize event-enum accessors
- **Files:** `src/types/v1/bonding_curve.rs` (add `transaction_hash()`; has `transaction_index()` at `:206`), `src/types/v2/events.rs` (`transaction_index()` added in 0.3; ensure `transaction_hash()` parity at `:184`).
- **Effort:** S · **API risk:** Additive.

### Task 2b — `Copy` + `all()` on v1 `EventType`
- **Files:** `src/types/v1/bonding_curve.rs:58` (derive list), add `all()` mirroring v2 `V2EventType::all()` (`src/types/v2/events.rs:47`).
- **Effort:** S · **API risk:** Additive (adding a derive/method is non-breaking).

### Task 2c — Rustdoc for deliberate asymmetries
- **Files:** `src/core/core.rs:629-705` (v2 quote returns bare `U256`, no `Router` to thread back — unlike v1 `:165-189`); `src/core/core.rs:882-903` (`wait_for_receipt` vs `get_receipt` `:246-260`; note v1 `create_token` skips receipt verification that v2 does); **empty-`pool_addresses`/`pairs` = full-scan is intended** (v2 indexer/stream already documented 2026-05-30; add the same note to v1 `src/stream/v1/dex/indexer.rs` for symmetry).
- **Effort:** S · **API risk:** None (doc-only). Regen `llms.txt` if wording surfaces there.

**Phase 2 exit:** v1/v2 surfaces symmetric where intended; deliberate gaps documented.

---

## Phase 3 — Breaking cleanup (single batch — 0.5.0 by default, or fold into 0.4.0 per decision point)

> **Gate:** `/plan-eng-review` + RFC in PR + CHANGELOG + `llms.txt` regen + README/examples updated in the same PR. Fresh branch `refactor/breaking-cleanup`.

### Task 3a — Remove deprecated `create/` module
- **Files:** delete `src/create/`, drop `pub mod create` at `src/lib.rs:64`. `TokenCreationClient` has **zero** call sites (src/examples/tests); `ALLOWED_IMAGE_TYPES` already canonical via `api`. `examples/create/` uses `ApiClient` directly — no example breaks.
- **Effort:** S · **API risk:** BREAKING (removes a `#[deprecated]` re-export). Since 0.4.0 is untagged, can land in 0.4.0 if Option A.

### Task 3b — Standardize indexer/stream provider injection
- **Files:** `src/stream/v1/dex/indexer.rs:30` (`new(rpc_url, ...)`, owns `connect_http`) → `new(provider: Arc<P: Provider + Clone>, ...)` matching curve indexers + v2 dex (`src/stream/v2/dex/indexer.rs:19`); align the 4 streams (currently `Arc<DynProvider>`) on the generic shape. Add `discover_*` convenience constructors that build the provider. Unblocks mock-transport tests for v1 indexers.
- **Effort:** M · **API risk:** BREAKING (`DexIndexer::new` signature, stream generic params).

### Task 3c — Trim `PoolMetadata` stream re-export
- **Files:** `src/stream/mod.rs:34` (+ `src/lib.rs:103, 134`) — `PoolMetadata` unused inside `stream/`. Keep it exported from `types`; drop the stream/prelude re-export.
- **Effort:** S · **API risk:** BREAKING (in prelude + crate root).

### Task 3d — Naming + event-location symmetry
- **Files:** add aliases or rename `NadFunSwapStream`/`NadFunSwapIndexer`/`NadFunSwapEvent` ↔ `CurveStreamV2`/`CurveIndexerV2` for a consistent v2 scheme (`src/stream/v2/mod.rs:9-10`, `src/stream/mod.rs:16-21`); move `NadFunSwapEvent`+decoder from `src/stream/v2/dex/events.rs` to `src/types/v2/dex.rs` (mirror v1 `types/v1/dex.rs`), keep `sol!` binding under `contracts/v2/`, re-export from `stream/v2/dex` for path compat.
- **Effort:** M · **API risk:** BREAKING if renamed (additive aliases are non-breaking — prefer aliases to defer the break).

**Phase 3 exit:** one clean breaking surface; CHANGELOG/llms.txt/README/examples synced; `/codex review` before PR.

---

## Self-review (spec coverage)

- Core findings (resolve_router, gas, tests) → 0.1, 1d, 2c ✅
- contracts/types (apply_tx_options, trade methods, vault map, event drift, derives) → 1a, 1b, 1c, 2a, 2b ✅
- stream (generic abstraction, v2 sort, naming, events.rs location) → 0.3, 1f, 3b, 3c, 3d ✅; empty-filter v1/v2 unification (v2 guards removed) → **DONE 2026-05-30** ✅
- api/token/constants/create (parse_response, key-leak, NetworkAddresses, create removal, permit dedup, Lens) → 0.2, 1e, 1g, 3a, 0.4 ✅; **NOTE:** `token/token.rs` EIP-712 permit dedup (manual keccak → alloy `sol!` EIP-712, `:221-297`) is an additional P2 internal refactor — **add as Task 1h** when kicking off Phase 1 (Effort M, API risk Internal-only). Tracked here so it isn't dropped.
- Out-of-scope: reserved view methods + router.rs split excluded ✅; empty-filter NOT excluded — resolved by unifying v1/v2 (v2 dex guards removed 2026-05-30, see Out of scope) ✅
```
Task 1h — EIP-712 permit via alloy sol! (deferred detail)
Files: src/token/token.rs:221-297 (generate_permit_signature, build_domain_separator) + test mirror :329-407.
Approach: define Permit via alloy::sol!, use SolStruct::eip712_signing_hash vs on-chain DOMAIN_SEPARATOR(); const typehash. Keep public method shapes.
```
```
