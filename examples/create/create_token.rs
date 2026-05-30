//! Create token example with initial buy
//!
//! This example demonstrates the complete token creation flow using Core.create_token():
//! 1. Calculate initial buy amount using Core.get_initial_buy_amount_out()
//! 2. Download image from URI and upload to metadata server
//! 3. Create metadata on server
//! 4. Get salt value from server
//! 5. Execute create transaction on bonding curve with initial buy
//!
//! All steps are handled automatically by the Core.create_token() method.
//!
//! ## Image Requirements
//!
//! **Allowed formats:** JPEG, PNG, WEBP, SVG only (GIF, BMP, TIFF are NOT supported)
//! **Max size:** 5MB
//! **Recommended:** 512x512 or 1024x1024 pixels
//! **NSFW:** Automatically detected and rejected
//!
//! ## Usage
//!
//! ```bash
//! export PRIVATE_KEY="your_private_key_here"
//! export RPC_URL="https://your-rpc-url"
//! export IMAGE_URI="https://i.imgur.com/yourimage.png"
//! cargo run --example create_token
//! ```

use alloy::primitives::utils::parse_ether;
use anyhow::Result;
use nadfun_sdk::{ActionId, ApiClient, Core, CreateTokenParams};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    let private_key = config.require_private_key()?;

    // Create Core instance
    let core = Core::new(config.rpc_url.clone(), private_key, config.network).await?;
    let creator_address = core.wallet_address();

    println!("🚀 Starting token creation flow...");
    println!("👤 Creator address: {}", creator_address);

    // Step 1: Get initial buy amount from CLI or use default
    let initial_buy_str = config.initial_buy.unwrap_or_else(|| "1.5".to_string());
    let initial_buy_mon = parse_ether(&initial_buy_str)?;

    println!("\n📊 Calculating initial buy amount...");
    println!("  Initial buy: {} MON", initial_buy_str);

    let amount_out = core.v1().get_initial_buy_amount_out(initial_buy_mon).await?;
    println!("  Tokens to receive: {}", amount_out);

    // Step 2: Get token creation parameters from CLI or use defaults
    let name = config.name.unwrap_or_else(|| "Test Token".to_string());
    let symbol = config.symbol.unwrap_or_else(|| "TEST".to_string());
    let description = config
        .description
        .unwrap_or_else(|| "A test token created via Rust SDK".to_string());

    // Image URI is required
    let image_uri = match config.image_uri {
        Some(uri) => uri,
        None => {
            eprintln!("❌ Image URI is required for token creation!");
            eprintln!("   Set it with: --image-uri https://your-image-url.png");
            eprintln!(
                "   Or use environment variable: export IMAGE_URI=https://your-image-url.png"
            );
            eprintln!();
            eprintln!("💡 Image Requirements:");
            eprintln!("   - Format: JPEG, PNG, WEBP, or SVG only");
            eprintln!("   - Size: Maximum 5MB");
            eprintln!("   - Recommended dimensions: 512x512 or 1024x1024");
            eprintln!("   - Must be publicly accessible URL");
            eprintln!("   - NSFW images will be automatically rejected");
            eprintln!();
            eprintln!("📌 Supported formats:");
            eprintln!("   ✅ image/jpeg - JPEG/JPG files");
            eprintln!("   ✅ image/png  - PNG files");
            eprintln!("   ✅ image/webp - WebP files");
            eprintln!("   ✅ image/svg+xml - SVG files");
            eprintln!("   ❌ GIF, BMP, TIFF, and other formats are NOT supported");
            anyhow::bail!("Image URI required");
        }
    };

    // Validate and normalize social media URLs (treat empty strings as None)
    let twitter = config.twitter.filter(|s| !s.is_empty()).map(|tw| {
        // Convert twitter.com to x.com if needed
        let normalized = if tw.contains("twitter.com") {
            tw.replace("twitter.com", "x.com")
        } else {
            tw.clone()
        };

        // Validate x.com format
        if !normalized.starts_with("https://") || !normalized.contains("x.com") {
            eprintln!("❌ Invalid Twitter URL: {}", tw);
            eprintln!("   Twitter URLs must:");
            eprintln!("   - Use https://");
            eprintln!("   - Contain x.com (not twitter.com)");
            eprintln!("   Example: https://x.com/mytoken");
            panic!("Invalid Twitter URL format");
        }
        normalized
    });

    let telegram = config.telegram.filter(|s| !s.is_empty()).and_then(|tg| {
        // Validate telegram format
        if !tg.starts_with("https://") || !tg.contains("t.me") {
            eprintln!("❌ Invalid Telegram URL: {}", tg);
            eprintln!("   Telegram URLs must:");
            eprintln!("   - Use https://");
            eprintln!("   - Contain t.me");
            eprintln!("   Example: https://t.me/mytoken");
            return None;
        }
        Some(tg)
    });

    let website = config.website.filter(|s| !s.is_empty()).and_then(|ws| {
        // Validate website format
        if !ws.starts_with("https://") {
            eprintln!("❌ Invalid Website URL: {}", ws);
            eprintln!("   Website URLs must use https://");
            eprintln!("   Example: https://mytoken.com");
            return None;
        }
        Some(ws)
    });

    println!("\n📝 Token Details:");
    println!("  Name: {}", name);
    println!("  Symbol: {}", symbol);
    println!("  Description: {}", description);
    println!("  Image URI: {}", image_uri);
    if let Some(ref ws) = website {
        println!("  Website: {}", ws);
    }
    if let Some(ref tw) = twitter {
        println!("  Twitter/X: {}", tw);
    }
    if let Some(ref tg) = telegram {
        println!("  Telegram: {}", tg);
    }

    let params = CreateTokenParams {
        name,
        symbol,
        description,
        image_uri,
        website,
        twitter,
        telegram,
        creator_address,
        amount_out,                          // Calculated from Lens
        value: initial_buy_mon,              // 1.5 MON
        action_id: ActionId::CapricornActor, // Use CapricornActor (1)
    };

    // Step 3: Create API client (with optional API key for higher rate limits)
    // Without API key, the SDK still works but with lower rate limits
    let api = ApiClient::new(config.network);
    // Or with API key: let api = ApiClient::new(config.network).with_api_key("your-api-key".to_string());

    // Step 4: Execute complete token creation flow (all steps handled automatically)
    println!("\n📋 Creating token with initial buy...");
    let result = core.v1().create_token(params, &api).await?;

    println!("\n🎉 Token created successfully!");
    println!("  Token address: {}", result.token_address);
    println!("  Metadata URI: {}", result.metadata_uri);
    println!("  Image URI: {}", result.image_uri);
    println!("  Salt: {}", result.salt);
    println!("  Transaction hash: {}", result.transaction_hash);
    println!("  Is NSFW: {}", result.is_nsfw);
    println!("  Initial tokens received: {}", amount_out);
    println!("\n🔗 View on Explorer:");
    println!(
        "  Token: https://explorer.example.com/address/{}",
        result.token_address
    );
    println!(
        "  Transaction: https://explorer.example.com/tx/{}",
        result.transaction_hash
    );

    Ok(())
}
