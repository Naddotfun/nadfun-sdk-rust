//! Token creation functionality (DEPRECATED)
//!
//! This module is deprecated. Use `ApiClient` directly for token creation.
//!
//! # Migration Guide
//!
//! Before:
//! ```rust,ignore
//! let api = ApiClient::new();
//! let client = TokenCreationClient::with_client(Arc::new(api));
//! let result = core.create_token(params, &client).await?;
//! ```
//!
//! After:
//! ```rust,ignore
//! let api = ApiClient::new().with_api_key("optional-key".to_string());
//! let result = core.create_token(params, &api).await?;
//! ```

use crate::api::ApiClient;
use std::sync::Arc;

/// Token creation client (DEPRECATED)
///
/// Use `ApiClient` directly instead.
#[deprecated(note = "Use ApiClient directly for token creation")]
pub struct TokenCreationClient {
    api_client: Arc<ApiClient>,
}

#[allow(deprecated)]
impl TokenCreationClient {
    /// Create from ApiClient (DEPRECATED)
    #[deprecated(note = "Use ApiClient directly")]
    pub fn with_client(api_client: Arc<ApiClient>) -> Self {
        Self { api_client }
    }

    /// Get the inner ApiClient
    pub fn api_client(&self) -> &ApiClient {
        &self.api_client
    }
}

// Re-export ALLOWED_IMAGE_TYPES for backward compatibility
pub use crate::api::ALLOWED_IMAGE_TYPES;
