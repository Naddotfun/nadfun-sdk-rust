# Nad.fun SDK Examples

This directory contains comprehensive examples demonstrating how to use the Nad.fun SDK for token creation, trading, and real-time event streaming.

> 🔀 **v1 vs v2 — you choose the method.** As of `0.4.0` a single `Core` instance serves both bonding-curve generations. The SDK does **not** auto-route trades: call `core.buy(...)` / `core.sell(...)` for v1 and the `*_v2` surface (`core.buy_v2(...)`, `core.buy_with_native_v2(...)`, `core.sell_to_native_v2(...)`, ...) for v2. To pick at runtime, ask the chain which generation a token belongs to with `core.detect_version(token)` (version only) or `core.detect_token_info(token)` (version **and** the v2 quote token). The [`unified_dispatch`](#mixed-version-dispatch) example shows the full pattern. Sections are grouped **v1 first, then v2**.

## 🎨 Token Creation Examples

### Token Creation (`create/create_token.rs`)
Create a new token with automatic image upload, metadata storage, and initial buy.

```bash
cargo run --example create_token -- \
  --private-key your_private_key_here \
  --rpc-url https://your-rpc-url \
  --network mainnet \
  --name "My Token" \
  --symbol "MTK" \
  --description "My awesome token" \
  --image-uri "https://i.imgur.com/yourimage.png" \
  --initial-buy "1.5"
```

**Image Requirements:**
- ✅ **Allowed formats**: JPEG, PNG, WEBP, SVG only
- ❌ **Not supported**: GIF, BMP, TIFF, and other formats
- 📏 **Max size**: 5MB
- 🖼️ **Recommended size**: 512x512 or 1024x1024 pixels
- 🔞 **NSFW**: Automatically detected and rejected

**Optional Parameters:**
```bash
--website "https://mytoken.com"
--twitter "https://x.com/mytoken"
--telegram "https://t.me/mytoken"
```

**Features:**
- 🖼️ **Automatic Image Upload**: Downloads from URL and uploads to IPFS
- 🤖 **NSFW Detection**: AI-powered content moderation
- 📝 **Metadata Storage**: Creates and stores token metadata
- 🎲 **Vanity Address**: Generates salt for custom token address
- 💰 **Initial Buy**: Purchases tokens during creation
- 🔐 **Deploy Fee**: Automatically calculated and included
- 🎭 **Action ID**: Uses `CapricornActor` (value: 1) for token creation

---

## 💰 Trading Examples

### 1. Buy Tokens (`core/buy.rs`)
Buy tokens with MON including advanced gas management and slippage protection.

```bash
# Using environment variables
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export TOKEN="0xTokenAddress"
cargo run --example buy

# Using command line arguments
cargo run --example buy -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- ⛽ **Smart Gas Management**: Real-time estimation vs default gas limits comparison
- 🔄 **Automatic Router Detection**: Bonding curve vs DEX routing
- 🛡️ **Slippage Protection**: 5% default with customizable amount_out_min
- 📊 **Network Gas Price Optimization**: EIP-1559 compatible with 3x multiplier
- ✅ **Balance Verification**: MON balance checking before execution
- 📝 **Transaction Verification**: Complete result validation

**Example Output:**
```
💰 Account MON balance: 10.5 MON
⛽ Network gas price: 25 gwei
⛽ Recommended gas price: 75 gwei
⛽ Estimated gas for buy contract call: 245123
⛽ Using default gas limit: 320000
🛡️ Slippage protection:
  Expected tokens: 1234567890123456789
  Minimum tokens (5% slippage): 1172839745617283950
✅ Buy successful!
  Transaction hash: 0x...
  Gas used: 247891
```

### 2. Gas Estimation (`core/gas_estimation.rs`)
Comprehensive gas estimation example for all trading operations with automatic problem solving.

```bash
cargo run --example gas_estimation -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- ⛽ **Unified Gas Estimation**: Uses `core.estimate_gas()` for BUY, SELL, and SELL PERMIT operations
- 🔧 **Automatic Problem Solving**: Handles token approval and EIP-2612 permit signatures automatically
- 📊 **Buffer Strategies**: Demonstrates different buffer calculation methods (fixed +50k, percentage 20%-25%)
- 💰 **Cost Analysis**: Shows estimated transaction costs at different gas prices
- ⚠️ **Real Network Conditions**: Uses actual network estimation with proper error handling

**Important Requirements:**
- **Token Approval**: SELL operations require approval (automatically handled)
- **Permit Signatures**: SELL PERMIT needs valid signatures (automatically generated)
- **Network Connection**: Live RPC required for accurate estimation

### 3. Sell Tokens (`core/sell.rs`)
Sell tokens for MON with automatic approval and intelligent gas optimization.

```bash
cargo run --example sell -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- 🔍 **Token Balance Verification**: Ensures sufficient token balance
- 📋 **Automatic Approval Handling**: Checks allowance and approves if needed
- ⛽ **Dynamic Gas Estimation**: Real-time gas estimation with safe defaults
- 🛡️ **Slippage Protection**: Configurable slippage tolerance
- 🔄 **Two-step Process**: Approve → Sell workflow
- 📊 **Gas Comparison**: Shows estimated vs default gas limits

### 4. Gasless Sell (`core/sell_permit.rs`)
Advanced gasless selling using EIP-2612 permit signatures.

```bash
cargo run --example sell_permit -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- 🔐 **EIP-2612 Permit Signatures**: Cryptographic gasless approvals
- ⚡ **One Transaction**: Combined approval + sell in single tx
- ⛽ **Optimized Gas**: Higher gas limits for complex permit transactions
- 🛡️ **Security**: Proper nonce and deadline management
- 📝 **Signature Details**: v, r, s component logging for transparency

## 🪙 Token Helper Examples

### 5. Basic ERC20 Operations (`token/basic_operations.rs`)
Comprehensive ERC20 token interaction patterns.

```bash
cargo run --example basic_operations -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress --recipient 0xRecipientAddress
```

**Features:**
- 📊 **Token Metadata**: Name, symbol, decimals, total supply retrieval
- 💰 **Balance Operations**: Check balances for any address
- 📝 **Allowance Management**: Check and set token approvals
- 💸 **Token Transfers**: Safe token transfer operations
- 🔄 **Complete Workflows**: End-to-end transaction examples

### 6. EIP-2612 Permit Signatures (`token/permit_signature.rs`)
Master gasless approvals with cryptographic permit signatures.

```bash
cargo run --example permit_signature -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress --recipient 0xRecipientAddress
```

**Features:**
- 🔐 **Permit Signature Generation**: EIP-2612 compliant signatures
- 🧮 **Domain Separator Calculation**: Proper EIP-712 domain handling
- 📊 **Nonce Management**: Account nonce tracking and management
- 🔍 **Signature Components**: Detailed v, r, s breakdown
- 🛡️ **Security Best Practices**: Deadline and nonce validation

## 📡 Event Streaming Examples

### 7. Bonding Curve Event Indexing (`stream/curve_indexer.rs`)
Historical bonding curve event analysis with batch processing.

```bash
# Fetch all bonding curve events
cargo run --example curve_indexer -- --rpc-url https://your-rpc-endpoint

# Filter by specific tokens
cargo run --example curve_indexer -- --rpc-url https://your-rpc-endpoint --tokens 0xToken1,0xToken2
```

**Features:**
- 📊 **Historical Data**: Fetch events from specific block ranges
- 🎯 **Event Filtering**: Create, Buy, Sell, Sync, Lock, Graduate events
- 🔄 **Batch Processing**: Efficient handling of large datasets
- 📈 **Statistics**: Event counts and analysis
- 🪙 **Token Filtering**: Focus on specific token addresses

### 8. Real-time Bonding Curve Streaming (`stream/curve_stream.rs`)
Live bonding curve event monitoring with WebSocket streaming.

```bash
# Monitor all bonding curve events
cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint

# Filter specific event types
EVENTS=Buy,Sell cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint

# Filter specific tokens
cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint --tokens 0xToken1,0xToken2

# Combined filtering (events AND tokens)
EVENTS=Buy,Sell cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint --tokens 0xToken1
```

**Features:**
- ⚡ **Real-time Streaming**: WebSocket-based low-latency event delivery
- 🎯 **Flexible Filtering**: Event types and token address filtering
- 🔄 **All Event Types**: Create, Buy, Sell, Sync, Lock, Graduate support
- 📊 **Live Processing**: Immediate event handling and analysis
- 🛡️ **Error Handling**: Robust connection management

### 9. DEX Event Indexing (`stream/dex_indexer.rs`)
Historical Capricorn CL swap event analysis with pool discovery.

```bash
# Auto-discover pools and fetch swap events
cargo run --example dex_indexer -- --rpc-url https://your-rpc-endpoint --tokens 0xToken1,0xToken2

# JSON array format
cargo run --example dex_indexer -- --rpc-url https://your-rpc-endpoint --tokens '["0xToken1","0xToken2"]'
```

**Features:**
- 🔍 **Automatic Pool Discovery**: Find Capricorn CL pools for tokens
- 📊 **Swap Event Analysis**: Complete swap transaction details
- 🏊 **Pool Metadata**: Pool addresses, fee tiers, token pairs
- 📈 **Historical Data**: Configurable block range processing
- 🎯 **Token-specific**: Focus on specific token trading activity

### 10. Real-time DEX Streaming (`stream/dex_stream.rs`)
Live Capricorn CL swap monitoring with pool auto-discovery.

```bash
# Monitor specific pools directly
POOLS=0xPool1,0xPool2 cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint

# Auto-discover pools for tokens
cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint --tokens 0xToken1,0xToken2

# Single token monitoring
cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint --token 0xTokenAddress
```

**Features:**
- 🔍 **Pool Auto-discovery**: Automatic Capricorn CL pool detection
- ⚡ **Real-time Swaps**: Live swap event monitoring
- 🏊 **Pool Metadata**: Complete pool information included
- 📊 **Swap Details**: amount0, amount1, sender, recipient, tick data
- 🎯 **Flexible Targeting**: Pool addresses or token-based discovery

### 12. Pool Discovery (`stream/pool_discovery.rs`)
Automated Capricorn CL pool address discovery utility.

```bash
# Discover pools for multiple tokens
cargo run --example pool_discovery -- --rpc-url https://your-rpc-endpoint --tokens 0xToken1,0xToken2

# Single token discovery
cargo run --example pool_discovery -- --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- 🔍 **Comprehensive Discovery**: Find all Capricorn CL pools for tokens
- 🏊 **Pool Information**: Addresses, fee tiers, token pairs
- 📊 **Multiple Tokens**: Batch discovery for token lists
- 🎯 **Targeted Search**: Single token or multi-token discovery
- 📝 **Detailed Output**: Complete pool metadata reporting

---

# 🆕 v2 Examples (NadFunRouter)

The examples below target the **v2** bonding curve + NadFunRouter surface, all exposed through the same unified `Core`. v2 supports arbitrary ERC-20 quote tokens (not just native MON), exact-output buys, and an updated event set (`Create`, `Buy`, `Sell`, `Sync`, `Graduate`, `SnipingPenalty`). Cargo target names are prefixed `v2_`.

> ℹ️ All v2 trade/stream examples read the same shared flags as v1 (`--private-key` / `PRIVATE_KEY`, `--rpc-url` / `RPC_URL`, `--ws-url` / `WS_URL`, `--token` / `TOKEN`, `--tokens` / `TOKENS`, `--network` / `NETWORK`). **Trade amounts are hardcoded inside each example** (e.g. 0.01 MON, 100 tokens) — edit the source to change them, there is no `--amount` flag.

## 🎨 v2 Token Creation

### v2 Token Creation (`v2/create_token.rs`)
Deploy a v2 token through `NadFunRouter` — image upload + IPFS metadata + salt + on-chain create with a creator-fee vault split and an initial buy, in one call (`core.create_token_v2`).

```bash
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export NAD_API_KEY="nadfun_xxxxxxxx"   # required for IPFS upload + metadata
cargo run --example v2_create_token -- \
  --private-key your_private_key_here \
  --rpc-url https://your-rpc-endpoint \
  --network testnet \
  --name "Rocket Pepe" \
  --symbol "RPEPE" \
  --description "Demo v2 token" \
  --image-uri "https://i.imgur.com/0qY8Vp6.png"
```

**Required env:** `PRIVATE_KEY`, `RPC_URL`, `NAD_API_KEY` (the API client uploads the image + metadata via `ApiClient::from_env`).
**Optional:** `--website`, `--twitter`, `--telegram` (same validation rules as v1). Defaults are baked in for `--name` / `--symbol` / `--description` / `--image-uri` if omitted.

**Features:**
- 🏭 **NadFunRouter Deploy**: Single `create_token_v2` call performs deploy + initial buy
- 🔥 **Vault Split**: Sample 50/50 creator-fee allocation between BurnVault and LPVault (`V2VaultAllocation`, BPS-based; vault addresses resolved from `constants` per network)
- 💸 **Creator Fee Rate**: `creator_fee_rate` in BPS (example uses 100 = 1.00%)
- 💰 **Initial Buy**: Fixed at 1.5 MON in the example (`buy_quote_amount`); `--initial-buy` is **not** wired into this example — edit the source to change it
- 🪙 **Native Payment**: `V2CreatePayment::Native` (deploy fee auto-included)
- 🎲 **Vanity Salt + NSFW**: Salt for the deterministic token address; AI NSFW screening on the image

## 💰 v2 Trading

### v2 Buy with Native MON (`v2/buy.rs`)
Buy a v2 token by sending native MON; the router auto-routes bonding-curve vs DEX based on graduation and wraps MON for you (`core.buy_with_native_v2`).

```bash
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export TOKEN="0xTokenAddress"
cargo run --example v2_buy

# Or with args:
cargo run --example v2_buy -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- 🪙 **Native Funding**: Sends 0.01 MON (hardcoded `value`); router wraps to the native quote
- 📊 **Quote First**: `core.get_amount_out_v2(token, amount, true)` with a zero-quote guard
- 🛡️ **Slippage**: `SlippageUtils::calculate_amount_out_min(expected, 5.0)` (5%)
- ⛽ **v2 Gas Estimation**: `core.estimate_gas_v2(V2GasEstimationParams::BuyWithNative(..))` + 20% buffer, falls back to 400k
- 📝 **Receipt Check**: `core.get_receipt(tx_hash)` status/block reporting

### v2 Buy with ERC-20 Quote (`v2/buy_erc20_quote.rs`)
Buy a v2 token paying with an **ERC-20 quote token** (e.g. USDT) instead of native MON — a v2-only capability (`core.buy_v2`).

```bash
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export TOKEN="0xTokenAddress"
cargo run --example v2_buy_erc20_quote

# Or with args:
cargo run --example v2_buy_erc20_quote -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- 🪙 **ERC-20 Quote**: `amount_in` is denominated in the quote token (example assumes 18 decimals; adjust for USDT's 6)
- 🔐 **Pre-approval Required**: Caller must approve the v2 router for `amount_in` of the quote token beforehand (use `TokenHelper`)
- 📊 **Quote + Slippage**: `get_amount_out_v2` then 5% `amount_out_min`
- 📝 **Receipt Check**: Status/block reporting

### v2 Sell to Native (`v2/sell.rs`)
Sell v2 tokens back to native MON (`core.sell_to_native_v2`).

```bash
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export TOKEN="0xTokenAddress"
cargo run --example v2_sell

# Or with args:
cargo run --example v2_sell -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- 💱 **Sell to Native**: Sells 100 tokens (hardcoded, 18 decimals) for MON
- 🔐 **Approval Required**: Caller must approve the router for `amount_in` of the token first (or use the permit flow below)
- 📊 **Reverse Quote**: `core.get_amount_out_v2(token, amount, false)` + 5% slippage
- 📝 **Receipt Check**: Status/block reporting

### v2 Exact-Output Buy (`v2/exact_out.rs`)
"I want exactly N tokens; spend at most M MON" — exact-output buy with native MON (`core.exact_out_buy_with_native_v2`).

```bash
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export TOKEN="0xTokenAddress"
cargo run --example v2_exact_out

# Or with args:
cargo run --example v2_exact_out -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**
- 🎯 **Exact Output**: Targets exactly 1 token out (`amount_out`), capping spend at `amount_in_max` (1 MON)
- 🔁 **Inverse Quote Guard**: `core.get_amount_in_v2(token, amount_out, true)` sanity-checks cost before sending
- 🪙 **Native Funding**: Spends MON up to the cap; refunds the remainder
- 📝 **Receipt Check**: Status/block reporting

### Permit-based v2 trades (no standalone example yet)
The gasless EIP-2612 permit variants — `core.buy_with_permit_v2`, `core.sell_with_permit_v2`, and `core.sell_to_native_with_permit_v2` — are available on the `Core` API but are **not** yet shown as standalone examples. See `v2/sell.rs` (which notes the permit flow) and the v1 `sell_permit` example for the permit pattern.

## 📡 v2 Event Streaming

### v2 Bonding Curve Streaming (`v2/curve_stream.rs`)
Subscribe to v2 `BondingCurve` events in real time over WebSocket (`stream::v2::CurveStreamV2`).

```bash
# All v2 bonding curve events
cargo run --example v2_curve_stream -- --ws-url wss://your-ws-endpoint --network testnet

# Filter event types via EVENTS env (Create,Buy,Sell,Sync,Graduate,SnipingPenalty)
EVENTS=Buy,Sell cargo run --example v2_curve_stream -- --ws-url wss://your-ws-endpoint

# Client-side token filter
cargo run --example v2_curve_stream -- --ws-url wss://your-ws-endpoint --tokens 0xToken1,0xToken2
```

**Features:**
- ⚡ **Real-time v2 Events**: `CurveStreamV2` over `--ws-url` / `WS_URL`
- 🎯 **Event Filtering**: `EVENTS` env → `subscribe_events(..)` with `V2EventType` (`Create`, `Buy`, `Sell`, `Sync`, `Graduate`, `SnipingPenalty`)
- 🪙 **Token Filtering**: `--tokens` → `filter_tokens(..)`
- 📊 **Decoded Fields**: `event_type`, `token`, `block_number`, `log_index`

### v2 DEX (Pair) Swap Streaming (`v2/dex_stream.rs`)
Subscribe to `NadFunPair` swap events for a token's pair, resolving pair addresses via the v2 registry (`stream::v2::NadFunSwapStream`).

```bash
# Resolve pairs for tokens, then stream their swaps
cargo run --example v2_dex_stream -- --ws-url wss://your-ws-endpoint --rpc-url https://your-rpc-endpoint --tokens 0xToken1,0xToken2 --network testnet
```

**Required:** `--tokens` / `TOKENS` (the example resolves each token's pair via `core.pool_address_v2`). Provide an RPC URL (`--rpc-url` / `RPC_URL`) for the registry lookup in addition to `--ws-url` / `WS_URL` for the stream. No private key needed — it uses a dummy key for the read-only `Core`.

**Features:**
- 🔍 **Pair Resolution**: `core.pool_address_v2(token)` per token (skips unregistered tokens)
- ⚡ **Real-time Swaps**: `NadFunSwapStream` over the resolved pairs
- 📊 **Swap Details**: `pair_address`, `sender`, `to`, `amount0_in/1_in/0_out/1_out`, `block_number`

## 🔍 v2 Pool Discovery

### Unified Pool Discovery (`v2/pool_discovery.rs`)
Discover pools for a list of tokens across **both** v1 (Capricorn CL) and v2 (NadFun) surfaces in one call (`stream::v2::discover_pools_unified`).

```bash
cargo run --example v2_pool_discovery -- --rpc-url https://your-rpc-endpoint --tokens 0xToken1,0xToken2 --network testnet
```

**Required:** `--tokens` / `TOKENS` and `--rpc-url` / `RPC_URL`.

**Features:**
- 🔀 **Cross-surface**: Finds v1 and v2 pools together via `discover_pools_unified`
- 🏷️ **Surface Tagging**: Each result reports `token`, `pool`, and `surface` (v1 vs v2)
- 📊 **Batch**: Multiple tokens in a single pass

## 🔀 Mixed-version Dispatch

### Unified v1/v2 Buy Dispatch (`unified_dispatch.rs`)
Receive an arbitrary token and route the buy through the correct v1 **or** v2 path on a single `Core`, choosing the v2 native-vs-ERC20 quote path automatically.

```bash
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"

# Works for either a v1 or a v2 token:
cargo run --example unified_dispatch -- --token 0xV1Token
cargo run --example unified_dispatch -- --token 0xV2Token
```

**How it routes:**
- `core.detect_token_info(token)` does one on-chain `TokenInfoLens` call returning both the **version** (`SdkVersion::V1` / `V2` / `None`) and the v2 **quote token**.
- `SdkVersion::V1` → `core.buy(BuyParams, router)` (router from `get_amount_out`).
- `SdkVersion::V2` → compares `info.quote_token` to `core.wrapped_native_v2()`:
  - quote == wrapped native (WMON) → `core.buy_with_native_v2(..)` (send MON).
  - quote == other ERC-20 → `core.buy_v2(..)` (pre-approve the quote token).
- `SdkVersion::None` → refuses to trade.

**Features:**
- 🧭 **Single Instance**: One `Core` handles both generations (0.4.0+) — no side-by-side clients
- 🔎 **On-chain Detection**: `detect_token_info` (or `detect_version` for version-only; `ApiClient::get_token` for the off-chain equivalent)
- 🛡️ **Slippage + Guard**: 5% `amount_out_min`; bails on unregistered tokens
- 💵 **Fixed Amount**: `value` is hardcoded to `0.01` (1e16) — MON sent on the native/v1 path, but the quote-token `amount_in` on the v2 ERC-20-quote path (so 0.01 of that quote token, not MON)

## ✅ v2 Smoke Test

### Read-only Contract Wiring Check (`v2/smoke.rs`)
Verify every wired v2 contract responds on the active network — **read-only**, no private key, no transactions (only `eth_call`).

```bash
cargo run --example v2_smoke -- --rpc-url https://dev-node.nadapp.net/ --network testnet

# Optional per-token registry probe:
cargo run --example v2_smoke -- --rpc-url https://your-rpc-endpoint --network testnet --token 0xTokenAddress
```

**Features:**
- 📇 **Address Inventory**: Prints every v2 constant for the network (router, factory, pair impl, registry, vaults, fee_to, lv_mon, ...)
- 📡 **Live View Probes**: `block_number`, `router.wrappedNative`, `factory.allPairs/impl/feeCollector`, `bondingCurve.isHalted/VERSION`, `registry.isRegistered(0x0)`
- 🪙 **Optional Token Probe**: Pass `--token` / `--tokens` for per-token `isRegistered` + `getPair`
- 🔐 **No Key Needed**: Uses `Core::with_provider(.., Address::ZERO, ..)` for view-only access

## ⛽ Gas Management Features

All trading examples now use the new unified gas estimation system:

### New Gas Estimation System
- **Real-time Network Estimation**: Uses `core.estimate_gas()` for live gas calculations
- **Automatic Problem Solving**: Handles token approval and permit signatures automatically
- **Network-based Calculation**: No more static fallback constants - all estimates from actual network conditions
- **Smart Buffer Strategies**: Multiple buffer calculation methods (fixed amounts, percentages)

### Gas Estimation Requirements
⚠️ **Important**: For accurate gas estimation, certain conditions must be met:

- **SELL Operations**: Require token approval for router (automatically handled in examples)
- **SELL PERMIT Operations**: Need valid EIP-2612 permit signatures (automatically generated in examples)
- **Token Balance**: Some token balance recommended for realistic estimation (examples use 1 token minimum)
- **Network Connection**: Live RPC connection required for real-time estimation

### Dynamic Gas Features
- **Real-time Estimation**: Actual contract call gas estimation using network conditions
- **Network Price Detection**: Current gas price with EIP-1559 optimization
- **Multiple Buffer Strategies**: Fixed amounts (+50k) and percentage-based (20%, 25%)
- **Cost Analysis**: Transaction cost estimates at different gas prices
- **Error Handling**: Graceful fallback when estimation fails

**Example Usage:**
```rust
use nadfun_sdk::{Core, GasEstimationParams};

// Unified gas estimation for any operation
let gas_params = GasEstimationParams::Buy { token, amount_in, amount_out_min, to, deadline };
let estimated_gas = core.estimate_gas(&router, gas_params).await?;

// Apply buffer strategy
let gas_with_buffer = estimated_gas * 120 / 100; // 20% buffer
```

## 🚀 Configuration

### Environment Variables
```bash
export RPC_URL="https://your-rpc-endpoint"
export WS_URL="wss://your-ws-endpoint" 
export PRIVATE_KEY="your_private_key_here"
export TOKEN="0xTokenAddress"
export TOKENS="0xToken1,0xToken2"
export RECIPIENT="0xRecipientAddress"
```

### CLI Arguments
All examples support command line arguments:
```bash
--rpc-url <URL>      # RPC URL for HTTP operations
--ws-url <URL>       # WebSocket URL for streaming  
--private-key <KEY>  # Private key for transactions
--token <ADDRESS>    # Single token address
--tokens <ADDRS>     # Multiple tokens: 'addr1,addr2' or '["addr1","addr2"]'
--recipient <ADDR>   # Recipient for transfers/allowances
```

## 📊 Key Features Demonstrated

### Smart Gas Management
- **Real-time vs Defaults**: Compare estimated gas with safe defaults
- **Network Optimization**: EIP-1559 compatible gas pricing
- **Router-specific**: Different limits for bonding curve vs DEX operations
- **Buffer Strategies**: 20% safety buffers with customization options

### Event Processing
- **Real-time Streaming**: WebSocket-based low-latency delivery
- **Historical Indexing**: Batch processing for analysis
- **Flexible Filtering**: Event types and token address filtering
- **Pool Discovery**: Automatic Capricorn CL pool detection

### Transaction Management
- **Slippage Protection**: Configurable tolerance levels
- **Approval Handling**: Automatic allowance checking and approval
- **Permit Signatures**: Gasless EIP-2612 approvals
- **Result Verification**: Complete transaction status validation

## 💡 Best Practices

- **Start with Trading**: Begin with buy/sell examples to understand gas management
- **Use Defaults First**: Default gas limits are tested and safe
- **Monitor Network**: Check gas prices during high activity
- **Test with Small Amounts**: Verify functionality before large transactions
- **Handle Errors**: All examples include proper error handling patterns
- **Secure Keys**: Never commit private keys to version control

## 🔧 Development Tips

- **HTTP for Indexing**: More reliable for historical data fetching
- **WebSocket for Streaming**: Lower latency for real-time monitoring
- **Parallel Processing**: Large datasets benefit from concurrent processing
- **Rate Limiting**: Monitor RPC provider limits and implement backoff
- **Local Caching**: Store frequently accessed data to reduce API calls