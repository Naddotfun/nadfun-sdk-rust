# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-05-28

### Added — unified `Core` (v1 + v2 from one instance) + v2 contract support

The SDK now ships a **single `Core`** that wires both the v1 (bonding curve
+ Capricorn CL DEX) and v2 (NadFunRouter + per-token registry + vaults)
contract surfaces. v1 trades go through the existing `buy` / `sell` /
`get_amount_out`; v2-only paths are exposed as `*_v2` methods. The
process-global `set_network` lock is gone — every entry point binds to a
`Network` at construction.

- **`Core`** absorbs the v0.3 `CoreV2`. v2 surface:
  - Trading: `buy_v2`, `buy_with_native_v2`, `buy_with_permit_v2`, `sell_v2`,
    `sell_to_native_v2`, `sell_with_permit_v2`,
    `sell_to_native_with_permit_v2`, `exact_out_buy_v2`,
    `exact_out_buy_with_native_v2`, `exact_out_sell_v2`,
    `exact_out_sell_to_native_v2`.
  - Creation: `create_v2`, `create_with_native_v2` (low-level), and
    `create_token_v2(V2CreateTokenParams, &ApiClient)` (end-to-end). The
    high-level path now verifies the on-chain `Create` event matches the
    predicted token address.
  - Quotes (price): `get_amount_out_v2`, `get_amount_in_v2`,
    `get_bonding_curve_amount_out_v2(_in)`, `get_dex_amount_out_v2(_in)`.
    Renamed from `quote_*` so the word "quote" is reserved for "quote
    token" (the trade's pricing currency, e.g. WMON / LvMON / USDT).
  - Pool / state: `is_graduated_v2`, `pool_address_v2`, `wrapped_native_v2`.
  - Gas: `estimate_gas_v2(V2GasEstimationParams)`.
  - Escape hatches: `router_v2()`, `factory_v2()`, `bonding_curve_v2()`,
    `token_registry_v2()` — return `&_` directly. v2 is always wired
    (`Core::new` fails loudly during construction if a future network
    ships without it), so no dead-branch guard is needed.
  - `Core::detect_version(token)` / `Core::detect_versions(tokens)` —
    on-chain version classification via `TokenVersionLens` when deployed
    (one RPC call covers both v1 + v2 registries; explicit `None` for
    unknown tokens) or `TokenRegistryV2::getPair` fallback. Result is
    cached in-process. Use this to dispatch user code at v1 / v2
    boundaries.
  - `SdkVersion::None` — token isn't registered on either system. New
    variant returned by `detect_version` when the Lens is wired.

- **`SdkVersion`** enum — `V1` / `V2` discriminator used by
  `SaltParams.version`, `ApiTokenInfo.version`, and user-side dispatch.

- **`ApiClient`** v2 surface:
  - `get_token(token)` returns `ApiTokenInfo` with the `version` field
    (null-tolerant deserialization).
  - `get_token_vaults(token)` returns `VaultState` for the v2 vault model
    (BurnVault, LPVault, CreatorFeeVault, GiftVault).
  - `prepare_token_creation_v2(&V2PrepareCreationParams)` returns a
    `V2PreparedCreation` that now carries server-normalized `name` and
    `symbol` strings (used as the actual on-chain inputs).
  - `SaltParams.version: Option<SdkVersion>` — `None` (v1, default) omits
    the field on the wire; `Some(V2)` enables v2 CREATE2 mining.

- **Streaming + indexing (v2)**:
  - `CurveStreamV2`, `CurveIndexerV2` — v2 `BondingCurve` events
    (`V2BondingCurveEvent` enum: Create / Buy / Sell / Sync / Graduate /
    SnipingPenalty).
  - `NadFunSwapStream`, `NadFunSwapIndexer` — `NadFunPair::Swap` events.
  - `discover_pools_unified(provider, tokens, network)` — surfaces pools
    from v1 (Capricorn CL) and v2 (`TokenRegistryV2::getPair`) in one call.

- **Examples** (`examples/v2/` + `examples/unified_dispatch.rs`):
  `v2_buy`, `v2_sell`, `v2_buy_erc20_quote`, `v2_exact_out`,
  `v2_create_token`, `v2_curve_stream`, `v2_dex_stream`,
  `v2_pool_discovery`, `unified_dispatch`, `v2_smoke`.

### Changed

- Internal source layout reorganized into version directories:
  `src/{core,contracts,types,stream}/{v1,v2}/`. The unified `Core` lives
  at `src/core/core.rs`.
