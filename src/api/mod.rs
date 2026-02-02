//! API client with optional authentication
//!
//! This module provides an API client that supports:
//! - No authentication (lower rate limit, backward compatible)
//! - API key authentication (higher rate limit)
//!
//! # Examples
//!
//! ```rust,ignore
//! // No auth - works but with lower rate limit
//! let api = ApiClient::new();
//!
//! // With API key (higher rate limit)
//! let api = ApiClient::new().with_api_key(api_key);
//!
//! // Upload image and create token
//! let upload = api.upload_image_from_uri("https://example.com/image.png").await?;
//! let metadata = api.post_metadata(params).await?;
//! let salt = api.post_salt(params).await?;
//! ```

use crate::constants::get_api_server_url;
use crate::types::{
    ApiErrorResponse, CreatedToken, CreatedTokenResponse, CreateTokenParams,
    CreatorBatchClaimParams, CreatorClaimParams, MetadataParams, PostMetadataData, PostSaltData,
    SaltParams, UploadImageData,
};
use alloy::primitives::{Address, B256, U256};
use anyhow::Result;
use reqwest::{Client, Method, RequestBuilder};
use std::str::FromStr;

/// Allowed image types for token creation
pub const ALLOWED_IMAGE_TYPES: [&str; 4] =
    ["image/jpeg", "image/png", "image/webp", "image/svg+xml"];

/// API client with optional authentication
///
/// Supports two modes:
/// - No auth: Works with lower rate limit (backward compatible)
/// - API key: For higher rate limits
pub struct ApiClient {
    http_client: Client,
    api_url: String,
    api_key: Option<String>,
}

impl ApiClient {
    /// Create a new API client without authentication
    ///
    /// This works with lower rate limits but is backward compatible.
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = ApiClient::new();
    /// ```
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
            api_url: get_api_server_url().to_string(),
            api_key: None,
        }
    }

    /// Create a new API client with API key from environment variable
    ///
    /// Reads API key from `NAD_API_KEY` environment variable.
    /// If not set, falls back to no authentication (lower rate limit).
    ///
    /// # Example
    /// ```rust,ignore
    /// // .env file or shell: export NAD_API_KEY=nadfun_xxxxx
    /// let client = ApiClient::from_env();
    /// ```
    pub fn from_env() -> Self {
        let api_key = std::env::var("NAD_API_KEY").ok();
        Self {
            http_client: Client::new(),
            api_url: get_api_server_url().to_string(),
            api_key,
        }
    }

    /// Set API key for higher rate limits
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = ApiClient::new().with_api_key(api_key);
    /// ```
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Build a request with authentication headers
    pub fn request(&self, method: Method, url: &str) -> RequestBuilder {
        let full_url = if url.starts_with("http") {
            url.to_string()
        } else {
            format!("{}{}", self.api_url, url)
        };

        let mut builder = self.http_client.request(method, &full_url);

        // Add API key if available
        if let Some(ref api_key) = self.api_key {
            builder = builder.header("X-API-Key", api_key);
        }

        builder
    }

    /// Convenience method for GET requests
    pub fn get(&self, url: &str) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    /// Convenience method for POST requests
    pub fn post(&self, url: &str) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    /// Convenience method for DELETE requests
    pub fn delete(&self, url: &str) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }

    /// Get the API URL
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Check if has API key
    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Set API key directly
    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = Some(api_key);
    }
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::new()
    }
}

// === Token Creation API ===

