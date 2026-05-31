# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Structured v2 quote-token registry**: new `QuoteToken` struct and
  `quote_tokens(Network) -> &'static [QuoteToken]`, re-exported from the crate
  root and `prelude`. Each entry carries `address`, `symbol`, `name`,
  `decimals`, and `is_native` and mirrors the api-server `GET /quote_token`
  source of truth (the authoritative DB list). Use it to resolve a quote
  token's metadata or to enumerate the native-funded quote tokens
  (`is_native == true`) accepted by `V2CreatePayment::Native` and the
  `*_with_native` trade methods — instead of a single hardcoded address. The
  list is a release-time snapshot; call the api-server endpoint for the live
  set. The `MON` row IS the wrapped native (WMON) per network — same address
  as the v1 `WMON` constant; the DB labels it `MON`/`MONAD`.
- v2 computed-helper parity on `core.v2()` (the v1 Lens utilities that have no
  v2 on-chain equivalent): `get_progress`, `available_buy_tokens`, and
  `get_initial_buy_amount_out(quote_token, amount_in, creator_fee_rate)`. These
  reproduce the on-chain bonding-curve math client-side from `get_curve` /
  `quote_config` (fee + constant-product + supply cap, verified against on-chain
  quotes). `get_initial_buy_amount_out` takes a `quote_token` (v2 genesis curves
  differ per quote token) and the token's `creator_fee_rate` (u16 bps, a
  per-token parameter not in the genesis config) — unlike v1's parameterless
  version. It returns the **exact** create-time `_initialBuy` output to the wei:
  the combined protocol + creator fee (one ceil `mulDivUp`) is deducted, then the
  constant-product / supply-cap math; the create-time buy is anti-sniping exempt.
  It errors (rather than returning `0`) when a positive buy's ceil-rounded fees
  consume the entire quote — the on-chain `_initialBuy` would revert in that
  case, so a `0` result would mislead callers.
- v2 passthrough parity on `core.v2()`: `is_halted`, `get_sniping_penalty`,
  `quote_token`, `get_curve` (→ new `V2Curve`), `quote_config` (→ new
  `V2QuoteConfig`), `get_dex_type`, `is_registered`, `is_locked`, and
  `get_reserves` (→ `PairReserves`). `is_locked` / `get_reserves` gate on
  `is_graduated` and error before graduation; `is_locked` reflects the
  post-graduation DEX-pair lock, which is distinct from the v1 bonding-curve
  `is_locked`. New types `V2Curve`, `V2QuoteConfig`, and `PairReserves` are
  re-exported from the crate root and `prelude`.

### Changed

- **BREAKING:** the 17 v2 address helpers now return `&'static str` directly
  instead of `Option<&'static str>` — `get_nadfun_router_v2`,
  `get_nadfun_factory_v2`, `get_nadfun_pair_impl_v2`, `get_nad_swap_adapter_v2`,
  `get_token_registry_v2`, `get_token_impl_v2`, `get_protocol_manager_v2`,
  `get_bonding_curve_v2`, `get_fee_collector_v2`, `get_creator_fee_processor_v2`,
  `get_lp_manager_v2`, `get_vault_registry_v2`, `get_burn_vault_v2`,
  `get_lp_vault_v2`, `get_creator_fee_vault_v2`, `get_gift_vault_v2`, and
  `get_token_info_lens`. v2 is deployed on every supported network, so every arm
  was already `Some(..)` and callers were forced into pointless `.expect()` /
  `.ok_or_else()?`. Drop the unwrap: `get_nadfun_router_v2(net).parse()?`.
  `get_fee_to_v2` keeps returning `Option` — `feeTo` is genuinely absent on
  Mainnet.
- **BREAKING:** `SaltParams.version` is now a required `SdkVersion` (was
  `Option<SdkVersion>`). A missing `version` no longer silently means v1; the
  field is always set explicitly (`SdkVersion::V1` / `SdkVersion::V2`). The wire
  form is unchanged — `#[serde(skip_serializing_if = "SdkVersion::is_v1")]`
  still omits the field for v1 requests (byte-identical to the pre-v2 SDK), and
  a missing field still deserializes to `SdkVersion::V1`.
