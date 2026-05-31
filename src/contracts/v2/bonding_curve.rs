//! NadFun v2 bonding curve binding.
//!
//! Wraps `BondingCurve.sol` view methods. Trading happens through
//! `NadFunRouter` (which calls into this contract); the SDK uses this binding
//! only for quoting, curve state inspection, and event decoding.

use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::{Context, Result};
use std::sync::Arc;

use crate::types::v2::V2Curve;

sol!(
    #[sol(rpc)]
    IBondingCurveV2,
    "abi/v2/BondingCurve.json"
);

pub struct BondingCurveV2<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> BondingCurveV2<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// Quote: expected output for `amountIn` of input. `is_buy = true` quotes
    /// quote→token; `false` quotes token→quote.
    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let amount_out = contract
            .getAmountOut(token, amount_in, is_buy)
            .call()
            .await?;
        Ok(amount_out)
    }

    /// Inverse quote: required input to receive `amountOut` of output.
    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let amount_in = contract
            .getAmountIn(token, amount_out, is_buy)
            .call()
            .await?;
        Ok(amount_in)
    }

    /// Sniping penalty rate (BPS) for a buy at the current block, used to
    /// preview the anti-sniping fee. Driven by `block.number - createdAtBlock`
    /// against the ProtocolManager's lookup table.
    pub async fn get_sniping_penalty(&self, token: Address) -> Result<U256> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let bps = contract.getSnipingPenalty(token).call().await?;
        Ok(bps)
    }

    /// Quote token address for `token` (e.g. WMON or USDT depending on config).
    pub async fn get_quote_token(&self, token: Address) -> Result<Address> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let quote = contract.getQuoteToken(token).call().await?;
        Ok(quote)
    }

    /// Whether the protocol is paused (no trading allowed).
    pub async fn is_halted(&self) -> Result<bool> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let halted = contract.isHalted().call().await?;
        Ok(halted)
    }

    /// Contract version (matches the `CurveVersion` enum on-chain).
    pub async fn version(&self) -> Result<u8> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let v = contract.VERSION().call().await?;
        Ok(v)
    }

    /// Linked CreatorFeeProcessor address (where creator fees are routed).
    pub async fn creator_fee_processor(&self) -> Result<Address> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let addr = contract.creatorFeeProcessor().call().await?;
        Ok(addr)
    }

    /// Full bonding curve state for `token` as stored on-chain.
    ///
    /// Maps the 16-field `IBondingCurve.Curve` ABI tuple to [`V2Curve`].
    /// Invariant: `curve.k == curve.virtual_quote_reserve * curve.virtual_token_reserve`.
    pub async fn get_curve(&self, token: Address) -> Result<V2Curve> {
        let contract = IBondingCurveV2::new(self.address, self.provider.as_ref());
        let raw = contract
            .getCurve(token)
            .call()
            .await
            .with_context(|| format!("getCurve({token}) failed"))?;
        Ok(V2Curve {
            token: raw.token,
            creator: raw.creator,
            quote_token: raw.quoteToken,
            virtual_quote_reserve: raw.virtualQuoteReserve,
            virtual_token_reserve: raw.virtualTokenReserve,
            k: raw.k,
            min_token_reserve: raw.minTokenReserve,
            initial_quote_reserve: raw.initialQuoteReserve,
            initial_token_reserve: raw.initialTokenReserve,
            created_at_block: raw.createdAtBlock,
            graduated: raw.graduated,
            creator_fee_rate: raw.creatorFeeRate,
            version: raw.version,
            dex_type: raw.dexType,
            pair: raw.pair,
            graduate_fee: raw.graduateFee,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
    use alloy::sol_types::SolValue;

    /// Offline ABI-decode test for `get_curve` field mapping.
    ///
    /// Fixture: 512-byte (16 × 32-word) ABI encoding with the following known values:
    ///   token              = 0x91e0FCbF3e1d51F3fcB9EE9EebbcBFf04FC1BF45
    ///   creator            = 0x1111...1111
    ///   quote_token        = 0x5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd (WMON testnet)
    ///   virtual_quote_reserve = 1_000 * 10^18
    ///   virtual_token_reserve = 800_000_000 * 10^18
    ///   k                  = virtual_quote_reserve * virtual_token_reserve
    ///   graduated          = false
    ///   creator_fee_rate   = 300 bps
    ///   version            = 2
    ///   pair               = Address::ZERO  (pre-graduation)
    ///
    /// Verified end-to-end by running the same bytes through an alloy probe crate.
    ///
    /// Key invariant (Rule 9): k == virtual_quote_reserve * virtual_token_reserve
    /// must hold — this is the constant-product AMM identity.
    #[test]
    fn get_curve_maps_all_fields_and_k_invariant() {
        // 512-byte (16 × 32-word) ABI encoding.
        // Verified by running through an alloy probe crate: token, quoteToken,
        // k == vQR*vTR, graduated=false, version=2.
        let fixture_hex = concat!(
            // word 0:  token = 0x91e0...BF45
            "00000000000000000000000091e0fcbf3e1d51f3fcb9ee9eebbcbff04fc1bf45",
            // word 1:  creator = 0x1111...1111
            "0000000000000000000000001111111111111111111111111111111111111111",
            // word 2:  quoteToken = WMON testnet
            "0000000000000000000000005a4e0bfdef88c9032cb4d24338c5eb3d3870bfdd",
            // word 3:  virtualQuoteReserve = 1_000 * 10^18
            "00000000000000000000000000000000000000000000003635c9adc5dea00000",
            // word 4:  virtualTokenReserve = 800_000_000 * 10^18
            "00000000000000000000000000000000000000000295be96e640669720000000",
            // word 5:  k = vQR * vTR (800_000_000_000 * 10^36)
            "0000000000000000000000008c213d9da502de454526f422cc34000000000000",
            // word 6:  minTokenReserve = 100_000_000 * 10^18
            "00000000000000000000000000000000000000000052b7d2dcc80cd2e4000000",
            // word 7:  initialQuoteReserve = 800 * 10^18
            "00000000000000000000000000000000000000000000002b5e3af16b18800000",
            // word 8:  initialTokenReserve = 900_000_000 * 10^18
            "000000000000000000000000000000000000000002e87669c308736a04000000",
            // word 9:  createdAtBlock = 12_345_678
            "0000000000000000000000000000000000000000000000000000000000bc614e",
            // word 10: graduated = false
            "0000000000000000000000000000000000000000000000000000000000000000",
            // word 11: creatorFeeRate = 300 bps
            "000000000000000000000000000000000000000000000000000000000000012c",
            // word 12: version = 2
            "0000000000000000000000000000000000000000000000000000000000000002",
            // word 13: dexType = 0
            "0000000000000000000000000000000000000000000000000000000000000000",
            // word 14: pair = Address::ZERO (pre-graduation)
            "0000000000000000000000000000000000000000000000000000000000000000",
            // word 15: graduateFee = 5 * 10^18
            "0000000000000000000000000000000000000000000000004563918244f40000",
        );

        let fixture_bytes = hex::decode(fixture_hex).expect("fixture hex must be valid");

        use IBondingCurve::Curve;
        let raw = Curve::abi_decode(&fixture_bytes).expect("fixture must ABI-decode");

        // Map to SDK type (same logic as BondingCurveV2::get_curve).
        let curve = V2Curve {
            token: raw.token,
            creator: raw.creator,
            quote_token: raw.quoteToken,
            virtual_quote_reserve: raw.virtualQuoteReserve,
            virtual_token_reserve: raw.virtualTokenReserve,
            k: raw.k,
            min_token_reserve: raw.minTokenReserve,
            initial_quote_reserve: raw.initialQuoteReserve,
            initial_token_reserve: raw.initialTokenReserve,
            created_at_block: raw.createdAtBlock,
            graduated: raw.graduated,
            creator_fee_rate: raw.creatorFeeRate,
            version: raw.version,
            dex_type: raw.dexType,
            pair: raw.pair,
            graduate_fee: raw.graduateFee,
        };

        assert_eq!(
            curve.token,
            address!("91e0FCbF3e1d51F3fcB9EE9EebbcBFf04FC1BF45"),
        );
        assert_eq!(
            curve.quote_token,
            address!("5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd"),
            "quote_token should be WMON testnet address"
        );
        assert!(!curve.graduated, "token should not be graduated");
        assert_eq!(curve.version, 2, "version must be 2");
        assert_eq!(curve.pair, Address::ZERO, "pair is ZERO before graduation");

        // Core AMM invariant: k == vQR * vTR (constant product).
        // A broken mapping (e.g. swapped fields) would cause this to fail.
        assert_eq!(
            curve.k,
            curve.virtual_quote_reserve * curve.virtual_token_reserve,
            "k must equal virtual_quote_reserve * virtual_token_reserve (constant product)"
        );
    }
}
