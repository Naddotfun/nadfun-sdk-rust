# Migrating `nadfun_sdk` 0.3.x → 0.4.0

0.4.0 collapses `CoreV2` into a unified `Core` and removes the
process-global `set_network` lock. Every entry point now takes a
`Network` argument at construction.

## 1. Replace `CoreV2` with `Core`

```rust
// Before (0.3.x) — two separate clients.
use nadfun_sdk::{Core, CoreV2, Network};
let core_v1 = Core::new(rpc.clone(), key.clone(), Network::Mainnet).await?;
let core_v2 = CoreV2::new(rpc, key, Network::Mainnet).await?;

// After (0.4.0) — one client handles both.
use nadfun_sdk::{Core, Network};
let core = Core::new(rpc, key, Network::Mainnet).await?;
```

| 0.3.x (`CoreV2::*`)                       | 0.4.0 (`core.v2().*`)                              |
|-------------------------------------------|----------------------------------------------------|
| `c.buy(params)`                           | `core.v2().buy(params)`                            |
| `c.buy_with_native(params, value)`        | `core.v2().buy_with_native(params)` (value in struct) |
| `c.buy_with_permit(params)`               | `core.v2().buy_with_permit(params)`                |
| `c.sell(params)`                          | `core.v2().sell(params)`                           |
| `c.sell_to_native(params)`                | `core.v2().sell_to_native(params)`                 |
| `c.sell_with_permit(params)`              | `core.v2().sell_with_permit(params)`               |
| `c.sell_to_native_with_permit(params)`    | `core.v2().sell_to_native_with_permit(params)`     |
| `c.exact_out_buy(params)`                 | `core.v2().exact_out_buy(params)`                  |
| `c.exact_out_buy_with_native(params)`     | `core.v2().exact_out_buy_with_native(params)`      |
| `c.exact_out_sell(params)`                | `core.v2().exact_out_sell(params)`                 |
| `c.exact_out_sell_to_native(params)`      | `core.v2().exact_out_sell_to_native(params)`       |
| `c.create(params)`                        | `core.v2().create(params)`                         |
| `c.create_with_native(params)`            | `core.v2().create_with_native(params)`             |
| `c.create_token(params, &api)`            | `core.v2().create_token(params, &api)`             |
| `c.quote(t, a, is_buy)`                   | `core.v2().get_amount_out(t, a, is_buy)`           |
| `c.quote_in(...)`                         | `core.v2().get_amount_in(...)`                     |
| `c.quote_bonding_curve(...)`              | `core.v2().get_bonding_curve_amount_out(...)`      |
| `c.quote_dex(...)`                        | `core.v2().get_dex_amount_out(...)`                |
| `c.is_graduated(t)`                       | `core.v2().is_graduated(t)`                        |
| `c.pool_address(t)`                       | `core.v2().pool_address(t)`                        |
| `c.wrapped_native()`                      | `core.v2().wrapped_native()`                       |
| `c.estimate_gas(params)`                  | `core.v2().estimate_gas(params)`                   |
| `c.router()` / `c.factory()` / …          | `core.v2().router()` / `core.v2().factory()` / …   |

The escape-hatch accessors (`core.v2().router()`, `.factory()`,
`.bonding_curve()`, `.token_registry()`) return `&_` directly. Every
supported `Network` ships with a v2 deployment, so `Core::new` either
wires v2 or fails loudly during construction — there's no dead branch
for these accessors to guard against.

Use `Core::detect_version(token)` (stateless on-chain `TokenInfoLens`
lookup, one RPC) or `api.get_token(token).version` to pick v1 vs v2 paths
from a token address you don't know up front. `SdkVersion::None`
distinguishes "arbitrary ERC-20" from "real v1 token" — be sure to handle
the variant in your `match`. For batch lookups use
`Core::detect_versions(tokens)` (one RPC for the whole list).

`Core::detect_token_info(token)` returns a `TokenInfo { version,
quote_token }` — the version **plus** the token's on-chain quote token —
in the same single call (`Core::detect_token_infos` for the batch). The
SDK does not auto-select a trade method from `quote_token`; use it to drive
the matrix in §7 yourself.

## 2. Drop `set_network` — pass `Network` to every constructor

```rust
// Before
use nadfun_sdk::{set_network, ApiClient, Core, CurveStream, Network};
set_network(Network::Mainnet);
let core = Core::new(rpc, key).await?;        // implicit network
let api = ApiClient::new();
let curve = CurveStream::new(ws_url).await?;

// After
use nadfun_sdk::{ApiClient, Core, CurveStream, Network};
let core = Core::new(rpc, key, Network::Mainnet).await?;
let api = ApiClient::new(Network::Mainnet);
let curve = CurveStream::new(ws_url, Network::Mainnet).await?;
```

