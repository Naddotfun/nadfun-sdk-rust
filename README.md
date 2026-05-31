# Nad.fun Rust SDK

A comprehensive Rust SDK for interacting with Nad.fun ecosystem contracts, including bonding curves, DEX trading, and real-time event monitoring.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
nadfun_sdk = "0.4.0"
```

## Quick Start

```rust
use nadfun_sdk::prelude::*; // Import everything you need
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_url = "https://your-rpc-endpoint".to_string();
    let private_key = "your_private_key_here".to_string();

    // Initialize Core - set network once, it's used everywhere automatically
    let core = Core::new(rpc_url.clone(), private_key.clone(), Network::Mainnet).await?;

    // 1. Get quote for buying tokens
    let token: Address = "0x...".parse()?;
    let mon_amount = parse_ether("0.1")?; // Buy with 0.1 MON
    let (router, expected_tokens) = core.v1().get_amount_out(token, mon_amount, true).await?;

    // 2. Apply slippage protection (5%)
    let min_tokens = SlippageUtils::calculate_amount_out_min(expected_tokens, 5.0);

    // 3. Estimate gas
    let gas_params = GasEstimationParams::Buy {
        token,
        amount_in: mon_amount,
        amount_out_min: min_tokens,
        to: core.wallet_address(),
        deadline: U256::from(9999999999999999u64),
    };
    let estimated_gas = core.v1().estimate_gas(&router, gas_params).await?;
    let gas_with_buffer = estimated_gas * 120 / 100; // Add 20% buffer

    // 4. Execute buy
    let buy_params = BuyParams {
        token,
        amount_in: mon_amount,
        amount_out_min: min_tokens,
        to: core.wallet_address(),
        deadline: U256::from(9999999999999999u64),
        gas_limit: Some(gas_with_buffer),
        gas_price: None, // Or use Some(GasPricing::Eip1559 { ... })
        nonce: None,     // Auto-increment
    };

    // Execute buy - returns tx_hash immediately
    let tx_hash = core.v1().buy(buy_params, router).await?;
    println!("Transaction submitted: {}", tx_hash);

    // Optionally wait for receipt to check status
    let receipt = core.get_receipt(tx_hash).await?;
    println!("Transaction confirmed in block: {:?}", receipt.block_number);
    println!("Gas used: {:?}", receipt.gas_used);
    println!("Status: {}", if receipt.status { "Success" } else { "Failed" });

    Ok(())
}
```

## v1 vs v2 on one `Core`

Nad.fun ships two generations of contracts. Starting in 0.4.0 a single
`Core` instance wires both:

| | v1 (legacy) | v2 (current) |
|---|---|---|
| Method names | `core.v1().buy` / `core.v1().sell` / `core.v1().get_amount_out` / `core.v1().create_token` | `core.v2().buy` / `core.v2().sell` / `core.v2().get_amount_out` / `core.v2().create_token` |
| Routers | `BondingCurveRouter` + `DexRouter` (Capricorn CL) | unified `NadFunRouter` |
| Quote tokens | MON only | MON + arbitrary ERC-20 |
| Vaults | n/a | Burn / LP / CreatorFee / Gift |
| Streaming | `CurveStream` / `DexStream` | `CurveStreamV2` / `NadFunSwapStream` |

Dispatch a v1 vs v2 path with the on-chain probe. The SDK does **not**
auto-route between generations — you pick the method by version:

```rust
// `detect_version` does one `TokenInfoLens` call and returns V1 / V2 / None.
match core.detect_version(token).await? {
    SdkVersion::V1 => {
        // v1 quote returns (router, expected); buy takes that router.
        let (router, expected) = core.v1().get_amount_out(token, mon_amount, true).await?;
        let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
        core.v1().buy(/* BuyParams { .. } */, router).await?;
    }
    SdkVersion::V2 => {
        // v2 quote is quote-agnostic; the trade method funds the buy.
        let expected = core.v2().get_amount_out(token, mon_amount, true).await?;
        let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);
        core.v2().buy_with_native(/* V2BuyWithNativeParams { .. } */).await?;
    }
    SdkVersion::None => { /* not a Nad.fun token — refuse */ }
}
```

Need the token's quote token alongside the version (to choose
`core.v2().buy_with_native` vs `core.v2().buy` for an ERC-20-quoted v2 token)? Use
`core.detect_token_info(token).await?` → `TokenInfo { version, quote_token }`.
The complete runnable dispatcher (including the v2 native-vs-ERC-20 quote
routing) is [`examples/unified_dispatch.rs`](examples/unified_dispatch.rs).

For UI / agent scenarios where the token comes from outside,
`api.get_token(token).await?.version` is the equivalent API-side answer.

The same version split applies to event streaming — pick the v1 or v2
stream/indexer by token version (see [Real-time Event Streaming](#-real-time-event-streaming)):

| | v1 | v2 |
|---|---|---|
| Curve stream | `CurveStream` | `CurveStreamV2` |
| Curve indexer | `CurveIndexer` | `CurveIndexerV2` |
| Swap stream | `DexStream` (Capricorn CL) | `NadFunSwapStream` (NadFunPair) |
| Swap indexer | `DexIndexer` | `NadFunSwapIndexer` |
| Pool discovery | `DexIndexer::discover_pools_for_tokens` | `stream::v2::discover_pools_unified` (both surfaces) |

### v2 Quick Start

```rust
use nadfun_sdk::*;
use alloy::primitives::{utils::parse_ether, Address, U256};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
    let token: Address = "0x...".parse()?;
    let mon_amount = parse_ether("0.1")?;

    // Auto-routed quote: bonding curve pre-graduation, NadFunPair post.
    let expected = core.v2().get_amount_out(token, mon_amount, true).await?;
    let min_out = SlippageUtils::calculate_amount_out_min(expected, 5.0);

    let tx = core.v2().buy_with_native(V2BuyWithNativeParams {
        token,
        to: core.wallet_address(),
        amount_out_min: min_out,
        deadline: U256::from(9_999_999_999u64),
        value: mon_amount,
        gas_limit: None,
        gas_price: None,
        nonce: None,
    }).await?;
    println!("tx: {tx}");
    Ok(())
}
```

End-to-end v2 token creation, exact-output, ERC-20 quote, permits, pool
discovery, and event streaming are covered under
[`examples/v2/`](examples/v2/).

#### v2 view / query parity

Thin `core.v2()` wrappers over the on-chain v2 views, mirroring the v1 query
surface:

```rust,ignore
// State / config
core.v2().is_halted().await?;                 // bool: protocol halted?
core.v2().is_registered(token).await?;        // bool: in v2 TokenRegistry?
core.v2().get_dex_type(token).await?;         // u8: DexType discriminator
core.v2().quote_token(token).await?;          // Address: MON(WMON) / LVMON / ERC-20
core.v2().get_sniping_penalty(token).await?;  // U256: anti-sniping bps now
core.v2().get_curve(token).await?;            // V2Curve: full bonding-curve state
core.v2().quote_config(quote_token).await?;   // V2QuoteConfig: genesis params + fees

