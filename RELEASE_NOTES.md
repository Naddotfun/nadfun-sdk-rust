# Release Notes

## v0.2.0 (2025-01-08)

### 🚀 Major Features

**Unified Gas Estimation System**

- **New API**: Added `trade.estimate_gas()` method for simplified gas estimation
- **Real Network Conditions**: Replaced static fallback constants with live network-based gas estimation
- **Type-Safe Parameters**: Introduced `GasEstimationParams` enum for Buy, Sell, and SellPermit operations
- **Automatic Problem Solving**: Built-in token approval and permit signature handling

### ✨ New Features

**Enhanced Trading Examples**

- **Gas Estimation Example**: New comprehensive `gas_estimation.rs` example with automatic approval and permit handling
- **Simplified API**: All trading examples now use `trade.estimate_gas(&router, params)` instead of complex manual estimation
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

### 📚 Documentation

**Enhanced Examples Documentation**

- **Gas Requirements**: Clear documentation of token approval and permit signature requirements
- **Usage Guidelines**: Step-by-step instructions for real-world gas estimation
- **Troubleshooting**: Common issues and solutions for gas estimation failures
- **Best Practices**: Recommended buffer strategies and error handling patterns

**Updated EXAMPLES.md**

- **Gas Estimation Section**: New dedicated section explaining gas system requirements
- **Renumbered Examples**: Reorganized example numbering with gas estimation as example #2
- **Enhanced Feature Lists**: Detailed feature descriptions for all examples

### 🛠 Technical Improvements

**Network-Based Estimation**

- **Live RPC Calls**: All gas estimation uses actual network conditions
- **Router-Specific Logic**: Different estimation logic for bonding curve vs DEX operations
- **Error Recovery**: Automatic retry and fallback mechanisms
- **Performance Optimized**: Minimal token amounts (1 token) for efficient estimation

**Example Enhancements**

- **Automatic Approval**: Examples handle token approval transparently
- **Real Signatures**: Permit examples use actual cryptographic signatures
- **Balance Checking**: Smart balance verification and amount adjustment
- **Cost Calculations**: Real-world transaction cost analysis

### ⚠️ Migration Guide

**For Users Upgrading from v0.1.x:**

1. **Replace Static Gas Constants**:

   ```rust
   // OLD (v0.1.x)
   let gas_limit = BondingCurveGas::BUY;

   // NEW (v0.2.0)
   let params = GasEstimationParams::Buy { token, amount_in, amount_out_min, to, deadline };
   let estimated_gas = trade.estimate_gas(&router, params).await?;
   ```

2. **Update Gas Estimation Calls**:

   ```rust
   // OLD (v0.1.x)
   let gas = estimate_gas(trade.provider().clone(), &router, /* individual params */).await?;

   // NEW (v0.2.0)
   let gas = trade.estimate_gas(&router, params).await?;
   ```

3. **Remove Fallback Dependencies**:
   - Remove imports: `BondingCurveGas`, `DexRouterGas`, `get_default_gas_limit`, `Operation`
   - Add imports: `GasEstimationParams`

### 📦 Dependencies

No changes to external dependencies. All improvements use existing Alloy framework capabilities.

### 🔍 Examples

**New Gas Estimation Example:**

```bash
cargo run --example gas_estimation -- --private-key your_key --rpc-url https://your-rpc --token 0xToken
```

**Updated Trading Examples:**

- `buy.rs`: Uses `trade.estimate_gas()` with 20% buffer strategy
- `sell.rs`: Automatic approval + `trade.estimate_gas()` with 15% buffer
- `sell_permit.rs`: Real permit signatures + `trade.estimate_gas()` with 25% buffer

### 🏆 Benefits

- **Developer Experience**: Simplified API reduces boilerplate code
- **Reliability**: Network-based estimation provides accurate gas predictions
- **Automation**: Automatic problem solving reduces integration complexity
- **Production Ready**: Real network conditions make examples suitable for actual trading

---

**Full Changelog**: [v0.1.1...v0.2.0](https://github.com/Naddotfun/nadfun-sdk-rust/compare/v0.1.1...v0.2.0)
