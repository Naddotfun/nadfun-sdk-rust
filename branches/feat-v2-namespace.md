# Branch: `feat/v2-namespace`

## Purpose

Phase 1 of the v2 namespace refactor: move `Core`'s flat v1/v2 trading surface
behind borrowed namespace handles `core.v1()` / `core.v2()`, dropping every
`_v2` suffix. Pure restructure — no new methods, no behavior change. Done while
0.4.0 is still unpublished so the `_v2` surface disappears with no deprecation
cycle. Spec: `docs/superpowers/specs/2026-05-30-v2-namespace-refactor-design.md`;
plan: `docs/superpowers/plans/2026-05-30-v2-namespace-phase1.md`.

## Changes

- **Scaffold** (`2d830b5`): `CoreV1<'a>` / `CoreV2<'a>` borrowed `Copy` handles
  (`{ pub(crate) core: &'a Core }`); `core.v1()`/`core.v2()` accessors;
  `Core`/`V1Contracts`/`V2Contracts` fields + `wait_for_receipt` widened to
  `pub(crate)`; module wiring + crate-root/prelude re-exports of `CoreV1`/`CoreV2`.
- **Move v2 surface** (`d67578f`): 30 v2 methods relocated `impl Core` →
  `impl CoreV2` with the `_v2` suffix dropped (3 create + 7 exact-in + 4
  exact-out + 6 quote + 5 query + 5 escape-hatch); bodies verbatim modulo
  `self.X` → `self.core.X`; flat `*_v2` deleted from `Core`;
  `tests/core_v2_api.rs` migrated.
- **Move v1 surface** (in `d67578f` / `46d2c27`): 18 v1 methods relocated
  `impl Core` → `impl CoreV1`; `estimate_gas` calls the free fn via aliased
  import; `tests/unified_core_dispatch.rs` gains a v1 surface compile-check.
  `impl Core` now holds only the 12 cross-cutting/lifecycle methods (`new`,
  `with_provider`, `v1`, `v2`, `detect_version[s]`, `detect_token_info[s]`,
  `get_receipt`, `provider`, `wallet_address`, `network`).
- **Examples** (`6c7ecb2`): all `examples/{core,create,creator,v2}/*` +
  `unified_dispatch.rs` migrated to the handle API; `examples/v2/smoke.rs`
  binds `core.v2()` to a local to satisfy escape-hatch borrow lifetimes.
- **Tests** (`a7ce89d`): `lifecycle_smoke.rs` migrated; `network_instance.rs`
  needed no change (cross-cutting only).
- **Docs** (`89e8684`, `0704e30`): CHANGELOG `[Unreleased]` BREAKING entry;
  `llms.txt`, `README.md`, `examples/EXAMPLES.md`, and module rustdoc
  (`core/core.rs`, `core/mod.rs`, `core/v1/mod.rs`, crate `lib.rs`) rewritten to
  the handle API.
- **Format** (`2f7835a`, `e23261a`): `cargo fmt` on the refactor-touched files,
  then a separate commit for the pre-existing `indexer.rs` debt.
- **Codex review fix** (`4f9c2db`): `/codex review` GATE **PASS** (0 P1, 1 P2).
  P2 AUTO-FIX — `core.v1()` / `core.v2()` methods now take `self` by value (the
  handle is `Copy`) and escape hatches return `&'a _`, so a stored future
  (`let f = core.v1().get_amount_out(..); f.await`) or a stored escape-hatch
  ref (`let r = core.v2().router();`) compiles instead of failing E0716. Added
  regression compile-test `_stored_handle_results_compile`.

## Verification

`cargo build`, `cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test` (all suites), `cargo build --examples` — all clean. Final
opus-level body-diff review against base (`749d8a5`) confirmed **zero behavior
drift** across all 48 relocated methods and an intact public surface (only
additive `CoreV1`/`CoreV2` + `v1()`/`v2()`).

Known pre-existing debt (NOT introduced here, intentionally left): `cargo fmt
--all --check` still reports `src/stream/v2/dex/indexer.rs` — that file is
byte-identical to base on this branch; fixing it belongs to a separate cleanup,
not this refactor.

## Outcome

Phase 1 complete and codex-reviewed (GATE PASS). PR:
https://github.com/Naddotfun/nadfun-sdk-rust/pull/4 (`feat/v2-namespace → v2`).
