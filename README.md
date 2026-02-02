# Nad.fun Rust SDK

A comprehensive Rust SDK for interacting with Nad.fun ecosystem contracts, including bonding curves, DEX trading, and real-time event monitoring.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
nadfun_sdk = "0.3.12"
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
    let (router, expected_tokens) = core.get_amount_out(token, mon_amount, true).await?;

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
    let estimated_gas = core.estimate_gas(&router, gas_params).await?;
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
    let tx_hash = core.buy(buy_params, router).await?;
    println!("Transaction submitted: {}", tx_hash);

    // Optionally wait for receipt to check status
    let receipt = core.get_receipt(tx_hash).await?;
    println!("Transaction confirmed in block: {:?}", receipt.block_number);
    println!("Gas used: {:?}", receipt.gas_used);
    println!("Status: {}", if receipt.status { "Success" } else { "Failed" });

    Ok(())
}
```

## Features

### 🔑 API Authentication

The SDK uses optional API key authentication for higher rate limits:

```rust
use nadfun_sdk::ApiClient;

// Option 1: Without API key (lower rate limit, but works)
let api = ApiClient::new();

// Option 2: With API key (higher rate limit)
let api = ApiClient::new().with_api_key("nadfun_xxxxx".to_string());

// Option 3: From environment variable (recommended)
// Set NAD_API_KEY in .env or shell
let api = ApiClient::from_env();
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

    let api = ApiClient::from_env(); // Reads NAD_API_KEY from env

    // 2. Calculate token output for initial buy
    let initial_buy = parse_ether("1.5")?; // 1.5 MON
    let tokens_out = core.get_initial_buy_amount_out(initial_buy).await?;

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
    let result = core.create_token(params, &api).await?;

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
let api = ApiClient::new();

// Get created tokens with reward info
let response = api.get_created_tokens(core.wallet_address(), 1, 10).await?;
println!("Found {} tokens", response.total_count);

// Claim rewards for each claimable token
for token in &response.tokens {
    if let Some(params) = ApiClient::build_claim_params(token) {
        println!("Claiming {} wei from {}", token.reward_info.amount, token.token_info.name);
        let tx_hash = core.claim_creator_reward(params).await?;
        println!("TX: {}", tx_hash);
    }
}