// Post-graduation pair views — Err before graduation (gated on is_graduated):
core.v2().is_locked(token).await?;            // bool: DEX-pair lock (NOT the v1 curve lock)
core.v2().get_reserves(token).await?;         // PairReserves of the graduated pair

// Computed helpers (v1 Lens parity; no v2 on-chain fn — math from curve/config):
core.v2().get_progress(token).await?;         // U256: curve progress in bps (0..=10000)
core.v2().available_buy_tokens(token).await?; // (U256, U256): (tokens left, quote needed)
core.v2().get_initial_buy_amount_out(quote_token, amount_in, creator_fee_rate).await?; // U256
```

`get_initial_buy_amount_out` takes a `quote_token` (v2 genesis curves differ per
quote token) and the token's `creator_fee_rate` (u16 bps, a per-token parameter
not present in the genesis config); v1's equivalent is parameterless. It returns
the **exact** create-time initial-buy output — the on-chain `_initialBuy` mints
this to the wei (combined protocol + creator fee, then constant-product + supply
cap; anti-sniping exempt). The computed helpers reproduce the on-chain
bonding-curve math, verified against on-chain quotes in `tests/v2_views_live.rs`.

`is_locked` reflects the post-graduation `NadFunPair` lock, distinct from the v1
bonding-curve `core.v1().is_locked`. New view types `V2Curve`, `V2QuoteConfig`,
and `PairReserves` are re-exported from the crate root and `prelude`.

## Features

### 🔑 API Authentication

The SDK uses optional API key authentication for higher rate limits:

```rust
use nadfun_sdk::{ApiClient, Network};

// Option 1: Without API key (lower rate limit, but works)
let api = ApiClient::new(Network::Mainnet);

// Option 2: With API key (higher rate limit)
let api = ApiClient::new(Network::Mainnet).with_api_key("nadfun_xxxxx".to_string());

// Option 3: From environment variable (recommended)
// Set NAD_API_KEY in .env or shell
let api = ApiClient::from_env(Network::Mainnet);
```

#### Environment Variable Setup

```bash
# .env file
NAD_API_KEY=nadfun_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx

# Or export in shell
export NAD_API_KEY=nadfun_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

#### Rate Limits

| Request Origin | API Key | Rate Limit |
|----------------|---------|------------|
| External (no key) | ❌ | 10 req/min |
| External (with key) | ✅ | 100 req/min |
| nad.fun, nadapp.net | - | Unlimited |

#### Getting an API Key

