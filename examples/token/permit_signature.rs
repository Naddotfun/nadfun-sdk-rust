//! EIP-2612 permit signature generation example
//!
//! This example demonstrates:
//! 1. Getting token permit-related data (nonce, domain separator)
//! 2. Generating EIP-2612 permit signatures
//! 3. Understanding permit signature components
//! 4. Manual domain separator calculation
//! 5. Permit signature verification concepts
//!
//! ## Usage
//!
//! ```bash
//! # Using environment variables
//! export PRIVATE_KEY="your_private_key_here"
//! export RPC_URL="https://your-rpc-url"
//! export TOKEN="0xTokenAddress"
//! cargo run --example permit_signature
//!
//! # Using command line arguments
//! cargo run --example permit_signature -- --private-key your_private_key_here --rpc-url https://your-rpc-url --token 0xTokenAddress
//! ```

use alloy::primitives::{Address, U256, utils::parse_ether};

use anyhow::Result;
use nadfun_sdk::TokenHelper;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    let private_key = config.require_private_key()?;

    // Token that supports EIP-2612 permits
    let token: Address = match config.token {
        Some(token_address) => token_address.parse()?,
        None => {
            eprintln!("⚠️  Token address required for this operation.");
            eprintln!("   Set it with: --token TOKEN_ADDRESS");
            eprintln!("   Or use environment variable: export TOKEN=TOKEN_ADDRESS");
            anyhow::bail!("Token address required");
        }
    };

    // Spender address (e.g., a DEX router or smart contract)
    let spender: Address = "0x9876543210987654321098765432109876543210".parse()?;

    // Amount to approve via permit
    let approve_amount = parse_ether("1000")?;

    // Create TokenHelper instance
    let token_helper = TokenHelper::new(config.rpc_url, private_key).await?;

    // Get wallet address from token helper
    let wallet = token_helper.wallet_address();

    println!("✍️  EIP-2612 Permit Signature Demo");
    println!("Wallet: {}", wallet);
    println!("Token: {}", token);
    println!("Spender: {}", spender);
    println!("Amount: {}", approve_amount);
    println!();

    // 1. Get token metadata for context
    println!("📋 Getting token information...");
    let metadata = token_helper.get_token_metadata(token).await?;
    println!("  Token: {} ({})", metadata.name, metadata.symbol);
    println!("  Decimals: {}", metadata.decimals);
    println!();

    // 2. Get current nonce for the wallet
    println!("🔢 Getting current nonce...");
    let current_nonce = token_helper.get_nonce(token, wallet).await?;
    println!("  Current nonce: {}", current_nonce);
    println!();

    // 3. Get domain separator
    println!("🏷️  Getting domain separator...");
    let domain_separator = token_helper.get_domain_separator(token).await?;
    println!("  Domain separator: {}", domain_separator);
    println!();

    // 4. Demonstrate manual domain separator calculation
    println!("🔧 Manual domain separator calculation...");
    let manual_domain_separator = token_helper.build_domain_separator(
        &metadata.name,
        token,
        1, // Ethereum mainnet chain ID
    );
    println!("  Manual calculation: {}", manual_domain_separator);

    if domain_separator == manual_domain_separator {
        println!("  ✅ Manual calculation matches on-chain value!");
    } else {
        println!("  ⚠️  Manual calculation differs (might be different chain ID or version)");
    }
    println!();

    // 5. Set deadline (1 hour from now)
    let deadline_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 3600;
    let deadline = U256::from(deadline_timestamp);

    println!("⏰ Setting permit deadline...");
    println!("  Deadline timestamp: {}", deadline_timestamp);
    println!("  Deadline: {}", deadline);
    println!();

    // 6. Generate permit signature
    println!("🔐 Generating permit signature...");
    println!("  This creates a cryptographic signature allowing gasless approvals");

    let (v, r, s) = token_helper
        .generate_permit_signature(token, wallet, spender, approve_amount, deadline)
        .await?;

    println!("  ✅ Permit signature generated!");
    println!("  v: {}", v);
    println!("  r: {}", r);
    println!("  s: {}", s);
    println!();

    // 7. Explain the signature components
    println!("📚 Understanding the signature:");
    println!("  v: Recovery parameter (27 or 28)");
    println!("  r: First 32 bytes of signature");
    println!("  s: Second 32 bytes of signature");
    println!();

    println!("💡 How to use this signature:");
    println!("  1. Include v, r, s in your permit transaction");
    println!("  2. Call permit() on the token contract");
    println!("  3. Or use it with functions like sellPermit() for gasless trading");
    println!();

    // 8. Show what the permit would do
    println!("📋 This permit signature authorizes:");
    println!("  Owner: {}", wallet);
    println!("  Spender: {}", spender);
    println!("  Amount: {}", approve_amount);
    println!("  Deadline: {} (timestamp)", deadline);
    println!("  Nonce: {}", current_nonce);
    println!();

    // 9. Security considerations
    println!("🛡️  Security considerations:");
    println!("  ⚠️  Never share your private key");
    println!("  ⚠️  Verify spender address before signing");
    println!("  ⚠️  Check amount and deadline carefully");
    println!("  ⚠️  Each signature can only be used once (nonce-based)");
    println!("  ⚠️  Signatures expire after deadline");
    println!();

    // 10. Next steps
    println!("🚀 Next steps:");
    println!("  1. Use this signature with a permit-supporting function");
    println!("  2. Or call token.permit(owner, spender, amount, deadline, v, r, s)");
    println!("  3. The approval will be set without requiring a separate transaction");
    println!();

    println!("✅ Permit signature demo completed!");

    Ok(())
}
