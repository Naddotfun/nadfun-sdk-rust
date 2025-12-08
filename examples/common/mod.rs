//! Common utilities for examples

use anyhow::Result;
use nadfun_sdk::Network;
use std::env;

/// Get configuration from environment variables or defaults
pub struct Config {
    pub rpc_url: String,
    pub ws_url: String,
    pub private_key: Option<String>,
    pub token: Option<String>,
    pub recipient: Option<String>,
    pub tokens: Vec<String>,
    pub network: Network,
    // Token creation fields
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub description: Option<String>,
    pub image_uri: Option<String>,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub initial_buy: Option<String>, // Amount of MON for initial buy (e.g., "1.5")
}

impl Config {
    /// Load configuration from environment variables with defaults
    pub fn from_env() -> Self {
        let tokens = env::var("TOKENS")
            .map(|s| parse_tokens_argument(&s).unwrap_or_default())
            .unwrap_or_default();

        let network = env::var("NETWORK")
            .map(|s| parse_network(&s).unwrap_or_default())
            .unwrap_or_default();

        Self {
            rpc_url: env::var("RPC_URL").unwrap_or_else(|_| "https://eth.merkle.io".to_string()),
            ws_url: env::var("WS_URL").unwrap_or_else(|_| "wss://eth.merkle.io".to_string()),
            private_key: env::var("PRIVATE_KEY").ok(),
            token: env::var("TOKEN").ok(),
            recipient: env::var("RECIPIENT").ok(),
            tokens,
            network,
            // Token creation fields from env
            name: env::var("NAME").ok(),
            symbol: env::var("SYMBOL").ok(),
            description: env::var("DESCRIPTION").ok(),
            image_uri: env::var("IMAGE_URI").ok(),
            website: env::var("WEBSITE").ok(),
            twitter: env::var("TWITTER").ok(),
            telegram: env::var("TELEGRAM").ok(),
            initial_buy: env::var("INITIAL_BUY").ok(),
        }
    }

    /// Load configuration from command line arguments
    pub fn from_args() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        let mut config = Self::from_env();

