# Release Notes

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