Constructor changes (every one now takes `Network` as a trailing arg):

- `ApiClient::new(network)`, `ApiClient::from_env(network)`
- `CurveStream::new(rpc, network)`
- `CurveIndexer::new(provider, network)`
- `DexStream::new(rpc, pools, network)`,
  `DexStream::discover_pools_for_tokens(rpc, tokens, network)`,
  `DexStream::discover_pool_for_token(rpc, token, network)`
- `DexIndexer::new(rpc, pools, network)`,
  `DexIndexer::discover_pools_for_tokens(rpc, tokens, network)`,
  `DexIndexer::discover_pool_for_token(rpc, token, network)`
- `CurveStreamV2::new(ws, network)`
- `CurveIndexerV2::new(provider, network)`
- `NadFunSwapStream::new(ws, pairs, network)` — also rejects empty
  `pairs` lists now (see Codex P3 #18).
- `NadFunSwapIndexer::new(provider, pairs, network)`
- `PoolDiscovery::new(provider, network)`
- `PoolMetadata::new(network)`
- `get_pool_addresses_for_tokens(provider, tokens, network)`
- `discover_pools_unified(provider, tokens, network)` — also dropped the
  `factory_v2_address` parameter; uses `TokenRegistryV2::getPair`
  internally.
- `constants::get_*()` — every helper takes `network: Network`. The
  `addresses::mainnet::*` flat default (`pub use mainnet::*;`) is gone;
  pick `addresses::mainnet::v1::*` / `addresses::mainnet::v2::*`
  explicitly if you read those constants directly.

## 3. `V2CreatePayment::Native` no longer carries a `value`

```rust
// Before
V2CreatePayment::Native { value: parse_ether("0.1")? }

// After — the native amount comes from V2CreateTokenParams.buy_quote_amount,
// and the variant now carries a required `quote_token: Address` (see §9).
V2CreatePayment::Native { quote_token: wmon }
```

`Core::create_token` (v2) sets `native_value = deployFee + params.buy_quote_amount`
on the on-chain call. Previously the two fields were settable independently
and could silently underfund the initial buy (Codex P1 #4).

## 9. `V2CreatePayment::Native` now carries a required `quote_token: Address` (Unreleased)

The SDK no longer bakes in the wrapped-native address (or a single LvMON
constant) for native-funded v2 creates, and it does not auto-resolve one. The
`Native` variant carries the native-equivalent quote token explicitly — the
caller always supplies it:

```rust
use nadfun_sdk::{quote_tokens, Network};

// Resolve the wrapped native (MON / WMON) from the structured registry…
let wmon = quote_tokens(Network::Mainnet)
    .iter()
    .find(|qt| qt.is_native && qt.symbol == "MON")
    .map(|qt| qt.address.parse().unwrap())
    .unwrap();

// Before (0.4.0)
// V2CreatePayment::Native

// After
V2CreatePayment::Native { quote_token: wmon }

// …or pass another native-equivalent the router honors (e.g. LVMON), or
// resolve on-chain with `core.v2().wrapped_native().await?`.
```

The caller-supplied address is used verbatim; anything the on-chain
`createWithNative` rejects reverts with `InvalidNativeQuoteToken`.

Also removed: `constants::get_lv_mon_v2` and `addresses::testnet::v2::LV_MON`.
Use the structured `quote_tokens(Network)` registry instead — it lists every
native-equivalent (`is_native == true`), including LVMON, mirroring the
api-server `GET /quote_token` source of truth. The v1 `get_wmon` / `WMON`
constants are unchanged.

## 4. `V2BuyWithNativeParams.value` is now a struct field

```rust
// Before
core.buy_with_native(params, value).await?;

// After
core.v2().buy_with_native(V2BuyWithNativeParams {
    /* existing fields */,
    value,
}).await?;
```

`V2GasEstimationParams::BuyWithNative` also collapsed from a struct
variant (`{ params, value }`) to a tuple variant carrying the same
`V2BuyWithNativeParams` (which now includes `value`):

```rust
// Before
V2GasEstimationParams::BuyWithNative { params, value }

// After
V2GasEstimationParams::BuyWithNative(params)  // params.value is the msg.value
```

## 5. `V2PreparedCreation` carries `name` + `symbol`

The salt server may normalize (trim, sanitize) the user-supplied name
and symbol before computing the CREATE2 hash. `core.v2().create_token`
now uses the server's normalized strings for the on-chain create, so
the deploy lands at the address the API predicts.

If you construct `V2PreparedCreation` directly (unusual — the field is
populated by `ApiClient::prepare_token_creation_v2`), add the two new
fields:

```rust
V2PreparedCreation {
    image_uri, metadata_uri, salt, token_address, is_nsfw,
    name: "...".into(),
    symbol: "...".into(),
}
```

## 6. Re-exports — alloy `sol!` types are no longer public

`use nadfun_sdk::types::*;` still works but no longer re-exports the
internal alloy `sol!`-generated contract modules (`IBondingCurve`,
`ICapricornCLPool`, `IBondingCurveV2Events`, `INadFunRouter`,
`INadFunPair`, etc.) at the crate root. If your code reached into those:

```rust
// Before
use nadfun_sdk::IBondingCurve;

// After — go through the explicit contracts module
use nadfun_sdk::contracts::v1::bonding_curve::IBondingCurve;
```

Closed Codex P1 #7 (glob-export collision between v1 and v2 internal
contract names).

## 7. v2: choosing the right trade method by `quote_token`

v2 introduces multi-quote-token support. **Price-quote** methods are
quote-token-agnostic — the router does the routing internally and you
call a single method regardless of the underlying currency. **Trade
execution** methods are not: native MON flows and ERC-20 flows have
separate router entrypoints, so you pick the SDK method based on the
token's `quote_token`.

### Quote-token-agnostic (no branching needed)

The router handles BC vs DEX + quote-token bookkeeping internally:

| Purpose                  | Method (works for any `quote_token`)        |
|--------------------------|---------------------------------------------|
| Price quote (exact-in)   | `core.v2().get_amount_out(token, amount_in, is_buy)` |
| Price quote (exact-out)  | `core.v2().get_amount_in(token, amount_out, is_buy)` |
| Force BC quote           | `core.v2().get_bonding_curve_amount_out(...)` |
| Force DEX quote          | `core.v2().get_dex_amount_out(...)`         |
| Pool address             | `core.v2().pool_address(token)`             |
| Graduation status        | `core.v2().is_graduated(token)`             |
| Wrapped native           | `core.v2().wrapped_native()`                |
| Version detect (v1/v2/None) | `core.detect_version(token)`             |
| Version + quote token (1 RPC) | `core.detect_token_info(token)` → `TokenInfo` |

### Trade execution — pick by `quote_token`

These hit different router entrypoints and **do** depend on the token's
`quote_token`:

| `quote_token` | Buy                              | Sell                                          |
|---------------|----------------------------------|-----------------------------------------------|
| **WMON**      | `core.v2().buy_with_native`      | `core.v2().sell_to_native` (router unwraps)   |
| **LvMON**     | `core.v2().buy_with_native`      | `core.v2().sell` (LvMON can't unwrap — ERC-20 path) |
| **Other ERC-20** (USDT, …) | `core.v2().buy`     | `core.v2().sell`                              |

Note the LvMON asymmetry: buying with native MON wraps into LvMON via
the LvMON minter on the way in, but selling back doesn't have a reverse
unwrap path — you receive raw LvMON tokens and must use the ERC-20
`core.v2().sell` form. WMON has a symmetric wrap/unwrap so both legs use the
native-flavored helpers.

`exact_out_*` and `*_with_permit` variants follow the same matrix
(`core.v2().exact_out_buy_with_native` for WMON/LvMON buy,
`core.v2().exact_out_buy` for other ERC-20s, etc.). To dispatch from a generic address, read the
`quote_token` on-chain in one call with `core.detect_token_info(token)`
(returns `{ version, quote_token }`) — or off-chain via
`api.get_token(token).quote_token`. The SDK never picks the trade method
for you; this matrix is yours to apply.

## 8. Rename: `quote_*` → `get_*amount_*` on v2

To reserve the word "quote" for "quote token" (the trade's pricing
currency), the v2 price-quote methods were renamed to match the v1
`get_amount_out` / `get_amount_in` family:

| 0.4.0-rc                          | 0.4.0                                          |
|-----------------------------------|------------------------------------------------|
| `c.quote_v2(t, a, is_buy)`        | `core.v2().get_amount_out(t, a, is_buy)`       |
| `c.quote_in_v2(...)`              | `core.v2().get_amount_in(...)`                 |
| `c.quote_bonding_curve_v2(...)`   | `core.v2().get_bonding_curve_amount_out(...)`  |
| `c.quote_bonding_curve_in_v2(...)`| `core.v2().get_bonding_curve_amount_in(...)`   |
| `c.quote_dex_v2(...)`             | `core.v2().get_dex_amount_out(...)`            |
| `c.quote_dex_in_v2(...)`          | `core.v2().get_dex_amount_in(...)`             |
