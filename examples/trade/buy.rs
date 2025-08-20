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
use nadfun_sdk::{GasEstimationParams, SlippageUtils, Trade};

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
    // Create Trade instance
    let trade = Trade::new(config.rpc_url, private_key).await?;

    // Get wallet address from trade instance
    let wallet = trade.wallet_address();

    // Check MON balance
    let mon_balance = trade
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

    let (router, amount_out) = trade.get_amount_out(token, mon_amount, true).await?;
    println!("router: {:?}", router);
    println!("amount_out: {}", amount_out);
    let slippage_percent = 5.0;
    let amount_out_min = SlippageUtils::calculate_amount_out_min(amount_out, slippage_percent);

    // Get current account nonce
    let current_nonce = trade
        .provider()
        .get_transaction_count(wallet)
        .block_id(BlockId::latest())
        .await?;
    println!("📊 Current account nonce: {}", current_nonce);

    // Get current network gas price
    let network_gas_price_raw = trade.provider().get_gas_price().await?;
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

    let estimated_gas = match trade.estimate_gas(&router, gas_params).await {
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

    // Apply 5% slippage protection

    println!("🛡️ Slippage protection:");
    println!("  Expected tokens: {}", amount_out);
    println!(
        "  Minimum tokens ({}% slippage): {}",
        slippage_percent, amount_out_min
    );

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

    println!(" Executing buy transaction...");

    // Execute buy transaction
    let result = trade.buy(buy_params, router).await?;

    if result.status {
        println!("✅ Buy successful!");
        println!("  Transaction hash: {}", result.transaction_hash);
        println!("  Block number: {:?}", result.block_number);
        println!("  Gas used: {:?}", result.gas_used);
    } else {
        println!("❌ Buy failed!");
        println!("  Transaction hash: {}", result.transaction_hash);
    }

    Ok(())
}
