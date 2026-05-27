use crate::{
    constants::{get_nadfun_router_v2, set_network, Network},
    contracts::NadFunRouter,
    types::*,
};
use alloy::{
    network::EthereumWallet,
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use anyhow::Result;
use std::sync::Arc;

/// High-level SDK client for NadFun contract v2.
///
/// v2 uses a single `NadFunRouter` for token creation, quotes, native trading,
/// ERC-20 quote-token trading, permit flows, and exact-output swaps.
pub struct CoreV2 {
    router: NadFunRouter<DynProvider>,
    provider: Arc<DynProvider>,
    wallet_address: Address,
    network: Network,
}

impl CoreV2 {
    pub async fn new(rpc_url: String, private_key: String, network: Network) -> Result<Self> {
        set_network(network);

        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();
        let router_address: Address = get_nadfun_router_v2()
            .ok_or_else(|| anyhow::anyhow!("NadFun contract v2 is not configured for {network:?}"))?
            .parse()?;

        let wallet = EthereumWallet::from(signer);
        let url = rpc_url.parse()?;
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);
        let dyn_provider = Arc::new(DynProvider::new(provider));
        let router = NadFunRouter::new(router_address, dyn_provider.clone());

        Ok(Self {
            router,
            provider: dyn_provider,
            wallet_address,
            network,
        })
    }

    pub async fn create(&self, params: V2CreateParams) -> Result<B256> {
        self.router.create(params).await
    }

    pub async fn create_with_native(&self, params: V2CreateWithNativeParams) -> Result<B256> {
        self.router.create_with_native(params).await
    }

    pub async fn buy(&self, params: V2BuyParams) -> Result<B256> {
        self.router.buy(params).await
    }

    pub async fn buy_with_native(
        &self,
        params: V2BuyWithNativeParams,
        value: U256,
    ) -> Result<B256> {
        self.router.buy_with_native(params, value).await
    }

    pub async fn buy_with_permit(&self, params: V2BuyWithPermitParams) -> Result<B256> {
        self.router.buy_with_permit(params).await
    }

    pub async fn sell(&self, params: V2SellParams) -> Result<B256> {
        self.router.sell(params).await
    }

    pub async fn sell_to_native(&self, params: V2SellToNativeParams) -> Result<B256> {
        self.router.sell_to_native(params).await
    }

    pub async fn sell_with_permit(&self, params: V2SellWithPermitParams) -> Result<B256> {
        self.router.sell_with_permit(params).await
    }

    pub async fn sell_to_native_with_permit(
        &self,
        params: V2SellToNativeWithPermitParams,
    ) -> Result<B256> {
        self.router.sell_to_native_with_permit(params).await
    }

    pub async fn exact_out_buy(&self, params: V2ExactOutBuyParams) -> Result<B256> {
        self.router.exact_out_buy(params).await
    }

    pub async fn exact_out_buy_with_native(
        &self,
        params: V2ExactOutBuyWithNativeParams,
    ) -> Result<B256> {
        self.router.exact_out_buy_with_native(params).await
    }

    pub async fn exact_out_sell(&self, params: V2ExactOutSellParams) -> Result<B256> {
        self.router.exact_out_sell(params).await
    }

    pub async fn exact_out_sell_to_native(
        &self,
        params: V2ExactOutSellToNativeParams,
    ) -> Result<B256> {
        self.router.exact_out_sell_to_native(params).await
    }

    pub async fn get_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router.get_amount_out(token, amount_in, is_buy).await
    }

    pub async fn get_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router.get_amount_in(token, amount_out, is_buy).await
    }

    pub async fn get_bonding_curve_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router
            .get_bonding_curve_amount_out(token, amount_in, is_buy)
            .await
    }

    pub async fn get_bonding_curve_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router
            .get_bonding_curve_amount_in(token, amount_out, is_buy)
            .await
    }

    pub async fn get_dex_amount_out(
        &self,
        token: Address,
        amount_in: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router
            .get_dex_amount_out(token, amount_in, is_buy)
            .await
    }

    pub async fn get_dex_amount_in(
        &self,
        token: Address,
        amount_out: U256,
        is_buy: bool,
    ) -> Result<U256> {
        self.router
            .get_dex_amount_in(token, amount_out, is_buy)
            .await
    }

    pub async fn is_graduated(&self, token: Address) -> Result<bool> {
        self.router.is_graduated(token).await
    }

    pub async fn bonding_curve(&self) -> Result<Address> {
        self.router.bonding_curve().await
    }

    pub async fn token_registry(&self) -> Result<Address> {
        self.router.token_registry().await
    }

    pub async fn wrapped_native(&self) -> Result<Address> {
        self.router.wrapped_native().await
    }

    pub async fn get_receipt(&self, tx_hash: B256) -> Result<TransactionResult> {
        let receipt = self
            .provider
            .get_transaction_receipt(tx_hash)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Transaction receipt not found"))?;

        Ok(TransactionResult {
            transaction_hash: receipt.transaction_hash,
            block_number: receipt.block_number,
            gas_used: Some(U256::from(receipt.gas_used)),
            status: receipt.status(),
            logs: receipt.logs().to_vec(),
        })
    }

    pub fn router(&self) -> &NadFunRouter<DynProvider> {
        &self.router
    }

    pub fn provider(&self) -> &Arc<DynProvider> {
        &self.provider
    }

    pub fn wallet_address(&self) -> Address {
        self.wallet_address
    }

    pub fn network(&self) -> Network {
        self.network
    }
}
