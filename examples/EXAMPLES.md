# Nad.fun SDK Examples

This directory contains comprehensive examples demonstrating how to use the Nad.fun SDK for token creation, trading, and real-time event streaming.

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
- 🎯 **Event Filtering**: Create, Buy, Sell, Sync, Lock, Listed events
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
- 🔄 **All Event Types**: Create, Buy, Sell, Sync, Lock, Listed support
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
use nadfun_sdk::{GasEstimationParams, Trade};

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