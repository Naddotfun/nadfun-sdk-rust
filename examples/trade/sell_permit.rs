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
use alloy::rpc::types::TransactionRequest;
use anyhow::Result;
use nadfun_sdk::types::SellPermitParams;
use nadfun_sdk::{get_default_gas_limit, Operation, TokenHelper, Trade};
use nadfun_sdk::{IBondingCurveRouter, IDexRouter};

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

    // Create Trade and TokenHelper instances
    let trade = Trade::new(config.rpc_url.clone(), private_key.clone()).await?;
    let token_helper = TokenHelper::new(config.rpc_url, private_key).await?;

    // Get wallet address from trade instance
    let wallet = trade.wallet_address();

    // Check token balance
    let balance = token_helper.balance_of(token, wallet).await?;
    if balance < token_amount {
        println!("❌ Insufficient token balance");
        println!("  Required: {}", token_amount);
        println!("  Available: {}", balance);
        return Ok(());
    }

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
    let current_nonce = trade
        .provider()
        .get_transaction_count(wallet)
        .block_id(BlockId::latest())
        .await?;
    println!("📊 Current account nonce: {}", current_nonce);

    // Create actual contract call data for gas estimation
    let estimated_gas = match &router {
        nadfun_sdk::trading::Router::BondingCurve(_) => {
            let contract_params = IBondingCurveRouter::SellPermitParams {
                amountIn: token_amount,
                amountOutMin: min_eth,
                amountAllowance: token_amount,
                token,
                to: wallet,
                deadline,
                v,
                r,
                s,
            };
            let contract = IBondingCurveRouter::new(router.address(), trade.provider().as_ref());
            let call_builder = contract.sellPermit(contract_params.clone());
            let call_data = call_builder.calldata();

            trade
                .provider()
                .estimate_gas(
                    TransactionRequest::default()
                        .to(router.address())
                        .from(wallet)
                        .input(call_data.clone().into()),
                )
                .await?
        }
        nadfun_sdk::trading::Router::Dex(_) => {
            let contract_params = IDexRouter::SellPermitParams {
                amountIn: token_amount,
                amountOutMin: min_eth,
                amountAllowance: token_amount,
                token,
                to: wallet,
                deadline,
                v,
                r,
                s,
            };
            let contract = IDexRouter::new(router.address(), trade.provider().as_ref());
            let call_builder = contract.sellPermit(contract_params.clone());
            let call_data = call_builder.calldata();

            trade
                .provider()
                .estimate_gas(
                    TransactionRequest::default()
                        .to(router.address())
                        .from(wallet)
                        .input(call_data.clone().into()),
                )
                .await?
        }
    };
    println!(
        "⛽ Estimated gas for sellPermit contract call: {}",
        estimated_gas
    );
    println!(
        "⛽ Using default gas limit: {}",
        get_default_gas_limit(&router, Operation::SellPermit)
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
        gas_limit: Some(get_default_gas_limit(&router, Operation::SellPermit)), // Use default gas limits with buffer included
        gas_price: Some(50_000_000_000), // 50 gwei gas price (higher for complex tx)
        nonce: Some(current_nonce),      // Use actual account nonce
    };

    println!("🚀 Executing gasless sell transaction...");
    println!("  This combines approval + sell in one transaction!");

    // Execute sell permit transaction (gasless)
    let result = trade.sell_permit(sell_permit_params, router).await?;

    if result.status {
        println!("✅ Gasless sell successful!");
        println!("  Transaction hash: {}", result.transaction_hash);
        println!("  Block number: {:?}", result.block_number);
        println!("  Gas used: {:?}", result.gas_used);
        println!("  💡 Saved gas by combining approval + sell in one tx!");
    } else {
        println!("❌ Gasless sell failed!");
        println!("  Transaction hash: {}", result.transaction_hash);
    }

    Ok(())
}
