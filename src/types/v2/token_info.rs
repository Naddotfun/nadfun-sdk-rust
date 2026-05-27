//! v2-only API-response types.
//!
//! The token-info struct that `GET /token/:token` returns is the same shape
//! used by v1's creator-rewards surface, so it lives at
//! [`crate::types::ApiTokenInfo`] (in `types/v1/creator.rs`) and is re-exported
//! flat. This module holds the strictly v2-only types: vault state.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Vault type discriminator returned by the `GET /vault/:token_id` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum VaultType {
    /// `BurnVault` — burns received quote tokens.
    Burn,
    /// `LPVault` — zaps received quote to LP via the configured IDexAdapter.
    Lp,
    /// `CreatorFeeVault` — holds creator fees for merkle claims.
    #[serde(rename = "CREATOR_FEE")]
    CreatorFee,
    /// `GiftVault` — time-locked gift distribution with auto-expiry buyback.
    Gift,
    /// User-extension or future vault type the SDK does not specifically model.
    Custom,
}

/// One vault slot attached to a v2 token.
///
/// `stats` is left as an opaque [`serde_json::Value`] because each vault type
/// returns a different breakdown (burn-vs-LP-vs-creator-fee-vs-gift). Callers
/// that need typed access should `serde_json::from_value` into their own
/// vault-type-specific struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub vault_id: String,
    pub bps: u16,
    pub name: String,
    pub active: bool,
    pub quote_amount: String,
    pub quote_amount_usd: String,
    #[serde(default)]
    pub last_executed_at: u64,
    pub vault_type: VaultType,
    #[serde(default)]
    pub stats: Value,
}

/// Response shape for `GET /vault/:token_id`.
///
/// v2-only. v1 tokens have no vault model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultState {
    pub token_id: String,
    pub quote_id: String,
    pub total_quote_amount: String,
    pub total_quote_amount_usd: String,
    pub vaults: Vec<VaultEntry>,
}
