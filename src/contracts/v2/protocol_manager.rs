//! `ProtocolManager` v2 binding (read-only slice used by the SDK).
//!
//! The protocol manager holds per-quote-token protocol config. The SDK reads
//! `deployFee(quoteToken)` to fund v2 token creation correctly — the on-chain
//! `NadFunRouter::create` / `createWithNative` requires
//! `msg.value >= deployFee(quoteToken) + buyQuoteAmount`.

use crate::types::v2::V2QuoteConfig;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::Result;
use std::sync::Arc;

sol! {
    #[sol(rpc)]
    interface IProtocolManagerV2 {
        // Field order on-chain-verified (testnet, 2026-05-31) against the
        // dedicated per-field getters. Do not reorder.
        struct QuoteConfig {
            uint8 decimals;
            uint256 virtualReserve;
            uint256 virtualTokenReserve;
            uint256 minTokenReserve;
            uint256 deployFee;
            uint256 graduateFee;
            uint16 curveProtocolFeeRate;
            uint16 dexProtocolFeeRate;
            uint256 settlementThreshold;
            bool active;
        }
        function deployFee(address quoteToken) external view returns (uint256);
        function getConfig(address quoteToken) external view returns (QuoteConfig memory);
    }
}

/// SDK wrapper around the v2 `ProtocolManager` view surface.
pub struct ProtocolManagerV2<P> {
    pub address: Address,
    pub provider: Arc<P>,
}

impl<P: Provider + Clone> ProtocolManagerV2<P> {
    pub fn new(address: Address, provider: Arc<P>) -> Self {
        Self { address, provider }
    }

    /// One-time deploy fee charged when creating a token quoted in
    /// `quote_token`. Denominated in the quote token.
    pub async fn deploy_fee(&self, quote_token: Address) -> Result<U256> {
        let contract = IProtocolManagerV2::new(self.address, self.provider.as_ref());
        Ok(contract.deployFee(quote_token).call().await?)
    }

    /// Full per-quote-token protocol config (genesis curve params + fees +
    /// graduation economics). See [`V2QuoteConfig`].
    pub async fn get_config(&self, quote_token: Address) -> Result<V2QuoteConfig> {
        let contract = IProtocolManagerV2::new(self.address, self.provider.as_ref());
        Ok(contract.getConfig(quote_token).call().await?.into())
    }
}

impl From<IProtocolManagerV2::QuoteConfig> for V2QuoteConfig {
    fn from(c: IProtocolManagerV2::QuoteConfig) -> Self {
        V2QuoteConfig {
            decimals: c.decimals,
            virtual_reserve: c.virtualReserve,
            virtual_token_reserve: c.virtualTokenReserve,
            min_token_reserve: c.minTokenReserve,
            deploy_fee: c.deployFee,
            graduate_fee: c.graduateFee,
            curve_protocol_fee_rate: c.curveProtocolFeeRate,
            dex_protocol_fee_rate: c.dexProtocolFeeRate,
            settlement_threshold: c.settlementThreshold,
            active: c.active,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::sol_types::SolValue;

    /// Round-trips the generated `QuoteConfig` struct through ABI
    /// encode/decode and the `From` mapping with a distinct value per field,
    /// so any field-swap in the mapping is caught. The 10-field ABI layout
    /// itself was verified field-by-field against the deployed testnet
    /// ProtocolManager (`0x2F98…fB41`) on 2026-05-31 — see the Phase 2 plan.
    #[test]
    fn quote_config_maps_every_field() {
        let on_chain = IProtocolManagerV2::QuoteConfig {
            decimals: 18,
            virtualReserve: U256::from(1u64),
            virtualTokenReserve: U256::from(2u64),
            minTokenReserve: U256::from(3u64),
            deployFee: U256::from(4u64),
            graduateFee: U256::from(5u64),
            curveProtocolFeeRate: 6,
            dexProtocolFeeRate: 7,
            settlementThreshold: U256::from(8u64),
            active: true,
        };
        let raw = on_chain.abi_encode();
        let decoded =
            IProtocolManagerV2::QuoteConfig::abi_decode(&raw).expect("decode QuoteConfig");
        let cfg: V2QuoteConfig = decoded.into();

        assert_eq!(cfg.decimals, 18);
        assert_eq!(cfg.virtual_reserve, U256::from(1u64));
        assert_eq!(cfg.virtual_token_reserve, U256::from(2u64));
        assert_eq!(cfg.min_token_reserve, U256::from(3u64));
        assert_eq!(cfg.deploy_fee, U256::from(4u64));
        assert_eq!(cfg.graduate_fee, U256::from(5u64));
        assert_eq!(cfg.curve_protocol_fee_rate, 6);
        assert_eq!(cfg.dex_protocol_fee_rate, 7);
        assert_eq!(cfg.settlement_threshold, U256::from(8u64));
        assert!(cfg.active);
    }
}
