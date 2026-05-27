# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added — v2 contract support (NadFunRouter ecosystem)

The SDK now ships **two parallel surfaces** for the two generations of
Nad.fun contracts. v1 callers keep their existing `Core` API — not a line
of v1 user code needs to change. v2 callers use the new `CoreV2`.

- **`CoreV2`** — high-level client for v2 contracts (`NadFunRouter` +
  `NadFunFactory` + `BondingCurveV2` + `TokenRegistryV2`).
  - Trading: `buy`, `buy_with_native`, `buy_with_permit`, `sell`,
    `sell_to_native`, `sell_with_permit`, `sell_to_native_with_permit`,
    `exact_out_buy`, `exact_out_buy_with_native`, `exact_out_sell`,
    `exact_out_sell_to_native`.
  - Creation: `create` / `create_with_native` (low-level) and
    `create_token(V2CreateTokenParams, &ApiClient)` (end-to-end with
    metadata + IPFS + salt mining).
  - Quotes: `quote`, `quote_in`, `quote_bonding_curve(_in)`,
    `quote_dex(_in)`, `is_graduated`, `pool_address`.
  - Gas: `estimate_gas(V2GasEstimationParams)` over all v2 operations.
  - Escape hatches: `router()`, `factory()`, `bonding_curve()`,
    `token_registry()`, `provider()`.
  - Constructors: `new(rpc, key, network)` and `with_provider(...)` for
    sharing the underlying provider/wallet with a v1 `Core` instance.

- **`SdkVersion`** enum — `V1` / `V2` discriminator used by
  `SaltParams.version`, `ApiTokenInfo.version`, and user-side dispatch.

- **`ApiClient`** extensions:
  - `get_token(token)` returns `ApiTokenInfo` with the `version` field —
    the recommended source of truth for client-side v1/v2 routing.
  - `get_token_vaults(token)` returns `VaultState` for the v2 vault model
    (BurnVault, LPVault, CreatorFeeVault, GiftVault).
  - `prepare_token_creation_v2(&V2PrepareCreationParams)` orchestrates
    image upload + metadata + salt mining (sends `version: "V2"`).
  - `with_api_url(url)` allows pointing at staging/local/mock endpoints
    (useful for tests).
  - `SaltParams.version: Option<SdkVersion>` — `None` (v1, default) omits
    the field from the wire; `Some(V2)` enables v2 CREATE2 mining.

- **Streaming + indexing (v2)**:
  - `CurveStreamV2`, `CurveIndexerV2` — v2 `BondingCurve` events
    (`V2BondingCurveEvent` enum: Create / Buy / Sell / Sync / Graduate /
    SnipingPenalty).
  - `NadFunSwapStream`, `NadFunSwapIndexer` — `NadFunPair::Swap` events
    (Uniswap V2 fork shape; carries `amount0In/1In/0Out/1Out`).
  - `discover_pools_unified(provider, tokens, factory_v2_address)` — one
    call surfaces pools from both v1 (Capricorn CL) and v2 (NadFun)
    surfaces.

- **Constants** restructured to `addresses::{mainnet,testnet}::{v1,v2}`
  submodules. Legacy flat path (`addresses::mainnet::BONDING_CURVE`) still
  works via `pub use v1::*`. v2 deployments are registered for both
  mainnet and testnet (16 addresses each network) with `get_*_v2()`
  helpers — `get_nadfun_router_v2`, `get_nadfun_factory_v2`,
  `get_token_registry_v2`, `get_bonding_curve_v2`, `get_protocol_manager_v2`,
  `get_fee_collector_v2`, `get_creator_fee_processor_v2`, `get_lp_manager_v2`,
  `get_vault_registry_v2`, `get_burn_vault_v2`, `get_lp_vault_v2`,
  `get_creator_fee_vault_v2`, `get_gift_vault_v2`, `get_nad_swap_adapter_v2`,
  `get_nadfun_pair_impl_v2`, `get_token_impl_v2`.

- **Examples** (`examples/v2/` + `examples/unified_dispatch.rs`):
  `v2_buy`, `v2_sell`, `v2_buy_erc20_quote`, `v2_exact_out`,
  `v2_create_token`, `v2_curve_stream`, `v2_dex_stream`,
  `v2_pool_discovery`, `unified_dispatch`.

### Changed

- Internal source layout reorganized into version directories:
  `src/{core,contracts,types,stream}/{v1,v2}/`. External import paths
  unchanged via `lib.rs` and per-module re-exports.

### Compatibility

- **No breaking changes** to the v1 public API. v1 callers (`Core`,
  `BuyParams`, `SellParams`, `CurveStream`, `DexStream`, `BondingCurveEvent`,
  `EventType`, `SwapEvent`, `PoolMetadata`, `TokenHelper`, `ApiClient`,
  `CreatorClient`, `PoolDiscovery`, `get_pool_addresses_for_tokens`,
  `estimate_gas`, `GasEstimationParams`, `Router`, `SlippageUtils`,
  `Network`, `set_network`, `get_current_network`,
  `get_nadfun_router_v2`, `get_creator_manager`, `get_creator_treasury`,
  `ALLOWED_IMAGE_TYPES`) keep their signatures.
- `get_nadfun_router_v2()` now returns `Some(_)` on `Network::Mainnet`
  too (previously only `Testnet`); the function shape is unchanged.

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
