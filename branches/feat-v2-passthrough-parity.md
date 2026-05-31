# Branch: `feat/v2-passthrough-parity`

## Purpose

Phase 2 of the v2 namespace work (off `v2`, after PR #4 merged). Adds v2
passthrough parity: thin `core.v2()` wrappers over existing on-chain v2 views,
plus two new public types. Additive only — no breaking change. Spec:
`docs/superpowers/specs/2026-05-30-v2-namespace-refactor-design.md` (Phase 2);
plan: `docs/superpowers/plans/2026-05-31-v2-passthrough-phase2.md`.

## R1 resolved (testnet live eth_call, 2026-05-31)

Deployed v2 = the **`minTokenReserve` variant** (SDK `abi/v2` matches). Confirmed
via discriminator calls: `curveProtocolFeeRate(address)` / `getMinTokenReserve`
succeed; `curveProtocolFee()` / `targetTokenAmount` revert. The `getCurve`
(16-field) and `getConfig` (10-field) tuple layouts were verified field-by-field
on-chain. See [[reference_v2_contract_variant]].

## Changes

- **Plan** (`eccbebb`): Phase 2 plan with the R1-verified tuples baked in.
- **`V2Curve` + `get_curve`** (`c30420d`): `BondingCurveV2::get_curve` binding +
  16-field `V2Curve` type. `getCurve` was already in `abi/v2/BondingCurve.json`.
  Offline decode test asserts `k == vQuote * vToken`.
- **`V2QuoteConfig` + `get_config`** (`97c9c64`): `ProtocolManagerV2::get_config`
  binding + 10-field `V2QuoteConfig` type. `From`-impl mapping shared by the
  binding and a distinct-per-field round-trip test (catches any field swap).
- **9 `core.v2()` methods** (`e872f99`): `is_halted`, `get_sniping_penalty`,
  `quote_token`, `get_curve`, `quote_config`, `get_dex_type`, `is_registered`,
  `is_locked`, `get_reserves`. `is_locked` / `get_reserves` resolve the pair via
  the registry and error before graduation; `is_locked` is the post-graduation
  pair lock (caveat-documented, distinct from v1 curve lock). Re-exported
  `V2Curve` / `V2QuoteConfig` (via `types::*`) + `PairReserves` from crate root
  and prelude. Compile-surface test extended.
- **Docs** (`9b8e0d3`): CHANGELOG `[Unreleased]` Added entry; llms.txt + README
  v2 query snippets.
- **Live drift guards** (this commit): `tests/v2_views_live.rs` (`#[ignore]`) —
  asserts the `getCurve` k-invariant and `getConfig` WMON decode against live
  testnet. Both pass (run `cargo test --test v2_views_live -- --ignored`).

## Verification

`cargo fmt --all --check`, `cargo clippy --all-targets --all-features -D warnings`,
`cargo test`, `cargo build --examples` — all clean. Live `#[ignore]` tests pass
against `dev-node.nadapp.net`.

## Outcome

Phase 2 complete. PR: https://github.com/Naddotfun/nadfun-sdk-rust/pull/5
(`feat/v2-passthrough-parity → v2`).

`/codex review` (codex 0.133.0, prompt-only form) found **4 P2s** across several
rounds, all fixed; the final confirmation read in full returned **0 P1 / 0 P2**
("no remaining correctness issues … cargo test, ignored live v2 view tests,
clippy, example builds all pass"):
- **Stale CHANGELOG/llms.txt** (`a91a9cc`) then **stale README** (`62abb8c`):
  the public API changed but the docs weren't updated — earlier doc edits this
  branch silently no-op'd (bad edit anchors). Added the 9 methods + 3 view types
  to CHANGELOG, llms.txt, and the README "v2 view / query parity" section.
- **Pre-graduation gating bug** (`a727f64`): the registry assigns a DEX pair at
  token creation, so the old `get_pair == ZERO` guard never caught the
  pre-graduation case `is_locked`/`get_reserves` advertised. Now gates on
  `router.is_graduated` via a shared `resolve_graduated_pair` helper.
- **`PairReserves` not re-exported** (`a727f64`): `get_reserves`' return type
  lived only in `contracts::v2`; now exported from the crate root + prelude
  alongside `V2Curve`/`V2QuoteConfig`.
- Also fixed before review: a mid-branch build break (`f2ac0d7`) — the new view
  types weren't re-exported through the explicit `types/mod.rs` list — and a
  `get_reserves` reserve-ordering doc clarification (`21e60af`).

All gates (`build` / `clippy -D warnings` / `test` / `fmt` / `examples`) and the
live `#[ignore]` drift guards verified green at HEAD.

## Update — full testnet coverage + efficiency fix (HEAD 3f8a09e)

- **Live testnet coverage for all 9 views** (`45d0e3e`): `tests/v2_views_live.rs`
  expanded 2 → 5 `#[ignore]` tests — every `core.v2()` view exercised against
  `dev-node.nadapp.net` (graduated token `0x78D804…7777`, registered-not-grad
  `0xf9F9…7777`, burn sink for the gate). **5 passed / 0 failed.** Two on-chain
  facts pinned: `k == vQuote*vToken` is pre-graduation only (frozen at genesis
  after), and `is_graduated()` reverts for unregistered tokens.
- **Single-`get_curve` pair resolution** (`3f8a09e`, Codex P2): `is_locked` /
  `get_reserves` resolve the graduated pair with one `get_curve` round-trip
  (returns both `graduated` + `pair`) instead of `is_graduated` + `get_pair`.
  Behavior unchanged; final codex re-review returned 0 findings.
