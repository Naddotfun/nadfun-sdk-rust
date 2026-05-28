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

use crate::constants::{get_api_server_url, Network};
use crate::types::{
    ApiErrorResponse, ApiTokenInfo, CreateTokenParams, CreatedToken, CreatedTokenResponse,
    CreatorBatchClaimParams, CreatorClaimParams, MetadataParams, PostMetadataData, PostSaltData,
    SaltParams, UploadImageData, V2PrepareCreationParams, V2PreparedCreation, VaultState,
};
use crate::version::SdkVersion;
use alloy::primitives::{Address, B256, U256};
use anyhow::Result;
use reqwest::{Client, Method, RequestBuilder};
use std::str::FromStr;

/// Allowed image types for token creation
pub const ALLOWED_IMAGE_TYPES: [&str; 4] =
    ["image/jpeg", "image/png", "image/webp", "image/svg+xml"];

/// API client with optional authentication.
///
/// Bound to a `Network` at construction — every request resolves against
/// that network's API URL. Two clients can coexist for mainnet + testnet in
/// the same process without interfering with each other.
pub struct ApiClient {
    http_client: Client,
    api_url: String,
    api_key: Option<String>,
    network: Network,
}

impl ApiClient {
    /// Create a new API client for `network` without authentication.
    ///
    /// Lower rate limits but no API key required. Add a key later with
    /// [`Self::with_api_key`] if you need higher limits.
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = ApiClient::new(Network::Mainnet);
    /// ```
    pub fn new(network: Network) -> Self {
        Self {
            http_client: Client::new(),
            api_url: get_api_server_url(network).to_string(),
            api_key: None,
            network,
        }
    }

    /// Create a new API client for `network` with the API key from
    /// `NAD_API_KEY`. Falls back to no-auth if the variable is unset.
    ///
    /// # Example
    /// ```rust,ignore
    /// // .env file or shell: export NAD_API_KEY=nadfun_xxxxx
    /// let client = ApiClient::from_env(Network::Mainnet);
    /// ```
    pub fn from_env(network: Network) -> Self {
        let api_key = std::env::var("NAD_API_KEY").ok();
        Self {
            http_client: Client::new(),
            api_url: get_api_server_url(network).to_string(),
            api_key,
            network,
        }
    }

