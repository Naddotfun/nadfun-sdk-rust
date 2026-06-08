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
use anyhow::Result;

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
/// time, given the quote token's genesis [`V2QuoteConfig`] and the token's
/// `creator_fee_rate` (basis points).
///
/// Reproduces the on-chain `BondingCurve._initialBuy` exactly (anti-sniping
/// EXEMPT — `_calculateFees(withSniping=false)`):
/// 1. fee: one combined rate `totalRate = curveProtocolFeeRate + creator_fee_rate`,
///    deducted as `amountAfterFees = amount_in − ceil(amount_in * totalRate / BPS)`
///    (the contract sums the rates into a single `mulDivUp`, so it ceils the
///    *combined* fee, not each leg separately; `totalRate >= BPS` consumes all).
/// 2. constant product: `out = virtualTokenReserve − ceil(k / (virtualReserve + amountAfterFees))`
///    with `k = virtual_reserve * virtual_token_reserve`.
/// 3. cap at `virtualTokenReserve − minTokenReserve`.
///
/// `creator_fee_rate` is a per-token parameter (chosen at create, stored on the
/// curve) and is NOT part of the genesis [`V2QuoteConfig`] — pass the value used
/// in `V2CreateTokenParams` / read from [`V2Curve::creator_fee_rate`].
///
/// Returns `Err` when the create-time buy would REVERT on-chain rather than mint:
/// a positive `amount_in` whose ceil-rounded fees consume the entire quote
/// (e.g. a dust input, or `total_rate >= BPS`) hits `_initialBuy`'s
/// `BondingCurveLibrary.getAmountOut` `require(amountIn > 0)`. A zero
/// `amount_in` is `Ok(0)` (the contract skips the buy when `quoteIn == 0`).
/// A degenerate genesis config (zero reserves) with a positive buy is also an
/// error (Codex P2).
pub fn initial_buy_amount_out(
    config: &V2QuoteConfig,
    amount_in: U256,
    creator_fee_rate: u16,
) -> Result<U256> {
    // A zero quote buy is a no-op on-chain (`if (quoteIn > 0)` guards the buy),
    // so it mints nothing without reverting.
    if amount_in.is_zero() {
        return Ok(U256::ZERO);
    }

    let reserve_in = config.virtual_reserve;
    let reserve_out = config.virtual_token_reserve;
    if reserve_in.is_zero() || reserve_out.is_zero() {
        return Err(anyhow::anyhow!(
            "initial_buy_amount_out: degenerate genesis config (zero reserve) cannot \
             quote a positive buy"
        ));
    }
    let k = reserve_in.saturating_mul(reserve_out);

    // Combined fee on the way in: protocol + creator, summed into one rate and
    // ceil-rounded as a single mulDivUp (matches `_calculateFees`). A combined
    // rate at or above BPS consumes the entire input.
    let total_rate = u64::from(config.curve_protocol_fee_rate) + u64::from(creator_fee_rate);
    let amount_in_after_fee = if total_rate >= BPS {
        U256::ZERO
    } else {
        let fee = mul_div_up(amount_in, U256::from(total_rate), U256::from(BPS));
        amount_in.saturating_sub(fee)
    };

    // If ceil-rounded fees consume the whole quote, the on-chain `_initialBuy`
    // calls `getAmountOut(0, ...)` which reverts (`require(amountIn > 0)`).
    // Mirror that revert so callers don't treat an invalid buy as a 0-token mint.
    if amount_in_after_fee.is_zero() {
        return Err(anyhow::anyhow!(
            "initial_buy_amount_out: fees consume the entire quote ({amount_in} wei at \
             {total_rate} bps); the on-chain initial buy would revert"
        ));
    }

    // out = reserve_out - ceil(k / (reserve_in + amount_in_after_fee))
    let new_reserve_in = reserve_in.saturating_add(amount_in_after_fee);
    let new_reserve_out = ceil_div(k, new_reserve_in);
    let mut out = reserve_out.saturating_sub(new_reserve_out);

    // Cap at the sellable supply (virtual_token_reserve - min_token_reserve).
    let available = reserve_out.saturating_sub(config.min_token_reserve);
    if out > available {
        out = available;
    }
    Ok(out)
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

/// Ceiling `mulDivUp(a, b, d)` = `ceil(a * b / d)` (Solady `FixedPointMathLib`).
/// Widens to `U512` for the `a * b` product so it never overflows for in-range
/// fee inputs, matching the contract's overflow-free intermediate. Returns 0
/// when any operand making the result 0 (`a==0`, `b==0`) or `d==0`.
fn mul_div_up(a: U256, b: U256, d: U256) -> U256 {
    if a.is_zero() || b.is_zero() || d.is_zero() {
        return U256::ZERO;
    }
    let prod = U512::from(a) * U512::from(b);
    let d512 = U512::from(d);
    let up = (prod - U512::from(1u64)) / d512 + U512::from(1u64);
    U256::from(up)
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
        // A zero quote buy is a no-op on-chain (`if (quoteIn > 0)`), so Ok(0).
        let c = cfg(1_000, 1_000, 200, 100);
        assert_eq!(
            initial_buy_amount_out(&c, U256::ZERO, 0).unwrap(),
            U256::ZERO
        );
    }

    #[test]
    fn initial_buy_matches_constant_product_no_fee() {
        // No fee (protocol 0, creator 0). reserve_in=1000, reserve_out=1000, k=1_000_000.
        // amount_in=1000 => new_reserve_in=2000 => ceil(1_000_000/2000)=500
        // => out = 1000 - 500 = 500.
        let c = cfg(1_000, 1_000, 0, 0);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_000u64), 0).unwrap(),
            U256::from(500u64)
        );
    }

    #[test]
    fn initial_buy_applies_fee_before_curve() {
        // Protocol fee 10% (1000 bps), no creator fee. amount_in=1000 => after fee 900.
        // new_reserve_in = 1000+900 = 1900 => ceil(1_000_000/1900)=527 (526.31..->527)
        // out = 1000 - 527 = 473.
        let c = cfg(1_000, 1_000, 0, 1_000);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_000u64), 0).unwrap(),
            U256::from(473u64)
        );
    }

    #[test]
    fn initial_buy_sums_protocol_and_creator_fee() {
        // Protocol 1000 bps + creator 1000 bps => combined 2000 bps via ONE
        // ceil(amount*totalRate/BPS) (matches the contract's single mulDivUp).
        // amount_in=1000 => fee ceil(1000*2000/10000)=200 => after fee 800.
        // new_reserve_in=1800 => ceil(1_000_000/1800)=556 (555.55..->556)
        // out = 1000 - 556 = 444.
        let c = cfg(1_000, 1_000, 0, 1_000);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_000u64), 1_000).unwrap(),
            U256::from(444u64)
        );
    }

    #[test]
    fn initial_buy_fee_is_ceiled() {
        // 1 bps on 1001 => floor=0 but contract mulDivUp ceils to 1.
        // protocol 1 bps, creator 0. amount_in=1001 => fee ceil(1001*1/10000)=1
        // => after fee 1000 => new_reserve_in=2000 => ceil(1_000_000/2000)=500
        // => out = 500. (A floor fee would give after-fee 1001 and out 501.)
        let c = cfg(1_000, 1_000, 0, 1);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_001u64), 0).unwrap(),
            U256::from(500u64)
        );
    }

    #[test]
    fn initial_buy_errors_when_fees_consume_quote() {
        // 1 wei with any non-zero fee: ceil fee = 1 => after-fee 0 => on-chain
        // `getAmountOut(0,..)` reverts. The helper must mirror that with an Err,
        // not a 0-token "valid" quote (Codex P2).
        let c = cfg(1_000, 1_000, 0, 100);
        assert!(initial_buy_amount_out(&c, U256::from(1u64), 0).is_err());
        // total_rate >= BPS also consumes everything for any positive input.
        assert!(initial_buy_amount_out(&c, U256::from(1_000u64), 10_000).is_err());
    }

    #[test]
    fn initial_buy_errors_on_degenerate_config() {
        // Zero genesis reserve with a positive buy cannot quote — error, not 0.
        let c = cfg(0, 1_000, 0, 0);
        assert!(initial_buy_amount_out(&c, U256::from(1_000u64), 0).is_err());
        // ...but a zero buy is still Ok(0) even on a degenerate config.
        assert_eq!(
            initial_buy_amount_out(&c, U256::ZERO, 0).unwrap(),
            U256::ZERO
        );
    }

    #[test]
    fn initial_buy_caps_at_available() {
        // min=900 => available = 1000-900 = 100. A huge buy caps at 100.
        let c = cfg(1_000, 1_000, 900, 0);
        assert_eq!(
            initial_buy_amount_out(&c, U256::from(1_000_000_000u64), 0).unwrap(),
            U256::from(100u64)
        );
    }

    /// Golden values proven to the wei on testnet against the live `_initialBuy`
    /// for the WMON genesis config (vReserve=70_000e18, vTokenReserve=1_060_569e21,
    /// protocol fee 100 bps), amount_in = 1 MON.
    fn wmon_genesis_cfg() -> V2QuoteConfig {
        V2QuoteConfig {
            decimals: 18,
            virtual_reserve: U256::from(70_000u64) * U256::from(10u64).pow(U256::from(18u64)),
            virtual_token_reserve: U256::from(1_060_569u64)
                * U256::from(10u64).pow(U256::from(21u64)),
            min_token_reserve: U256::ZERO,
            deploy_fee: U256::ZERO,
            graduate_fee: U256::ZERO,
            curve_protocol_fee_rate: 100,
            dex_protocol_fee_rate: 0,
            settlement_threshold: U256::ZERO,
            active: true,
        }
    }

    #[test]
    fn initial_buy_golden_with_creator_fee() {
        // creator_fee_rate = 100 (1%) => protocol+creator = 200 bps total.
        let c = wmon_genesis_cfg();
        let one_mon = U256::from(10u64).pow(U256::from(18u64));
        assert_eq!(
            initial_buy_amount_out(&c, one_mon, 100).unwrap(),
            U256::from_str_radix("14847758131386160593751", 10).unwrap()
        );
    }

    #[test]
    fn initial_buy_golden_no_creator_fee() {
        // creator_fee_rate = 0 => protocol-only (100 bps); must equal the prior
        // protocol-only value so the zero-creator case is unchanged.
        let c = wmon_genesis_cfg();
        let one_mon = U256::from(10u64).pow(U256::from(18u64));
        assert_eq!(
            initial_buy_amount_out(&c, one_mon, 0).unwrap(),
            U256::from_str_radix("14999263724698750689097", 10).unwrap()
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
