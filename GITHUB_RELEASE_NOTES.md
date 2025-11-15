# Release Notes

## v0.3.0 (2025-01-14)

### 🚀 Major Changes

#### **Capricorn CL Integration**
- **Breaking**: Replaced Capricorn CL contracts with Capricorn CL (Concentrated Liquidity) implementation
- **ABI-Based Contracts**: All contract interfaces now load from external ABI files for better maintainability
- **Type Safety**: Enhanced type safety with proper Capricorn CL pool and factory interfaces

#### **Global Network Configuration**
- **New API**: Added `set_network()` and `get_current_network()` for global network management
- **Simplified Usage**: Set network once, all contract addresses automatically resolve to correct network
- **Network Enum**: New `Network` enum with `Mainnet` and `Testnet` variants

#### **Token Creation Feature**
- **Complete Flow**: New `Trade.create_token()` method handles entire token creation process
- **Integrated API**: Automatic image upload, metadata creation, and salt generation
- **TokenCreationClient**: Standalone client for advanced token creation workflows
- **On-Chain Deployment**: Seamless integration with bonding curve router

#### **Consistent Naming**
- **DEX Terminology**: Unified naming from "Uniswap" to "DEX" throughout the SDK
- **Clear Abstractions**: `DexStream`, `DexIndexer`, `DexRouter`, `DexFactory`
- **Better Semantics**: Code now reflects actual DEX implementation (Capricorn CL)

### ✨ New Features

#### **Token Creation**
```rust
use nadfun_sdk::{Trade, CreateTokenParams, Network};
use alloy::primitives::utils::parse_ether;

// Initialize with network
let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;

// Create token with single method call
let params = CreateTokenParams {
    name: "My Token".to_string(),
    symbol: "MTK".to_string(),
    description: "My awesome token".to_string(),
    image_uri: "https://example.com/image.png".to_string(),
    amount_out: parse_ether("1000000")?,
    value: parse_ether("1.5")?,
    // ...
};

let result = core.create_token(params).await?;
```

#### **Network Management**
```rust
use nadfun_sdk::{Network, set_network, get_bonding_curve_router};

// Set network globally
set_network(Network::Testnet);

// All addresses automatically use testnet
let router_address = get_bonding_curve_router();
```

#### **Capricorn CL Pools**
```rust
use nadfun_sdk::{DexStream, DexIndexer, ICapricornCLPool};

// Stream DEX events from Capricorn CL pools
let stream = DexStream::new(ws_url, pool_addresses).await?;
let events = stream.subscribe().await?;
```

### 🔧 Breaking Changes

#### **Contract Interfaces**
```rust
// OLD (v0.2.x)
use nadfun_sdk::{UniswapSwapStream, UniswapSwapIndexer};

// NEW (v0.3.0)
use nadfun_sdk::{DexStream, DexIndexer};
```

#### **Network Configuration**
```rust
// OLD (v0.2.x)
let core = Core::new(rpc_url, private_key).await?;

// NEW (v0.3.0)
let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
```

#### **Contract Constants**
```rust
// OLD (v0.2.x)
use nadfun_sdk::constants::UNISWAP_V3_FACTORY;

// NEW (v0.3.0)
use nadfun_sdk::get_dex_factory;
let factory = get_dex_factory();
```

### ⚠️ Migration Guide

**Step 1: Update Dependencies**
```toml
[dependencies]
nadfun_sdk = "0.3.0"
```

**Step 2: Update Network Initialization**
```rust
let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
```

**Step 3: Rename DEX Types**
- `UniswapSwapStream` → `DexStream`
- `UniswapSwapIndexer` → `DexIndexer`
- `UniswapV3Pool` → `ICapricornCLPool`

**Step 4: Update Contract Address Usage**
```rust
use nadfun_sdk::{get_bonding_curve_router, get_dex_router};
let router = get_bonding_curve_router();
```

### 📦 Installation

```toml
[dependencies]
nadfun_sdk = "0.3.0"
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