- `src/types/mod.rs` and `src/lib.rs` switched from `pub use *` glob to
  explicit re-exports — internal alloy `sol!`-generated types
  (`IBondingCurve`, `ICapricornCLPool`, `IBondingCurveV2Events`,
  `INadFunRouter`, etc.) are no longer publicly visible at the crate
  root (Codex P1 #7).

### Fixed

- v2 event decoder reads indexed fields from topics via
  `SolEvent::decode_log(&log.inner)`. The pre-0.4 `decode_log_data` form
  silently zeroed `creator` / `token` / `pair` / `buyer` / `seller`
  (Codex P1 #1).
- `Core::create_token_v2` waits for the receipt and asserts the on-chain
  `Create` event matches the API-predicted token address (Codex P1 #3).
- `ApiTokenInfo.version` tolerates explicit `null` on the wire — not
  just missing-field (Codex P1 #6).
- `PoolDiscovery::get_pools_for_tokens` uses the network-correct WMON
  (Codex P1 #5).
- `estimate_gas` (v1) rejects `Address::ZERO` as `to` (which doubles as
  the from-address for the estimate); `Core::estimate_gas_v2` errors
  when `wallet_address == Address::ZERO` (Codex P2 #9).
- `discover_pools_unified` uses `TokenRegistryV2::getPair` instead of
  guessing a WMON pair via the factory — works for non-WMON quote tokens
  (Codex P2 #11).
- `Core::create_token_v2` uses server-normalized name/symbol from
  `V2PreparedCreation` for the on-chain create call (Codex P2 #15).
- `VaultType::Custom` is marked `#[serde(other)]` — unknown server-side
  vault types deserialize to `Custom` instead of failing the response
  (Codex P3 #17).
- `NadFunSwapStream::new` rejects empty pair lists (Codex P3 #18).
- `cargo clippy --all-targets --all-features -- -D warnings` clean
  (Codex P1 #8).

### Breaking changes

- **`CoreV2` removed.** Use `Core::*_v2` methods. See [`MIGRATION.md`].
- **`set_network` / `get_current_network` removed.** `Network` is now an
  instance field on every entry point.
- `ApiClient::new()` → `ApiClient::new(network)`.
- `ApiClient::from_env()` → `ApiClient::from_env(network)`.
- `CurveStream::new(rpc_url)` → `CurveStream::new(rpc_url, network)`.
  Same shape for `DexStream`, `CurveStreamV2`, `NadFunSwapStream`.
- `CurveIndexer::new(provider)` → `CurveIndexer::new(provider, network)`.
  Same shape for `DexIndexer`, `CurveIndexerV2`, `NadFunSwapIndexer`.
- `PoolDiscovery::new(provider)` → `PoolDiscovery::new(provider, network)`.
- `PoolMetadata::new()` → `PoolMetadata::new(network)`.
- `get_pool_addresses_for_tokens(provider, tokens)` →
  `get_pool_addresses_for_tokens(provider, tokens, network)`.
- `discover_pools_unified(provider, tokens, factory_v2_address)` →
  `discover_pools_unified(provider, tokens, network)`.
- `constants::get_*()` helpers all take a `Network` argument.
- `V2CreatePayment::Native { value }` → `V2CreatePayment::Native`. The
  native amount is now drawn from `V2CreateTokenParams.buy_quote_amount`
  (Codex P1 #4).
- `V2BuyWithNativeParams` gains a `value` field; `Core::buy_with_native_v2`
  is single-arg, and `V2GasEstimationParams::BuyWithNative` collapses to
  a tuple variant (Codex P2 #10).
- `V2PreparedCreation` gains `name` and `symbol` fields (Codex P2 #15).
- `addresses::mainnet::*` no longer flat-exports v1 names — the
  `pub use mainnet::*;` default in `addresses` is gone.
- `lib.rs` switched from `pub use types::*;` to explicit re-exports;
  alloy `sol!`-generated types are no longer publicly visible at the
  crate root.

[`MIGRATION.md`]: ./MIGRATION.md

## [0.3.12] - 2025-02-02

### Added

- **API Key Authentication** - Optional API key support for higher rate limits
  - `ApiClient::new()` - No auth (10 req/min)
  - `ApiClient::from_env()` - Load API key from `NAD_API_KEY` environment variable
  - `ApiClient::new().with_api_key()` - Explicit API key (100 req/min)

- **Creator Rewards** - Claim trading fee rewards for created tokens
  - `ApiClient::get_created_tokens()` - Query created tokens with reward info
  - `ApiClient::build_claim_params()` - Build claim parameters from reward info
  - `ApiClient::build_batch_claim_params()` - Build batch claim parameters
  - `Core::claim_creator_reward()` - Claim reward for single token
  - `Core::claim_creator_rewards_batch()` - Batch claim for multiple tokens

### Changed

- **API URL Updated** - Mainnet API URL changed to `https://api.nadapp.net`
- **Agent API Paths** - Token creation now uses `/agent/*` endpoints
  - `/agent/token/image` - Image upload
  - `/agent/token/metadata` - Metadata creation
  - `/agent/salt` - Salt generation
  - `/agent/token/created/:address` - Get created tokens
- **Simplified Architecture** - `TokenCreationClient` deprecated, use `ApiClient` directly
- **Core::create_token()** now takes `&ApiClient` instead of `&TokenCreationClient`

### Deprecated

- `TokenCreationClient` - Use `ApiClient` directly for all API operations

### Migration Guide

```rust
// Before (deprecated)
let api = ApiClient::new();
let client = TokenCreationClient::with_client(Arc::new(api));
core.create_token(params, &client).await?;

// After (recommended)
let api = ApiClient::from_env(); // or ApiClient::new().with_api_key(key)
core.create_token(params, &api).await?;

// Creator rewards
let response = api.get_created_tokens(address, 1, 10).await?;
if let Some(params) = ApiClient::build_claim_params(&token) {
    core.claim_creator_reward(params).await?;
}
```

## [0.3.0] - 2025-01-XX

### Changed - Fast Transaction Submission

**BREAKING CHANGE**: All trading functions now return transaction hash immediately instead of waiting for receipt.

#### Modified Functions

All these functions now return `Result<B256>` instead of `Result<TransactionResult>`:

**BondingCurve Router** (`bonding_curve.rs`):
- `buy()` - Returns tx_hash immediately
- `sell()` - Returns tx_hash immediately
- `sell_permit()` - Returns tx_hash immediately
- `exact_out_buy()` - Returns tx_hash immediately
- `exact_out_sell()` - Returns tx_hash immediately
- `exact_out_sell_permit()` - Returns tx_hash immediately

**DEX Router** (`dex.rs`):
- `buy()` - Returns tx_hash immediately
- `sell()` - Returns tx_hash immediately
- `sell_permit()` - Returns tx_hash immediately
- `exact_out_buy()` - Returns tx_hash immediately
- `exact_out_sell()` - Returns tx_hash immediately
- `exact_out_sell_permit()` - Returns tx_hash immediately

**Core Client** (`core.rs`):
- `buy()` - Returns tx_hash immediately
- `sell()` - Returns tx_hash immediately
- `sell_permit()` - Returns tx_hash immediately

### Added

- **`Core::get_receipt(tx_hash: B256)`** - New function to retrieve transaction receipt
  - Returns `Result<TransactionResult>` with full transaction details
  - Use this when you need to check transaction status, gas used, or logs
  - Supports polling for receipt until it's available

### Migration Guide

#### Before (v0.2.x):

```rust
// Old - Waits for confirmation automatically (slow)
let result = core.buy(params, router).await?;
println!("Status: {}", result.status);
println!("Gas used: {:?}", result.gas_used);
```

#### After (v0.3.0):

```rust
// New - Returns immediately (fast!)
let tx_hash = core.buy(params, router).await?;
println!("Submitted: {}", tx_hash);

// Check status later when needed
let receipt = core.get_receipt(tx_hash).await?;
println!("Status: {}", receipt.status);
println!("Gas used: {:?}", receipt.gas_used);
```

### Benefits

1. **Faster Trading Bots**: Submit transactions in milliseconds instead of waiting 2-15 seconds for confirmation
2. **Better Control**: Choose when to wait for confirmation
3. **Batch Operations**: Submit multiple transactions quickly, then check their status together
4. **Fire and Forget**: For some use cases, you don't need to wait at all

### Examples Updated

All trading examples have been updated to demonstrate the new pattern:
- `examples/core/buy.rs` - Shows immediate submission and optional receipt checking
- `examples/core/sell.rs` - Demonstrates sell with receipt verification
- `examples/core/sell_permit.rs` - Gasless sell with new pattern

## [0.2.0] - 2024-XX-XX

### Added

- Unified gas estimation system (`GasEstimationParams`)
- Network-based gas estimation for all operations
- Automatic token approval handling for SELL operations
- Real EIP-2612 permit signature generation
- Comprehensive gas estimation examples

### Changed

- Replaced static gas constants with dynamic network estimation
- Improved error handling for gas estimation failures

## [0.1.0] - 2024-XX-XX

### Added

- Initial release
- Trading functionality (buy/sell on bonding curves and DEX)
- Token creation with metadata and image upload
- Real-time event streaming
- Historical data indexing
- Pool discovery utilities
