//! Basic ERC20 token operations example
//!
//! This example demonstrates:
//! 1. Token metadata retrieval (name, symbol, decimals, total supply)
//! 2. Balance checking
//! 3. Allowance management
//! 4. Token transfers
//! 5. Basic error handling
//!
//! ## Usage
//!
//! ```bash
//! # Using environment variables
//! export PRIVATE_KEY="your_private_key_here"
//! export RPC_URL="https://your-rpc-url"
//! export TOKEN="0xTokenAddress"
//! export RECIPIENT="0xRecipientAddress"
//! cargo run --example basic_operations
//!
//! # Using command line arguments
//! cargo run --example basic_operations -- --private-key your_private_key_here --rpc-url https://your-rpc-url --token 0xTokenAddress --recipient 0xRecipientAddress
//! ```

use alloy::primitives::{Address, U256, utils::parse_ether};
use anyhow::Result;
use nadfun_sdk::TokenHelper;

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    let private_key = config.require_private_key()?;

    // Token address from CLI or environment
    let token: Address = match config.token {
        Some(token_address) => token_address.parse()?,
        None => {
            eprintln!("⚠️  Token address required for this operation.");
            eprintln!("   Set it with: --token TOKEN_ADDRESS");
            eprintln!("   Or use environment variable: export TOKEN=TOKEN_ADDRESS");
            anyhow::bail!("Token address required");
        }
    };

    // Recipient address for transfer/allowance examples
    let recipient: Address = match config.recipient {
        Some(recipient_address) => recipient_address.parse()?,
        None => {
            eprintln!("⚠️  Recipient address required for this operation.");
            eprintln!("   Set it with: --recipient RECIPIENT_ADDRESS");
            eprintln!("   Or use environment variable: export RECIPIENT=RECIPIENT_ADDRESS");
            anyhow::bail!("Recipient address required");
        }
    };

    // Create TokenHelper instance
    let token_helper = TokenHelper::new(config.rpc_url, private_key).await?;

    // Get wallet address from token helper
    let wallet = token_helper.wallet_address();

    println!("🪙 Token Helper Basic Operations Demo");
    println!("Wallet: {}", wallet);
    println!("Token: {}", token);
    println!("Recipient: {}", recipient);
    println!();

    // 1. Get token metadata
    println!("📋 Getting token metadata...");
    let metadata = token_helper.get_token_metadata(token).await?;

    println!("  Name: {}", metadata.name);
    println!("  Symbol: {}", metadata.symbol);
    println!("  Decimals: {}", metadata.decimals);
    println!("  Total Supply: {}", metadata.total_supply);
    println!();

    // 2. Check balances
    println!("💰 Checking balances...");
    let wallet_balance = token_helper.balance_of(token, wallet).await?;
    let recipient_balance = token_helper.balance_of(token, recipient).await?;

    println!("  Wallet balance: {}", wallet_balance);
    println!("  Recipient balance: {}", recipient_balance);
    println!();

    // 3. Check allowances
    println!("🔐 Checking allowances...");
    let allowance_to_recipient = token_helper.allowance(token, wallet, recipient).await?;

    println!(
        "  Allowance (wallet → recipient): {}",
        allowance_to_recipient
    );
    println!();

    // 4. Approve tokens (if wallet has balance)
    if wallet_balance > U256::ZERO {
        let approve_amount = parse_ether("100")?; // Approve 100 tokens

        if wallet_balance >= approve_amount {
            println!("✅ Approving {} tokens to recipient...", approve_amount);

            let approve_tx = token_helper
                .approve(token, recipient, approve_amount)
                .await?;
            println!("  Approval transaction: {}", approve_tx);

            // Wait for transaction to be mined
            println!("  ⏳ Waiting for approval to be mined...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            // Check updated allowance
            let updated_allowance = token_helper.allowance(token, wallet, recipient).await?;
            println!("  Updated allowance: {}", updated_allowance);
            println!();
        } else {
            println!("  ⚠️  Insufficient balance for approval demo");
            println!("  Required: {}", approve_amount);
            println!("  Available: {}", wallet_balance);
            println!();
        }
    } else {
        println!("  ⚠️  Wallet has no tokens - skipping approval demo");
        println!();
    }

    // 5. Transfer tokens (if wallet has balance)
    if wallet_balance > U256::ZERO {
        let transfer_amount = std::cmp::min(wallet_balance / U256::from(10), parse_ether("10")?); // Transfer 10% or 10 tokens max

        if transfer_amount > U256::ZERO {
            println!("💸 Transferring {} tokens to recipient...", transfer_amount);

            let transfer_tx = token_helper
                .transfer(token, recipient, transfer_amount)
                .await?;
            println!("  Transfer transaction: {}", transfer_tx);

            // Wait for transaction to be mined
            println!("  ⏳ Waiting for transfer to be mined...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            // Check updated balances
            let new_wallet_balance = token_helper.balance_of(token, wallet).await?;
            let new_recipient_balance = token_helper.balance_of(token, recipient).await?;

            println!("  Updated balances:");
            println!(
                "    Wallet: {} (was {})",
                new_wallet_balance, wallet_balance
            );
            println!(
                "    Recipient: {} (was {})",
                new_recipient_balance, recipient_balance
            );
            println!();
        } else {
            println!("  ⚠️  Transfer amount too small - skipping transfer demo");
            println!();
        }
    } else {
        println!("  ⚠️  Wallet has no tokens - skipping transfer demo");
        println!();
    }

    // 6. Demonstrate transfer_from (if there's allowance)
    let final_allowance = token_helper.allowance(token, wallet, recipient).await?;
    if final_allowance > U256::ZERO {
        // For this demo, we'd need the recipient's private key to call transfer_from
        // This is just to show the API exists
        println!("📤 TransferFrom demo:");
        println!("  Available allowance: {}", final_allowance);
        println!("  ℹ️  transferFrom requires the spender's private key");
        println!("  ℹ️  In practice, this would be called by a smart contract or approved spender");
        println!();
    }

    println!("✅ Token operations demo completed!");

    Ok(())
}
