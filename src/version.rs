//! Token version discriminator.
//!
//! Nad.fun ships two generations of contracts that the SDK exposes side-by-side:
//! - [`SdkVersion::V1`] — original bonding curve + Capricorn CL DEX surface, wrapped by
//!   [`crate::Core`].
//! - [`SdkVersion::V2`] — unified `NadFunRouter` + vault ecosystem, wrapped by
//!   [`crate::CoreV2`].
//!
//! The SDK does **not** auto-dispatch between versions — callers pick the
//! `Core` / `CoreV2` they need, and use this enum at API boundaries that talk
//! about versioned resources (e.g. salt mining for token creation, or the
//! `TokenInfo.version` field returned by the Nad.fun API).
//!
//! For mixed-token-list scenarios, see `examples/unified_dispatch.rs` for a
//! ~15-line user-side dispatch pattern that queries either `ApiClient::get_token`
//! or `TokenRegistryV2::is_registered` and routes between `Core` and `CoreV2`.

use serde::{Deserialize, Serialize};

/// Token version a Nad.fun token belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SdkVersion {
    /// Original v1 contracts (bonding curve + Capricorn CL DEX).
    #[default]
    V1,
    /// v2 contracts (NadFun unified router + vault ecosystem).
    V2,
}

impl SdkVersion {
    /// Wire-form string used by the Nad.fun API (`"V1"` / `"V2"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            SdkVersion::V1 => "V1",
            SdkVersion::V2 => "V2",
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
    }

    #[test]
    fn serde_roundtrip_uppercase() {
        let v1_json = serde_json::to_string(&SdkVersion::V1).unwrap();
        let v2_json = serde_json::to_string(&SdkVersion::V2).unwrap();
        assert_eq!(v1_json, "\"V1\"");
        assert_eq!(v2_json, "\"V2\"");
        let v1: SdkVersion = serde_json::from_str("\"V1\"").unwrap();
        let v2: SdkVersion = serde_json::from_str("\"V2\"").unwrap();
        assert_eq!(v1, SdkVersion::V1);
        assert_eq!(v2, SdkVersion::V2);
    }

    #[test]
    fn display_matches_wire_form() {
        assert_eq!(SdkVersion::V1.to_string(), "V1");
        assert_eq!(SdkVersion::V2.to_string(), "V2");
    }
}
