//! v2-only API-response types.
//!
//! The token-info struct that `GET /token/:token` returns is the same shape
//! used by v1's creator-rewards surface, so it lives at
//! [`crate::types::ApiTokenInfo`] (in `types/v1/creator.rs`) and is re-exported
//! flat. This module holds the strictly v2-only types: vault state.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Vault type discriminator returned by the `GET /vault/:token_id` endpoint.
///
/// Any unknown server-side `vault_type` string deserializes to
/// [`VaultType::Custom`] instead of failing the entire response — keeps the
/// SDK forward-compatible with vaults shipped after this SDK version
/// (Codex P3 #17).
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
    #[serde(other)]
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vault_types_round_trip() {
        assert_eq!(
            serde_json::from_str::<VaultType>("\"BURN\"").unwrap(),
            VaultType::Burn
        );
        assert_eq!(
            serde_json::from_str::<VaultType>("\"LP\"").unwrap(),
            VaultType::Lp
        );
        assert_eq!(
            serde_json::from_str::<VaultType>("\"CREATOR_FEE\"").unwrap(),
            VaultType::CreatorFee
        );
        assert_eq!(
            serde_json::from_str::<VaultType>("\"GIFT\"").unwrap(),
            VaultType::Gift
        );
    }

    /// Codex P3 #17: unknown vault types fall back to Custom instead of
    /// failing deserialization.
    #[test]
    fn unknown_vault_type_falls_back_to_custom() {
        let v: VaultType = serde_json::from_str("\"FUTURE_VAULT\"").unwrap();
        assert_eq!(v, VaultType::Custom);
        let v: VaultType = serde_json::from_str("\"\"").unwrap();
        assert_eq!(v, VaultType::Custom);
    }
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
