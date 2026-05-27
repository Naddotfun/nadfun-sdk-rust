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

/// API token information (Creator API specific - distinct from trade.rs TokenInfo)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTokenInfo {
    pub token_id: String,
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub description: String,
    pub is_graduated: bool,
    pub is_nsfw: bool,
    pub twitter: String,
    pub telegram: String,
    pub website: String,
    pub created_at: u64,
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
