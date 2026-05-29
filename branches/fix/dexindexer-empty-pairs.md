# fix/dexindexer-empty-pairs

## Purpose

Guard the v1 `DexIndexer` against an empty `pool_addresses` list. An empty
address filter serializes to no address constraint (alloy's
`FilterSet::to_value_or_array` returns `None` for an empty set), so
`eth_getLogs` matches every Capricorn CL `Swap` log in the range and returns
unrelated pools' swaps as if they belonged to the indexed token.

Surfaced during the round-3 `/codex review` of PR #2 (`refactor/unified-core`),
which flagged the same gap on the v2 `NadFunSwapIndexer`. That fix landed in
PR #2; this branch fixes the v1 sibling on `mainnet` so the v2 line absorbs it
on the next rebase.

## Changes

- `DexIndexer::fetch_events` short-circuits to `Ok(Vec::new())` when
  `pool_addresses` is empty, before building the filter / hitting the RPC.
- `DexIndexer::fetch_all_events` short-circuits too, skipping the needless
  `get_block_number` round-trip.
- Added two `#[tokio::test]` cases proving the empty path returns `Ok([])`
  without querying (provider points at an unreachable port).

Reachability: `discover_pools_for_tokens` skips tokens with no DEX pool, so a
not-yet-graduated token yields an empty `pool_addresses` and triggers the gap.
Happy-path usage (real pool addresses) was never affected.

## Outcome

_Pending PR → `mainnet`._