impl ApiClient {
    /// Download image from URI and upload to metadata server
    ///
    /// # Arguments
    /// * `image_uri` - URL of the image to download and upload
    ///
    /// # Returns
    /// * `UploadImageData` - Contains the uploaded image URI and NSFW status
    pub async fn upload_image_from_uri(&self, image_uri: &str) -> Result<UploadImageData> {
        // Download image (external URL, no API key needed)
        let response = self.http_client.get(image_uri).send().await?;

        // Get content type from response header
        let header_content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let image_bytes = response.bytes().await?;

        // Detect image type from magic bytes if header doesn't have it
        let content_type = if let Some(ct) = header_content_type {
            if ct.starts_with("image/") {
                // Validate header content type against allowed types
                let base_type = ct.split(';').next().unwrap_or(&ct).trim();
                if !ALLOWED_IMAGE_TYPES.contains(&base_type) {
                    anyhow::bail!(
                        "Image type '{}' from URL is not allowed. Allowed types: {}",
                        base_type,
                        ALLOWED_IMAGE_TYPES.join(", ")
                    );
                }
                base_type.to_string()
            } else {
                detect_image_type(&image_bytes)?
            }
        } else {
            detect_image_type(&image_bytes)?
        };

        // Upload to metadata server (Agent API)
        let upload_url = format!("{}/agent/token/image", self.api_url);

        let response = self
            .request(Method::POST, &upload_url)
            .header("Content-Type", content_type)
            .body(image_bytes)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(&response_text) {
                anyhow::bail!("Image upload failed: {}", error_response.error);
            } else {
                anyhow::bail!(
                    "Image upload failed with status {}: {}",
                    status,
                    response_text
                );
            }
        }

        let upload_data: UploadImageData = serde_json::from_str(&response_text).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse upload response: {}. Body: {}",
                e,
                response_text
            )
        })?;

        Ok(upload_data)
    }

    /// Upload image bytes directly
    ///
    /// # Arguments
    /// * `image_bytes` - Raw image bytes
    /// * `content_type` - MIME type of the image
    pub async fn upload_image_bytes(
        &self,
        image_bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<UploadImageData> {
        let upload_url = format!("{}/agent/token/image", self.api_url);
        let response = self
            .request(Method::POST, &upload_url)
            .header("Content-Type", content_type)
            .body(image_bytes)
            .send()
            .await?;

        let upload_data: UploadImageData = response.json().await?;
        Ok(upload_data)
    }

    /// Create metadata on server
    ///
    /// # Arguments
    /// * `params` - Metadata parameters including name, symbol, description, etc.
    pub async fn post_metadata(&self, params: MetadataParams) -> Result<PostMetadataData> {
        let metadata_url = format!("{}/agent/token/metadata", self.api_url);

        let response = self
            .request(Method::POST, &metadata_url)
            .json(&params)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(&response_text) {
                anyhow::bail!("Metadata upload failed: {}", error_response.error);
            } else {
                anyhow::bail!(
                    "Metadata upload failed with status {}: {}",
                    status,
                    response_text
                );
            }
        }

        let metadata_data: PostMetadataData =
            serde_json::from_str(&response_text).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse metadata response: {}. Body: {}",
                    e,
                    response_text
                )
            })?;

        Ok(metadata_data)
    }

    /// Get salt value from server
    ///
    /// # Arguments
    /// * `params` - Salt parameters including creator address and metadata URI
    pub async fn post_salt(&self, params: SaltParams) -> Result<PostSaltData> {
        let salt_url = format!("{}/agent/salt", self.api_url);
        let response = self
            .request(Method::POST, &salt_url)
            .json(&params)
            .send()
            .await?;

        let salt_data: PostSaltData = response.json().await?;
        Ok(salt_data)
    }

    /// Execute complete token creation preparation flow
    ///
    /// This function handles:
    /// 1. Download image from URI and upload to metadata server
    /// 2. Create metadata on server
    /// 3. Get salt value from server
    ///
    /// Returns: (metadata_uri, image_uri, salt_bytes, token_address, is_nsfw)
    ///
    /// Note: The actual transaction must be executed separately using Core.create_token()
    pub async fn prepare_token_creation(
        &self,
        params: &CreateTokenParams,
    ) -> Result<(String, String, [u8; 32], String, bool)> {
        // Step 1: Download and upload image
        let upload_result = self.upload_image_from_uri(&params.image_uri).await?;

        // Step 2: Create metadata
        let metadata_params = MetadataParams {
            name: params.name.clone(),
            symbol: params.symbol.clone(),
            image_uri: upload_result.image_uri.clone(),
            description: params.description.clone(),
            website: params
                .website
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_default(),
            twitter: params
                .twitter
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_default(),
            telegram: params
                .telegram
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_default(),
            is_nsfw: upload_result.is_nsfw,
        };
        let metadata_result = self.post_metadata(metadata_params).await?;

        // Step 3: Get salt and token address
        let salt_params = SaltParams {
            creator: format!("{:?}", params.creator_address),
            metadata_uri: metadata_result.metadata_uri.clone(),
            name: metadata_result.metadata.name.clone(),
            symbol: metadata_result.metadata.symbol.clone(),
        };
        let salt_result = self.post_salt(salt_params).await?;

        // Convert salt hex string to bytes32
        let salt_bytes = parse_salt_to_bytes32(&salt_result.salt)?;

        Ok((
            metadata_result.metadata_uri,
            upload_result.image_uri,
            salt_bytes,
            salt_result.address,
            upload_result.is_nsfw,
        ))
    }
}

