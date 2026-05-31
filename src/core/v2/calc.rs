//! Pure bonding-curve math for the v2 computed helpers.
//!
//! v2 has no on-chain `getProgress` / `availableBuyTokens` /
//! `getInitialBuyAmountOut` (unlike the v1 Lens), so these reproduce the
//! on-chain arithmetic client-side. The formulas mirror
//! `nadfun-contract-v2/src/core/BondingCurve.sol` +
//! `libraries/BondingCurveLibrary.sol` exactly; keep them in lock-step with the
//! contract and guard with the live drift tests in `tests/v2_views_live.rs`.

use crate::types::v2::{V2Curve, V2QuoteConfig};
use alloy::primitives::{U256, U512};

/// Basis-points denominator (`BPS` in the contracts).
const BPS: u64 = 10_000;

/// Bonding-curve progress in basis points (0–10000 = 0–100%).
///
/// Fraction of the curve's sellable supply that has been bought:
/// `(initialTokenReserve − virtualTokenReserve) / (initialTokenReserve − minTokenReserve)`.
/// A graduated curve reads 10000 (its `virtual_token_reserve` has reached
/// `min_token_reserve`); the result is clamped to 10000 for safety.
pub fn progress_bps(curve: &V2Curve) -> U256 {
    if curve.graduated {
        return U256::from(BPS);
    }
    // Denominator: total sellable supply before graduation. Guard the
    // degenerate config where it would be zero (min >= initial).
    let denom = curve
        .initial_token_reserve
        .saturating_sub(curve.min_token_reserve);
    if denom.is_zero() {
        return U256::ZERO;
    }
    let sold = curve
        .initial_token_reserve
        .saturating_sub(curve.virtual_token_reserve);
    // `sold` can exceed `denom` only in degenerate states; treat as 100%.
    if sold >= denom {
        return U256::from(BPS);
    }
    // Widen to U512 for the `sold * BPS` product: `sold` can be up to ~1e27
    // and BPS is 1e4, well within U256, but a defensive widening removes any
    // overflow doubt for arbitrary on-chain values (Codex P2). The result is
    // `< BPS` (since `sold < denom`), so it always fits back into U256.
    let num = U512::from(sold) * U512::from(BPS);
    let bps = num / U512::from(denom);
    U256::from(bps)
}

/// Tokens still buyable on the curve before graduation:
/// `virtual_token_reserve − min_token_reserve`. Zero once graduated.
pub fn available_buy_tokens(curve: &V2Curve) -> U256 {
    if curve.graduated {
        return U256::ZERO;
    }
    curve
        .virtual_token_reserve
        .saturating_sub(curve.min_token_reserve)
}

/// Tokens received for an initial buy of `amount_in` quote at token-creation
/// time, given the quote token's genesis [`V2QuoteConfig`].
///
/// Reproduces `BondingCurve.getAmountOut(token, amountIn, isBuy=true)` applied
/// to the genesis curve (`k = virtual_reserve * virtual_token_reserve`):
/// 1. fee: `amountInAfterFee = amount_in − amount_in*curveProtocolFeeRate/BPS`
/// 2. constant product: `out = virtualTokenReserve − ceil(k / (virtualReserve + amountInAfterFee))`
/// 3. cap at `virtualTokenReserve − minTokenReserve`.
pub fn initial_buy_amount_out(config: &V2QuoteConfig, amount_in: U256) -> U256 {
    let reserve_in = config.virtual_reserve;
    let reserve_out = config.virtual_token_reserve;
    if reserve_in.is_zero() || reserve_out.is_zero() {
        return U256::ZERO;
    }
    let k = reserve_in.saturating_mul(reserve_out);

    // Protocol fee on the way in (BPS).
    let fee =
        amount_in.saturating_mul(U256::from(config.curve_protocol_fee_rate)) / U256::from(BPS);
    let amount_in_after_fee = amount_in.saturating_sub(fee);

    // out = reserve_out - ceil(k / (reserve_in + amount_in_after_fee))
    let new_reserve_in = reserve_in.saturating_add(amount_in_after_fee);
    let new_reserve_out = ceil_div(k, new_reserve_in);
    let mut out = reserve_out.saturating_sub(new_reserve_out);

    // Cap at the sellable supply (virtual_token_reserve - min_token_reserve).
    let available = reserve_out.saturating_sub(config.min_token_reserve);
    if out > available {
        out = available;
    }
    out
}

