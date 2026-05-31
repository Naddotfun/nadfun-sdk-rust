//! Deploy a v2 token via NadFunRouter — end-to-end flow (image upload +
//! metadata + salt + on-chain create with native MON).

use alloy::primitives::{utils::parse_ether, Address, U256};
use anyhow::Result;
use nadfun_sdk::{
    ApiClient, Core, GasPricing, V2CreatePayment, V2CreateTokenParams, V2DexType, V2VaultAllocation,
};

#[path = "../common/mod.rs"]
mod common;
use common::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_args()?;
    config.print();

    let private_key = config.require_private_key()?;
    let core = Core::new(config.rpc_url, private_key, config.network).await?;
    let creator = core.wallet_address();

    // Resolve vault addresses from constants (allows the example to run
    // against testnet or mainnet without re-typing addresses).
    let burn_vault: Address = nadfun_sdk::constants::get_burn_vault_v2(config.network)
        .ok_or_else(|| anyhow::anyhow!("BurnVault not configured for this network"))?
        .parse()?;
    let lp_vault: Address = nadfun_sdk::constants::get_lp_vault_v2(config.network)
        .ok_or_else(|| anyhow::anyhow!("LPVault not configured for this network"))?
        .parse()?;

    // Native create needs an explicit native-equivalent quote token. Resolve
    // the wrapped native (MON / WMON) from the structured quote-token registry
    // — any entry with `is_native == true` is valid (e.g. MON or LVMON). You
    // could also use `core.v2().wrapped_native().await?`.
    let wmon: Address = nadfun_sdk::quote_tokens(config.network)
        .iter()
        .find(|qt| qt.is_native && qt.symbol == "MON")
        .ok_or_else(|| anyhow::anyhow!("no native MON quote token for this network"))?
        .address
        .parse()?;

    let api = ApiClient::from_env(config.network);

    let initial_buy = parse_ether("1.5")?; // 1.5 MON for the creator's initial buy

    let params = V2CreateTokenParams {
        name: config.name.unwrap_or_else(|| "Rocket Pepe".to_string()),
        symbol: config.symbol.unwrap_or_else(|| "RPEPE".to_string()),
        description: config
            .description
            .unwrap_or_else(|| "Demo v2 token".to_string()),
        image_uri: config
            .image_uri
            .unwrap_or_else(|| "https://i.imgur.com/0qY8Vp6.png".to_string()),
        website: config.website,
        twitter: config.twitter,
        telegram: config.telegram,
        creator_address: creator,
        creator_fee_rate: 100, // 1.00% in BPS
        // Sample 50/50 vault split — burn half of the creator fees, route
        // the other half to LP.
        vaults: vec![
            V2VaultAllocation {
                vault: burn_vault,
                bps: 5_000,
                setup_data: Default::default(),
            },
            V2VaultAllocation {
                vault: lp_vault,
                bps: 5_000,
                setup_data: Default::default(),
            },
        ],
        dex_type: V2DexType::NadFun,
        buy_quote_amount: initial_buy,
        payment: V2CreatePayment::Native { quote_token: wmon },
        deadline: U256::from(9_999_999_999_u64),
        gas_limit: None,
        gas_price: Some(GasPricing::Legacy),
        nonce: None,
    };

    let result = core.v2().create_token(params, &api).await?;
    println!("✅ token deployed: {}", result.token_address);
    println!("metadata_uri: {}", result.metadata_uri);
    println!("image_uri:    {}", result.image_uri);
    println!("salt:         {:?}", result.salt);
    println!("tx:           {}", result.transaction_hash);
    println!("NSFW:         {}", result.is_nsfw);

    let receipt = core.get_receipt(result.transaction_hash).await?;
    println!(
        "status: {}, block: {:?}",
        receipt.status, receipt.block_number
    );

    Ok(())
}
