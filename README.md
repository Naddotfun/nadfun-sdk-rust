# Nad.fun Rust SDK

A comprehensive Rust SDK for interacting with Nad.fun ecosystem contracts, including bonding curves, DEX trading, and real-time event monitoring.

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
nadfun-sdk = "0.1.0"
```

## Quick Start

```rust
use nadfun_sdk::prelude::*; // Import everything you need
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_url = "https://your-rpc-endpoint".to_string();
    let private_key = "your_private_key_here".to_string();

    // Trading
    let trade = Trade::new(rpc_url.clone(), private_key.clone()).await?;
    let token: Address = "0x...".parse()?;
    let (router, amount_out) = trade.get_amount_out(token, parse_ether("0.1")?, true).await?;

    // Token operations
    let token_helper = TokenHelper::new(rpc_url, private_key).await?;
    let balance = token_helper.balance_of(token, "0x...".parse()?).await?;

    Ok(())
}
```

## Features

### 🚀 Trading

Execute buy/sell operations on bonding curves with slippage protection:

```rust
use nadfun_sdk::{Trade, SlippageUtils, types::BuyParams};

// Get quote and execute buy
let (router, expected_tokens) = trade.get_amount_out(token, mon_amount, true).await?;
let min_tokens = SlippageUtils::calculate_amount_out_min(expected_tokens, 5.0);

let buy_params = BuyParams {
    token,
    amount_in: mon_amount,
    amount_out_min: min_tokens,
    to: wallet_address,
    deadline: U256::from(deadline),
    gas_limit: Some(get_default_gas_limit(&router, Operation::Buy)), // Use default gas limits
    gas_price: Some(50_000_000_000), // 50 gwei
    nonce: None, // Auto-detect
};

let result = trade.buy(buy_params, router).await?;
```

### ⛽ Gas Management

The SDK provides intelligent gas management with both real-time estimation and optimized defaults:

#### Default Gas Limits (Recommended)

Based on comprehensive contract testing with 20% safety buffer:

```rust
use nadfun_sdk::{Operation, get_default_gas_limit, BondingCurveGas, DexRouterGas, Router};

// Simple usage with defaults (recommended)
let buy_params = BuyParams {
    token,
    amount_in: mon_amount,
    amount_out_min: min_tokens,
    to: wallet_address,
    deadline: U256::from(deadline),
    gas_limit: Some(get_default_gas_limit(&router, Operation::Buy)), // Safe default
    gas_price: Some(50_000_000_000), // 50 gwei
    nonce: None, // Auto-detect
};
```

#### Real-time Gas Estimation

Examples demonstrate both estimation and defaults for comparison:

```rust
// Get real-time gas estimation for specific transaction
let estimated_gas = match &router {
    Router::BondingCurve(_) => {
        let contract_params = IBondingCurveRouter::BuyParams { /* ... */ };
        let contract = IBondingCurveRouter::new(router.address(), provider);
        let call_data = contract.buy(contract_params).calldata();

        provider.estimate_gas(TransactionRequest::default()
            .to(router.address())
            .from(wallet)
            .value(amount)
            .input(call_data.into())
        ).await?
    }
    Router::Dex(_) => { /* similar for DEX */ }
};

// Compare with defaults
println!("⛽ Estimated gas: {}", estimated_gas);
println!("⛽ Default gas limit: {}", get_default_gas_limit(&router, Operation::Buy));
```

#### Gas Constants

Access gas limits directly if needed:

```rust
// Bonding Curve gas limits
let bc_buy = BondingCurveGas::BUY;        // 320,000
let bc_sell = BondingCurveGas::SELL;      // 170,000
let bc_permit = BondingCurveGas::SELL_PERMIT; // 210,000

// DEX Router gas limits (higher due to complexity)
let dex_buy = DexRouterGas::BUY;          // 350,000
let dex_sell = DexRouterGas::SELL;        // 200,000
let dex_permit = DexRouterGas::SELL_PERMIT; // 250,000
```

#### Custom Gas Strategies

```rust
// Strategy 1: Use defaults (safest)
gas_limit: Some(get_default_gas_limit(&router, Operation::Buy))

// Strategy 2: Add custom buffer to estimation
gas_limit: Some(estimated_gas + 50_000)

// Strategy 3: Use estimation with fallback to default
gas_limit: Some(estimated_gas.max(get_default_gas_limit(&router, Operation::Buy)))
```

#### Gas Price Optimization

```rust
// Get current network conditions
let network_gas_price = provider.get_gas_price().await?;
let recommended_gas_price = network_gas_price * 300 / 100; // 3x for EIP-1559

let params = BuyParams {
    // ... other fields
    gas_price: Some(recommended_gas_price.try_into().unwrap_or(50_000_000_000)),
};
```

**Gas Limits Summary:**

- **Bonding Curve**: Buy: 320k, Sell: 170k, SellPermit: 210k
- **DEX Router**: Buy: 350k, Sell: 200k, SellPermit: 250k
- **All limits include 20% safety buffer based on forge test data**

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

Find Uniswap V3 pool addresses for tokens:

```rust
use nadfun_sdk::stream::UniswapSwapIndexer;

// Auto-discover pools for multiple tokens
let indexer = UniswapSwapIndexer::discover_pools_for_tokens(provider, tokens).await?;
let pools = indexer.pool_addresses();

// Discover pool for single token
let indexer = UniswapSwapIndexer::discover_pool_for_token(provider, token).await?;
```

### 💱 DEX Monitoring

Monitor Uniswap V3 swap events:

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

### Trading Examples

```bash
# Using environment variables
export PRIVATE_KEY="your_private_key_here"
export RPC_URL="https://your-rpc-endpoint"
export TOKEN="0xTokenAddress"
export RECIPIENT="0xRecipientAddress"  # For token operations

cargo run --example buy              # Buy tokens with gas comparison
cargo run --example sell             # Sell tokens with gas optimization
cargo run --example sell_permit      # Gasless sell with permit signature
cargo run --example basic_operations # Token operations (requires recipient)

# Using command line arguments
cargo run --example buy -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
cargo run --example sell -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
cargo run --example sell_permit -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress
cargo run --example basic_operations -- --private-key your_private_key_here --rpc-url https://your-rpc-endpoint --token 0xTokenAddress --recipient 0xRecipientAddress
```

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
- ✅ Real-time Uniswap V3 swap events
- ✅ Pool metadata included
- ✅ WebSocket streaming

#### 🔍 Pool Discovery

**5. pool_discovery** - Automated pool address discovery

```bash
# Find Uniswap V3 pools for multiple tokens
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
- `SwapEvent`: Uniswap V3 swap events with complete metadata
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

- Bonding Curve: `0x52D34d8536350Cd997bCBD0b9E9d722452f341F5`
- Bonding Curve Router: `0x4F5A3518F082275edf59026f72B66AC2838c0414`
- DEX Router: `0x4FBDC27FAE5f99E7B09590bEc8Bf20481FCf9551`
- WMON Token: `0x760AfE86e5de5fa0Ee542fc7B7B713e1c5425701`

## Error Handling

The SDK uses `anyhow::Result` for error handling:

```rust
use anyhow::Result;

async fn example() -> Result<()> {
    let trade = Trade::new(rpc_url, private_key).await?;
    let result = trade.get_amount_out(token, amount, true).await?;
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
- **Pool Discovery**: Automatic Uniswap V3 pool detection for tokens

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
- 🐛 [Issues](https://github.com/your-org/nadfun-sdk/issues) - Bug reports and feature requests
- 💬 [Discussions](https://github.com/your-org/nadfun-sdk/discussions) - Community support