1. **Login**: Visit [nad.fun](https://nad.fun) and connect your wallet
2. **Navigate**: Go to Settings → API Keys
3. **Create**: Click "Generate API Key"

```bash
# Or via API (requires session cookie from login)
curl -X POST https://api.nadapp.net/api-key \
  -H "Content-Type: application/json" \
  -H "Cookie: nadfun-v3-api=<your_session>" \
  -d '{
    "name": "My SDK Integration",
    "description": "Rust SDK for trading bot",
    "expires_in_days": 365
  }'
```

**Response:**
```json
{
  "id": 7185139933124608001,
  "api_key": "nadfun_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "key_prefix": "nadfun_xxxxxxxx",
  "name": "My SDK Integration"
}
```

> ⚠️ **Important**: The `api_key` is shown only once! Store it securely.

#### API Key Management

```bash
# List your API keys
curl https://api.nadapp.net/api-key -H "Cookie: nadfun-v3-api=<your_session>"

# Delete an API key
curl -X DELETE https://api.nadapp.net/api-key/<id> -H "Cookie: nadfun-v3-api=<your_session>"
```

#### Limits & Security

- **Max 5 keys** per account
- Keys can have **expiration dates** (or unlimited)
- **Revoke immediately** if compromised
- Never commit API keys to git - use `.env` files

### 🎨 Token Creation

Launch tokens on Nad.fun bonding curve with full metadata, image upload, and initial buy in a single transaction.

#### Complete Flow

```rust
use nadfun_sdk::{ActionId, ApiClient, Core, CreateTokenParams, Network};
use alloy::primitives::utils::parse_ether;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize clients
    let core = Core::new(
        "https://rpc.monad.xyz".to_string(),
        std::env::var("PRIVATE_KEY")?,
        Network::Mainnet,
    ).await?;

    let api = ApiClient::from_env(Network::Mainnet); // Reads NAD_API_KEY from env

    // 2. Calculate token output for initial buy
    let initial_buy = parse_ether("1.5")?; // 1.5 MON
    let tokens_out = core.v1().get_initial_buy_amount_out(initial_buy).await?;

    // 3. Configure token parameters
    let params = CreateTokenParams {
        name: "Rocket Pepe".to_string(),
        symbol: "RPEPE".to_string(),
        description: "The fastest pepe in the galaxy 🚀".to_string(),
        image_uri: "https://i.imgur.com/your-image.png".to_string(),
        website: Some("https://rocketpepe.xyz".to_string()),
        twitter: Some("https://x.com/rocketpepe".to_string()),
        telegram: Some("https://t.me/rocketpepe".to_string()),
        creator_address: core.wallet_address(),
        amount_out: tokens_out,
        value: initial_buy,
        action_id: ActionId::CapricornActor,
    };

    // 4. Create token (uploads image, creates metadata, deploys contract)
    let result = core.v1().create_token(params, &api).await?;

    println!("✅ Token deployed: {}", result.token_address);
    println!("📄 Metadata: {}", result.metadata_uri);
    println!("🔞 NSFW: {}", result.is_nsfw);

    Ok(())
}
```

#### What Happens Under the Hood

1. **Image Upload** → Downloads from URI, validates format, uploads to IPFS
2. **Metadata Creation** → Creates JSON metadata with all token info
3. **Salt Mining** → Generates vanity address for your token
4. **Contract Deploy** → Deploys token + initial buy in one transaction

#### Supported Image Formats

| Format | MIME Type |
|--------|-----------|
| JPEG | `image/jpeg` |
| PNG | `image/png` |
| WebP | `image/webp` |
| SVG | `image/svg+xml` |

#### Actor Types

| ActionId | Description |
|----------|-------------|
| `CapricornActor` | Standard token launch (default) |
| `AmplifyActor` | Amplified marketing features |

### 💰 Creator Rewards

Claim trading fee rewards for tokens you created:

```rust
use nadfun_sdk::{ApiClient, Core, Network};

// Initialize
let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
let api = ApiClient::new(Network::Mainnet);

// Get created tokens with reward info
let response = api.get_created_tokens(core.wallet_address(), 1, 10).await?;
println!("Found {} tokens", response.total_count);

// Claim rewards for each claimable token
for token in &response.tokens {
    if let Some(params) = ApiClient::build_claim_params(token) {
        println!("Claiming {} wei from {}", token.reward_info.amount, token.token_info.name);
        let tx_hash = core.v1().claim_creator_reward(params).await?;
        println!("TX: {}", tx_hash);
    }
}

// Or batch claim all at once (more gas efficient)
if let Some(batch_params) = ApiClient::build_batch_claim_params(&response.tokens) {
    let tx_hash = core.v1().claim_creator_rewards_batch(batch_params).await?;
    println!("Batch claim TX: {}", tx_hash);
}
```

**Features:**
- 📊 Query claimable rewards via API
- 🧾 Merkle proof-based claiming
- 📦 Batch claim for gas efficiency
- 💸 Automatic wMON → MON conversion

### 🚀 Trading

Execute buy/sell operations on bonding curves with slippage protection:

> v1 trading shown here. For v2 (`core.v2().buy` / `core.v2().buy_with_native` /
> `core.v2().sell_to_native` / `core.v2().get_amount_out`) see [v2 Quick Start](#v2-quick-start)
> and [`examples/v2/`](examples/v2/). Choose the path by token version
> (`core.detect_version(token)`), as shown above.

```rust
use nadfun_sdk::{Core, SlippageUtils, GasEstimationParams};
use nadfun_sdk::types::{BuyParams, GasPricing};

// Get quote and execute buy (v1)
let (router, expected_tokens) = core.v1().get_amount_out(token, mon_amount, true).await?;
let min_tokens = SlippageUtils::calculate_amount_out_min(expected_tokens, 5.0);

// Use new unified gas estimation system
let gas_params = GasEstimationParams::Buy {
    token,
    amount_in: mon_amount,
    amount_out_min: min_tokens,
    to: wallet_address,
    deadline: U256::from(deadline),
};

// Get accurate gas estimation from network
let estimated_gas = core.v1().estimate_gas(&router, gas_params).await?;
let gas_with_buffer = estimated_gas * 120 / 100; // Add 20% buffer

let buy_params = BuyParams {
    token,
    amount_in: mon_amount,
    amount_out_min: min_tokens,
    to: wallet_address,
    deadline: U256::from(deadline),
    gas_limit: Some(gas_with_buffer), // Use network-based estimation
    gas_price: Some(GasPricing::LegacyWithPrice { gas_price: 50_000_000_000 }), // 50 gwei
    nonce: None,                       // Auto-detect
};

// Execute buy - returns tx_hash immediately (fast!)
let tx_hash = core.v1().buy(buy_params, router).await?;
println!("Transaction submitted: {}", tx_hash);

// Later, check the transaction status if needed
let receipt = core.get_receipt(tx_hash).await?;
if receipt.status {
    println!("Trade successful! Gas used: {:?}", receipt.gas_used);
}
```

#### Fast Transaction Submission

**New in v0.3.0**: All trading functions now return transaction hash immediately without waiting for confirmation. This makes your trading bot much faster!

```rust
// OLD - Waits for confirmation (slow)
let result = core.v1().buy(params, router).await?;  // Waits ~2-15 seconds

// NEW - Returns immediately (fast!)
let tx_hash = core.v1().buy(params, router).await?;  // Returns in milliseconds
println!("Submitted: {}", tx_hash);

// Check status later when you need it
let receipt = core.get_receipt(tx_hash).await?;
println!("Confirmed: {}", receipt.status);
```

### ⛽ Gas Management

**v0.2.0 introduces a unified gas estimation system** that replaces static constants with real-time network estimation.

**v0.3.0 adds EIP-1559 gas pricing support** for better transaction fee control:

#### Gas Pricing Options (New in v0.3.1)

```rust
use nadfun_sdk::types::GasPricing;

// Option 1: Legacy (default) - uses network gas price
let gas_price = Some(GasPricing::Legacy);

// Option 2: Legacy with explicit gas price
let gas_price = Some(GasPricing::LegacyWithPrice {
    gas_price: 50_000_000_000, // 50 gwei
});

// Option 3: EIP-1559 (recommended for Monad)
let gas_price = Some(GasPricing::Eip1559 {
    max_fee_per_gas: 100_000_000_000,        // 100 gwei max
    max_priority_fee_per_gas: 2_000_000_000, // 2 gwei tip
});

// Use in BuyParams/SellParams
let buy_params = BuyParams {
    // ... other fields
    gas_price,  // Unified gas pricing field
    nonce: None,
};
```

#### Unified Gas Estimation (New in v0.2.0)

```rust
use nadfun_sdk::{Core, GasEstimationParams};

// Create gas estimation parameters for any operation
let gas_params = GasEstimationParams::Buy {
    token,
    amount_in: mon_amount,
    amount_out_min: min_tokens,
    to: wallet_address,
    deadline: U256::from(deadline),
};

// Get real-time gas estimation from network
let estimated_gas = core.v1().estimate_gas(&router, gas_params).await?;

// Apply buffer strategy
let gas_with_buffer = estimated_gas * 120 / 100; // 20% buffer
```

#### Gas Estimation Parameters

```rust
pub enum GasEstimationParams {
    // For buying tokens
    Buy { token, amount_in, amount_out_min, to, deadline },

    // For selling tokens (requires token approval)
    Sell { token, amount_in, amount_out_min, to, deadline },

    // For gasless selling with permits
    SellPermit { token, amount_in, amount_out_min, to, deadline, v, r, s },
}
```

#### Automatic Problem Solving

The new system automatically handles common issues:

- **Token Approval**: SELL operations automatically check and approve tokens
- **Permit Signatures**: SELL PERMIT operations generate real EIP-2612 signatures
- **Network Conditions**: Uses actual network state instead of static estimates
- **Error Recovery**: Graceful fallback when estimation fails

#### Buffer Strategies

```rust
// Fixed buffer amounts
let gas_fixed_buffer = estimated_gas + 50_000;  // +50k gas

// Percentage-based buffers
let gas_20_percent = estimated_gas * 120 / 100; // 20% buffer
let gas_25_percent = estimated_gas * 125 / 100; // 25% buffer (for complex operations)

// Choose based on operation complexity
let final_gas = match operation_type {
    "buy" => estimated_gas * 120 / 100,        // 20% buffer
    "sell" => estimated_gas * 115 / 100,       // 15% buffer
    "sell_permit" => estimated_gas * 125 / 100, // 25% buffer
    _ => estimated_gas + 50_000,               // Fixed buffer
};
```

#### Migration from v0.1.x

```rust
// OLD (v0.1.x) - Static constants
use nadfun_sdk::{BondingCurveGas, get_default_gas_limit, Operation};
let gas_limit = get_default_gas_limit(&router, Operation::Buy);

// NEW (v0.2.0) - Network-based estimation
use nadfun_sdk::GasEstimationParams;
let params = GasEstimationParams::Buy { token, amount_in, amount_out_min, to, deadline };
let estimated_gas = core.v1().estimate_gas(&router, params).await?;
let gas_limit = estimated_gas * 120 / 100; // Apply buffer
```

**⚠️ Important Notes:**

- **SELL Operations**: Require token approval for router (automatically handled in examples)
- **SELL PERMIT Operations**: Need valid EIP-2612 permit signatures (automatically generated)
- **Network Connection**: Live RPC required for accurate estimation

### 📊 Token Operations

Interact with ERC-20 tokens and get metadata:

```rust
use nadfun_sdk::TokenHelper;

let token_helper = TokenHelper::new(rpc_url, private_key).await?;

// Get token metadata
let metadata = token_helper.get_token_metadata(token).await?;
println!("Token: {} ({})", metadata.name, metadata.symbol);

// Check balances and allowances
let balance = token_helper.balance_of(token, wallet).await?;
let allowance = token_helper.allowance(token, owner, spender).await?;

// Approve tokens
let tx = token_helper.approve(token, spender, amount).await?;
```

### 🔄 Real-time Event Streaming

Monitor bonding curve and DEX events in real-time. Every stream takes the
`Network` it is bound to. **Pick the v1 or v2 stream by token version** —
the SDK does not multiplex generations for you.

#### Bonding Curve Streaming (v1)

```rust
use nadfun_sdk::stream::CurveStream;
use nadfun_sdk::types::{BondingCurveEvent, EventType};
use nadfun_sdk::Network;
use futures_util::{pin_mut, StreamExt};

// Create WebSocket stream (network is required)
let curve_stream = CurveStream::new("wss://your-ws-endpoint".to_string(), Network::Mainnet).await?;

// Configure filters (optional)
let curve_stream = curve_stream
    .subscribe_events(vec![EventType::Buy, EventType::Sell])
    .filter_tokens(vec![token_address]);

// Subscribe and process events
let stream = curve_stream.subscribe().await?;
pin_mut!(stream);

while let Some(event_result) = stream.next().await {
    match event_result {
        Ok(event) => {
            println!("Event: {:?} for token {}", event.event_type(), event.token());
        }
        Err(e) => println!("Error: {}", e),
    }
}
```

#### Bonding Curve Streaming (v2)

For v2 tokens, use `CurveStreamV2` with `V2EventType` filters:

```rust
use nadfun_sdk::stream::v2::CurveStreamV2;
use nadfun_sdk::{Network, V2EventType};
use futures_util::{pin_mut, StreamExt};

let stream = CurveStreamV2::new("wss://your-ws-endpoint".to_string(), Network::Mainnet)
    .await?
    // V2EventType: Create, Buy, Sell, Sync, Graduate, SnipingPenalty
    .subscribe_events(vec![V2EventType::Buy, V2EventType::Sell])
    .filter_tokens(vec![token_address]);

let s = stream.subscribe().await?;
pin_mut!(s);
while let Some(item) = s.next().await {
    if let Ok(event) = item {
        println!("v2 {:?} for token {}", event.event_type(), event.token());
    }
}
```

#### DEX Swap Streaming (v1 — Capricorn CL)

```rust
use nadfun_sdk::stream::DexStream;
use nadfun_sdk::Network;
use futures_util::{pin_mut, StreamExt};

// Auto-discover pools for tokens (network is required)
let swap_stream = DexStream::discover_pools_for_tokens(
    "wss://your-ws-endpoint".to_string(),
    vec![token_address],
    Network::Mainnet,
).await?;
// Or monitor explicit pools: DexStream::new(ws_url, pool_addresses, network)

// Subscribe and process events
let stream = swap_stream.subscribe().await?;
pin_mut!(stream);

while let Some(event_result) = stream.next().await {
    match event_result {
        Ok(event) => {
            // SwapEvent fields are flat — `event.pool_address`, not `event.pool_metadata.*`.
            println!("Swap in pool {}: {} -> {}",
                event.pool_address, event.amount0, event.amount1);
        }
        Err(e) => println!("Error: {}", e),
    }
}
```

#### DEX Swap Streaming (v2 — NadFunPair)

For v2 tokens, swaps come from `NadFunPair` contracts via `NadFunSwapStream`.
Resolve pair addresses first (`core.v2().pool_address(token)`), then stream:

```rust
use nadfun_sdk::stream::v2::NadFunSwapStream;
use nadfun_sdk::Network;
use futures_util::{pin_mut, StreamExt};

// `pairs` are NadFunPair addresses (resolve via core.v2().pool_address(token)).
let stream = NadFunSwapStream::new("wss://your-ws-endpoint".to_string(), pairs, Network::Mainnet).await?;
let s = stream.subscribe().await?;
pin_mut!(s);
while let Some(item) = s.next().await {
    if let Ok(swap) = item {
        // NadFunPair Swap (Uniswap-V2 shape): amount{0,1}_{in,out}, `to`, `pair_address`.
        println!("swap pair={} 0in={} 1out={}", swap.pair_address, swap.amount0_in, swap.amount1_out);
    }
}
```

> ⚠️ **Empty `pairs` = no address filter.** `NadFunSwapStream::new` (and
> `NadFunSwapIndexer::new`) with an empty `pairs` vec subscribes to *every*
> Swap-signature log on chain. That signature is the standard Uniswap-V2 `Swap`, so
> results can include unrelated non-NadFun V2-fork contracts — `pair_address`
> is the emitting contract, not a verified NadFun pair. Pass explicit pairs (and
> bound the block range / result size) to scope and trust the stream.

### 📈 Historical Data Analysis

Fetch and analyze historical events. The indexer takes the `Network` it
targets; choose v1 (`CurveIndexer`) or v2 (`CurveIndexerV2`) by token version.

```rust
use nadfun_sdk::stream::{CurveIndexer, EventType};
use nadfun_sdk::Network;

let provider = Arc::new(ProviderBuilder::new().connect_http(http_url.parse()?));
let indexer = CurveIndexer::new(provider, Network::Mainnet);

// Fetch events from block range
let events = indexer.fetch_events(
    18_000_000,
    18_010_000,
    vec![EventType::Create, EventType::Buy],
    None // all tokens
).await?;

println!("Found {} events", events.len());

// v2 equivalent — same shape, V2EventType filters:
// let v2_indexer = nadfun_sdk::stream::v2::CurveIndexerV2::new(provider, Network::Mainnet);
```

### 🔍 Pool Discovery

Find Capricorn CL (v1) pool addresses for tokens. Discovery takes the RPC
URL plus the target `Network` (not a pre-built provider):

```rust
use nadfun_sdk::stream::DexIndexer;
use nadfun_sdk::Network;

// Auto-discover pools for multiple tokens
let indexer = DexIndexer::discover_pools_for_tokens(rpc_url.clone(), tokens, Network::Mainnet).await?;
let pools = indexer.pool_addresses();

// Discover pool for a single token
let indexer = DexIndexer::discover_pool_for_token(rpc_url, token, Network::Mainnet).await?;
```

For a **version-agnostic** sweep that resolves pools across *both* v1
(Capricorn CL) and v2 (NadFunPair) surfaces in one call, use
`stream::v2::discover_pools_unified`:

```rust
use nadfun_sdk::stream::v2::discover_pools_unified;

// Returns Vec<PoolLocation { token, pool, surface }>, surface = Capricorn | NadFun.
let pools = discover_pools_unified(provider, tokens, Network::Mainnet).await?;
for p in pools {
    println!("token={} pool={} surface={:?}", p.token, p.pool, p.surface);
}
```

### 💱 DEX Monitoring

Monitor Capricorn CL (v1) swap events historically:

```rust
use nadfun_sdk::stream::DexIndexer;
use nadfun_sdk::Network;

// Auto-discover pools for tokens
let indexer = DexIndexer::discover_pools_for_tokens(rpc_url, tokens, Network::Mainnet).await?;
let swaps = indexer.fetch_events(from_block, to_block).await?;

for swap in swaps {
    // SwapEvent fields are flat — use `swap.pool_address`.
    println!("Swap in pool {}: {} -> {}",
        swap.pool_address,
        swap.amount0,
        swap.amount1
    );
}
```

For v2 (NadFunPair) swap history, use `NadFunSwapIndexer::new(provider, pairs, network)`
(same `fetch_events(from, to)` / `fetch_all_events(start, batch)` interface;
mind the empty-`pairs` caveat above).

## Examples

The SDK includes comprehensive examples in the `examples/` directory:

### Token Creation Examples

```bash
# Create a new token
cargo run --example create_token -- \
  --private-key your_private_key \
  --rpc-url https://your-rpc-url \
  --network mainnet \
  --name "My Token" \
  --symbol "MTK" \
  --description "My awesome token" \
  --image-uri "https://i.imgur.com/yourimage.png" \
  --initial-buy "1.5"
```

### Trading Examples

```bash
# Using environment variables
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export TOKEN="0xTokenAddress"
export RECIPIENT="0xRecipientAddress"  # For token operations

cargo run --example buy              # Buy tokens with network-based gas estimation
cargo run --example sell             # Sell tokens with automatic approval handling
cargo run --example sell_permit      # Gasless sell with real permit signatures
cargo run --example gas_estimation   # Comprehensive gas estimation example
cargo run --example basic_operations # Token operations (requires recipient)

# Using command line arguments
cargo run --example buy -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
cargo run --example sell -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
cargo run --example sell_permit -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
cargo run --example gas_estimation -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
cargo run --example basic_operations -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress --recipient 0xRecipientAddress
```

### Gas Estimation Example (New in v0.2.0)

```bash
# Comprehensive gas estimation with automatic problem solving
cargo run --example gas_estimation -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
```

**Features:**

- **Unified Gas Estimation**: Demonstrates `core.v1().estimate_gas()` for all operation types
- **Automatic Approval**: Handles token approval for SELL operations automatically
- **Real Permit Signatures**: Generates valid EIP-2612 signatures for SELL PERMIT operations
- **Buffer Strategies**: Shows different buffer calculation methods (fixed +50k, percentage 20%-25%)
- **Cost Analysis**: Real-time transaction cost estimates at different gas prices
- **Error Handling**: Graceful fallback when estimation fails

### Token Examples

```bash
cargo run --example basic_operations # Basic ERC-20 operations
cargo run --example permit_signature # EIP-2612 permit signatures
```

### Stream Examples

The SDK provides 5 comprehensive streaming examples organized by category:

#### 🔄 Bonding Curve Examples

**1. curve_indexer** - Historical bonding curve event analysis

```bash
# Fetch historical Create, Buy, Sell events
cargo run --example curve_indexer -- --rpc-url https://your-rpc-endpoint

# With specific tokens and time range
cargo run --example curve_indexer -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens 0xToken1,0xToken2
```

**2. curve_stream** - Real-time bonding curve monitoring

```bash
# Scenario 1: Monitor all bonding curve events
cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint

# Scenario 2: Filter specific event types (Buy/Sell only)
EVENTS=Buy,Sell cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint

# Scenario 3: Filter specific tokens only
cargo run --example curve_stream -- \
  --ws-url wss://your-ws-endpoint \
  --tokens 0xToken1,0xToken2

# Scenario 4: Combined filtering (events AND tokens)
EVENTS=Buy,Sell cargo run --example curve_stream -- \
  --ws-url wss://your-ws-endpoint \
  --tokens 0xToken1
```

**Features:**

- ✅ All event types: Create, Buy, Sell, Sync, Lock, Graduate
- ✅ Event type filtering via `EVENTS` environment variable
- ✅ Token filtering via `--tokens` argument
- ✅ Combined filtering (events + tokens)
- ✅ Real-time WebSocket streaming
- ✅ Automatic event decoding

#### 💱 DEX Examples

**3. dex_indexer** - Historical DEX swap data analysis

```bash
# Discover pools and fetch historical swap events
cargo run --example dex_indexer -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens 0xToken1,0xToken2

# Batch process with JSON array format
cargo run --example dex_indexer -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens '["0xToken1","0xToken2"]'
```

**4. dex_stream** - Real-time DEX swap monitoring

```bash
# Scenario 1: Monitor specific pool addresses directly
POOLS=0xPool1,0xPool2 cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint

# Scenario 2: Auto-discover pools for multiple tokens
cargo run --example dex_stream -- \
  --ws-url wss://your-ws-endpoint \
  --tokens 0xToken1,0xToken2

# Scenario 3: Single token pool discovery
cargo run --example dex_stream -- \
  --ws-url wss://your-ws-endpoint \
  --token 0xTokenAddress
```

**Features:**

- ✅ Automatic pool discovery for tokens
- ✅ Direct pool address monitoring
- ✅ Single token pool discovery
- ✅ Real-time Capricorn CL swap events
- ✅ Pool metadata included
- ✅ WebSocket streaming

#### 🔍 Pool Discovery

**5. pool_discovery** - Automated pool address discovery

```bash
# Find Capricorn CL pools for multiple tokens
cargo run --example pool_discovery -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens 0xToken1,0xToken2

# Discover pools for single token
cargo run --example pool_discovery -- \
  --rpc-url https://your-rpc-endpoint \
  --token 0xTokenAddress
```

### v2 Examples

v2 example targets use a `v2_` prefix. Trading, creation, and streaming all
go through the unified `Core` (`core.v2().*` handle methods) or the `stream::v2` module:

```bash
# Mixed-token dispatch: routes buy through v1 or v2 by detected version
cargo run --example unified_dispatch -- --private-key your_private_key_here --token 0xToken

# v2 trading (native MON in / out, ERC-20 quote, exact-output)
cargo run --example v2_buy             -- --private-key your_private_key_here --token 0xToken
cargo run --example v2_sell            -- --private-key your_private_key_here --token 0xToken
cargo run --example v2_buy_erc20_quote -- --private-key your_private_key_here --token 0xToken
cargo run --example v2_exact_out       -- --private-key your_private_key_here --token 0xToken

# v2 token creation (NadFunRouter + vault split)
cargo run --example v2_create_token    -- --private-key your_private_key_here

# v2 streaming + discovery
cargo run --example v2_curve_stream    -- --ws-url wss://your-ws-endpoint
cargo run --example v2_dex_stream      -- --rpc-url https://your-rpc-endpoint --ws-url wss://your-ws-endpoint --tokens 0xToken
cargo run --example v2_pool_discovery  -- --rpc-url https://your-rpc-endpoint --tokens 0xToken
```

### Testing & Verification

All examples have been tested and verified working. Here are ready-to-run test commands:

#### 🔄 Real-time Streaming Tests

```bash
# Test bonding curve streaming (all events)
cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint

# Test DEX swap streaming (auto-discover pools)
cargo run --example dex_stream -- \
  --ws-url wss://your-ws-endpoint \
  --tokens 0xYourTokenAddress

# Test with event filtering
EVENTS=Buy,Sell cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint

# Test with specific pool monitoring
POOLS=0xPool1,0xPool2 cargo run --example dex_stream -- --ws-url wss://your-ws-endpoint
```

#### 📊 Historical Data Tests

```bash
# Test bonding curve historical analysis
cargo run --example curve_indexer -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens 0xYourTokenAddress

# Test pool discovery
cargo run --example pool_discovery -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens 0xToken1,0xToken2

# Test DEX historical analysis
cargo run --example dex_indexer -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens 0xYourTokenAddress
```

#### ⚡ Quick Validation

```bash
# Minimal test - just connect and verify
cargo run --example curve_stream -- --ws-url wss://your-ws-endpoint
# Should output: "Listening for ALL bonding curve events..."

cargo run --example dex_stream -- --token 0xTokenAddress --ws-url wss://your-ws-endpoint
# Should output: "Discovered X pools for 1 tokens"
```

## Core Types

### Event Types (v1)

- `BondingCurveEvent`: Unified enum for all bonding curve events
  - `Create`, `Buy`, `Sell`, `Sync`, `Lock`, `Graduate` variants
  - Methods: `.token()`, `.event_type()`, `.block_number()`, `.transaction_index()`
- `SwapEvent`: Capricorn CL swap events. Fields are flat (no `pool_metadata`):
  - `sender`, `recipient`, `amount0: I256`, `amount1: I256`, `sqrt_price_x96: U256`,
    `liquidity`, `tick`, `pool_address`, `block_number`, `transaction_hash`,
    `transaction_index`, `log_index`
- `EventType`: Enum for filtering bonding curve events
  - Variants: `Create`, `Buy`, `Sell`, `Sync`, `Lock`, `Graduate`

### Event Types (v2)

- `V2BondingCurveEvent` / `V2EventType`: v2 bonding curve events and filter enum
  - `V2EventType` variants: `Create`, `Buy`, `Sell`, `Sync`, `Graduate`, `SnipingPenalty`
- `NadFunSwapEvent`: NadFunPair (Uniswap-V2-shaped) swap events
  - `sender`, `to`, `amount0_in`, `amount1_in`, `amount0_out`, `amount1_out`,
    `pair_address`, `block_number`, `transaction_hash`, `transaction_index`, `log_index`

### Stream Types

v1 (re-exported at `nadfun_sdk::stream::*`):

- `CurveStream`: bonding curve streaming — `CurveStream::new(ws_url, network)`,
  `.subscribe_events()`, `.filter_tokens()`, `.subscribe()` →
  `Stream<Item = Result<BondingCurveEvent>>`
- `CurveIndexer`: bonding curve history — `CurveIndexer::new(provider, network)`,
  `.fetch_events(from, to, events, tokens)`, `.fetch_all_events(start, batch, events, tokens)`
- `DexStream`: Capricorn CL swap streaming — `DexStream::new(ws_url, pools, network)`,
  `::discover_pools_for_tokens(ws_url, tokens, network)`,
  `::discover_pool_for_token(ws_url, token, network)` → `Stream<Item = Result<SwapEvent>>`
- `DexIndexer`: Capricorn CL swap history — `::discover_pools_for_tokens(rpc_url, tokens, network)`,
  `::discover_pool_for_token(rpc_url, token, network)`, `.fetch_events(from, to)`,
  `.fetch_all_events(start, batch)`, `.pool_addresses()`

v2 (under `nadfun_sdk::stream::v2`):

- `CurveStreamV2`: `CurveStreamV2::new(ws_url, network)` → `Result<CurveStreamV2>`; `.subscribe().await?` → `Pin<Box<dyn Stream<Item = Result<V2BondingCurveEvent>> + Send>>`
- `CurveIndexerV2`: `CurveIndexerV2::new(provider, network)`
- `NadFunSwapStream`: `NadFunSwapStream::new(ws_url, pairs, network)` → `Result<NadFunSwapStream>`; `.subscribe().await?` → `Pin<Box<dyn Stream<Item = Result<NadFunSwapEvent>> + Send>>`
- `NadFunSwapIndexer`: `NadFunSwapIndexer::new(provider, pairs, network)`
- `discover_pools_unified(provider, tokens, network)` → `Vec<PoolLocation { token, pool, surface }>`
  (`PoolSurface::{Capricorn, NadFun}`) — resolves both v1 and v2 surfaces

> Empty `pairs` on `NadFunSwapStream` / `NadFunSwapIndexer` = no address filter
> (every `NadFunPair::Swap`, including unrelated V2-fork contracts). Scope with
> explicit pairs and a bounded block range.

### Trading Types

- `BuyParams` / `SellParams`: Parameters for buy/sell operations
- `TransactionResult`: Transaction result with status and metadata
- `SlippageUtils`: Utilities for slippage calculations

### Token Types

- `TokenMetadata`: Name, symbol, decimals, total supply
- EIP-2612 permit signatures are produced by `TokenHelper::generate_permit_signature`, which returns a `(u8, B256, B256)` `(v, r, s)` tuple (no dedicated public type)

## Configuration

### Environment Variables

```bash
export RPC_URL="https://your-rpc-endpoint"
export PRIVATE_KEY="your_private_key_here"
export WS_URL="wss://your-ws-endpoint"
export TOKEN="0xTokenAddress"
export TOKENS="0xToken1,0xToken2"  # Multiple tokens for monitoring
export RECIPIENT="0xRecipientAddress"
```

### CLI Arguments

All examples support command line arguments for configuration:

```bash
# Available options
--rpc-url <URL>      # RPC URL (default: https://eth.merkle.io)
--ws-url <URL>       # WebSocket URL (default: wss://eth.merkle.io)
--private-key <KEY>  # Private key for transactions
--token <ADDRESS>    # Token address for operations
--tokens <ADDRS>     # Token addresses: 'addr1,addr2' or '["addr1","addr2"]'
--recipient <ADDR>   # Recipient address for transfers/allowances
--help, -h           # Show help

# Example usage
cargo run --example sell_permit -- \
  --rpc-url https://your-rpc-endpoint \
  --private-key your_private_key_here \
  --token 0xYourTokenAddress

# Example with recipient (for token operations)
cargo run --example basic_operations -- \
  --private-key your_private_key_here \
  --rpc-url https://your-rpc-endpoint \
  --token 0xYourTokenAddress \
  --recipient 0xRecipientAddress

# Example with multiple tokens for monitoring
cargo run --example dex_indexer -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens 0xToken1,0xToken2,0xToken3

# Example with JSON array format
cargo run --example pool_discovery -- \
  --rpc-url https://your-rpc-endpoint \
  --tokens '["0xToken1","0xToken2"]'
```

### Contract Addresses

All contract addresses are defined in `constants.rs` (the source of truth),
organized by network and version (`addresses::{mainnet,testnet}::{v1,v2}`).
Access them via the typed helpers — `get_bonding_curve(network)`,
`get_nadfun_router_v2(network)` (returns `Option`), etc. The v2 set has many
more contracts (vaults, registry, bonding curve, lens); the most useful are
listed below.

#### Mainnet — v1

- DEX Factory: `0x6B5F564339DbAD6b780249827f2198a841FEB7F3`
- WMON Token: `0x3bd359C1119dA7Da1D913D1C4D2B7c461115433A`
- Bonding Curve: `0xA7283d07812a02AFB7C09B60f8896bCEA3F90aCE`
- Bonding Curve Router: `0x6F6B8F1a20703309951a5127c45B49b1CD981A22`
- DEX Router: `0x0B79d71AE99528D1dB24A4148b5f4F865cc2b137`
- Lens: `0x7e78A8DE94f21804F7a17F4E8BF9EC2c872187ea`

#### Mainnet — v2

- NadFun Router: `0x8986C8fD44eb85294A725a7e61AF35E76bA26F91`
- NadFun Factory: `0xA25b13127e63ddae6d0b35570FF3D39dBD621001`
- Token Registry: `0x3CBF1E9F8847A4c968Bb2636696723CC82b91565`
- Bonding Curve (v2): `0x9f3832732923252A21044F21eE6bd87F09514ae4`
- TokenInfoLens (v1/v2 classifier): `0x40c126f92DAD5C26D3b36aA7F2A949265FA534cB`

#### Testnet — v1

- DEX Factory: `0xd0a37cf728CE2902eB8d4F6f2afc76854048253b`
- WMON Token: `0x5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd`
- Bonding Curve: `0x1228b0dc9481C11D3071E7A924B794CfB038994e`
- Bonding Curve Router: `0x865054F0F6A288adaAc30261731361EA7E908003`
- DEX Router: `0x5D4a4f430cA3B1b2dB86B9cFE48a5316800F5fb2`
- Lens: `0xB056d79CA5257589692699a46623F901a3BB76f1`

#### Testnet — v2

- NadFun Router: `0x75588668999cA0557b78046b8a5E86b47b9234ec`
- NadFun Factory: `0x59C51c66B79c68F63d5446940CD13b6968788e36`
- Token Registry: `0x2Bc127be900aD290E703Cd2C71eB0EDCa162C898`
- Bonding Curve (v2): `0x27063a38eC0D3281D354090EB92e669Ed1eB956C`
- TokenInfoLens (v1/v2 classifier): `0xFC635B7A09cac1A643F5148F8e05Bcd979A8bcC4`

### Supported quote tokens (v2)

`quote_tokens(Network)` returns the known v2 quote tokens — the pricing
currencies a v2 curve can trade against — mirroring the api-server
`GET /quote_token` registry (the authoritative DB source). `is_native == true`
means the router can wrap native MON (`msg.value`) into it, so it is a valid
`quote_token` for `V2CreatePayment::Native { quote_token }` and the
`*_with_native` trades. `MON` is the wrapped native (WMON) per network — same
address as the v1 `WMON` constant; the DB labels it `MON`/`MONAD`.

```rust
use nadfun_sdk::{quote_tokens, Network};

for qt in quote_tokens(Network::Mainnet) {
    println!("{} ({}) {} decimals, native={}", qt.symbol, qt.address, qt.decimals, qt.is_native);
}
```

#### Mainnet

| Symbol | Name | Address | Decimals | Native |
|--------|------|---------|----------|--------|
| MON | MONAD | `0x3bd359C1119dA7Da1D913D1C4D2B7c461115433A` | 18 | yes |
| LVMON | LeverUpMon | `0x91b81bfbe3A747230F0529Aa28d8b2Bc898E6D56` | 18 | yes |

#### Testnet

| Symbol | Name | Address | Decimals | Native |
|--------|------|---------|----------|--------|
| MON | MONAD | `0x5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd` | 18 | yes |
| LVMON | LeverUpMon | `0xBe3fa50514D9617ce645a02B34F595541AF02b6b` | 18 | yes |

## Error Handling

The SDK uses `anyhow::Result` for error handling:

```rust
use anyhow::Result;

async fn example() -> Result<()> {
    let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
    let result = core.v1().get_amount_out(token, amount, true).await?;
    Ok(())
}
```

## Performance & Reliability

### ✅ Verified Features

- **Real-time Streaming**: WebSocket-based event delivery tested and working
- **Event Decoding**: Automatic parsing of bonding curve and swap events
- **Connection Stability**: Streams remain alive and process events continuously
- **Error Handling**: Graceful error handling with `Result<Event>` pattern
- **Multiple Scenarios**: All streaming scenarios tested and verified

### 📊 Tested Scenarios

- **Bonding Curve**: 4 scenarios (all events, filtered events, filtered tokens, combined)
- **DEX Streaming**: 3 scenarios (specific pools, token discovery, single token)
- **Historical Data**: Block range processing with automatic batching
- **Pool Discovery**: Automatic Capricorn CL pool detection for tokens

### ⚡ Performance Features

- **Efficient Filtering**: Network-level filtering for event types
- **Client-side Filtering**: Token-based filtering for precise control
- **Concurrent Processing**: Parallel block processing for historical data
- **Memory Efficient**: Stream-based processing without buffering

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Submit a pull request

## Support

- 📖 [Examples](examples/) - Comprehensive usage examples
- 🐛 [Issues](https://github.com/Naddotfun/nadfun-sdk-rust/issues) - Bug reports and feature requests
