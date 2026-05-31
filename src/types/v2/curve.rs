//! On-chain bonding curve state type for v2.

use alloy::primitives::{Address, U256};

/// Snapshot of a v2 bonding curve's on-chain state, as returned by
/// `BondingCurve.getCurve(address)`.
///
/// Field order matches the 16-field ABI tuple exactly:
/// `(token, creator, quoteToken, virtualQuoteReserve, virtualTokenReserve, k,
///   minTokenReserve, initialQuoteReserve, initialTokenReserve, createdAtBlock,
///   graduated, creatorFeeRate, version, dexType, pair, graduateFee)`.
#[derive(Debug, Clone, PartialEq)]
pub struct V2Curve {
    /// The bonding-curve token address.
    pub token: Address,
    /// Address that created (deployed) this token.
    pub creator: Address,
    /// Quote token used for this curve (e.g. WMON, LvMON, or an ERC-20).
    pub quote_token: Address,
    /// Current virtual quote-token reserve (AMM state).
    pub virtual_quote_reserve: U256,
    /// Current virtual token reserve (AMM state).
    pub virtual_token_reserve: U256,
    /// Constant-product constant. **Pre-graduation only**,
    /// `k == virtual_quote_reserve * virtual_token_reserve`. After graduation
    /// this is frozen at the genesis value while the virtual reserves keep
    /// moving, so the equality no longer holds — do not assert it once
    /// [`Self::graduated`] is `true`.
    pub k: U256,
    /// Minimum token reserve — the curve graduates when `virtual_token_reserve` drops to this.
    pub min_token_reserve: U256,
    /// Initial (deployment-time) virtual quote reserve.
    pub initial_quote_reserve: U256,
    /// Initial (deployment-time) virtual token reserve.
    pub initial_token_reserve: U256,
    /// Block number at which the token was created (used for anti-sniping penalty).
    pub created_at_block: u64,
    /// Whether the token has graduated to the DEX. When `true`, `pair` is set.
    pub graduated: bool,
    /// Creator fee rate in basis points (max 10 000 = 100%).
    pub creator_fee_rate: u16,
    /// On-chain `CurveVersion` discriminator (e.g. `2` for v2 curves).
    pub version: u8,
    /// On-chain `DexType` discriminator for the post-graduation DEX.
    pub dex_type: u8,
    /// DEX pair address after graduation. `Address::ZERO` before graduation.
    pub pair: Address,
    /// Quote-token fee collected by the protocol on graduation.
    pub graduate_fee: U256,
}
