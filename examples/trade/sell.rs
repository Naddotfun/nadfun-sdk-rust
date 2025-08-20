//! Sell token example with slippage protection and gas optimization
//!
//! This example demonstrates:
//! 1. Getting quote for token sale
//! 2. Calculating slippage protection (amount_out_min)
//! 3. Approving token spending (if needed)
//! 4. Configuring optimized gas parameters for sell transactions
//! 5. Executing sell transaction
//! 6. Verifying transaction success
//!
//! ## Usage
//!
//! ```bash
//! # Using environment variables
//! export PRIVATE_KEY="your_private_key_here"
//! export RPC_URL="https://your-rpc-url"
//! export TOKEN="0xTokenAddress"
//! cargo run --example sell
//!
//! # Using command line arguments
//! cargo run --example sell -- --private-key your_private_key_here --rpc-url https://your-rpc-url --token 0xTokenAddress
//! ```

use alloy::eips::BlockId;
use alloy::primitives::{utils::parse_ether, Address, U256};
use alloy::providers::Provider;
use anyhow::Result;
use nadfun_sdk::types::SellParams;
use nadfun_sdk::{GasEstimationParams, SlippageUtils, TokenHelper, Trade};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    let private_key = config.require_private_key()?;

    // Token to sell
    let token: Address = match config.token {
        Some(token_address) => token_address.parse()?,
        None => {
            eprintln!("⚠️  Token address required for this operation.");
            eprintln!("   Set it with: --token TOKEN_ADDRESS");
            eprintln!("   Or use environment variable: export TOKEN=TOKEN_ADDRESS");
            anyhow::bail!("Token address required");
        }
    };

    // Amount of tokens to sell (0.001 token - very small test)
    let token_amount = parse_ether("1")?;

    // Slippage protection (5%)
    let slippage_percent = 5.0;

    // Create Trade and TokenHelper instances
    let trade = Trade::new(config.rpc_url.clone(), private_key.clone()).await?;
    let token_helper = TokenHelper::new(config.rpc_url, private_key).await?;

    // Get wallet address from trade instance
    let wallet = trade.wallet_address();

    // Get quote: how much ETH we'll receive
    let (router, expected_eth) = trade.get_amount_out(token, token_amount, false).await?;

    println!("📊 Quote:");
    println!("  Tokens to sell: {}", token_amount);
    println!(
        "  Expected ETH: {} ETH",
        alloy::primitives::utils::format_ether(expected_eth)
    );
    println!("  Router: {:?}", router);

    // Use 95% of expected amount as minimum (5% slippage)
    let min_eth = SlippageUtils::calculate_amount_out_min(expected_eth, slippage_percent);

    println!("🛡️  Slippage protection:");
    println!("  Slippage tolerance: {}%", slippage_percent);
    println!(
        "  Expected ETH: {} ETH",
        alloy::primitives::utils::format_ether(expected_eth)
    );
    println!(
        "  Minimum ETH: {} ETH",
        alloy::primitives::utils::format_ether(min_eth)
    );

    // Check current allowance
    let current_allowance = token_helper
        .allowance(token, wallet, router.address())
        .await?;
    println!("current_allowance: {}", current_allowance);
    if current_allowance < token_amount {
        println!("📝 Approving token spending...");

        // Approve token spending
        let approve_tx = token_helper
            .approve(token, router.address(), token_amount)
            .await?;
        println!("  Approval tx: {}", approve_tx);

        // Wait a bit for approval to be mined
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }

    // Set deadline (5 minutes from now)
    let deadline = U256::from(9999999999999999u64);

    println!("⏰ Deadline: {}", deadline);

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

    // Use new unified gas estimation system
    let gas_params = GasEstimationParams::Sell {
        token,
        amount_in: token_amount,
        amount_out_min: min_eth,
        to: wallet,
        deadline,
    };

    let estimated_gas = match trade.estimate_gas(&router, gas_params).await {
        Ok(gas) => {
            println!("⛽ Estimated gas for sell: {}", gas);
            gas
        }
        Err(e) => {
            println!("⚠️ Gas estimation failed: {}", e);
            println!("⛽ Using fallback gas limit: 200000");
            200000
        }
    };

    // Add 15% buffer to estimated gas
    let gas_with_buffer = estimated_gas * 115 / 100;
    println!("⛽ Gas with 15% buffer: {}", gas_with_buffer);

    // Prepare sell parameters with minimal amountOutMin for testing
    let sell_params = SellParams {
        amount_in: token_amount,
        amount_out_min: min_eth, // 1 wei to eliminate slippage issues
        token,
        to: wallet,
        deadline,
        gas_limit: Some(gas_with_buffer), // Use estimated gas with buffer
        gas_price: Some(recommended_gas_price.try_into().unwrap_or(50_000_000_000)), // Use higher gas price
        nonce: Some(current_nonce), // Use actual account nonce
    };

    println!("📝 Sell params:");
    println!("  Token: {}", token);
    println!("  Amount in: {}", token_amount);
    println!("  Min amount out: {}", min_eth);
    println!("  To: {}", wallet);
    println!("  Deadline: {}", deadline);

    println!("🚀 Executing sell transaction...");

    // Execute sell transaction
    let result = trade.sell(sell_params, router).await?;

    if result.status {
        println!("✅ Sell successful!");
        println!("  Transaction hash: {}", result.transaction_hash);
        println!("  Block number: {:?}", result.block_number);
        println!("  Gas used: {:?}", result.gas_used);
    } else {
        println!("❌ Sell failed!");
        println!("  Transaction hash: {}", result.transaction_hash);
    }

    Ok(())
}
