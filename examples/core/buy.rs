//! Buy token example with slippage protection and custom gas settings
//!
//! This example demonstrates:
//! 1. Getting quote for token purchase
//! 2. Calculating slippage protection (amount_out_min)
//! 3. Configuring custom gas parameters (limit, price, nonce)
//! 4. Executing buy transaction
//! 5. Verifying transaction success
//!
//! ## Usage
//!
//! ```bash
//! # Using environment variables
//! export PRIVATE_KEY="your_private_key_here"
//! export RPC_URL="https://your-rpc-url"
//! export TOKEN="0xTokenAddress"
//! cargo run --example buy
//!
//! # Using command line arguments
//! cargo run --example buy -- --private-key your_private_key_here --rpc-url https://your-rpc-url --token 0xTokenAddress
//! ```

use alloy::eips::BlockId;
use alloy::primitives::{utils::parse_ether, Address, U256};
use alloy::providers::Provider;
use anyhow::Result;
use nadfun_sdk::types::BuyParams;
use nadfun_sdk::{Core, GasEstimationParams, SlippageUtils};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    let private_key = config.require_private_key()?;

    // Token to buy
    let token: Address = match config.token {
        Some(token_address) => token_address.parse()?,
        None => {
            eprintln!("⚠️  Token address required for this operation.");
            eprintln!("   Set it with: --token TOKEN_ADDRESS");
            eprintln!("   Or use environment variable: export TOKEN=TOKEN_ADDRESS");
            anyhow::bail!("Token address required");
        }
    };

    // Amount of MON to spend (0.001 MON - even smaller amount to test)
    let mon_amount = parse_ether("1")?;
    println!("mon_amount: {}", mon_amount);
    // Create Core instance with network
    let core = Core::new(config.rpc_url, private_key, config.network).await?;

    // Get wallet address from core instance
    let wallet = core.wallet_address();

    // Check MON balance
    let mon_balance = core
        .provider()
        .get_balance(wallet)
        .block_id(BlockId::latest())
        .await?;
    println!(
        "💰 Account MON balance: {} MON",
        alloy::primitives::utils::format_ether(mon_balance)
    );

    if mon_balance < mon_amount {
        println!("❌ Insufficient MON balance!");
        println!(
            "  Required: {} MON",
            alloy::primitives::utils::format_ether(mon_amount)
        );
        println!(
            "  Available: {} MON",
            alloy::primitives::utils::format_ether(mon_balance)
        );
        return Ok(());
    }

    // Check token status before buying
    println!("🔍 Checking token status...");
    let is_locked = core.is_locked(token).await?;
    let is_graduated = core.is_graduated(token).await?;
    println!("  Is locked: {}", is_locked);
    println!("  Is graduated: {}", is_graduated);

    if is_locked {
        println!("⚠️  Warning: Token is locked!");
    }

    let (router, amount_out) = core.get_amount_out(token, mon_amount, true).await?;
    println!("📊 Quote:");
    println!("  Router: {:?}", router);
    println!("  Router address: {}", router.address());
    println!("  MON amount in: {}", mon_amount);
    println!("  Expected tokens out: {}", amount_out);

    // Check if amount_out is valid (not zero)
    if amount_out == U256::ZERO {
        println!("❌ Invalid quote: amount_out is zero!");
        println!("   This token might not be tradeable or the pool doesn't exist.");
        anyhow::bail!("Cannot buy: invalid quote received");
    }

    let slippage_percent = 5.0;
    let amount_out_min = SlippageUtils::calculate_amount_out_min(amount_out, slippage_percent);

    println!("🛡️ Slippage protection:");
    println!("  Expected tokens: {}", amount_out);
    println!("  Minimum tokens ({}% slippage): {}", slippage_percent, amount_out_min);

    // Verify amount_out_min is reasonable
    if amount_out_min == U256::ZERO {
        println!("❌ amount_out_min calculated as zero!");
        println!("   Expected amount_out: {}", amount_out);
        anyhow::bail!("Invalid slippage calculation");
    }

    // Get current account nonce
    let current_nonce = core
        .provider()
        .get_transaction_count(wallet)
        .block_id(BlockId::latest())
        .await?;
    println!("📊 Current account nonce: {}", current_nonce);

    // Get current network gas price
    let network_gas_price_raw = core.provider().get_gas_price().await?;
    let network_gas_price = U256::from(network_gas_price_raw);
    let recommended_gas_price = network_gas_price * U256::from(300) / U256::from(100); // 200% higher than network for EIP-1559
    println!(
        "⛽ Network gas price: {} gwei",
        network_gas_price / U256::from(1_000_000_000)
    );
    println!(
        "⛽ Recommended gas price: {} gwei",
        recommended_gas_price / U256::from(1_000_000_000)
    );

    // === GAS ESTIMATION ===

    // Use new unified gas estimation system
    let deadline = U256::from(9999999999999999u64);
    let gas_params = GasEstimationParams::Buy {
        token,
        amount_in: mon_amount,
        amount_out_min,
        to: wallet,
        deadline,
    };

    let estimated_gas = match core.estimate_gas(&router, gas_params).await {
        Ok(gas) => {
            println!("⛽ Estimated gas for buy: {}", gas);
            gas
        }
        Err(e) => {
            println!("⚠️ Gas estimation failed: {}", e);
            println!("⛽ Using fallback gas limit: 300000");
            300000
        }
    };

    // Add 20% buffer to estimated gas
    let gas_with_buffer = estimated_gas * 120 / 100;
    println!("⛽ Gas with 20% buffer: {}", gas_with_buffer);

    let buy_params = BuyParams {
        token,
        amount_in: mon_amount,
        amount_out_min, // Use slippage-protected amount
        to: wallet,
        deadline,
        gas_limit: Some(gas_with_buffer), // Use estimated gas with buffer
        gas_price: Some(recommended_gas_price.try_into().unwrap_or(50_000_000_000)), // Use higher gas price
        nonce: Some(current_nonce), // Use actual account nonce
    };

    println!("🚀 Executing buy transaction...");

    // Execute buy transaction - returns tx_hash immediately
    let tx_hash = core.buy(buy_params, router).await?;
    println!("✅ Transaction submitted!");
    println!("  Transaction hash: {}", tx_hash);

    // Wait for transaction receipt
    println!("⏳ Waiting for confirmation...");
    let receipt = core.get_receipt(tx_hash).await?;

    if receipt.status {
        println!("✅ Buy successful!");
        println!("  Block number: {:?}", receipt.block_number);
        println!("  Gas used: {:?}", receipt.gas_used);
    } else {
        println!("❌ Buy failed!");
    }

    Ok(())
}