/// Ceiling division `ceil(a / b)` for `U256` (the contract's `mulDivUp(a, 1, b)`).
/// Returns 0 when `b == 0` (callers guard non-zero reserves first).
///
/// Uses `(a - 1) / b + 1` rather than `(a + b - 1) / b` so the intermediate
/// never overflows even when `a` is near `U256::MAX` (Codex P2).
fn ceil_div(a: U256, b: U256) -> U256 {
    if b.is_zero() || a.is_zero() {
        return U256::ZERO;
    }
    (a - U256::from(1u64)) / b + U256::from(1u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Address;

    fn curve(initial: u128, virtual_t: u128, min: u128, graduated: bool) -> V2Curve {
        V2Curve {
            token: Address::ZERO,
            creator: Address::ZERO,
            quote_token: Address::ZERO,
            virtual_quote_reserve: U256::ZERO,
            virtual_token_reserve: U256::from(virtual_t),
            k: U256::ZERO,
            min_token_reserve: U256::from(min),
            initial_quote_reserve: U256::ZERO,
            initial_token_reserve: U256::from(initial),
            created_at_block: 0,
            graduated,
            creator_fee_rate: 0,
            version: 2,
            dex_type: 0,
            pair: Address::ZERO,
            graduate_fee: U256::ZERO,
        }
    }

    #[test]
    fn progress_zero_when_fresh() {
        // fresh: virtual == initial => 0 sold => 0%.
        let c = curve(1_000, 1_000, 200, false);
        assert_eq!(progress_bps(&c), U256::ZERO);
    }

    #[test]
    fn progress_full_at_min_reserve() {
        // virtual reached min => 100% (10000 bps).
        let c = curve(1_000, 200, 200, false);
        assert_eq!(progress_bps(&c), U256::from(10_000u64));
    }

    #[test]
    fn progress_half() {
        // initial 1000, min 200 => sellable 800; sold 400 => virtual 600 => 50%.
        let c = curve(1_000, 600, 200, false);
        assert_eq!(progress_bps(&c), U256::from(5_000u64));
    }

    #[test]
    fn progress_graduated_is_full() {
        let c = curve(1_000, 999, 200, true);
        assert_eq!(progress_bps(&c), U256::from(10_000u64));
    }

    #[test]
    fn progress_degenerate_denominator_is_zero() {
        let c = curve(200, 200, 200, false); // initial == min
        assert_eq!(progress_bps(&c), U256::ZERO);
    }

    #[test]
    fn available_is_virtual_minus_min() {
        let c = curve(1_000, 600, 200, false);
        assert_eq!(available_buy_tokens(&c), U256::from(400u64));
    }

    #[test]
    fn available_zero_when_graduated() {
        let c = curve(1_000, 600, 200, true);
        assert_eq!(available_buy_tokens(&c), U256::ZERO);
    }

    fn cfg(v_reserve: u128, v_token: u128, min: u128, fee_bps: u16) -> V2QuoteConfig {
        V2QuoteConfig {
            decimals: 18,
            virtual_reserve: U256::from(v_reserve),
            virtual_token_reserve: U256::from(v_token),
            min_token_reserve: U256::from(min),
            deploy_fee: U256::ZERO,
            graduate_fee: U256::ZERO,
            curve_protocol_fee_rate: fee_bps,
            dex_protocol_fee_rate: 0,
            settlement_threshold: U256::ZERO,
            active: true,
        }
    }

    #[test]
    fn initial_buy_zero_in_zero_out() {
        let c = cfg(1_000, 1_000, 200, 100);
        assert_eq!(initial_buy_amount_out(&c, U256::ZERO), U256::ZERO);
    }

    #[test]
    fn initial_buy_matches_constant_product_no_fee() {
        // No fee. reserve_in=1000, reserve_out=1000, k=1_000_000.
        // amount_in=1000 => new_reserve_in=2000 => ceil(1_000_000/2000)=500
        // => out = 1000 - 500 = 500.
        let c = cfg(1_000, 1_000, 0, 0);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_000u64)),
            U256::from(500u64)
        );
    }

    #[test]
    fn initial_buy_applies_fee_before_curve() {
        // fee 10% (1000 bps). amount_in=1000 => after fee 900.
        // new_reserve_in = 1000+900 = 1900 => ceil(1_000_000/1900)=527 (526.31..->527)
        // out = 1000 - 527 = 473.
        let c = cfg(1_000, 1_000, 0, 1_000);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_000u64)),
            U256::from(473u64)
        );
    }

    #[test]
    fn initial_buy_caps_at_available() {
        // min=900 => available = 1000-900 = 100. A huge buy caps at 100.
        let c = cfg(1_000, 1_000, 900, 0);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_000_000_000u64)),
            U256::from(100u64)
        );
    }

    #[test]
    fn ceil_div_no_overflow_near_max() {
        // (a + b - 1) would wrap here; (a-1)/b + 1 must not (Codex P2).
        assert_eq!(
            ceil_div(U256::MAX, U256::from(2u64)),
            U256::MAX / U256::from(2u64) + U256::from(1u64)
        );
        assert_eq!(ceil_div(U256::MAX, U256::MAX), U256::from(1u64));
        assert_eq!(ceil_div(U256::ZERO, U256::from(5u64)), U256::ZERO);
        assert_eq!(
            ceil_div(U256::from(10u64), U256::from(3u64)),
            U256::from(4u64)
        );
    }

    #[test]
    fn progress_clamps_without_overflow_when_sold_exceeds_denom() {
        // Degenerate: sold >= denom must read 10000, not a wrapped/under value.
        // initial huge, virtual below min => sold > denom.
        let c = curve(1_000, 100, 200, false); // virtual(100) < min(200) => sold 900 > denom 800
        assert_eq!(progress_bps(&c), U256::from(10_000u64));
    }
}
