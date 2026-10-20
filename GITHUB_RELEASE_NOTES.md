# Release Notes

## v0.7.0 (2026-10-20)

```toml
nadfun_sdk = "0.7.0"
```

### ✨ New

- **Receipt waiting:** `Core::wait_for_receipt` with `ReceiptWaitOptions` defaults to 350 ms polling, a 120 s deadline, and no finality. It checks receipt hash and status, and fails fast on RPC errors. `finalized: true` waits for the receipt block to finalize, re-fetches the receipt, and rejects a changed or missing receipt; `get_receipt` remains a single lookup.
- **Synchronous sends:** `Core::send_transaction_sync` returns a mined receipt with status checked, but not necessarily finalized. It falls back to one ordinary send only on `-32601`; code 4 plus a 32-byte hash in `data` (string or `{ "hash": "0x…" }`) is treated as accepted, and receipt polling continues through transient RPC errors.
- **Native balance pre-check:** `Core::check_native_send_balance` checks value + gas limit × fee cap; callers must use those same values for the send.
- Measured on testnet (2026-10-20): sync send ~0.53 s to receipt; finalized adds ~0.59 s. Priority fees 1/2/5 gwei showed no latency difference at the 100 gwei base-fee floor (median 626/588/611 ms; 30 tx each).

### 🔧 Fixes

- Earlier releases treated Monad sync timeouts with a hash in a `data` object as rejected. The nonce guard now recognizes only code 4 with a hash as accepted; hash-bearing errors with another code leave the nonce reusable.

### ⚠️ Behaviour Changes

- Sync errors other than `-32601` and pre-submission validation errors report `outcome unknown; do not resend blindly`; fallback transport failures without a JSON-RPC response do too. Timeouts after known submission start with `transaction submitted; outcome unknown`.

### 📝 Documentation

- `GasPricing` docs clarify: `Legacy` leaves fee fields unset and the signing filler signs type 2; `LegacyWithPrice` sets `gas_price` and signs type 1 when an access list is attached; `Eip1559` sets both fee caps. The default remains `Legacy`.

Residual risks:
1. A synchronous-send receipt confirms inclusion and execution status, not finality; use `wait_for_receipt` with `finalized: true` when needed.
2. A gateway that returns `-32601` after executing the sync request is not detected, so the ordinary-send fallback may resubmit it.
3. The native balance check applies no 10 MON reserve and reserves no funds; balance, gas, or fee values can change before the send.