    /// The network this client is bound to.
    pub fn network(&self) -> Network {
        self.network
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

    /// Override the base API URL.
    ///
    /// Useful for pointing the client at a custom deployment (staging, local
    /// mock, etc.) or for tests against wiremock. By default the client uses
    /// `get_api_server_url()` for the current network.
    pub fn with_api_url(mut self, api_url: String) -> Self {
        self.api_url = api_url;
        self
    }
}

impl Default for ApiClient {
    /// Default to a mainnet client. Prefer [`ApiClient::new`] with an
    /// explicit `Network` for clarity.
    fn default() -> Self {
        Self::new(Network::Mainnet)
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

    /// Internal helper shared between [`Self::prepare_token_creation`] (v1) and
    /// [`Self::prepare_token_creation_v2`].
    ///
    /// Downloads the image from `image_uri`, uploads it to IPFS, and creates
    /// the JSON metadata document on the metadata server. Returns the IPFS
    /// image URI, the metadata URI, the (potentially-normalized) name/symbol
    /// the server stored, and the NSFW flag.
    #[allow(clippy::too_many_arguments)]
    async fn upload_image_and_metadata(
        &self,
        name: &str,
        symbol: &str,
        description: &str,
        image_uri: &str,
        website: Option<&str>,
        twitter: Option<&str>,
        telegram: Option<&str>,
    ) -> Result<(UploadImageData, PostMetadataData)> {
        let upload_result = self.upload_image_from_uri(image_uri).await?;

        let metadata_params = MetadataParams {
            name: name.to_string(),
            symbol: symbol.to_string(),
            image_uri: upload_result.image_uri.clone(),
            description: description.to_string(),
            website: website
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            twitter: twitter
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            telegram: telegram
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_default(),
            is_nsfw: upload_result.is_nsfw,
        };
        let metadata_result = self.post_metadata(metadata_params).await?;
        Ok((upload_result, metadata_result))
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
        let (upload_result, metadata_result) = self
            .upload_image_and_metadata(
                &params.name,
                &params.symbol,
                &params.description,
                &params.image_uri,
                params.website.as_deref(),
                params.twitter.as_deref(),
                params.telegram.as_deref(),
            )
            .await?;

        // Get salt and token address (v1: no version field on the wire)
        let salt_params = SaltParams {
            creator: format!("{:?}", params.creator_address),
            metadata_uri: metadata_result.metadata_uri.clone(),
            name: metadata_result.metadata.name.clone(),
            symbol: metadata_result.metadata.symbol.clone(),
            version: None,
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

    /// Execute the v2 token-creation preparation flow.
    ///
    /// Mirrors [`Self::prepare_token_creation`] but requests salt mining with
    /// `version: "V2"`, which makes the server compute the CREATE2 address
    /// against the v2 BondingCurve + Token implementation (see
    /// `api-server v2/src/services/token/salt.rs`).
    ///
    /// The returned [`V2PreparedCreation`] feeds directly into
    /// `CoreV2::create_token` (next step) — the caller does not normally
    /// invoke this method directly.
    pub async fn prepare_token_creation_v2(
        &self,
        params: &V2PrepareCreationParams,
    ) -> Result<V2PreparedCreation> {
        let (upload_result, metadata_result) = self
            .upload_image_and_metadata(
                &params.name,
                &params.symbol,
                &params.description,
                &params.image_uri,
                params.website.as_deref(),
                params.twitter.as_deref(),
                params.telegram.as_deref(),
            )
            .await?;

        let salt_params = SaltParams {
            creator: format!("{:?}", params.creator_address),
            metadata_uri: metadata_result.metadata_uri.clone(),
            name: metadata_result.metadata.name.clone(),
            symbol: metadata_result.metadata.symbol.clone(),
            version: Some(SdkVersion::V2),
        };
        let salt_result = self.post_salt(salt_params).await?;

        let salt_bytes = parse_salt_to_bytes32(&salt_result.salt)?;
        let token_address: Address = salt_result.address.parse().map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse token address '{}' from salt response: {}",
                salt_result.address,
                e
            )
        })?;

        Ok(V2PreparedCreation {
            image_uri: upload_result.image_uri,
            metadata_uri: metadata_result.metadata_uri,
            salt: B256::from(salt_bytes),
            token_address,
            is_nsfw: upload_result.is_nsfw,
            // Server-normalized name/symbol — used by the salt miner, so
            // the on-chain create must match these exact strings to land
            // at the predicted address. Codex P2 #15.
            name: metadata_result.metadata.name,
            symbol: metadata_result.metadata.symbol,
        })
    }

    /// Look up the API's record for a single token: name/symbol/socials,
    /// creator profile, graduation flag, and — critically — the
    /// [`SdkVersion`] discriminator.
    ///
    /// Use this for client-side v1/v2 dispatch when you receive a token
    /// address from outside (e.g. a wallet UI or message). For tight loops,
    /// cache the result; the version of a token does not change.
    pub async fn get_token(&self, token: Address) -> Result<ApiTokenInfo> {
        // `format!("{:?}", token)` produces the EIP-55 mixed-case `0x...` form.
        let url = format!("{}/token/{:?}", self.api_url, token);
        let response = self.get(&url).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body) {
                anyhow::bail!("get_token({:?}) failed: {}", token, err.error);
            }
            anyhow::bail!("get_token({:?}) failed with {}: {}", token, status, body);
        }
        serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!("Failed to parse get_token response: {}. Body: {}", e, body)
        })
    }

    /// Look up the v2 vault state for a token (fees per vault slot, totals,
    /// per-vault stats). v2-only — calling this on a v1 token will yield a
    /// 404 from the API server.
    pub async fn get_token_vaults(&self, token: Address) -> Result<VaultState> {
        let url = format!("{}/vault/{:?}", self.api_url, token);
        let response = self.get(&url).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body) {
                anyhow::bail!("get_token_vaults({:?}) failed: {}", token, err.error);
            }
            anyhow::bail!(
                "get_token_vaults({:?}) failed with {}: {}",
                token,
                status,
                body
            );
        }
        serde_json::from_str(&body).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse get_token_vaults response: {}. Body: {}",
                e,
                body
            )
        })
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
