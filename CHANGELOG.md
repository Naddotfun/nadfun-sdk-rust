# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.12] - 2025-02-02

### Added

- **API Key Authentication** - Optional API key support for higher rate limits
  - `ApiClient::new()` - No auth (10 req/min)
  - `ApiClient::from_env()` - Load API key from `NAD_API_KEY` environment variable
  - `ApiClient::new().with_api_key()` - Explicit API key (100 req/min)

- **Creator Rewards** - Claim trading fee rewards for created tokens
  - `ApiClient::get_created_tokens()` - Query created tokens with reward info
  - `ApiClient::build_claim_params()` - Build claim parameters from reward info
  - `ApiClient::build_batch_claim_params()` - Build batch claim parameters
  - `Core::claim_creator_reward()` - Claim reward for single token
  - `Core::claim_creator_rewards_batch()` - Batch claim for multiple tokens

### Changed

- **API URL Updated** - Mainnet API URL changed to `https://api.nadapp.net`
- **Agent API Paths** - Token creation now uses `/agent/*` endpoints
  - `/agent/token/image` - Image upload
  - `/agent/token/metadata` - Metadata creation
  - `/agent/salt` - Salt generation
  - `/agent/token/created/:address` - Get created tokens
- **Simplified Architecture** - `TokenCreationClient` deprecated, use `ApiClient` directly
- **Core::create_token()** now takes `&ApiClient` instead of `&TokenCreationClient`

### Deprecated

- `TokenCreationClient` - Use `ApiClient` directly for all API operations

### Migration Guide

```rust
// Before (deprecated)
let api = ApiClient::new();
let client = TokenCreationClient::with_client(Arc::new(api));
core.create_token(params, &client).await?;

// After (recommended)
let api = ApiClient::from_env(); // or ApiClient::new().with_api_key(key)
core.create_token(params, &api).await?;

// Creator rewards
let response = api.get_created_tokens(address, 1, 10).await?;
if let Some(params) = ApiClient::build_claim_params(&token) {
    core.claim_creator_reward(params).await?;
}
```

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