Full details: [CHANGELOG.md](https://github.com/Naddotfun/nadfun-sdk-rust/blob/main/CHANGELOG.md).

---

## v0.6.0 (2026-10-19)

```toml
nadfun_sdk = "0.6.0"
```

### ⚡ Gas savings: EIP-2930 access lists by default

Signing providers created by `Core::new` and `TokenHelper::new` now call `eth_createAccessList` before every unsigned send and gas estimate, and attach the generated list. On Monad, cold account access costs 10,100 gas and a cold storage slot 8,100, and the full gas limit is charged, so pre-warming saves real MON.

- Measured on mainnet (read-only re-estimate): V2 `sell` 552,896 → 263,088 (**−52.4%**), `exactOutBuy` 560,209 → 274,963 (**−50.9%**). Savings vary by call.
- Measured on testnet (V1 buy, live sends): the list is attached for both default and explicit gas limits.
- Lists are filtered (empty-key sender/destination entries removed) and validated (≤ 1,024 addresses + keys; intrinsic cost `2,400 × addresses + 1,900 × keys` < `gasUsed`). An empty list is never attached.
- Probe failure, rejection, or a probe taking over 5 s falls back to a request without a generated list.
- Caller-supplied lists are preserved. **Opt out:** pass an empty `access_list`.

### Explicit gas limits

- The generated list is kept when the RPC-reported `gasUsed` ≤ your limit, and dropped when it is higher.
- When the list is dropped or unavailable, the SDK runs a list-less estimate (30 s bound):
  - estimate above your limit → `Err` before a nonce is taken
  - revert-shaped error (JSON-RPC code 3 or a message containing "revert") → sent list-less with your limit, so pipelined approve → trade still works
  - any other error or timeout → contextual `Err` before a nonce is taken

### ⚠️ Behaviour Changes

- `gas_limit: None` estimates are now list-aware and sent without headroom. If you reuse an estimate, keep about **10%** headroom (previously 15–25% was suggested).
- A transaction with an access list and `GasPricing::LegacyWithPrice` is signed as EIP-2930 type 1. `GasPricing::Legacy` still yields EIP-1559 type 2.
- Estimates with state or block-state overrides skip list generation. When `from` is absent, the probe and the estimate use the signing account.

### Cost and residual risks

1. One extra RPC call (`eth_createAccessList`) per send or estimate; on RPCs that do not support it, one failing call.
2. The generated list and its `gasUsed` come from the RPC and are not cross-checked. With an explicit limit, a wrong or stale list can run out of gas and pay the full limit.
3. After a revert-shaped estimate, a genuinely reverting transaction is still sent and pays the full limit (same as before 0.6).

No public API changes. Full details: [CHANGELOG.md](https://github.com/Naddotfun/nadfun-sdk-rust/blob/main/CHANGELOG.md).

---

## v0.5.0 (2026-10-16)

### 🔧 Fixes

- **Nonce gap after a failed send** - a send that reverted at gas estimation or was rejected by the node used to leave a nonce gap on `Core::new` / `TokenHelper::new` providers, so later transactions returned a hash but never mined
  - Gas estimation now runs before a nonce is taken
  - `Core::new` and `TokenHelper::new` share one nonce state per (chain, account) in the process
  - Sends from one account are serialized; concurrent calls queue and get consecutive nonces
  - Node rejection (incl. "nonce too high") -> nonce reused, or the chain count if it is already past it; "nonce too low" / "already known" / "already imported" -> skipped; on sync sends only, an error carrying the tx hash -> treated as sent; transport error, timeout (30 s per RPC under the lock) or dropped send -> reused on the next send
  - Mined count (`latest`) read before every automatic send (one extra RPC) and after a refused send; if we are ahead and it has not moved for 30 s since we last sent the nonce the chain is waiting on (sends past a gap do not count), that tx is presumed dropped and the next send refills the gap (stall recovery)
  - Caller-supplied nonces pass through and move the shared state past them

Residual risks:
1. Ambiguous outcome (transport error / timeout): the tx may have landed. The
   next send reuses its nonce, so only one of the two executes — the caller may
   get `Err` for the one that executed, or `Ok(hash)` for one that never mines.
2. A live tx not included for more than 30 s is "presumed dropped": the next
   send after stall recovery re-signs its nonce and can replace that slow but
   alive tx (same outcome as 1).
3. A tx the node accepted and then dropped: later sends stall for up to 30 s,
   then recover by reusing its nonce.
4. First use after a process restart while the previous process still has
   unmined txs: those nonces are reused (Monad `pending` == `latest`).
5. Not covered: providers passed to `Core::with_provider`, other processes or
   wallets on the same key, a devnet reset that keeps the chain id (repaired by
   the 30 s stall recovery).
6. Cost: +1 `eth_getTransactionCount` per automatic send (+1 more after a
   refused send); sends are serialized per account process-wide, and a sync
   send may hold the account for up to 120 s.
7. Errors are classified by partial message text; unknown wordings fall back
   to max(n, chain count).
8. Raw sends (`send_raw_transaction*`) through the same provider are not
   intercepted, so the state can be stale afterwards: the next send may fail
   once ("nonce too low") and then recover. Envelopes signed by another key and
   requests with another `from` pass through without touching the state.

### ⚠️ Behaviour Changes

- Sends from one account are serialized process-wide (concurrent calls queue; a sync send can hold the account up to 120 s)
- +1 `eth_getTransactionCount` per automatic send (+1 more after a refused send)
- 30 s stall resync: if the chain does not move for 30 s since the SDK last sent the nonce it is waiting on, the next send re-signs that nonce (can replace a slow but alive tx)

### 📦 Dependencies

- Minimum `alloy` raised to `1.1.2`

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.5.0"
```

---

## v0.4.2 (2026-10-16)

### 🚀 New Features

- **Multi-quote trading** - every on-chain allowed v2 quote is in `quote_tokens(network)`
  - Mainnet (7): MON/WMON, LVMON, BNCB, USDC (6 decimals), aWEN, aQQQ, aHOOD
  - Testnet (9): MON/WMON, LVMON, AUSD (6), HOODB, USDC (6), BNCB, aWEN, aQQQ, aHOOD
  - Tokens on a non-native quote trade through the ERC-20 `create` / `buy` / `sell` (+ permit) methods, with amounts in the quote's own decimals
- **API-based token detection** - `Core::detect_token_info_with_api` / `Core::detect_version_with_api`
  - Reads the v2 `TokenRegistry` first (a registry entry is V2 + its quote); only unregistered tokens ask the api-server (V1 -> WMON, api-server 404 -> `SdkVersion::None`)
  - Use these on mainnet: the shipped mainnet `TOKEN_INFO_LENS` address is not a TokenInfoLens, so the lens-based `detect_*` methods revert there (they still work on testnet)
- **`ApiClient::get_quote_tokens()`** - api-server quote-token catalog (`ApiQuoteToken`, `#[non_exhaustive]`); not filtered on the on-chain allow-list, so use `quote_tokens()` or on-chain config for anything that moves funds

```rust
let api = ApiClient::new(Network::Mainnet);
let info = core.detect_token_info_with_api(&api, token).await?;
let native = quote_tokens(Network::Mainnet)
    .iter()
    .any(|q| q.is_native && q.address.parse::<Address>().ok() == Some(info.quote_token));
if native {
    // core.v2().buy_with_native(..) with MON
} else {
    let decimals = core.v2().quote_config(info.quote_token).await?.decimals;
    // approve the router, then core.v2().buy(..) with an amount in `decimals`
}
```

### 🔧 Fixes & Changes

- **`ApiClient::get_token` fixed** - parses the live `{"token_info": {..}}` response (it failed on every live response before); a 404 reports `token not found (404)`
- **Mainnet API URL** - `https://api.nad.fun` (was `https://api.nadapp.net`); testnet unchanged
- **Router ABI refreshed** to the latest `NadFunRouter`; existing methods keep their signatures and behavior
- **Native-path constraints documented**
  - `buy_with_native` / `create_with_native` work only for `is_native` quotes (WMON, LVMON); others revert `NativeQuoteMustUseRoute` (`InvalidNativeQuoteToken` on a router without a swap executor)
  - `sell_to_native` / `sell_to_native_with_permit` / `exact_out_sell_to_native` are WMON-quote only
- **`examples/unified_dispatch.rs`** - API-based detection, `is_native` routing, and quote-decimal scaling

### ⚠️ Deprecated (removed in 0.5.0)

- Nine unused v2 address getters and their constants: `get_creator_fee_processor_v2`, `get_fee_collector_v2`, `get_fee_to_v2`, `get_gift_vault_v2`, `get_lp_manager_v2`, `get_nad_swap_adapter_v2`, `get_nadfun_pair_impl_v2`, `get_token_impl_v2`, `get_vault_registry_v2`

### 🧪 Testing

- Testnet anvil-fork E2E: `scripts/e2e-fork.sh` (report at `target/e2e-fork-report.json`) — USDC-quoted create/buy/sell including large sizes and the graduation boundary, WMON native regression, the `NativeQuoteMustUseRoute` revert, and API detection

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.4.2"
```

---

## v0.4.1 (2026-07-01)

### 🚀 New Features

- **v2 DEX live `Sync` stream** - `NadFunSyncStream` pushes `NadFunPair::Sync(reserve0, reserve1)` snapshots over WebSocket on every trade/mint/burn
  - `NadFunSyncEvent`, `decode_nadfun_sync_event`, `nadfun_sync_signature`, re-exported at `nadfun_sdk::stream::*`
  - `reserve0` / `reserve1` follow the pair's `token0` / `token1` order; map a side via `core.v2().quote_token(token)`

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.4.1"
```

---

## v0.4.0 (2026-05-31)

### 🚀 New Features

- **Unified `Core` for v1 + v2** - one instance, namespace handles `core.v1()` / `core.v2()`
  - v2 trading (`buy`, `buy_with_native`, `buy_with_permit`, `sell`, `sell_to_native`, permit and exact-out variants), creation (`create`, `create_with_native`, `create_token`), quotes, and gas estimation on the `NadFunRouter`
  - `Core::detect_version` / `Core::detect_token_info` (`TokenInfo { version, quote_token }`) and `SdkVersion::None`
- **v2 views and helpers** - `get_curve`, `quote_config`, `is_halted`, `get_sniping_penalty`, `get_reserves`, `get_progress`, `available_buy_tokens`, exact `get_initial_buy_amount_out`
- **`quote_tokens(network)`** - structured v2 quote-token registry (`QuoteToken { address, symbol, name, decimals, is_native }`)
- **v2 streaming** - `CurveStreamV2`, `CurveIndexerV2`, `NadFunSwapStream`, `NadFunSwapIndexer`, `discover_pools_unified`
- **ApiClient v2** - `get_token` with `version`, `get_token_vaults`, `prepare_token_creation_v2`

### ⚠️ Breaking Changes

- Trading methods moved behind `core.v1()` / `core.v2()`; `CoreV2` and the `*_v2` suffixes are gone
- `Network` is passed to every constructor (`ApiClient::new(network)`, streams, indexers, pool discovery); `set_network` removed
- v2 address helpers return `&'static str` (only `get_fee_to_v2` stays `Option`)
- `V2CreatePayment::Native { quote_token }`, required `SaltParams.version`, `V2BuyWithNativeParams.value`; `get_lv_mon_v2` / `LV_MON` removed

See [`MIGRATION.md`](MIGRATION.md) for the upgrade path.

### 🔧 Fixes

- v2 event decoding reads indexed fields from topics; `create_token` verifies the on-chain `Create` event against the predicted address; null-tolerant `ApiTokenInfo.version`; network-correct WMON in pool discovery

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.4.0"
```

---

## v0.3.12 (2025-02-02)

### 🚀 New Features

- **API key authentication** - `ApiClient::from_env()` (`NAD_API_KEY`) / `.with_api_key()` for higher rate limits
- **Creator rewards** - `ApiClient::get_created_tokens()`, `build_claim_params()`, `build_batch_claim_params()`, `Core::claim_creator_reward()`, `Core::claim_creator_rewards_batch()`

### 🔧 Changes

- Mainnet API URL `https://api.nadapp.net`; token creation uses `/agent/*` endpoints
- `TokenCreationClient` deprecated — use `ApiClient` directly

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.12"
```

---

## v0.3.10 (2025-12-23)

### 🔧 Code Quality

- **Fixed all clippy warnings** - 24 warnings resolved
  - Removed useless `.into()` and `.try_into()` conversions
  - Replaced manual range checks with `Range::contains()`
  - Simplified redundant closures
  - Added `Default` implementation for `PoolMetadata`
  - Added `#[allow]` attributes for intentional patterns (module inception, many arguments)

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.10"
```

---

## v0.3.9 (2025-12-23)

### 🚀 Enhancement

- **Unified `DexEvent` enum** - Single decoder for all DEX pool events
  - `DexEvent::Swap(SwapEvent)` - Token swap events
  - `DexEvent::Mint(MintEvent)` - Liquidity add events
  - `DexEvent::Burn(BurnEvent)` - Liquidity remove events

- **New `decode_dex_event()` function** - Automatically matches and decodes any DEX event

```rust
use nadfun_sdk::types::{decode_dex_event, DexEvent};

match decode_dex_event(log)? {
    DexEvent::Swap(swap) => println!("Swap: {:?}", swap),
    DexEvent::Mint(mint) => println!("Mint: {:?}", mint),
    DexEvent::Burn(burn) => println!("Burn: {:?}", burn),
}

// Common methods on DexEvent
let pool = event.pool_address();
let block = event.block_number();
let tx_hash = event.transaction_hash();
let event_type = event.event_type(); // "Swap", "Mint", or "Burn"
```

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.9"
```

---

## v0.3.8 (2025-12-23)

### 🚀 New Features

- **`MintEvent` type** - DEX pool liquidity addition events
- **`BurnEvent` type** - DEX pool liquidity removal events
- **Decode functions** - `decode_mint_event()`, `decode_burn_event()`
- **Event signatures** - `MINT_SIGNATURE`, `BURN_SIGNATURE`

```rust
use nadfun_sdk::types::{decode_mint_event, decode_burn_event, MintEvent, BurnEvent};

// Decode liquidity events
let mint: MintEvent = decode_mint_event(log)?;
let burn: BurnEvent = decode_burn_event(log)?;

// Get amounts based on token order
let wmon_added = mint.wmon_amount(wmon_is_token0);
let tokens_removed = burn.token_amount(wmon_is_token0);
```

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.8"
```

---

## v0.3.7 (2025-12-23)

### 🔧 Update

- **Testnet contract addresses updated** - All testnet addresses updated to latest deployment

| Contract | Address |
|----------|---------|
| DEX_FACTORY | `0xd0a37cf728CE2902eB8d4F6f2afc76854048253b` |
| WMON | `0x5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd` |
| BONDING_CURVE | `0x1228b0dc9481C11D3071E7A924B794CfB038994e` |
| BONDING_CURVE_ROUTER | `0x865054F0F6A288adaAc30261731361EA7E908003` |
| DEX_ROUTER | `0x5D4a4f430cA3B1b2dB86B9cFE48a5316800F5fb2` |
| LENS | `0xB056d79CA5257589692699a46623F901a3BB76f1` |

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.7"
```

---

## v0.3.6 (2025-12-23)

### 🚀 Enhancement

- **Network-aware API server URL** - API URL now automatically switches based on network setting
  - Mainnet: `https://api.nad.fun`
  - Testnet: `https://dev-api.nad.fun`

- **New `get_api_server_url()` function** - Returns correct API URL for current network

```rust
use nadfun_sdk::constants::{set_network, Network, get_api_server_url};

set_network(Network::Mainnet);
assert_eq!(get_api_server_url(), "https://api.nad.fun");

set_network(Network::Testnet);
assert_eq!(get_api_server_url(), "https://dev-api.nad.fun");
```

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.6"
```

---

## v0.3.5 (2025-01-16)

### 🔧 Update

- **Testnet contract addresses updated**

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.5"
```

---

## v0.3.4 (2025-01-16)

### 🚀 Enhancement

- **`create` function now returns tx_hash immediately** - No longer waits for receipt
  - `bonding_curve.create()` returns `B256` (tx_hash) instead of `(Address, TransactionResult)`
  - Token address is now obtained from salt API response (CREATE2 pre-calculation)
  - Faster token creation flow

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.4"
```

---

## v0.3.3 (2025-01-16)

### 🔧 Update

- **Testnet contract addresses updated** - All testnet addresses updated to latest deployment

| Contract | Address |
|----------|---------|
| DEX_FACTORY | `0xE6dc50f36E26bAfC5f103021e01EF111402Cd940` |
| BONDING_CURVE | `0x985Ae3529A1875698772C5fFbc66b8327049E094` |
| BONDING_CURVE_ROUTER | `0x2D729C91aB77a887b3579aa50f55B50E5bC6dE46` |
| DEX_ROUTER | `0x8a7697098da7F8692325046DB98F3dA9c480B529` |
| LENS | `0x143dc84ac094edECABeF577Fb326aa4d9893D97B` |

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.3"
```

---

## v0.3.2 (2025-01-16)

### 🔧 Enhancement

- **`create` function now supports EIP-1559** - Token creation now uses unified `GasPricing` like other trade functions
  - `gas_price: Option<u128>` → `gas_price: Option<GasPricing>`

```rust
// Example: Create token with EIP-1559
bonding_curve_router.create(
    name, symbol, token_uri, amount_out, salt, action_id, value,
    Some(300_000), // gas_limit
    Some(GasPricing::Eip1559 {
        max_fee_per_gas: 100_000_000_000,
        max_priority_fee_per_gas: 2_000_000_000,
    }),
    None, // nonce
).await?;
```

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.2"
```

---

## v0.3.1 (2025-01-16)

### 🔧 API Simplification

**Unified `gas_price` Field** - Merged `gas_price` and `gas_pricing` into a single field for cleaner API:

#### Before (v0.3.0)
```rust
let buy_params = BuyParams {
    // ... other fields
    gas_price: Some(50_000_000_000),    // Legacy u128
    gas_pricing: Some(GasPricing::Eip1559 { ... }), // Separate field
};
```

#### After (v0.3.1)
```rust
let buy_params = BuyParams {
    // ... other fields
    gas_price: Some(GasPricing::LegacyWithPrice { gas_price: 50_000_000_000 }),
    // Or: Some(GasPricing::Eip1559 { max_fee_per_gas, max_priority_fee_per_gas })
    // Or: Some(GasPricing::Legacy) for network default
    // Or: None for network default
};
```

### ⚠️ Breaking Changes

- **`gas_price` field type changed**: `Option<u128>` → `Option<GasPricing>`
- **`gas_pricing` field removed**: Use `gas_price` with `GasPricing` enum instead
- Affected structs: `BuyParams`, `SellParams`, `SellPermitParams`, `ExactOutBuyParams`, `ExactOutSellParams`, `ExactOutSellPermitParams`

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.1"
```

---

## v0.3.0 (2025-01-16)

### 🚀 Major Changes

This major release introduces comprehensive token creation capabilities, enhanced contract integration with the latest Capricorn DEX, improved developer experience, and better debugging support.

#### **Token Creation System**
- **Complete Token Creation Flow** - End-to-end token creation with automatic metadata and image handling
  - `Core::create_token()` - One-call token creation with initial buy
  - Automatic deploy fee calculation and inclusion
  - Image upload with format validation (JPEG, PNG, WEBP, SVG only)
  - NSFW detection and automatic rejection
  - Metadata storage on IPFS
  - Salt generation for vanity addresses
  - Initial buy transaction integration
  - **ActionId Enum** - Type-safe actor selection (`CapricornActor` = 1, `AmplifyActor` = 2)

- **Token Creation Client** (`src/create/`)
  - `TokenCreationClient` - Handles image upload, metadata creation, and salt mining
  - Automatic image type detection from magic bytes
  - Support for URL-based image downloads
  - Integration with Nad.fun metadata API (`https://dev-api-server.nad.fun`)

- **Image Validation**
  - Strict format enforcement: JPEG, PNG, WEBP, SVG only
  - GIF, BMP, TIFF explicitly rejected with clear error messages
  - Magic byte detection for accurate type identification
  - Maximum 5MB file size validation

#### **Core Architecture Improvements**
- **New `Core` Module** (`src/core/`) - Unified interface for all trading and token operations
  - Replaces legacy `trading` module with cleaner API
  - Automatic router selection (Bonding Curve vs DEX)
  - Built-in slippage protection utilities
  - Advanced gas estimation system
  - Deploy fee management

- **Enhanced Lens Integration**
  - `get_initial_buy_amount_out()` - Calculate tokens received during creation
  - `get_deploy_fee()` - Query current deploy fee from bonding curve
  - Improved query efficiency for token state checks

#### **Contract Updates**
- **Updated Contract Addresses** - All contracts now point to latest Capricorn CL deployment
  - BondingCurve: `0x175ed6583EdA113Bd0C0Bb7B473f760006651a99`
  - BondingCurveRouter: `0x92f96f59137f41ECF8cD9a3C7E70E9e7db9deadE`
  - DexRouter: `0x006d317A4176b356aF3764db4d811bc953E33be9`
  - DexFactory: `0x99f4Aa293dcEfFA11aB0c03C359db45d05c7C863`
  - Lens: `0xD1cd9821dA319ec214375f6cd155A940e28e758d`

- **New ABI Files**
  - `IBondingCurve.json` - Complete bonding curve contract interface
  - `IBondingCurveRouter.json` - Trading router with `actionId` support
  - `ICapricornCLFactory.json` - Capricorn CL factory for pool discovery
  - `ICapricornCLPool.json` - Full Capricorn concentrated liquidity pool interface
  - `IDexRouter.json` - DEX router for graduated tokens
  - `ILens.json` - Batch query optimization contract

#### **EIP-1559 Gas Pricing Support**
- **New `GasPricing` Enum** - Flexible gas pricing strategy for transactions
  - `GasPricing::Legacy` - Default legacy gas pricing (Type 0)
  - `GasPricing::LegacyWithPrice { gas_price }` - Legacy with explicit gas price
  - `GasPricing::Eip1559 { max_fee_per_gas, max_priority_fee_per_gas }` - EIP-1559 (Type 2, recommended)
- **Updated Trade Params** - All trade parameter structs now include `gas_price: Option<GasPricing>` field
  - `BuyParams`, `SellParams`, `SellPermitParams`
  - `ExactOutBuyParams`, `ExactOutSellParams`, `ExactOutSellPermitParams`
- **Backward Compatible** - `gas_price: None` uses network default

#### **Event Streaming Enhancements**
- **Graduate Event Support** - Full support for token graduation events
  - Stream monitoring for `CurveGraduate` events
  - Historical indexing of graduation data
  - Event breakdown statistics in indexer

- **Lock Event Support** - Track token lock events
  - `CurveTokenLocked` event decoding
  - Lock status monitoring in streams
  - Historical lock event indexing

### ✨ New Features

#### **Token Creation Example**
```rust
use nadfun_sdk::{ActionId, Core, CreateTokenParams, Network};
use alloy::primitives::utils::parse_ether;

// Initialize with network
let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;

// Create token with single method call
let params = CreateTokenParams {
    name: "My Token".to_string(),
    symbol: "MTK".to_string(),
    description: "My awesome token".to_string(),
    image_uri: "https://example.com/image.png".to_string(),
    website: Some("https://example.com".to_string()),
    twitter: Some("@mytoken".to_string()),
    telegram: Some("@mytokenchat".to_string()),
    creator_address: wallet_address,
    amount_out: parse_ether("1000000")?,
    value: parse_ether("1.5")?, // 1.5 MON
    action_id: ActionId::CapricornActor, // Use CapricornActor (1)
};

let result = core.create_token(params).await?;
println!("Token created at: {}", result.token_address);
```

#### **EIP-1559 Gas Pricing Example**
```rust
use nadfun_sdk::types::{BuyParams, GasPricing};

// Use EIP-1559 for better fee control
let buy_params = BuyParams {
    token,
    amount_in: mon_amount,
    amount_out_min: min_tokens,
    to: wallet_address,
    deadline,
    gas_limit: Some(300_000),
    gas_price: Some(GasPricing::Eip1559 {
        max_fee_per_gas: 100_000_000_000,        // 100 gwei
        max_priority_fee_per_gas: 2_000_000_000, // 2 gwei tip
    }),
    nonce: None,
};
```

### 🔧 Breaking Changes

#### **Module Restructuring**
- **`trading` → `core`** - Main module renamed for better clarity
  ```rust
  // OLD (v0.2.x)
  use nadfun_sdk::trading::Trading;
  let trading = Trading::new(rpc_url, private_key).await?;

  // NEW (v0.3.0)
  use nadfun_sdk::Core;
  let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
  ```

#### **Gas Price Enhancement**
- Buy/Sell now use 3x network gas price (previously 2x)
- Sell permit uses dynamic network pricing (previously hardcoded 50 gwei)
- 20-25% gas limit buffer added for complex transactions

#### **URL Validation**
- Twitter URLs must use `x.com` (not `twitter.com`)
- Telegram URLs must use `t.me`
- All URLs must use `https://`
- Automatic `twitter.com` → `x.com` conversion

#### **Token Creation Parameters**
- `CreateTokenParams` now includes `action_id: ActionId` field (required)
- Deploy fee automatically calculated and added to transaction value
- Optional social media URLs (empty strings treated as `None`)
- **ActionId Enum** - Choose between `ActionId::CapricornActor` (1) or `ActionId::AmplifyActor` (2)

#### **Trade Parameter Updates**
- All trade params (`BuyParams`, `SellParams`, etc.) now include `gas_price: Option<GasPricing>` field
- Use `gas_price: None` for network default gas pricing
- **New `GasPricing` Enum**:
  ```rust
  pub enum GasPricing {
      Legacy,                                    // Network default
      LegacyWithPrice { gas_price: u128 },       // Explicit legacy price
      Eip1559 { max_fee_per_gas: u128, max_priority_fee_per_gas: u128 },
  }
  ```

### ⚠️ Migration Guide

#### From v0.2.x to v0.3.0

**Step 1: Update Imports**
```rust
// Old
use nadfun_sdk::trading::Trading;
use nadfun_sdk::trading::trade;

// New
use nadfun_sdk::Core;
```

**Step 2: Update Initialization**
```rust
// Old
let trading = Trading::new(rpc_url, private_key).await?;

// New
let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
```

**Step 3: Update Example Paths**
```bash
# Old
cargo run --example buy  # From examples/trade/

# New
cargo run --example buy  # From examples/core/
```

**Step 4: Social Media URLs**
```rust
// Old - would fail
twitter: Some("https://twitter.com/project".to_string())

// New - automatically converted
twitter: Some("https://twitter.com/project".to_string())  // → x.com
// Or manually use x.com
twitter: Some("https://x.com/project".to_string())
```

### 🐛 Bug Fixes

- **Content-Type Detection** - Fixed image upload failures
  - Now uses magic bytes for accurate type detection
  - Handles incorrect server-provided Content-Type headers
  - Validates against allowed formats before upload

- **Deploy Fee Calculation** - Fixed transaction reverts
  - Deploy fee now properly fetched from bonding curve contract
  - Total value = initial buy amount + deploy fee
  - Prevents `INVALID_INPUTS` errors during creation

- **Event Streaming** - Fixed missing events
  - Graduate events now properly decoded and emitted
  - Lock events included in default event subscriptions
  - Improved error messages for unknown event signatures

- **Gas Estimation** - Fixed "transaction fee too low" errors
  - Dynamic network gas price queries
  - Proper multiplication for higher priority
  - Buffer added for complex transactions

### 📊 Statistics

- **+5,753 lines added** across 52 files
- **-737 lines removed** (refactoring and cleanup)
- **15 new ABI files** for complete contract coverage
- **2 new major features** (token creation, enhanced streaming)
- **4 new example files** demonstrating capabilities
- **100% backward compatibility** for core trading operations (with import path updates)

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.1"
```

**Full Changelog**: [v0.2.1...v0.3.0](https://github.com/Naddotfun/nadfun-sdk-rust/compare/v0.2.1...v0.3.0)

---

## v0.2.0 (2024-08-20)

### 🚀 Major Features

**Unified Gas Estimation System**
- **New API**: Added `core.estimate_gas()` method for simplified gas estimation
- **Real Network Conditions**: Replaced static fallback constants with live network-based gas estimation
- **Type-Safe Parameters**: Introduced `GasEstimationParams` enum for Buy, Sell, and SellPermit operations
- **Automatic Problem Solving**: Built-in token approval and permit signature handling

### ✨ New Features

**Enhanced Trading Examples**
- **Gas Estimation Example**: New comprehensive `gas_estimation.rs` example with automatic approval and permit handling
- **Simplified API**: All trading examples now use `core.estimate_gas(&router, params)` instead of complex manual estimation
- **Buffer Strategies**: Demonstrates multiple gas buffer calculation methods (fixed amounts, percentages)
- **Cost Analysis**: Real-time transaction cost estimates at different gas prices

**Smart Gas Management**
- **Automatic Token Approval**: SELL operations automatically handle token approval when needed
- **Real Permit Signatures**: SELL PERMIT operations generate valid EIP-2612 signatures automatically
- **Multiple Buffer Options**: Fixed (+30k, +50k) and percentage-based (15%, 20%, 25%) buffer strategies
- **Enhanced Error Handling**: Graceful fallback mechanisms when estimation fails

### 🔧 API Changes

**Breaking Changes**
- **Removed Static Constants**: Eliminated `BondingCurveGas`, `DexRouterGas`, and `get_default_gas_limit()` functions
- **New Gas Estimation**: `estimate_gas()` now requires `GasEstimationParams` enum instead of individual parameters
- **Trade Method Addition**: Added `estimate_gas()` method to `Trade` struct for convenience

**New Types**
```rust
// New unified parameter enum
pub enum GasEstimationParams {
    Buy { token, amount_in, amount_out_min, to, deadline },
    Sell { token, amount_in, amount_out_min, to, deadline },
    SellPermit { token, amount_in, amount_out_min, to, deadline, v, r, s },
}

// New Trade method
impl Trade {
    pub async fn estimate_gas(&self, router: &Router, params: GasEstimationParams) -> Result<u64>
}
```

### ⚠️ Migration Guide

**For Users Upgrading from v0.1.x:**

1. **Replace Static Gas Constants**:
   ```rust
   // OLD (v0.1.x)
   let gas_limit = BondingCurveGas::BUY;
   
   // NEW (v0.2.0)
   let params = GasEstimationParams::Buy { token, amount_in, amount_out_min, to, deadline };
   let estimated_gas = core.estimate_gas(&router, params).await?;
   ```

2. **Update Gas Estimation Calls**:
   ```rust
   // OLD (v0.1.x)
   let gas = estimate_gas(core.provider().clone(), &router, /* individual params */).await?;
   
   // NEW (v0.2.0)
   let gas = core.estimate_gas(&router, params).await?;
   ```

3. **Remove Fallback Dependencies**:
   - Remove imports: `BondingCurveGas`, `DexRouterGas`, `get_default_gas_limit`, `Operation`
   - Add imports: `GasEstimationParams`

### 🔍 Examples

**New Gas Estimation Example:**
```bash
cargo run --example gas_estimation -- --private-key your_key --rpc-url https://your-rpc --token 0xToken
```

**Updated Trading Examples:**
- `buy.rs`: Uses `core.estimate_gas()` with 20% buffer strategy
- `sell.rs`: Automatic approval + `core.estimate_gas()` with 15% buffer
- `sell_permit.rs`: Real permit signatures + `core.estimate_gas()` with 25% buffer

### 🏆 Benefits

- **Developer Experience**: Simplified API reduces boilerplate code
- **Reliability**: Network-based estimation provides accurate gas predictions
- **Automation**: Automatic problem solving reduces integration complexity
- **Production Ready**: Real network conditions make examples suitable for actual trading

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.2.0"
```

**Full Changelog**: [v0.1.1...v0.2.0](https://github.com/Naddotfun/nadfun-sdk-rust/compare/v0.1.1...v0.2.0)