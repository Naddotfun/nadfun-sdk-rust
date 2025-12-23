//! Gasless sell example using EIP-2612 permit signatures with advanced gas configuration
//!
//! This example demonstrates:
//! 1. Getting quote for token sale
//! 2. Calculating slippage protection (amount_out_min)  
//! 3. Generating EIP-2612 permit signature (gasless approval)
//! 4. Configuring higher gas parameters for complex permit transactions
//! 5. Executing sell_permit transaction (approval + sell in one tx)
//! 6. Verifying transaction success
//!
//! ## Usage
//!
//! ```bash
//! # Using environment variables
//! export PRIVATE_KEY="your_private_key_here"
//! export RPC_URL="https://your-rpc-url"
//! export TOKEN="0xTokenAddress"
//! cargo run --example sell_permit
//!
//! # Using command line arguments
//! cargo run --example sell_permit -- --private-key your_private_key_here --rpc-url https://your-rpc-url --token 0xTokenAddress
//! ```

use alloy::eips::BlockId;
use alloy::primitives::{utils::parse_ether, Address, U256};
use alloy::providers::Provider;
use anyhow::Result;
use nadfun_sdk::types::{GasPricing, SellPermitParams};
use nadfun_sdk::{Core, GasEstimationParams, TokenHelper};

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

    // Amount of tokens to sell (1000 tokens with 18 decimals)
    let token_amount = parse_ether("1000")?;

    // Slippage protection (5%)
    let slippage_percent = 5.0;

    // Create Core and TokenHelper instances with network
    let core = Core::new(config.rpc_url.clone(), private_key.clone(), config.network).await?;
    let token_helper = TokenHelper::new(config.rpc_url, private_key).await?;

    // Get wallet address from core instance
    let wallet = core.wallet_address();

    // Check token balance
    let balance = token_helper.balance_of(token, wallet).await?;
    if balance < token_amount {
        println!("❌ Insufficient token balance");
        println!("  Required: {}", token_amount);
        println!("  Available: {}", balance);
        return Ok(());
    }

    // Get quote: how much ETH we'll receive
    let (router, expected_eth) = core.get_amount_out(token, token_amount, false).await?;

    println!("📊 Quote:");
    println!("  Tokens to sell: {}", token_amount);
    println!(
        "  Expected ETH: {} ETH",
        alloy::primitives::utils::format_ether(expected_eth)
    );
    println!("  Router: {:?}", router);

    // Use 95% of expected amount as minimum (5% slippage)
    let min_eth = expected_eth * U256::from(95) / U256::from(100);

    println!("🛡️  Slippage protection:");
    println!("  Slippage tolerance: {}%", slippage_percent);
    println!(
        "  Minimum ETH: {} ETH",
        alloy::primitives::utils::format_ether(min_eth)
    );

    // Set deadline (5 minutes from now)
    let deadline = U256::from(9999999999999999u64);

    println!("✍️  Generating permit signature...");

    // Generate permit signature (gasless approval)
    let (v, r, s) = token_helper
        .generate_permit_signature(token, wallet, router.address(), token_amount, deadline)
        .await?;

    println!("  Permit signature generated");
    println!("  v: {}", v);
    println!("  r: {}", r);
    println!("  s: {}", s);
    println!("  💡 Using custom gas settings for permit transaction");

    // Get current account nonce
    let current_nonce = core
        .provider()
        .get_transaction_count(wallet)
        .block_id(BlockId::latest())
        .await?;
    println!("📊 Current account nonce: {}", current_nonce);

    // Use new unified gas estimation system
    let gas_params = GasEstimationParams::SellPermit {
        token,
        amount_in: token_amount,
        amount_out_min: min_eth,
        to: wallet,
        deadline,
        v,
        r: r.into(),
        s: s.into(),
    };

    let estimated_gas = match core.estimate_gas(&router, gas_params).await {
        Ok(gas) => {
            println!("⛽ Estimated gas for sell permit: {}", gas);
            gas
        }
        Err(e) => {
            println!("⚠️ Gas estimation failed: {}", e);
            println!("⛽ Using fallback gas limit: 250000");
            250000
        }
    };

    // Add 25% buffer to estimated gas (permit transactions can be more complex)
    let gas_with_buffer = estimated_gas * 125 / 100;
    println!("⛽ Gas with 25% buffer: {}", gas_with_buffer);

    // Get current network gas price
    let network_gas_price_raw = core.provider().get_gas_price().await?;
    let network_gas_price = U256::from(network_gas_price_raw);
    let recommended_gas_price = network_gas_price * U256::from(300) / U256::from(100); // 3x network gas price
    println!(
        "⛽ Network gas price: {} gwei",
        network_gas_price / U256::from(1_000_000_000)
    );
    println!(
        "⛽ Recommended gas price (3x): {} gwei",
        recommended_gas_price / U256::from(1_000_000_000)
    );

    // Prepare sell permit parameters
    let sell_permit_params = SellPermitParams {
        amount_in: token_amount,
        amount_out_min: min_eth,
        amount_allowance: token_amount, // Allow exactly the amount we're selling
        token,
        to: wallet,
        deadline,
        v,
        r,
        s,
        gas_limit: Some(gas_with_buffer), // Use estimated gas with buffer
        gas_price: Some(GasPricing::LegacyWithPrice {
            gas_price: recommended_gas_price.try_into().unwrap_or(100_000_000_000),
        }),
        nonce: Some(current_nonce), // Use actual account nonce
    };

    println!("🚀 Executing gasless sell transaction...");
    println!("  This combines approval + sell in one transaction!");

    // Execute sell permit transaction (gasless) - returns tx_hash immediately
    let tx_hash = core.sell_permit(sell_permit_params, router).await?;
    println!("✅ Transaction submitted!");
    println!("  Transaction hash: {}", tx_hash);

    // Wait for transaction receipt
    println!("⏳ Waiting for confirmation...");
    let receipt = core.get_receipt(tx_hash).await?;

    if receipt.status {
        println!("✅ Gasless sell successful!");
        println!("  Block number: {:?}", receipt.block_number);
        println!("  Gas used: {:?}", receipt.gas_used);
        println!("  💡 Saved gas by combining approval + sell in one tx!");
    } else {
        println!("❌ Gasless sell failed!");
    }

    Ok(())
}
