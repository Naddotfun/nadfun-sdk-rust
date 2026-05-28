//! Example: Claim creator rewards from CreatorTreasury
//!
//! This example shows how to:
//! 1. Query created tokens and their reward info from API
//! 2. Build claim parameters from reward_info
//! 3. Execute claim transaction on CreatorTreasury
//!
//! Usage:
//! ```bash
//! cargo run --example claim_reward -- \
//!   --private-key $PRIVATE_KEY \
//!   --rpc-url $RPC_URL \
//!   --network testnet
//! ```

use anyhow::Result;
use clap::Parser;
use nadfun_sdk::prelude::*;

#[derive(Parser, Debug)]
#[command(name = "claim_reward")]
#[command(about = "Claim creator rewards from CreatorTreasury")]
struct Args {
    #[arg(long, env = "PRIVATE_KEY")]
    private_key: String,

    #[arg(long, env = "RPC_URL")]
    rpc_url: String,

    #[arg(long, default_value = "testnet")]
    network: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let network = if args.network == "mainnet" {
        Network::Mainnet
    } else {
        Network::Testnet
    };

    let core = Core::new(args.rpc_url, args.private_key, network).await?;

    println!("Wallet: {}", core.wallet_address());
    println!("Network: {:?}", core.network());

    // 1. Get created tokens and their reward info from API
    let api = ApiClient::new(network);
    // Or with API key for higher rate limits:
    // let api = ApiClient::new(network).with_api_key("your-api-key".to_string());
    let response = api.get_created_tokens(core.wallet_address(), 1, 10).await?;

    println!("\nFound {} tokens", response.total_count);

    // 2. Find claimable tokens
    let claimable_tokens: Vec<_> = response
        .tokens
        .iter()
        .filter(|t| t.reward_info.claimable)
        .collect();

    if claimable_tokens.is_empty() {
        println!("No claimable rewards found");
        return Ok(());
    }

    println!("\nClaimable rewards:");
    for token in &claimable_tokens {
        println!(
            "  Token: {} ({})",
            token.token_info.name, token.token_info.token_id
        );
        println!("  Amount: {} wei", token.reward_info.amount);
        println!("  Proof length: {}", token.reward_info.proof.len());
        println!();
    }

    // 3. Claim rewards individually
    for token in claimable_tokens {
        if let Some(params) = ApiClient::build_claim_params(token) {
            println!("Claiming reward for: {}", token.token_info.name);

            let tx_hash = core.claim_creator_reward(params).await?;
            println!("TX submitted: {}", tx_hash);

            // Wait for transaction to be mined
            println!("Waiting for confirmation...");
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            match core.get_receipt(tx_hash).await {
                Ok(receipt) => {
                    println!(
                        "Status: {}",
                        if receipt.status { "Success" } else { "Failed" }
                    );
                }
                Err(_) => {
                    println!("Receipt not yet available. Check explorer for status.");
                }
            }
            println!();
        }
    }

    // Alternative: Batch claim all rewards at once (more gas efficient)
    // if let Some(batch_params) = ApiClient::build_batch_claim_params(&response.tokens) {
    //     println!("Batch claiming {} tokens", batch_params.tokens.len());
    //     let tx_hash = core.claim_creator_rewards_batch(batch_params).await?;
    //     println!("Batch claim TX: {}", tx_hash);
    //
    //     let receipt = core.get_receipt(tx_hash).await?;
    //     println!("Status: {}", if receipt.status { "Success" } else { "Failed" });
    // }

    Ok(())
}
