//! Token creation functionality
//!
//! This module provides the complete token creation flow:
//! 1. Download image from URI and upload to metadata server
//! 2. Create metadata on server
//! 3. Get salt value from server
//! 4. Execute create transaction on bonding curve

use crate::types::{
    CreateTokenParams, MetadataParams, PostMetadataData, PostSaltData, SaltParams, UploadImageData,
};
use anyhow::Result;
use reqwest;

/// Base API server URL
pub const API_SERVER_URL: &str = "https://api.nad.fun";

/// Allowed image types for token creation
pub const ALLOWED_IMAGE_TYPES: [&str; 4] =
    ["image/jpeg", "image/png", "image/webp", "image/svg+xml"];

/// Token creation client
pub struct TokenCreationClient {
    http_client: reqwest::Client,
    api_url: String,
}

impl TokenCreationClient {
    /// Create a new token creation client
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            api_url: API_SERVER_URL.to_string(),
        }
    }

    /// Download image from URI and upload to metadata server
    pub async fn upload_image_from_uri(&self, image_uri: &str) -> Result<UploadImageData> {
        // Download image
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
                // Detect from magic bytes
                detect_image_type(&image_bytes)?
            }
        } else {
            detect_image_type(&image_bytes)?
        };

        // Upload to metadata server
        let upload_url = format!("{}/metadata/image", self.api_url);

        let response = self
            .http_client
            .post(&upload_url)
            .header("Content-Type", content_type)
            .body(image_bytes)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        // Check if response is an error
        if !status.is_success() {
            // Try to parse as error response
            if let Ok(error_response) =
                serde_json::from_str::<crate::types::ApiErrorResponse>(&response_text)
            {
                anyhow::bail!("Image upload failed: {}", error_response.error);
            } else {
                anyhow::bail!(
                    "Image upload failed with status {}: {}",
                    status,
                    response_text
                );
            }
        }

        // Try to parse the success response
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
    pub async fn upload_image_bytes(
        &self,
        image_bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<UploadImageData> {
        let upload_url = format!("{}/metadata/image", self.api_url);
        let response = self
            .http_client
            .post(&upload_url)
            .header("Content-Type", content_type)
            .body(image_bytes)
            .send()
            .await?;

        let upload_data: UploadImageData = response.json().await?;
        Ok(upload_data)
    }

    /// Create metadata on server
    pub async fn post_metadata(&self, params: MetadataParams) -> Result<PostMetadataData> {
        let metadata_url = format!("{}/metadata/metadata", self.api_url);

        let response = self
            .http_client
            .post(&metadata_url)
            .json(&params)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        // Check if response is an error
        if !status.is_success() {
            if let Ok(error_response) =
                serde_json::from_str::<crate::types::ApiErrorResponse>(&response_text)
            {
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
    pub async fn post_salt(&self, params: SaltParams) -> Result<PostSaltData> {
        let salt_url = format!("{}/token/salt", self.api_url);
        let response = self
            .http_client
            .post(&salt_url)
            .json(&params)
            .send()
            .await?;

        let salt_data: PostSaltData = response.json().await?;
        Ok(salt_data)
    }
}

impl Default for TokenCreationClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCreationClient {
    /// Execute complete token creation flow
    ///
    /// This function handles the entire token creation process:
    /// 1. Download image from URI and upload to metadata server
    /// 2. Create metadata on server
    /// 3. Get salt value from server
    /// 4. Return all prepared data for transaction execution (including is_nsfw status and token address)
    ///
    /// Returns an error if the server detects NSFW content (is_nsfw = true)
    ///
    /// Note: The actual transaction must be executed separately using
    /// BondingCurveRouter::create() because it requires a wallet provider
    pub async fn prepare_token_creation(
        &self,
        params: &CreateTokenParams,
    ) -> Result<(String, String, [u8; 32], String, bool)> {
        // Step 1: Download and upload image
        let upload_result = self.upload_image_from_uri(&params.image_uri).await?;

        // // Reject NSFW content immediately
        // if upload_result.is_nsfw {
        //     anyhow::bail!("NSFW content detected. Token creation rejected.");
        // }

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
            salt_result.address, // Token address from CREATE2 calculation
            upload_result.is_nsfw, // Return is_nsfw status from server
        ))
    }
}

/// Detect image type from magic bytes
fn detect_image_type(bytes: &[u8]) -> Result<String> {
    if bytes.len() < 12 {
        anyhow::bail!("File too small to be a valid image");
    }

    // Check magic bytes and validate against allowed types
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

    // Verify content type is in allowed list
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