// === Creator Reward API ===

impl ApiClient {
    /// Get tokens created by a specific address with their reward information
    ///
    /// # Arguments
    /// * `creator_address` - The address of the creator
    /// * `page` - Page number (1-indexed)
    /// * `limit` - Number of results per page
    pub async fn get_created_tokens(
        &self,
        creator_address: Address,
        page: u32,
        limit: u32,
    ) -> Result<CreatedTokenResponse> {
        let url = format!(
            "{}/agent/token/created/{}?page={}&limit={}",
            self.api_url, creator_address, page, limit
        );

        let response = self.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("API request failed: {}", response.status()));
        }

        let data: CreatedTokenResponse = response.json().await?;
        Ok(data)
    }

    /// Build claim parameters from a created token's reward info
    ///
    /// Returns None if the token is not claimable or has zero amount
    pub fn build_claim_params(token: &CreatedToken) -> Option<CreatorClaimParams> {
        if !token.reward_info.claimable {
            return None;
        }

        let amount = U256::from_str(&token.reward_info.amount).ok()?;
        if amount.is_zero() {
            return None;
        }

        let proof: Vec<B256> = token
            .reward_info
            .proof
            .iter()
            .filter_map(|p| p.parse().ok())
            .collect();

        let token_address: Address = token.token_info.token_id.parse().ok()?;

        Some(CreatorClaimParams {
            token: token_address,
            amount,
            merkle_proof: proof,
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
    }

    /// Build batch claim parameters from multiple created tokens
    ///
    /// Filters out non-claimable tokens and returns None if no tokens are claimable
    pub fn build_batch_claim_params(tokens: &[CreatedToken]) -> Option<CreatorBatchClaimParams> {
        let claimable: Vec<_> = tokens.iter().filter_map(Self::build_claim_params).collect();

        if claimable.is_empty() {
            return None;
        }

        Some(CreatorBatchClaimParams {
            tokens: claimable.iter().map(|p| p.token).collect(),
            amounts: claimable.iter().map(|p| p.amount).collect(),
            merkle_proofs: claimable.iter().map(|p| p.merkle_proof.clone()).collect(),
            gas_limit: None,
            gas_price: None,
            nonce: None,
        })
    }
}

// === Helper Functions ===

/// Detect image type from magic bytes
fn detect_image_type(bytes: &[u8]) -> Result<String> {
    if bytes.len() < 12 {
        anyhow::bail!("File too small to be a valid image");
    }

    let content_type = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        "image/svg+xml"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        anyhow::bail!("GIF format is not supported. Allowed formats: JPEG, PNG, WEBP, SVG")
    } else {
        anyhow::bail!(
            "Unsupported image format. Allowed formats: JPEG, PNG, WEBP, SVG. First bytes: {:?}",
            &bytes[..8.min(bytes.len())]
        )
    };

    if !ALLOWED_IMAGE_TYPES.contains(&content_type) {
        anyhow::bail!(
            "Image type '{}' is not allowed. Allowed types: {}",
            content_type,
            ALLOWED_IMAGE_TYPES.join(", ")
        );
    }

    Ok(content_type.to_string())
}

/// Parse salt hex string to bytes32 array
fn parse_salt_to_bytes32(salt_hex: &str) -> Result<[u8; 32]> {
    let salt_hex = salt_hex.trim_start_matches("0x");
    let salt_vec = hex::decode(salt_hex)?;

    if salt_vec.len() != 32 {
        anyhow::bail!("Salt must be 32 bytes, got {}", salt_vec.len());
    }

    let mut salt_bytes = [0u8; 32];
    salt_bytes.copy_from_slice(&salt_vec);
    Ok(salt_bytes)
}
