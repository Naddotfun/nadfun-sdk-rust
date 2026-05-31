//! On-chain per-quote-token protocol config for v2.

use alloy::primitives::U256;

/// Per-quote-token bonding-curve + protocol configuration, as returned by
/// `ProtocolManager.getConfig(address)`.
///
/// One config exists per supported quote token (WMON, LvMON, an ERC-20, ...).
/// It defines the genesis curve parameters a newly created token inherits, plus
/// the protocol fees and graduation economics. Field order matches the 10-field
/// ABI tuple exactly:
/// `(decimals, virtualReserve, virtualTokenReserve, minTokenReserve, deployFee,
///   graduateFee, curveProtocolFeeRate, dexProtocolFeeRate, settlementThreshold,
///   active)`.
#[derive(Debug, Clone, PartialEq)]
pub struct V2QuoteConfig {
    /// Decimals of the quote token.
    pub decimals: u8,
    /// Genesis virtual quote reserve a new curve starts with.
    pub virtual_reserve: U256,
    /// Genesis virtual token reserve a new curve starts with.
    pub virtual_token_reserve: U256,
    /// Minimum token reserve — the curve graduates when its virtual token
    /// reserve drops to this value.
    pub min_token_reserve: U256,
    /// One-time fee (in the quote token) charged on token creation.
    pub deploy_fee: U256,
    /// Quote-token fee taken by the protocol at graduation.
    pub graduate_fee: U256,
    /// Protocol fee rate on bonding-curve trades, in basis points.
    pub curve_protocol_fee_rate: u16,
    /// Protocol fee rate on post-graduation DEX trades, in basis points.
    pub dex_protocol_fee_rate: u16,
    /// Quote-token threshold that triggers settlement.
    pub settlement_threshold: U256,
    /// Whether this quote token is currently allowed for new curves.
    pub active: bool,
}
