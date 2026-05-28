//! Token version discriminator.
//!
//! Nad.fun ships two generations of contracts that the unified
//! [`crate::Core`] wires together:
//! - [`SdkVersion::V1`] — original bonding curve + Capricorn CL DEX
//!   surface (`buy`, `sell`, `get_amount_out`, `create_token`).
//! - [`SdkVersion::V2`] — unified `NadFunRouter` + vault ecosystem
//!   (`buy_v2`, `sell_v2`, `create_token_v2`, …).
//! - [`SdkVersion::None`] — token is not registered on either system.
//!   Returned by [`crate::Core::detect_version`] when the on-chain
//!   `TokenVersionLens` reports an unknown token (or, in the fallback
//!   path, when `TokenRegistryV2::getPair` returns `Address::ZERO`).
//!
//! Use [`crate::Core::detect_version`] to classify a token from its
//! address, then dispatch into the right `Core::*` method family.
//!
//! ## Wire form
//!
//! The API sends `"V1"` / `"V2"` (or omits the field for legacy v1
//! responses, which deserialize to [`SdkVersion::V1`]). [`SdkVersion::None`]
//! is an SDK-only state — the API never returns it.

use serde::{Deserialize, Serialize};

/// Token version a Nad.fun token belongs to.
///
/// Default is [`SdkVersion::V1`] for backward compatibility with the
/// pre-v2 API which omitted the field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SdkVersion {
    /// Original v1 contracts (bonding curve + Capricorn CL DEX).
    #[default]
    V1,
    /// v2 contracts (NadFun unified router + vault ecosystem).
    V2,
    /// Token is not registered on either v1 or v2.
    ///
    /// Returned by [`crate::Core::detect_version`] for arbitrary ERC-20
    /// addresses that aren't Nad.fun-deployed tokens. The wire-form is
    /// `"NONE"` but the API never emits this — it only originates from
    /// the SDK's on-chain version probe.
    None,
}

impl SdkVersion {
    /// Wire-form string used by the Nad.fun API (`"V1"` / `"V2"`). The
    /// SDK-only [`SdkVersion::None`] renders as `"NONE"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            SdkVersion::V1 => "V1",
            SdkVersion::V2 => "V2",
            SdkVersion::None => "NONE",
        }
    }

    /// Whether this is a v2 token.
    pub fn is_v2(&self) -> bool {
        matches!(self, SdkVersion::V2)
    }

    /// Whether this is a v1 token.
    pub fn is_v1(&self) -> bool {
        matches!(self, SdkVersion::V1)
    }

    /// Whether this token is not registered on either system.
    pub fn is_none(&self) -> bool {
        matches!(self, SdkVersion::None)
    }
}

impl std::fmt::Display for SdkVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_v1() {
        assert_eq!(SdkVersion::default(), SdkVersion::V1);
    }

    #[test]
    fn predicates_match_variant() {
        assert!(SdkVersion::V1.is_v1());
        assert!(!SdkVersion::V1.is_v2());
        assert!(SdkVersion::V2.is_v2());
        assert!(!SdkVersion::V2.is_v1());
        assert!(SdkVersion::None.is_none());
        assert!(!SdkVersion::None.is_v1());
        assert!(!SdkVersion::None.is_v2());
    }

    #[test]
    fn serde_roundtrip_uppercase() {
        let v1_json = serde_json::to_string(&SdkVersion::V1).unwrap();
        let v2_json = serde_json::to_string(&SdkVersion::V2).unwrap();
        let none_json = serde_json::to_string(&SdkVersion::None).unwrap();
        assert_eq!(v1_json, "\"V1\"");
        assert_eq!(v2_json, "\"V2\"");
        assert_eq!(none_json, "\"NONE\"");
        let v1: SdkVersion = serde_json::from_str("\"V1\"").unwrap();
        let v2: SdkVersion = serde_json::from_str("\"V2\"").unwrap();
        let none: SdkVersion = serde_json::from_str("\"NONE\"").unwrap();
        assert_eq!(v1, SdkVersion::V1);
        assert_eq!(v2, SdkVersion::V2);
        assert_eq!(none, SdkVersion::None);
    }

    #[test]
    fn display_matches_wire_form() {
        assert_eq!(SdkVersion::V1.to_string(), "V1");
        assert_eq!(SdkVersion::V2.to_string(), "V2");
        assert_eq!(SdkVersion::None.to_string(), "NONE");
    }
}
