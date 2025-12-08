# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2025-01-XX

### Changed - Fast Transaction Submission

**BREAKING CHANGE**: All trading functions now return transaction hash immediately instead of waiting for receipt.

#### Modified Functions

All these functions now return `Result<B256>` instead of `Result<TransactionResult>`:

**BondingCurve Router** (`bonding_curve.rs`):
- `buy()` - Returns tx_hash immediately
- `sell()` - Returns tx_hash immediately
- `sell_permit()` - Returns tx_hash immediately
- `exact_out_buy()` - Returns tx_hash immediately
- `exact_out_sell()` - Returns tx_hash immediately
- `exact_out_sell_permit()` - Returns tx_hash immediately

**DEX Router** (`dex.rs`):
- `buy()` - Returns tx_hash immediately
- `sell()` - Returns tx_hash immediately
- `sell_permit()` - Returns tx_hash immediately
- `exact_out_buy()` - Returns tx_hash immediately
- `exact_out_sell()` - Returns tx_hash immediately
- `exact_out_sell_permit()` - Returns tx_hash immediately

**Core Client** (`core.rs`):
- `buy()` - Returns tx_hash immediately
- `sell()` - Returns tx_hash immediately
- `sell_permit()` - Returns tx_hash immediately

### Added

- **`Core::get_receipt(tx_hash: B256)`** - New function to retrieve transaction receipt
  - Returns `Result<TransactionResult>` with full transaction details
  - Use this when you need to check transaction status, gas used, or logs
  - Supports polling for receipt until it's available

### Migration Guide

#### Before (v0.2.x):

```rust
// Old - Waits for confirmation automatically (slow)
let result = core.buy(params, router).await?;
println!("Status: {}", result.status);
println!("Gas used: {:?}", result.gas_used);
```

#### After (v0.3.0):

```rust
// New - Returns immediately (fast!)
let tx_hash = core.buy(params, router).await?;
println!("Submitted: {}", tx_hash);

// Check status later when needed
let receipt = core.get_receipt(tx_hash).await?;
println!("Status: {}", receipt.status);
println!("Gas used: {:?}", receipt.gas_used);
```

### Benefits

1. **Faster Trading Bots**: Submit transactions in milliseconds instead of waiting 2-15 seconds for confirmation
2. **Better Control**: Choose when to wait for confirmation
3. **Batch Operations**: Submit multiple transactions quickly, then check their status together
4. **Fire and Forget**: For some use cases, you don't need to wait at all

### Examples Updated

All trading examples have been updated to demonstrate the new pattern:
- `examples/core/buy.rs` - Shows immediate submission and optional receipt checking
- `examples/core/sell.rs` - Demonstrates sell with receipt verification
- `examples/core/sell_permit.rs` - Gasless sell with new pattern

## [0.2.0] - 2024-XX-XX

### Added

- Unified gas estimation system (`GasEstimationParams`)
- Network-based gas estimation for all operations
- Automatic token approval handling for SELL operations
- Real EIP-2612 permit signature generation
- Comprehensive gas estimation examples

### Changed

- Replaced static gas constants with dynamic network estimation
- Improved error handling for gas estimation failures

## [0.1.0] - 2024-XX-XX

### Added

- Initial release
- Trading functionality (buy/sell on bonding curves and DEX)
- Token creation with metadata and image upload
- Real-time event streaming
- Historical data indexing
- Pool discovery utilities
