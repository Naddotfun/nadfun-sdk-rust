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

_(running list — append as work lands)_

- [ ] (placeholder)

## Outcome

_(fill at PR/merge: final summary + key commit hashes + PR link)_
