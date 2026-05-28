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

| 0.3.x (`CoreV2::*`)                       | 0.4.0 (`Core::*`)                              |
|-------------------------------------------|------------------------------------------------|
| `c.buy(params)`                           | `c.buy_v2(params)`                             |
| `c.buy_with_native(params, value)`        | `c.buy_with_native_v2(params)` (value in struct) |
| `c.buy_with_permit(params)`               | `c.buy_with_permit_v2(params)`                 |
| `c.sell(params)`                          | `c.sell_v2(params)`                            |
| `c.sell_to_native(params)`                | `c.sell_to_native_v2(params)`                  |
| `c.sell_with_permit(params)`              | `c.sell_with_permit_v2(params)`                |
| `c.sell_to_native_with_permit(params)`    | `c.sell_to_native_with_permit_v2(params)`      |
| `c.exact_out_buy(params)`                 | `c.exact_out_buy_v2(params)`                   |
| `c.exact_out_buy_with_native(params)`     | `c.exact_out_buy_with_native_v2(params)`       |
| `c.exact_out_sell(params)`                | `c.exact_out_sell_v2(params)`                  |
| `c.exact_out_sell_to_native(params)`      | `c.exact_out_sell_to_native_v2(params)`        |
| `c.create(params)`                        | `c.create_v2(params)`                          |
| `c.create_with_native(params)`            | `c.create_with_native_v2(params)`              |
| `c.create_token(params, &api)`            | `c.create_token_v2(params, &api)`              |
| `c.quote(t, a, is_buy)`                   | `c.quote_v2(t, a, is_buy)`                     |
| `c.quote_in(...)`                         | `c.quote_in_v2(...)`                           |
| `c.quote_bonding_curve(...)`              | `c.quote_bonding_curve_v2(...)`                |
| `c.quote_dex(...)`                        | `c.quote_dex_v2(...)`                          |
| `c.is_graduated(t)`                       | `c.is_graduated_v2(t)`                         |
| `c.pool_address(t)`                       | `c.pool_address_v2(t)`                         |
| `c.wrapped_native()`                      | `c.wrapped_native_v2()`                        |
| `c.estimate_gas(params)`                  | `c.estimate_gas_v2(params)`                    |
| `c.router()` / `c.factory()` / …          | `c.router_v2()` / `c.factory_v2()` / …         |

The escape-hatch accessors (`router_v2`, `factory_v2`,
`bonding_curve_v2`, `token_registry_v2`) return `&_` directly. Every
supported `Network` ships with a v2 deployment, so `Core::new` either
wires v2 or fails loudly during construction — there's no dead branch
for these accessors to guard against.

Use `Core::detect_version(token)` (with in-process cache) or
`api.get_token(token).version` to pick v1 vs v2 paths from a token
address you don't know up front.

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

// After — the native amount comes from V2CreateTokenParams.buy_quote_amount
V2CreatePayment::Native
```

`Core::create_token_v2` sets `native_value = params.buy_quote_amount` on
the on-chain call. Previously the two fields were settable independently
and could silently underfund the initial buy (Codex P1 #4).

## 4. `V2BuyWithNativeParams.value` is now a struct field

```rust
// Before
core.buy_with_native(params, value).await?;

// After
core.buy_with_native_v2(V2BuyWithNativeParams {
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
and symbol before computing the CREATE2 hash. `Core::create_token_v2`
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
