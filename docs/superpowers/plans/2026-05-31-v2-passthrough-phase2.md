# v2 Passthrough Parity — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) tracking.

**Goal:** Add 9 additive `core.v2()` passthrough methods + 2 public types (`V2Curve`, `V2QuoteConfig`) that wrap existing on-chain v2 views. No breaking change.

**Architecture:** Thin wrappers over `BondingCurveV2` / `ProtocolManagerV2` / `TokenRegistryV2` / `NadFunPair` bindings. Two new tuple-backed types decode contract structs whose field layout is **on-chain verified** (see R1 below).

**Tech Stack:** Rust 2021, alloy 1.0.24, anyhow.

**Spec:** `docs/superpowers/specs/2026-05-30-v2-namespace-refactor-design.md` (Phase 2 section).
**Branch:** `feat/v2-passthrough-parity` off `v2`.

## ✅ R1 RESOLVED (2026-05-31, testnet live eth_call — do NOT re-litigate)

Deployed v2 = **`minTokenReserve` variant** (`nadfun-contract-v2`; SDK `abi/v2` matches). Confirmed: `curveProtocolFeeRate(address)` works, `curveProtocolFee()`/`targetTokenAmount` revert.

**`BondingCurve.getCurve(token)` → 16-field `Curve`** (verified: k == vQuote×vToken):
`(address token, address creator, address quoteToken, uint256 virtualQuoteReserve, uint256 virtualTokenReserve, uint256 k, uint256 minTokenReserve, uint256 initialQuoteReserve, uint256 initialTokenReserve, uint64 createdAtBlock, bool graduated, uint16 creatorFeeRate, uint8 version, uint8 dexType, address pair, uint256 graduateFee)`

**`ProtocolManager.getConfig(quoteToken)` → 10-field `QuoteConfig`** (verified field-by-field vs dedicated getters, pinned block):
`(uint8 decimals, uint256 virtualReserve, uint256 virtualTokenReserve, uint256 minTokenReserve, uint256 deployFee, uint256 graduateFee, uint16 curveProtocolFeeRate, uint16 dexProtocolFeeRate, uint256 settlementThreshold, bool active)`

Test fixtures: testnet RPC `https://dev-node.nadapp.net/`, BondingCurve `0x27063a38eC0D3281D354090EB92e669Ed1eB956C`, ProtocolManager `0x2F98030aBD7c59e3E5Dc6b4b66b6008821d0fB41`, Registry `0x2Bc127be900aD290E703Cd2C71eB0EDCa162C898`, WMON `0x5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd`, V2 token `0x91e0fcbf3e1d51f3fcb9ee9eebbcbff04fc1bf45`.

## Method map (all additive, on `core.v2()` / `CoreV2`)

| Method | Backing binding | Binding work |
|---|---|---|
| `is_halted() -> bool` | `BondingCurveV2::is_halted` | exists |
| `get_sniping_penalty(token) -> U256` | `BondingCurveV2::get_sniping_penalty` | exists |
| `quote_token(token) -> Address` | `BondingCurveV2::get_quote_token` | exists |
| `get_curve(token) -> V2Curve` | `BondingCurveV2::get_curve` (NEW) | add binding + `V2Curve` |
| `quote_config(quote_token) -> V2QuoteConfig` | `ProtocolManagerV2::get_config` (NEW) | extend PM binding + `V2QuoteConfig` |
| `get_dex_type(token) -> u8` | `TokenRegistryV2::get_dex_type` | exists |
| `is_registered(token) -> bool` | `TokenRegistryV2::is_registered` | exists |
| `is_locked(token) -> bool` | Registry `get_pair` → `NadFunPair::is_locked` | resolve pair; caveat (post-grad DEX-pair lock ≠ v1 curve lock) |
| `get_reserves(token) -> V2Reserves` | Registry `get_pair` → `NadFunPair::get_reserves` | resolve pair; reuse existing `PairReserves` (rename/re-export decision) |

## Tasks

### Task 1: `V2Curve` type + `BondingCurveV2::get_curve` binding
- [ ] Add `IBondingCurveV2.getCurve` to the `sol!` interface in `src/contracts/v2/bonding_curve.rs` (16-field struct above) + `pub async fn get_curve(&self, token) -> Result<V2Curve>`.
- [ ] Define `pub struct V2Curve { ... }` in `src/types/v2/` (mirror existing v2 type style; one field per Curve member, alloy types).
- [ ] TDD: a decode unit test using the captured raw bytes for token `0x91e0...bF45` (assert k == vQuote*vToken, token, quoteToken==WMON, graduated==false).
- [ ] Commit.

### Task 2: `V2QuoteConfig` type + `ProtocolManagerV2::get_config` binding
- [ ] Extend `IProtocolManagerV2` `sol!` in `src/contracts/v2/protocol_manager.rs` with `getConfig(address)` (10-field struct above) + `pub async fn get_config(&self, quote_token) -> Result<V2QuoteConfig>`.
- [ ] Define `pub struct V2QuoteConfig { ... }`.
- [ ] TDD: decode unit test against captured WMON config bytes (decimals==18, active==true).
- [ ] Commit.

### Task 3: `CoreV2` passthrough methods (9)
- [ ] Add all 9 methods to `src/core/v2/handle.rs` (take `self` by value, matching the by-value handle convention from Phase 1). `is_locked`/`get_reserves` resolve the pair via `self.core.v2.token_registry.get_pair(token)` first; if pair == ZERO, return a clear "not graduated / no pair" error.
- [ ] Doc-comment the `is_locked` caveat (post-graduation DEX-pair lock, semantically different from v1 `is_locked`).
- [ ] TDD: extend `tests/core_v2_api.rs` `_core_v2_methods_compile` with the 9 new methods (compile-surface check).
- [ ] Commit.

### Task 4: Public re-exports + docs
- [ ] Re-export `V2Curve`, `V2QuoteConfig` (and `V2Reserves`/`PairReserves` if surfaced) from `src/lib.rs` crate root + `prelude`.
- [ ] CHANGELOG `[Unreleased]` → Added entry (9 methods + 2 types).
- [ ] `llms.txt` + README: additive snippets for the new query surface.
- [ ] Commit.

### Task 5: Verify + branch doc + codex
- [ ] `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo build --examples`.
- [ ] Optional live decode test (`#[ignore]`) hitting testnet getCurve/getConfig and asserting the invariants above.
- [ ] `branches/feat-v2-passthrough-parity.md` concept doc.
- [ ] `/codex review` → PR `feat/v2-passthrough-parity → v2`.

## Notes
- `is_locked` returning a *graduated-pair* lock is a deliberate semantic divergence from v1; document, don't hide.
- `get_reserves` only meaningful post-graduation (pre-grad has no pair). Error clearly pre-grad.
- Phase 3 (computed helpers) reuses `get_curve` + `get_config` — keep both types ergonomic for arithmetic (U256 fields public).
