//! Token creation types

use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};

/// Image upload response
#[derive(Debug, Serialize, Deserialize)]
pub struct UploadImageData {
    pub image_uri: String,
    pub is_nsfw: bool,
}

/// API Error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: String,
}

/// Metadata parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataParams {
    pub name: String,
    pub symbol: String,
    pub image_uri: String,
    pub description: String,
    pub website: String,
    pub twitter: String,
    pub telegram: String,
    pub is_nsfw: bool,
}

/// Metadata response
#[derive(Debug, Serialize, Deserialize)]
pub struct PostMetadataData {
    pub metadata_uri: String,
    pub metadata: MetadataInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataInfo {
    pub name: String,
    pub symbol: String,
}

/// Salt request parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct SaltParams {
    pub creator: String,
    pub metadata_uri: String,
    pub name: String,
    pub symbol: String,
}

/// Salt response
#[derive(Debug, Serialize, Deserialize)]
pub struct PostSaltData {
    pub salt: String,
}

/// Complete token creation parameters
#[derive(Debug, Clone)]
pub struct CreateTokenParams {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub image_uri: String,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub creator_address: Address,
    /// Amount of tokens to receive from initial buy.
    /// Use Core.get_initial_buy_amount_out(value) to calculate this value.
    pub amount_out: U256,
    pub value: U256, // MON amount to send (typically 1.5 MON)
}

/// Result of the complete token creation flow
#[derive(Debug)]
pub struct TokenCreationResult {
    pub token_address: Address,
    pub metadata_uri: String,
    pub image_uri: String,
    pub salt: String,
    pub transaction_hash: alloy::primitives::TxHash,
    pub is_nsfw: bool, // NSFW status from server detection
}