        // Simple argument parsing
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--rpc-url" => {
                    if i + 1 < args.len() {
                        config.rpc_url = args[i + 1].clone();
                        i += 2;
                    } else {
                        anyhow::bail!("--rpc-url requires a value");
                    }
                }
                "--ws-url" => {
                    if i + 1 < args.len() {
                        config.ws_url = args[i + 1].clone();
                        i += 2;
                    } else {
                        anyhow::bail!("--ws-url requires a value");
                    }
                }
                "--private-key" => {
                    if i + 1 < args.len() {
                        config.private_key = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--private-key requires a value");
                    }
                }
                "--token" => {
                    if i + 1 < args.len() {
                        config.token = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--token requires a value");
                    }
                }
                "--recipient" => {
                    if i + 1 < args.len() {
                        config.recipient = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--recipient requires a value");
                    }
                }
                "--tokens" => {
                    if i + 1 < args.len() {
                        config.tokens = parse_tokens_argument(&args[i + 1])?;
                        i += 2;
                    } else {
                        anyhow::bail!("--tokens requires a value");
                    }
                }
                "--network" => {
                    if i + 1 < args.len() {
                        config.network = parse_network(&args[i + 1])?;
                        i += 2;
                    } else {
                        anyhow::bail!("--network requires a value");
                    }
                }
                "--name" => {
                    if i + 1 < args.len() {
                        config.name = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--name requires a value");
                    }
                }
                "--symbol" => {
                    if i + 1 < args.len() {
                        config.symbol = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--symbol requires a value");
                    }
                }
                "--description" => {
                    if i + 1 < args.len() {
                        config.description = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--description requires a value");
                    }
                }
                "--image-uri" => {
                    if i + 1 < args.len() {
                        config.image_uri = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--image-uri requires a value");
                    }
                }
                "--website" => {
                    if i + 1 < args.len() {
                        config.website = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--website requires a value");
                    }
                }
                "--twitter" => {
                    if i + 1 < args.len() {
                        config.twitter = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--twitter requires a value");
                    }
                }
                "--telegram" => {
                    if i + 1 < args.len() {
                        config.telegram = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--telegram requires a value");
                    }
                }
                "--initial-buy" => {
                    if i + 1 < args.len() {
                        config.initial_buy = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--initial-buy requires a value");
                    }
                }
                "--help" | "-h" => {
                    println!("Usage: cargo run --example <example> [OPTIONS]");
                    println!();
                    println!("Options:");
                    println!("  --rpc-url <URL>      RPC URL (default: https://eth.merkle.io)");
                    println!("  --ws-url <URL>       WebSocket URL (default: wss://eth.merkle.io)");
                    println!("  --private-key <KEY>  Private key for transactions");
                    println!("  --token <ADDRESS>    Token address for operations");
                    println!("  --tokens <ADDRS>     Token addresses: 'addr1,addr2' or '[\"addr1\",\"addr2\"]'");
                    println!("  --recipient <ADDR>   Recipient address for transfers/allowances");
                    println!("  --network <NET>      Network: mainnet or testnet (default: mainnet)");
                    println!();
                    println!("Token creation options:");
                    println!("  --name <NAME>        Token name");
                    println!("  --symbol <SYMBOL>    Token symbol");
                    println!("  --description <DESC> Token description");
                    println!("  --image-uri <URL>    Image URL (required for token creation)");
                    println!("  --initial-buy <MON>  MON amount for initial buy (default: 1.5)");
                    println!("  --website <URL>      Website URL (must use https://)");
                    println!("  --twitter <URL>      Twitter/X URL (must use https://x.com)");
                    println!("  --telegram <URL>     Telegram URL (must use https://t.me)");
                    println!();
                    println!("  --help, -h           Show this help");
                    println!();
                    println!("Environment variables:");
                    println!("  RPC_URL       Override default RPC URL");
                    println!("  WS_URL        Override default WebSocket URL");
                    println!("  PRIVATE_KEY   Set private key");
                    println!("  TOKEN         Set token address");
                    println!("  TOKENS        Set token addresses (comma-separated or JSON array)");
                    println!("  RECIPIENT     Set recipient address");
                    println!("  NETWORK       Set network (mainnet or testnet)");
                    println!();
                    println!("Token creation environment variables:");
                    println!("  NAME          Token name");
                    println!("  SYMBOL        Token symbol");
                    println!("  DESCRIPTION   Token description");
                    println!("  IMAGE_URI     Image URL");
                    println!("  INITIAL_BUY   MON amount for initial buy");
                    println!("  WEBSITE       Website URL");
                    println!("  TWITTER       Twitter URL");
                    println!("  TELEGRAM      Telegram URL");
                    std::process::exit(0);
                }
                _ => i += 1,
            }
        }

        Ok(config)
    }

    /// Get private key or return error if not provided
    pub fn require_private_key(&self) -> Result<String> {
        match &self.private_key {
            Some(key) => Ok(key.clone()),
            None => {
                eprintln!("⚠️  Private key required for this operation.");
                eprintln!("   Set it with: --private-key YOUR_KEY");
                eprintln!("   Or use environment variable: export PRIVATE_KEY=YOUR_KEY");
                anyhow::bail!("Private key required");
            }
        }
    }

    /// Print configuration
    pub fn print(&self) {
        println!("📋 Configuration:");
        println!("  RPC URL: {}", self.rpc_url);
        println!("  WS URL: {}", self.ws_url);
        println!("  Network: {:?}", self.network);
        println!(
            "  Private Key: {}",
            if self.private_key.is_some() {
                "✅ Provided"
            } else {
                "❌ Not provided"
            }
        );
        println!(
            "  Token: {}",
            if let Some(ref token) = self.token {
                token
            } else {
                "❌ Not provided"
            }
        );
        println!(
            "  Tokens: {}",
            if self.tokens.is_empty() {
                "❌ Not provided".to_string()
            } else {
                self.tokens.join(", ")
            }
        );
        println!(
            "  Recipient: {}",
            if let Some(ref recipient) = self.recipient {
                recipient
            } else {
                "❌ Not provided"
            }
        );
        println!();
    }
}

/// Parse network argument
fn parse_network(input: &str) -> Result<Network> {
    match input.trim().to_lowercase().as_str() {
        "mainnet" => Ok(Network::Mainnet),
        "testnet" => Ok(Network::Testnet),
        _ => anyhow::bail!("Invalid network: {}. Use 'mainnet' or 'testnet'", input),
    }
}

/// Parse tokens argument - supports both comma-separated and JSON array formats
fn parse_tokens_argument(input: &str) -> Result<Vec<String>> {
    let trimmed = input.trim();

    // Check if it's a JSON array format like ["addr1", "addr2"]
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let content = &trimmed[1..trimmed.len() - 1]; // Remove [ and ]
        return Ok(content
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|s| !s.is_empty())
            .collect());
    }

    // Default: comma-separated format
    Ok(input
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}
