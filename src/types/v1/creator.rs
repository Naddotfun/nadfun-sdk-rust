//! Creator-related types for reward claiming
//!
//! This module contains types for interacting with the CreatorTreasury contract
//! and the API for creator reward information.

use alloy::primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};

use super::GasPricing;

/// API에서 받아오는 reward_info 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardInfo {
    /// Total reward amount in wei
    pub amount: String,
    /// Already claimed amount in wei
    pub claimed_amount: String,
    /// Merkle proof for verification
    pub proof: Vec<String>,
    /// Whether the reward is claimable
    pub claimable: bool,
}

/// Creator Claim parameters for single token
#[derive(Debug, Clone)]
pub struct CreatorClaimParams {
    /// Token address created by the creator
    pub token: Address,
    /// Amount to claim in wei
    pub amount: U256,
    /// Merkle proof for verification
    pub merkle_proof: Vec<B256>,
    /// Optional gas limit
    pub gas_limit: Option<u64>,
    /// Optional gas pricing strategy
    pub gas_price: Option<GasPricing>,
    /// Optional nonce
    pub nonce: Option<u64>,
}

/// Batch Claim parameters for multiple tokens
#[derive(Debug, Clone)]
pub struct CreatorBatchClaimParams {
    /// Token addresses
    pub tokens: Vec<Address>,
    /// Amounts to claim
    pub amounts: Vec<U256>,
    /// Merkle proofs for each token
    pub merkle_proofs: Vec<Vec<B256>>,
    /// Optional gas limit
    pub gas_limit: Option<u64>,
    /// Optional gas pricing strategy
    pub gas_price: Option<GasPricing>,
    /// Optional nonce
    pub nonce: Option<u64>,
}

/// Creator profile attached to an [`ApiTokenInfo`] in v2 API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCreatorInfo {
    pub account_id: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub image_uri: Option<String>,
}

/// API token information.
///
/// Returned by `GET /token/:token` (v2 surface) and embedded in
/// `GET /profile/tokens/created/:account_id` (creator-rewards surface). All
/// fields beyond name/symbol/token_id are tolerated as missing or null so
/// the same struct deserializes against both v1 and v2 response variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTokenInfo {
    pub token_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    #[serde(default, deserialize_with = "deserialize_string_default_empty")]
    pub description: String,
    pub is_graduated: bool,
    #[serde(default)]
    pub is_nsfw: bool,
    #[serde(default, deserialize_with = "deserialize_string_default_empty")]
    pub twitter: String,
    #[serde(default, deserialize_with = "deserialize_string_default_empty")]
    pub telegram: String,
    #[serde(default, deserialize_with = "deserialize_string_default_empty")]
    pub website: String,
    #[serde(default)]
    pub created_at: u64,
    /// v2-only — present on `GET /token/:token` responses. Absent on v1
    /// `CreatedToken.token_info` embeds.
    #[serde(default)]
    pub creator: Option<ApiCreatorInfo>,
    /// v2-only.
    #[serde(default)]
    pub is_cto: bool,
    /// `"V1"` or `"V2"`. Defaults to V1 if the field is missing on the wire
    /// (legacy v1 API responses that pre-date the discriminator).
    #[serde(default)]
    pub version: crate::version::SdkVersion,
}

impl ApiTokenInfo {
    /// Parse `token_id` into an [`alloy::primitives::Address`].
    pub fn token_address(&self) -> anyhow::Result<alloy::primitives::Address> {
        self.token_id
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid token_id {:?}: {}", self.token_id, e))
    }
}

/// Tolerate `null` -> empty-string for fields that v1 historically sent as
/// empty strings but v2 may serialize as `null`.
fn deserialize_string_default_empty<'de, D>(de: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(de)?.unwrap_or_default())
}

/// API market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMarketInfo {
    pub market_type: String,
    pub token_id: String,
    pub market_id: String,
    pub reserve_native: String,
    pub reserve_token: String,
    pub token_price: String,
    pub price_usd: String,
}

/// API balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBalanceInfo {
    pub balance: String,
    pub token_price: String,
    pub created_at: u64,
}

/// Created token information from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedToken {
    pub token_info: ApiTokenInfo,
    pub market_info: ApiMarketInfo,
    pub balance_info: ApiBalanceInfo,
    pub reward_info: RewardInfo,
}

/// API response for created tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedTokenResponse {
    pub tokens: Vec<CreatedToken>,
    pub total_count: u64,
}
