---
name: testnet-live
description: Run the SDK's #[ignore]d live testnet tests against a real Monad RPC. Default runs the 8 read-only view/dispatch checks (no funds, no private key needed). Use --full to also run the 2 funded lifecycle smokes (real txs, spends testnet funds). Use when asked to "run the live tests", "verify on testnet", "check the v2 utils on-chain", "testnet smoke", or "confirm it works against the real chain".
---

# testnet-live

Runs the live, network-touching tests that `cargo test` skips by default (they are `#[ignore]`d because they hit a real RPC). Use this to get a *fresh green run* of the on-chain behavior — not just the offline unit/integration suite.

## Two tiers (cost differs — read this first)

| Tier | Tests | Key needed | Cost |
|------|-------|-----------|------|
| **read-only** (default) | `v2_views_live` (7) + `unified_core_dispatch` (1) = **8** | none — falls back to an ephemeral key | `eth_call` only, **free** |
| **lifecycle** (`--full`) | `lifecycle_smoke` (2: v1 + v2 lifecycle) | **funded** `TESTNET_PRIVATE_KEY` | **real transactions, spends testnet funds** |

Default to read-only. Only run `--full` when the user explicitly asks and confirms a funded key is set — it sends real txs.

## Environment

All optional except where noted:

- `TESTNET_RPC_URL` — overrides the in-repo default testnet node (`DEFAULT_TESTNET_RPC`). Set this to point at a different RPC.
- `TESTNET_PRIVATE_KEY` — **required and must be funded** for `--full`. Read-only tier ignores it (uses an ephemeral key).
- `TESTNET_V2_TOKEN` / `TESTNET_V2_GRADUATED_TOKEN` / `TESTNET_V1_TOKEN` — optional address overrides; tests use pinned defaults if unset.

Never hardcode keys. Read them from the environment only.

## Steps

1. **Pick the tier.** Default = read-only. `--full` = read-only + lifecycle. If `--full`, first confirm with the user that `TESTNET_PRIVATE_KEY` is set and funded (real txs spend money) before running.

2. **Run, capturing output to a file** (so results can be read back, not guessed):

   Read-only (default):
   ```bash
   cargo test --test v2_views_live --test unified_core_dispatch -- --ignored \
     2>&1 | tee /tmp/testnet-live.txt
   ```

   Full (`--full`, funded key required):
   ```bash
   cargo test --test v2_views_live --test unified_core_dispatch --test lifecycle_smoke \
     -- --ignored 2>&1 | tee /tmp/testnet-live.txt
   ```

3. **Read the result file before reporting.** This is a hard rule for this repo (past sessions logged fake "GATE PASS" by trusting a streamed tail). Confirm with:
   ```bash
   grep -E "test result:|FAILED|panicked|error\[" /tmp/testnet-live.txt
   ```
   Then `Read` the file if anything is non-obvious.

4. **Report honestly.** Sum `passed` / `failed` across all `test result:` lines. State exactly which tier ran. If any test `FAILED`, surface the failure output — do not round up to "passed". If the read-only default ran, say plainly that the funded lifecycle tier was *not* run.

## Notes

- These tests are read-only `eth_call`s (default tier) — safe to run repeatedly, no state change.
- The k == vQuote·vToken invariant only holds pre-graduation; the tests already account for this (graduated tokens read stored genesis k). Don't "fix" an apparent mismatch on a graduated token.
- `isGraduated(unregistered_token)` reverts (returns `Err`, not `Ok(false)`) — expected, the tests assert it.
- If a test fails, drop into `superpowers:systematic-debugging` and reproduce against testnet before touching contract code. Pin the token/block when reproducing.
