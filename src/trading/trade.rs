use crate::{
    constants::*,
    contracts::{BondingCurveRouter, DexRouter, LensContract},
    types::*,
};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{DynProvider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use anyhow::Result;
use std::sync::Arc;

pub struct Trade {
    bonding_curve_router: BondingCurveRouter<DynProvider>,
    dex_router: DexRouter<DynProvider>,
    lens: LensContract<DynProvider>,
    provider: Arc<DynProvider>,
    wallet_address: Address,
}

impl Trade {
    /// Create a new Trade instance from a private key string (recommended)
    pub async fn new(rpc_url: String, private_key: String) -> Result<Trade> {
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();

        // Use default contract addresses
        let lens_address: Address = LENS_ADDRESS.parse()?;
        let bonding_curve_router_address: Address = BONDING_CURVE_ROUTER.parse()?;
        let dex_router_address: Address = DEX_ROUTER.parse()?;
        let bonding_curve_address: Address = BONDING_CURVE.parse()?;

        let wallet = EthereumWallet::from(signer);
        let url = rpc_url.parse()?;
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);
        let dyn_provider = Arc::new(DynProvider::new(provider));

        let bonding_curve_router = BondingCurveRouter::new(
            bonding_curve_router_address,
            bonding_curve_address,
            dyn_provider.clone(),
        );

        let dex_router = DexRouter::new(dex_router_address, dyn_provider.clone());
        let lens = LensContract::new(lens_address, dyn_provider.clone());

        Ok(Trade {
            bonding_curve_router,
            dex_router,
            lens,
            provider: dyn_provider,
            wallet_address,
        })
    }
}

impl Trade {
    // Auto-routing functions using lens contract
    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<(Router, U256)> {
        let (router_address, amount_out) =
            self.lens.get_amount_out(token, amount_in, is_buy).await?;

        let router = if router_address == self.dex_router.address {
            Router::Dex(router_address)
        } else if router_address == self.bonding_curve_router.address {
            Router::BondingCurve(router_address)
        } else {
            return Err(anyhow::anyhow!(
                "Unknown router address: {}",
                router_address
            ));
        };

        Ok((router, amount_out))
    }

    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<(Router, U256)> {
        let (router_address, amount_in) =
            self.lens.get_amount_in(token, amount_out, is_buy).await?;

        let router = if router_address == self.dex_router.address {
            Router::Dex(router_address)
        } else if router_address == self.bonding_curve_router.address {
            Router::BondingCurve(router_address)
        } else {
            return Err(anyhow::anyhow!(
                "Unknown router address: {}",
                router_address
            ));
        };

        Ok((router, amount_in))
    }

    pub async fn buy(&self, params: BuyParams, router: Router) -> Result<TransactionResult> {
        match router {
            Router::Dex(_) => self.dex_router.buy(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.buy(params).await,
        }
    }

    pub async fn sell(&self, params: SellParams, router: Router) -> Result<TransactionResult> {
        match router {
            Router::Dex(_) => self.dex_router.sell(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.sell(params).await,
        }
    }

    /// Sell tokens using SellPermitParams struct
    /// User must provide valid permit signature (v, r, s)
    pub async fn sell_permit(
        &self,
        params: SellPermitParams,
        router: Router,
    ) -> Result<TransactionResult> {
        match router {
            Router::Dex(_) => self.dex_router.sell_permit(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.sell_permit(params).await,
        }
    }

    // Bonding curve specific functions
    pub async fn available_buy_tokens(&self, token: Address) -> Result<(U256, U256)> {
        self.bonding_curve_router.available_buy_tokens(token).await
    }

    pub async fn get_curve_state(&self, token: Address) -> Result<CurveState> {
        self.bonding_curve_router.get_curve_state(token).await
    }

    // Utility functions
    pub async fn is_listed(&self, token: Address) -> Result<bool> {
        self.bonding_curve_router.is_listed(token).await
    }

    pub async fn is_locked(&self, token: Address) -> Result<bool> {
        self.bonding_curve_router.is_locked(token).await
    }

    // Access to individual routers (advanced usage)
    pub fn bonding_curve_router(&self) -> &BondingCurveRouter<DynProvider> {
        &self.bonding_curve_router
    }

    pub fn dex_router(&self) -> &DexRouter<DynProvider> {
        &self.dex_router
    }

    pub fn lens(&self) -> &LensContract<DynProvider> {
        &self.lens
    }

    pub fn provider(&self) -> &Arc<DynProvider> {
        &self.provider
    }

    pub fn wallet_address(&self) -> Address {
        self.wallet_address
    }
}
