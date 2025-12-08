# Release Notes - v0.3.0

**Release Date:** 2025-01-16

This major release introduces comprehensive token creation capabilities, enhanced contract integration with the latest Capricorn DEX, improved developer experience, and better debugging support.

---

## 🎨 New Features

### Token Creation System
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

### Core Architecture Improvements
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

### Contract Updates
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

### Event Streaming Enhancements
- **Graduate Event Support** - Full support for token graduation events
  - Stream monitoring for `CurveGraduate` events
  - Historical indexing of graduation data
  - Event breakdown statistics in indexer

- **Lock Event Support** - Track token lock events
  - `CurveTokenLocked` event decoding
  - Lock status monitoring in streams
  - Historical lock event indexing

- **Improved Debugging**
  - Detailed WebSocket connection logging
  - Event signature display in streams
  - Token filter status visualization
  - Block-by-block event counting
  - Connection status indicators

---

## 🔧 Breaking Changes

### Module Restructuring
- **`trading` → `core`** - Main module renamed for better clarity
  - `use nadfun_sdk::trading::*` → `use nadfun_sdk::Core`
  - `Trading::new()` → `Core::new()`
  - Old `trading` module removed

- **Examples Reorganization**
  - `examples/trade/` → `examples/core/` - All trading examples moved
  - `examples/create/` - New token creation examples added

### API Changes
- **Gas Price Enhancement** - Gas price calculation improved
  - Buy/Sell now use 3x network gas price (previously 2x)
  - Sell permit uses dynamic network pricing (previously hardcoded 50 gwei)
  - 20-25% gas limit buffer added for complex transactions

- **URL Validation** - Social media URLs now strictly validated
  - Twitter URLs must use `x.com` (not `twitter.com`)
  - Telegram URLs must use `t.me`
  - All URLs must use `https://`
  - Automatic `twitter.com` → `x.com` conversion

- **Token Creation Parameters**
  - `CreateTokenParams` now includes `action_id: ActionId` field (required)
  - Deploy fee automatically calculated and added to transaction value
  - Optional social media URLs (empty strings treated as `None`)
  - **ActionId Enum** - Choose between `ActionId::CapricornActor` (1) or `ActionId::AmplifyActor` (2)

---

## 📚 Documentation

### New Documentation Files
- **`TOKEN_CREATION_FLOW.md`** - Complete API documentation for token creation
  - Step-by-step flow diagrams
  - API endpoint specifications
  - Request/response examples
  - Error handling guide
  - Image requirements and validation rules

### Updated Examples
- **`examples/create/create_token.rs`** - Full token creation example
  - CLI argument support for all parameters
  - Environment variable configuration
  - Comprehensive error messages
  - Image format requirements clearly stated

- **`examples/EXAMPLES.md`** - Enhanced with token creation section
  - Image requirements prominently displayed
  - Supported/unsupported formats listed
  - Best practices and tips included

- **All Trading Examples** - Updated paths and improved docs
  - `core/buy.rs` - Enhanced with token status checks
  - `core/sell.rs` - Dynamic gas pricing
  - `core/sell_permit.rs` - Network gas price integration
  - `core/gas_estimation.rs` - Comprehensive gas analysis

### CLI Improvements
- **Common Module Enhancement** (`examples/common/`)
  - Support for all token creation parameters
  - `--name`, `--symbol`, `--description` flags
  - `--image-uri` with validation
  - `--initial-buy` for custom amounts
  - `--website`, `--twitter`, `--telegram` social links
  - Improved help messages with examples

---

## 🐛 Bug Fixes

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

---

## 🔍 Developer Experience

### Enhanced Debugging
- **Stream Diagnostics** - Detailed connection and filter information
  - WebSocket URL validation
  - Event signature display
  - Token filter visualization
  - Connection status logging
  - Event counting and statistics

- **Transaction Debugging** - Clear parameter display
  - Deploy fee breakdown
  - Total value calculation shown
  - Gas settings displayed
  - Token metadata preview

### Error Messages
- **User-Friendly Errors** - Actionable error messages
  - Missing image URI shows requirements and examples
  - Invalid URLs show correct format with examples
  - API errors parsed and displayed clearly
  - Validation failures include fix suggestions

### Code Quality
- **Type Safety** - Enhanced type definitions
  - `CreateTokenParams` with comprehensive fields
  - `ApiErrorResponse` for error handling
  - `GraduateEvent` and `LockEvent` structures
  - Better `Router` enum with address access

---

## 📊 Statistics

- **+5,753 lines added** across 52 files
- **-737 lines removed** (refactoring and cleanup)
- **15 new ABI files** for complete contract coverage
- **2 new major features** (token creation, enhanced streaming)
- **4 new example files** demonstrating capabilities
- **100% backward compatibility** for core trading operations (with import path updates)

---

## 🚀 Migration Guide

### From v0.2.x to v0.3.0

#### Update Imports
```rust
// Old
use nadfun_sdk::trading::Trading;
use nadfun_sdk::trading::trade;

// New
use nadfun_sdk::Core;
```

#### Update Initialization
```rust
// Old
let trading = Trading::new(rpc_url, private_key).await?;

// New
let core = Core::new(rpc_url, private_key, Network::Mainnet).await?;
```

#### Update Example Paths
```bash
# Old
cargo run --example buy  # From examples/trade/

# New
cargo run --example buy  # From examples/core/
```

#### Social Media URLs
```rust
// Old - would fail
twitter: Some("https://twitter.com/project".to_string())

// New - automatically converted
twitter: Some("https://twitter.com/project".to_string())  // → x.com
// Or manually use x.com
twitter: Some("https://x.com/project".to_string())
```

---

## 🔮 Future Plans

- **v0.4.0 Roadmap**
  - Advanced pool analytics
  - Multi-token batch operations
  - WebSocket reconnection handling
  - Event replay capabilities
  - Enhanced error recovery

---

## 👥 Contributors

- Core development and architecture improvements
- Token creation system implementation
- Documentation and examples
- Contract integration updates
- Debugging and quality improvements

---

## 📄 License

MIT License - See LICENSE file for details

---

## 🔗 Links

- **Repository**: https://github.com/Naddotfun/nadfun-sdk-rust
- **Documentation**: See README.md and examples/
- **Issues**: https://github.com/Naddotfun/nadfun-sdk-rust/issues
- **Nad.fun Platform**: https://nad.fun
