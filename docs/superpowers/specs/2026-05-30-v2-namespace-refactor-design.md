# v2 Namespace Refactor + v1↔v2 Parity — Design Spec

> 3-PR effort off `v2`. Written 2026-05-30. Follows the existing
> `docs/superpowers/` convention (cf. `plans/2026-05-30-unified-core-refactor.md`,
> whose Outcome names this work: "the `*_v2` suffix is the seam this next
> refactor (`core.v1()`/`core.v2()` namespace) will remove").

## Purpose

Restructure `Core`'s flat v1/v2 method surface into `core.v1()` / `core.v2()`
namespace handles, dropping every `_v2` suffix (29 public items: 3 create +
11 trade + 6 quote + 5 query + 4 escape-hatch), **and** bring v2 to v1-level
ergonomic parity so users who knew v1 land in v2 without friction. Done now
because 0.4.0 is **unpublished** (last crates.io tag `v0.2.0`), so the `_v2`
surface can disappear with no deprecated-alias cycle.

## Locked decisions

1. **Namespace via accessor methods + borrowed handles.** `core.v1()` /
   `core.v2()` return `CoreV1<'_>` / `CoreV2<'_>`, each a `Copy` wrapper around
   `&Core` (zero-alloc, no lifetime leakage beyond the handle type). `Core`'s
   internal `v1: V1Contracts` / `v2: V2Contracts` fields stay as-is; handle
   methods read `self.core.v1.…` / `self.core.v2.…`.
2. **Cross-cutting methods stay on `Core`**; `detect_*` on `Core`; v1 creator
   rewards on `core.v1()`.
3. **Max v2 parity** where v2 contracts back it — passthrough + computed
   helpers (see Phase 2 / Phase 3). No fabricated methods.
4. **Explicit dispatch preserved** ([[feedback_explicit_dispatch]]) and
   **stateless** ([[feedback_stateless_sdk]]): handles add no state; computed
   helpers do pure math on freshly-fetched on-chain data, no caching.
5. **3 separate PRs** (one `/codex review` each), all branched off `v2`.

## Method placement

### `Core` (cross-cutting / lifecycle) — unchanged surface
`new`, `with_provider`, `v1()`, `v2()`, `detect_version`, `detect_versions`,
`detect_token_info`, `detect_token_infos`, `get_receipt`, `provider()`,
`wallet_address()`, `network()`.

### `core.v1()` — moved from `Core`, no signature change
`get_amount_out`, `get_amount_in`, `buy`, `sell`, `sell_permit`,
`available_buy_tokens`, `is_locked`, `is_graduated`, `get_initial_buy_amount_out`,
`get_deploy_fee`, `get_progress`, `estimate_gas`, `create_token`,
`claim_creator_reward`, `claim_creator_rewards_batch`; escape hatches
`bonding_curve_router()`, `dex_router()`, `lens()`.

### `core.v2()` — `_v2` suffix dropped from every method
Existing: `create`, `create_with_native`, `create_token`, `buy`,
`buy_with_native`, `buy_with_permit`, `sell`, `sell_to_native`,
`sell_with_permit`, `sell_to_native_with_permit`, `exact_out_buy`,
`exact_out_buy_with_native`, `exact_out_sell`, `exact_out_sell_to_native`,
`get_amount_out`, `get_amount_in`, `get_bonding_curve_amount_out`,
`get_bonding_curve_amount_in`, `get_dex_amount_out`, `get_dex_amount_in`,
`is_graduated`, `pool_address`, `wrapped_native`, `deploy_fee`, `estimate_gas`;
escape hatches `router()`, `factory()`, `bonding_curve()`, `token_registry()`,
`token_info_lens()`.

New methods land in Phases 2–3 below.

---

## Phase 1 — Namespace refactor (breaking)

Branch `feat/v2-namespace` off `v2`. Pure restructure: **no new methods, no
behavior change.** Every `_v2` method moves verbatim under `core.v2()` minus the
suffix; every v1 method moves under `core.v1()`.

### File structure (mirror v1/v2 per CLAUDE.md)
- `src/core/core.rs` — `Core` struct + `V1Contracts`/`V2Contracts` +
  `new`/`with_provider` + cross-cutting methods + `v1()`/`v2()` accessors +
  internal builders + `wait_for_receipt`. Shrinks substantially.
- `src/core/v1/handle.rs` (new) — `CoreV1<'a>` + all v1 handle methods.
- `src/core/v2/` (new dir) — `mod.rs` + `handle.rs` for `CoreV2<'a>`.
- `src/lib.rs` + `prelude` — add `pub use` for `CoreV1`, `CoreV2`.

### Public API / breaking
- All flat `Core::buy_v2`, `Core::get_amount_out`, … removed. No deprecated
  aliases (0.4.0 unpublished; `_v2` never shipped; v1 namespacing is part of the
  0.2→0.4 break already in flight).
- `CHANGELOG.md [Unreleased]` (0.4.0): breaking entry. `llms.txt` regenerated.
  `README.md`, `examples/EXAMPLES.md`, all examples + tests updated to the
  handle form (~97 call sites: `examples/{core,create,creator,v2}/*`,
  `examples/unified_dispatch.rs`; `tests/{core_v2_api,lifecycle_smoke,
  network_instance,unified_core_dispatch}.rs`).

### Tests (TDD)
- Dispatch tests: `core.v1().buy` → bonding/dex routing intact;
  `core.v2().*` → router delegation intact. Move/rename existing assertions.
- `cargo build --examples` + `cargo clippy --all-targets -- -D warnings` +
  touched-module tests are the regression gate.

---

## Phase 2 — v2 passthrough parity (additive)

Branch `feat/v2-passthrough-parity` off `v2` (after Phase 1 merges). Each method
is a thin wrapper over an existing on-chain view. Additive — no breaking change.

| New `core.v2()` method | Backing | Binding work |
|---|---|---|
| `is_halted()` | `BondingCurve.isHalted` | binding exists |
| `get_sniping_penalty(token)` | `BondingCurve.getSnipingPenalty` | binding exists |
| `get_curve(token) -> V2Curve` | `BondingCurve.getCurve` | **add wrapper + `V2Curve` type; verify tuple vs deployed** |
| `quote_token(token)` | `BondingCurve.getQuoteToken` | binding exists |
| `quote_config(quote_token) -> V2QuoteConfig` | `ProtocolManager.getConfig` | **extend PM binding + `V2QuoteConfig` type** |
| `is_locked(token)` | `Registry.getPair` → `Pair.isLocked` | resolve pair first; **caveat: post-graduation DEX-pair lock, ≠ v1 curve lock** |
| `get_reserves(token)` | `Registry.getPair` → `Pair.getReserves` | resolve pair first (post-graduation) |
| `get_dex_type(token)` | `Registry.getDexType` | binding exists |
| `is_registered(token)` | `Registry.isRegistered` | binding exists |

New public types (in `src/types/v2.rs`, re-exported): `V2Curve`,
`V2QuoteConfig`. `V2Curve` mirrors the `getCurve` tuple (token, creator,
quoteToken, virtualQuoteReserve, virtualTokenReserve, k, minTokenReserve,
initialQuoteReserve, initialTokenReserve, createdAtBlock, graduated,
creatorFeeRate, version, dexType, pair, graduateFee) — **field set MUST be
verified against the deployed contract's ABI before merge** (see Risk R1).

Tests: each method gets a unit/integration test; new types covered by decode
tests. `llms.txt`/README/examples updated additively.

---

## Phase 3 — v2 computed helpers (additive, curve math)

Branch `feat/v2-computed-parity` off `v2` (after Phase 2). Replicates the
on-chain bonding-curve math client-side for the three v1 utils with no v2 view
function. **Stateless pure math** on freshly-fetched curve/config data.

Verified against `nadfun-contract-v2/src/core/BondingCurve.sol` +
`ProtocolManager.sol` (constant-product on virtual reserves; graduation when
`virtualTokenReserve == minTokenReserve`):

| New `core.v2()` method | Formula (source-verified) | Inputs |
|---|---|---|
| `get_progress(token) -> U256` | `(initialTokenReserve − virtualTokenReserve) · 10000 / (initialTokenReserve − minTokenReserve)`, cap 10000, graduated→10000 | `getCurve(token)` |
| `available_buy_tokens(token) -> (U256, U256)` | available = `virtualTokenReserve − minTokenReserve`; required quote = on-chain `getAmountIn(token, available, true)` | `getCurve` + 1 quote RPC (align tuple order with v1) |
| `get_initial_buy_amount_out(quote_token, amount_in) -> U256` | genesis curve `k = virtualReserve · virtualTokenReserve`, apply `getAmountOut` after curve protocol fee, cap at `virtualTokenReserve − minTokenReserve` | `ProtocolManager.getConfig(quote_token)` |

Note: v2 `get_initial_buy_amount_out` **takes a `quote_token` arg** (v2 curves
differ per quote token; v1's is parameterless because all v1 tokens share fixed
genesis params). Documented divergence.

### Drift mitigation (Rule 9 — mandatory)
- Each formula carries a comment citing the exact contract source location.
- Integration tests cross-check SDK math against on-chain truth on a pinned
  testnet block: e.g. `get_initial_buy_amount_out` vs a freshly created token's
  `getAmountOut` at genesis; `available_buy_tokens` required-quote vs on-chain
  `getAmountIn`; `get_progress` against a recomputed reference.

---

## Out of scope

- **v2 creator-reward claim** — no claim function exists on v2 (creator fees
  auto-route via `creatorFeeRate` + CreatorFeeProcessor). `claim_creator_reward`
  stays v1-only.
- Any contract-side change (e.g. adding a v2 Lens). If a native v2 `getProgress`
  ships later, swap the computed helper for the passthrough.

## Risks

- **R1 — deployed contract variant (BLOCKER for Phase 2/3).** Two source repos
  share remote `Naddotfun/nadfun-contract-v2`: the `minTokenReserve` variant
  (`nadfun-contract-v2`, which the SDK's current `abi/v2` matches, with
  per-quote `curveProtocolFeeRate`) and the `targetTokenAmount` /
  `nadfun-contract-v2-router-create` variant (global `curveProtocolFee()`).
  Math and fee basis differ. **Pin the deployed variant via a live testnet
  `eth_call` (getCurve shape + ProtocolManager fee fns) before writing any
  formula or `V2Curve`/`V2QuoteConfig` type.** Confirm `getCurve` tuple matches
  the SDK ABI; regenerate `abi/v2/*` from the canonical artifact if stale.
  Contract source is local at `/Users/gyu/project/nads-pump/nadfun-contract-v2/`.
- **R2 — exact fee treatment.** `get_initial_buy_amount_out` must replicate the
  precise fee applied in `BondingCurveLibrary` / `_initialBuy`. Pin from source
  during Phase 3 and assert against on-chain in tests.

## Verification (all phases)
`cargo fmt --all` · `cargo clippy --all-targets --all-features -- -D warnings` ·
`cargo test` (touched modules; multi-thread tokio where concurrency matters) ·
`cargo build --examples` · `cargo llvm-cov` 80%+ on touched modules ·
`/codex review` per PR before opening.

## Outcome
_(filled at merge: final summary + key commit hashes + PR links per phase)_
