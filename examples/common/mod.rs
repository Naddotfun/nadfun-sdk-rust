//! Common utilities for examples

use anyhow::Result;
use std::env;

/// Get configuration from environment variables or defaults
pub struct Config {
    pub rpc_url: String,
    pub ws_url: String,
    pub private_key: Option<String>,
    pub token: Option<String>,
    pub recipient: Option<String>,
    pub tokens: Vec<String>,
}

impl Config {
    /// Load configuration from environment variables with defaults
    pub fn from_env() -> Self {
        let tokens = env::var("TOKENS")
            .map(|s| parse_tokens_argument(&s).unwrap_or_default())
            .unwrap_or_default();

        Self {
            rpc_url: env::var("RPC_URL").unwrap_or_else(|_| "https://eth.merkle.io".to_string()),
            ws_url: env::var("WS_URL").unwrap_or_else(|_| "wss://eth.merkle.io".to_string()),
            private_key: env::var("PRIVATE_KEY").ok(),
            token: env::var("TOKEN").ok(),
            recipient: env::var("RECIPIENT").ok(),
            tokens,
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
                    println!("  --help, -h           Show this help");
                    println!();
                    println!("Environment variables:");
                    println!("  RPC_URL       Override default RPC URL");
                    println!("  WS_URL        Override default WebSocket URL");
                    println!("  PRIVATE_KEY   Set private key");
                    println!("  TOKEN         Set token address");
                    println!("  TOKENS        Set token addresses (comma-separated or JSON array)");
                    println!("  RECIPIENT     Set recipient address");
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