- **BREAKING:** `V2CreatePayment::Native` now carries a required
  `quote_token: Address` (was a unit variant). For native-funded v2 creates the
  SDK no longer bakes in the wrapped-native address and does not auto-resolve
  it — the caller supplies the native-equivalent quote token explicitly (WMON
  or LVMON). Resolve it from `quote_tokens(network)` (any entry with
  `is_native == true`) or `core.v2().wrapped_native()`. Anything the on-chain
  `createWithNative` rejects reverts with `InvalidNativeQuoteToken`. Migration:
  `V2CreatePayment::Native` → `V2CreatePayment::Native { quote_token: wmon }`
  (with `wmon` resolved as above).
- **BREAKING:** removed the baked LvMON quote-token constants — the
  `constants::get_lv_mon_v2` helper and the `addresses::testnet::v2::LV_MON`
  constant. The SDK no longer ships a single-token native-quote allowlist; use
  the structured `quote_tokens(Network)` registry (which includes LVMON with
  `is_native == true`) or pass the address explicitly via
  `V2CreatePayment::Native { quote_token: Some(addr) }`. The v1 `get_wmon` /
  `WMON` constants are unchanged (v1 DEX pool discovery still uses them).
- **BREAKING:** `Core` trading methods moved behind namespace handles
  `core.v1()` / `core.v2()`. The flat `Core::buy`, `Core::get_amount_out`, …
  and all `*_v2` methods are removed; call `core.v1().buy(...)` /
  `core.v2().buy(...)` instead (the `_v2` suffix is dropped). Cross-version
  methods (`detect_version`, `detect_token_info`, `get_receipt`) stay on `Core`.
  New `CoreV1` / `CoreV2` handle types are re-exported from the crate root and
  prelude.

- **v2 DEX streaming/indexing now mirrors v1 on empty input.**
  `NadFunSwapIndexer` (`fetch_events` / `fetch_all_events`) and
  `NadFunSwapStream::new` no longer short-circuit or reject an empty `pairs`
  list. As with v1 `DexIndexer` / `DexStream`, an empty list means "no address
  filter" and receives every NadFunPair `Swap` in scope. Signatures are
  unchanged; `NadFunSwapStream::new` no longer returns the "at least one pair
  address is required" error.
- **v2 DEX indexer sorts by `(block, transaction_index, log_index)`.**
  `NadFunSwapIndexer` now orders results by the full triple, matching v1
  `DexIndexer` (previously `(block, log_index)`, which could tie when
  `transaction_index` / `log_index` metadata is missing).

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
    ships without it), so no dead-branch guard is needed. Plus
    `token_info_lens()` for the raw `TokenInfoLens` binding.
  - `Core::detect_version(token)` / `Core::detect_versions(tokens)` —
    on-chain version classification via the `TokenInfoLens` view contract
    (one RPC covers both v1 + v2 registries; explicit `None` for unknown
    tokens). Stateless — each call hits the chain; cache externally if
    needed.
  - `Core::detect_token_info(token)` / `Core::detect_token_infos(tokens)` —
    return `TokenInfo { version, quote_token }`: the version **and** the
    token's on-chain quote token in the same single `TokenInfoLens` call.
    The SDK does not auto-select a trade method from `quote_token`; see the
    quote-routing matrix in `MIGRATION.md` §7.
  - `SdkVersion::None` — token isn't registered on either system. New
    variant returned by `detect_version`.

- **`SdkVersion`** enum — `V1` / `V2` / `None` discriminator used by
  `SaltParams.version`, `ApiTokenInfo.version`, and user-side dispatch.
- **`TokenInfo`** struct (`version` + `quote_token`) — on-chain
  classification result from `Core::detect_token_info`. **Note:** this
  replaces the unused v1 `TokenInfo` (`{ metadata, balance, nonce }`),
  which was a defined-but-never-produced public type and has been removed.
  Use `TokenMetadata` (still present) for token metadata.

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
