//! Gas Estimation Example
//!
//! This example demonstrates how to use the new unified `estimate_gas` function
//! to get accurate gas estimates for trading operations.
//!
//! ## Important Notes
//!
//! - **Token Approval Required**: For SELL and SELL PERMIT operations, tokens must be
//!   approved for the router before gas estimation will work properly. This example
//!   automatically handles token approval when needed.
//!
//! - **Real Network Conditions**: Gas estimation uses actual network calls and will
//!   fail if proper token balances and approvals are not in place.
//!
//! - **Automatic Problem Solving**: The example includes automatic approval handling
//!   and real permit signature generation to ensure gas estimation succeeds.

use alloy::primitives::{utils::parse_ether, Address, U256};
use anyhow::Result;
use nadfun_sdk::{GasEstimationParams, SlippageUtils, TokenHelper, Trade};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args().unwrap_or_else(|_| Config::from_env());

    println!("📋 Configuration:");
    println!("  RPC URL: {}", config.rpc_url);
    println!("  WS URL: {}", config.ws_url);
    println!("  Private Key: ✅ Provided");
    println!(
        "  Token: {}",
        config
            .token
            .as_ref()
            .unwrap_or(&"Default token".to_string())
    );
    println!(
        "  Tokens: {}",
        if config.tokens.is_empty() {
            "❌ Not provided"
        } else {
            "✅ Provided"
        }
    );
    println!(
        "  Recipient: {}",
        if config.recipient.is_some() {
            "✅ Provided"
        } else {
            "❌ Not provided"
        }
    );
    println!();

    // Initialize trading interface
    let private_key = config.require_private_key()?;
    let trade = Trade::new(config.rpc_url.clone(), private_key.clone()).await?;
    let token_helper = TokenHelper::new(config.rpc_url, private_key).await?;
    let token: Address = config
        .token
        .unwrap_or_else(|| "0x1234567890123456789012345678901234567890".to_string())
        .parse()?;
    let wallet = trade.wallet_address();

    println!("🔍 Wallet: {}", wallet);
    println!("🪙 Token: {}", token);
    println!();

    // Example amounts
    let mon_amount = parse_ether("0.01")?; // 0.01 MON for buying
    let token_amount = parse_ether("1")?; // 1 token for gas estimation
    let deadline = U256::from(9999999999999999u64);

    // Get router information
    let (router, expected_tokens) = trade.get_amount_out(token, mon_amount, true).await?;
    let min_tokens = SlippageUtils::calculate_amount_out_min(expected_tokens, 5.0);

    println!("📊 Router: {:?}", router);
    println!("💱 Expected tokens from 0.01 MON: {}", expected_tokens);
    println!("🛡️ Min tokens (5% slippage): {}", min_tokens);
    println!();

    // === BUY GAS ESTIMATION ===
    println!("⛽ === BUY GAS ESTIMATION ===");

    let buy_params = GasEstimationParams::Buy {
        token,
        amount_in: mon_amount,
        amount_out_min: min_tokens,
        to: wallet,
        deadline,
    };

    let buy_gas = match trade.estimate_gas(&router, buy_params).await {
        Ok(gas) => {
            println!("📈 Estimated gas for BUY: {}", gas);
            gas
        }
        Err(e) => {
            println!("⚠️ BUY gas estimation failed: {}", e);
            return Err(e);
        }
    };

    // Different buffer strategies
    let buy_gas_with_buffer_fixed = buy_gas + 50_000;
    let buy_gas_with_buffer_percent = buy_gas * 120 / 100; // 20% buffer

    println!(
        "  📊 With fixed buffer (+50k): {}",
        buy_gas_with_buffer_fixed
    );
    println!("  📊 With 20% buffer: {}", buy_gas_with_buffer_percent);
    println!();

    // === SELL GAS ESTIMATION ===
    println!("⛽ === SELL GAS ESTIMATION ===");
    println!("⚠️  NOTE: SELL operations require token approval for the router!");
    println!("    This example will automatically approve tokens if needed.");

    // Check actual token balance
    let token_balance = match token_helper.balance_of(token, wallet).await {
        Ok(balance) => {
            println!("💰 Token balance: {}", balance);
            balance
        }
        Err(e) => {
            println!("⚠️ Could not check token balance: {}", e);
            U256::ZERO
        }
    };

    let actual_sell_amount = if token_balance >= token_amount {
        token_amount // Use 1 token for estimation
    } else if token_balance > U256::ZERO {
        // Use smaller amount if available balance is less than 1 token
        std::cmp::min(token_balance, parse_ether("0.1")?)
    } else {
        parse_ether("0.1")? // Use very small amount for estimation even if no balance
    };

    println!("🔄 Using amount for estimation: {}", actual_sell_amount);

    let (sell_router, expected_mon) =
        match trade.get_amount_out(token, actual_sell_amount, false).await {
            Ok((router, amount)) => (router, amount),
            Err(e) => {
                println!("⚠️ Could not get sell quote: {}", e);
                // Use buy router as fallback
                (router.clone(), U256::from(1000000)) // 1 wei as fallback
            }
        };

    // Check allowance for the router
    let allowance = match token_helper
        .allowance(token, wallet, sell_router.address())
        .await
    {
        Ok(allowance) => {
            println!("🔒 Current allowance for router: {}", allowance);
            allowance
        }
        Err(e) => {
            println!("⚠️ Could not check allowance: {}", e);
            U256::ZERO
        }
    };

    if allowance < actual_sell_amount {
        println!(
            "⚠️ Insufficient allowance! Need: {}, Have: {}",
            actual_sell_amount, allowance
        );
        println!("🔧 Approving tokens for router...");

        match token_helper
            .approve(token, sell_router.address(), actual_sell_amount)
            .await
        {
            Ok(tx_hash) => {
                println!("✅ Approval successful: {}", tx_hash);
                println!("⏳ Waiting for approval to be mined...");
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
            Err(e) => {
                println!("⚠️ Approval failed: {}", e);
                println!("🔧 Gas estimation may still fail due to lack of approval");
            }
        }
    } else {
        println!("✅ Sufficient allowance available");
    }

    let _min_mon = SlippageUtils::calculate_amount_out_min(expected_mon, 5.0);

    let sell_params = GasEstimationParams::Sell {
        token,
        amount_in: actual_sell_amount,
        amount_out_min: U256::from(1), // Use very low minimum to avoid revert
        to: wallet,
        deadline,
    };

    let sell_gas = match trade.estimate_gas(&sell_router, sell_params).await {
        Ok(gas) => {
            println!("📈 Estimated gas for SELL: {}", gas);
            gas
        }
        Err(e) => {
            println!("⚠️ SELL gas estimation failed: {}", e);
            println!("🔄 Skipping SELL gas comparison");
            0 // Use 0 to indicate failure
        }
    };

    // Different buffer strategies
    let sell_gas_with_buffer_fixed = sell_gas + 30_000;
    let sell_gas_with_buffer_percent = sell_gas * 115 / 100; // 15% buffer

    println!(
        "  📊 With fixed buffer (+30k): {}",
        sell_gas_with_buffer_fixed
    );
    println!("  📊 With 15% buffer: {}", sell_gas_with_buffer_percent);
    println!();

    // === SELL PERMIT GAS ESTIMATION ===
    println!("⛽ === SELL PERMIT GAS ESTIMATION ===");
    println!("⚠️  NOTE: SELL PERMIT operations require real permit signatures!");
    println!("    This example generates real EIP-2612 permit signatures for accurate estimation.");

    // Generate real permit signature for gas estimation
    let (v, r, s) = match token_helper
        .generate_permit_signature(
            token,
            wallet,
            sell_router.address(),
            actual_sell_amount,
            deadline,
        )
        .await
    {
        Ok((v, r, s)) => {
            println!("✅ Generated valid permit signature");
            (v, r.into(), s.into())
        }
        Err(e) => {
            println!("⚠️ Permit signature generation failed: {}", e);
            println!("🔧 Using dummy signature - estimation will likely fail");
            (27u8, [0u8; 32], [0u8; 32])
        }
    };

    let sell_permit_params = GasEstimationParams::SellPermit {
        token,
        amount_in: actual_sell_amount,
        amount_out_min: U256::from(1), // Use very low minimum to avoid revert
        to: wallet,
        deadline,
        v,
        r,
        s,
    };

    let sell_permit_gas = match trade.estimate_gas(&sell_router, sell_permit_params).await {
        Ok(gas) => {
            println!("📈 Estimated gas for SELL PERMIT: {}", gas);
            gas
        }
        Err(e) => {
            println!("⚠️ SELL PERMIT gas estimation failed: {}", e);
            println!("🔄 Skipping SELL PERMIT gas comparison");
            0 // Use 0 to indicate failure
        }
    };

    // Different buffer strategies
    let permit_gas_with_buffer = sell_permit_gas * 125 / 100; // 25% buffer (permits can be more complex)

    println!("  📊 With 25% buffer: {}", permit_gas_with_buffer);
    println!();

    // === GAS COMPARISON ===
    println!("📊 === GAS COMPARISON ===");
    println!("  🔵 BUY: {} gas", buy_gas);
    println!("  🔴 SELL: {} gas", sell_gas);
    println!("  🟣 SELL PERMIT: {} gas", sell_permit_gas);
    println!();

    // Calculate costs (assuming 50 gwei gas price)
    let gas_price_gwei = 50u64;
    let gas_price_wei = gas_price_gwei * 1_000_000_000; // Convert to wei

    let buy_cost_wei = buy_gas * gas_price_wei;
    let sell_cost_wei = sell_gas * gas_price_wei;
    let permit_cost_wei = sell_permit_gas * gas_price_wei;

    println!("💰 === ESTIMATED COSTS (at 50 gwei) ===");
    println!(
        "  🔵 BUY: {} wei (~{:.6} MON)",
        buy_cost_wei,
        buy_cost_wei as f64 / 1e18
    );
    println!(
        "  🔴 SELL: {} wei (~{:.6} MON)",
        sell_cost_wei,
        sell_cost_wei as f64 / 1e18
    );
    println!(
        "  🟣 SELL PERMIT: {} wei (~{:.6} MON)",
        permit_cost_wei,
        permit_cost_wei as f64 / 1e18
    );
    println!();

    Ok(())
}
