# refactor/unified-core

> **Base**: `v2` · **Target**: `mainnet` (release PR for `0.4.0`)
> **Started**: 2026-05-28

## Purpose

Collapse `Core` and `CoreV2` into a single `Core` that handles v1 and v2 tokens
(bonding-curve trade, token create, streams) from one instance, and remove the
process-global `set_network` lock in favor of a `Network` field on each entry
point (`Core`, `ApiClient`, `CurveStream`, `DexStream`, indexers, `PoolDiscovery`).
Bundles the Codex P1/P2/P3 findings from the v2 review into the same minor
release.

Ships as `0.4.0` (pre-1.0 minor breaking allowed; SemVer-safe).

## Changes

- [x] Phase A — drop `set_network` global, thread `Network` through every
      entry point (`Core`, `ApiClient`, streams, indexers, `PoolDiscovery`,
      `PoolMetadata`, `constants::get_*` helpers). Closes Codex P1 #5 + #7.
- [x] Phase B — collapse `CoreV2` into unified `Core` with v2 surface
      exposed as `*_v2` methods. Add `Core::detect_version(token)` dispatch
      primitive with in-process cache.
- [x] Phase C — Codex P1 close-outs:
      - #1: v2 event decoder uses `SolEvent::decode_log` so indexed
        fields land in topics, not zero.
      - #3: `create_token_v2` verifies the on-chain Create event matches
        the predicted token address.
      - #4: `V2CreatePayment::Native` drops `value`; `msg.value` is drawn
        from `buy_quote_amount`.
      - #6: `ApiTokenInfo.version` deserializer tolerates explicit `null`.
      - #8: `cargo clippy --all-targets --all-features -- -D warnings`
        clean.
- [x] Phase D — Codex P2: `estimate_gas` rejects `Address::ZERO` (#9),
      `V2BuyWithNativeParams.value` field (#10), `discover_pools_unified`
      uses `TokenRegistryV2::getPair` (#11), `V2PreparedCreation` carries
      server-normalized name/symbol (#15).
- [x] Phase E — Codex P3: `VaultType::Custom #[serde(other)]` (#17),
      `NadFunSwapStream` rejects empty pairs (#18).
- [x] Phase F — `Cargo.toml` `0.4.0`, `CHANGELOG.md` `[0.4.0]` section,
      `MIGRATION.md`, `README.md` rewrite, `llms.txt` rewrite.

## Outcome

_(fill at PR/merge: final summary + key commit hashes + PR link)_