// Or batch claim all at once (more gas efficient)
if let Some(batch_params) = ApiClient::build_batch_claim_params(&response.tokens) {
    let tx_hash = core.claim_creator_rewards_batch(batch_params).await?;
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

```rust
use nadfun_sdk::{Trade, SlippageUtils, GasEstimationParams, types::BuyParams};

// Get quote and execute buy
let (router, expected_tokens) = core.get_amount_out(token, mon_amount, true).await?;
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
let estimated_gas = core.estimate_gas(&router, gas_params).await?;
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
let tx_hash = core.buy(buy_params, router).await?;
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
let result = core.buy(params, router).await?;  // Waits ~2-15 seconds

// NEW - Returns immediately (fast!)
let tx_hash = core.buy(params, router).await?;  // Returns in milliseconds
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
use nadfun_sdk::{GasEstimationParams, Trade};

// Create gas estimation parameters for any operation
let gas_params = GasEstimationParams::Buy {
    token,
    amount_in: mon_amount,
    amount_out_min: min_tokens,
    to: wallet_address,
    deadline: U256::from(deadline),
};

// Get real-time gas estimation from network
let estimated_gas = core.estimate_gas(&router, gas_params).await?;

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
let estimated_gas = core.estimate_gas(&router, params).await?;
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

Monitor bonding curve and DEX events in real-time:

#### Bonding Curve Streaming

```rust
use nadfun_sdk::stream::CurveStream;
use nadfun_sdk::types::{BondingCurveEvent, EventType};
use futures_util::{pin_mut, StreamExt};

// Create WebSocket stream
let curve_stream = CurveStream::new("wss://your-ws-endpoint".to_string()).await?;

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

#### DEX Swap Streaming

```rust
use nadfun_sdk::stream::UniswapSwapStream;
use futures_util::{pin_mut, StreamExt};

// Auto-discover pools for tokens
let swap_stream = UniswapSwapStream::discover_pools_for_tokens(
    "wss://your-ws-endpoint".to_string(),
    vec![token_address]
).await?;

// Subscribe and process events
let stream = swap_stream.subscribe().await?;
pin_mut!(stream);

while let Some(event_result) = stream.next().await {
    match event_result {
        Ok(event) => {
            println!("Swap in pool {}: {} -> {}",
                event.pool_address, event.amount0, event.amount1);
        }
        Err(e) => println!("Error: {}", e),
    }
}
```

### 📈 Historical Data Analysis

Fetch and analyze historical events:

```rust
use nadfun_sdk::stream::{CurveIndexer, EventType};

let provider = Arc::new(ProviderBuilder::new().connect_http(http_url.parse()?));
let indexer = CurveIndexer::new(provider);

// Fetch events from block range
let events = indexer.fetch_events(
    18_000_000,
    18_010_000,
    vec![EventType::Create, EventType::Buy],
    None // all tokens
).await?;

println!("Found {} events", events.len());
```

### 🔍 Pool Discovery

Find Capricorn CL pool addresses for tokens:

```rust
use nadfun_sdk::stream::UniswapSwapIndexer;

// Auto-discover pools for multiple tokens
let indexer = UniswapSwapIndexer::discover_pools_for_tokens(provider, tokens).await?;
let pools = indexer.pool_addresses();

// Discover pool for single token
let indexer = UniswapSwapIndexer::discover_pool_for_token(provider, token).await?;
```

### 💱 DEX Monitoring

Monitor Capricorn CL swap events:

```rust
use nadfun_sdk::stream::UniswapSwapIndexer;

// Auto-discover pools for tokens
let indexer = UniswapSwapIndexer::discover_pools_for_tokens(provider, tokens).await?;
let swaps = indexer.fetch_events(from_block, to_block).await?;

for swap in swaps {
    println!("Swap in pool {}: {} -> {}",
        swap.pool_metadata.pool_address,
        swap.amount0,
        swap.amount1
    );
}
```

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

- **Unified Gas Estimation**: Demonstrates `core.estimate_gas()` for all operation types
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

- ✅ All event types: Create, Buy, Sell, Sync, Lock, Listed
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

### Event Types

- `BondingCurveEvent`: Unified enum for all bonding curve events
  - `Create`, `Buy`, `Sell`, `Sync`, `Lock`, `Listed` variants
  - Methods: `.token()`, `.event_type()`, `.block_number()`, `.transaction_index()`
- `SwapEvent`: Capricorn CL swap events with complete metadata
  - Fields: `pool_address`, `amount0`, `amount1`, `sender`, `recipient`, `liquidity`, `tick`, `sqrt_price_x96`
- `EventType`: Enum for filtering bonding curve events
  - Variants: `Create`, `Buy`, `Sell`, `Sync`, `Lock`, `Listed`

### Stream Types

- `CurveStream`: Bonding curve event streaming
  - Methods: `.subscribe_events()`, `.filter_tokens()`, `.subscribe()`
  - Returns: `Pin<Box<dyn Stream<Item = Result<BondingCurveEvent>> + Send>>`
- `UniswapSwapStream`: DEX swap event streaming
  - Methods: `.new()`, `.discover_pools_for_tokens()`, `.discover_pool_for_token()`, `.subscribe()`
  - Returns: `Pin<Box<dyn Stream<Item = Result<SwapEvent>> + Send>>`

### Trading Types

- `BuyParams` / `SellParams`: Parameters for buy/sell operations
- `TradeResult`: Transaction result with status and metadata
- `SlippageUtils`: Utilities for slippage calculations

### Token Types

- `TokenMetadata`: Name, symbol, decimals, total supply
- `PermitSignature`: EIP-2612 permit signature data

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
--rpc-url <URL>      # RPC URL (default: https://your-rpc-endpoint)
--ws-url <URL>       # WebSocket URL (default: wss://your-ws-endpoint)
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

All contract addresses are defined in `constants.rs`:

#### Mainnet

- DEX Factory: `0x6B5F564339DbAD6b780249827f2198a841FEB7F3`
- WMON Token: `0x3bd359C1119dA7Da1D913D1C4D2B7c461115433A`
- Bonding Curve: `0xA7283d07812a02AFB7C09B60f8896bCEA3F90aCE`
- Bonding Curve Router: `0x6F6B8F1a20703309951a5127c45B49b1CD981A22`
- DEX Router: `0x0B79d71AE99528D1dB24A4148b5f4F865cc2b137`
- Lens: `0x7e78A8DE94f21804F7a17F4E8BF9EC2c872187ea`

#### Testnet

- DEX Factory: `0xE6dc50f36E26bAfC5f103021e01EF111402Cd940`
- WMON Token: `0x760AfE86e5de5fa0Ee542fc7B7B713e1c5425701`
- Bonding Curve: `0xbD40afc47F0a42680819513d556C2eBCcd1eBC68`
- Bonding Curve Router: `0xC703bCe420882b1A35428773B92731adCB4a1f7f`
- DEX Router: `0x65586647FC66221c5f208F9b8FC0A93C72e3a598`
- Lens: `0x181B05cD8D73564A22C17825F3413A0f30634CCF`

## Error Handling

The SDK uses `anyhow::Result` for error handling:

```rust
use anyhow::Result;

async fn example() -> Result<()> {
    let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
    let result = core.get_amount_out(token, amount, true).await?;
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
