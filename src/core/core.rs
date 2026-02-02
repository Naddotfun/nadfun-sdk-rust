use crate::{
    api::ApiClient,
    constants::*,
    contracts::{BondingCurveRouter, CreatorClient, DexRouter, Lens},
    core::gas::{estimate_gas, GasEstimationParams},
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

pub struct Core {
    bonding_curve_router: BondingCurveRouter<DynProvider>,
    dex_router: DexRouter<DynProvider>,
    lens: Lens<DynProvider>,
    provider: Arc<DynProvider>,
    wallet_address: Address,
    network: Network,
}

impl Core {
    /// Create a new Core instance from a private key string (recommended)
    ///
    /// # Arguments
    /// * `rpc_url` - RPC endpoint URL
    /// * `private_key` - Private key string (with or without 0x prefix)
    /// * `network` - Network to use (Mainnet or Testnet)
    pub async fn new(rpc_url: String, private_key: String, network: Network) -> Result<Core> {
        // Set the global network configuration
        crate::constants::set_network(network);

        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();

        // Use current network contract addresses (automatically uses the network we just set)
        let lens_address: Address = get_lens_address().parse()?;
        let bonding_curve_router_address: Address = get_bonding_curve_router().parse()?;
        let dex_router_address: Address = get_dex_router().parse()?;
        let bonding_curve_address: Address = get_bonding_curve().parse()?;

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
        let lens = Lens::new(lens_address, dyn_provider.clone());

        Ok(Core {
            bonding_curve_router,
            dex_router,
            lens,
            provider: dyn_provider,
            wallet_address,
            network,
        })
    }
}

impl Core {
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

    pub async fn buy(&self, params: BuyParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.dex_router.buy(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.buy(params).await,
        }
    }

    pub async fn sell(&self, params: SellParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.dex_router.sell(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.sell(params).await,
        }
    }

    /// Sell tokens using SellPermitParams struct
    /// User must provide valid permit signature (v, r, s)
    pub async fn sell_permit(&self, params: SellPermitParams, router: Router) -> Result<B256> {
        match router {
            Router::Dex(_) => self.dex_router.sell_permit(params).await,
            Router::BondingCurve(_) => self.bonding_curve_router.sell_permit(params).await,
        }
    }

    /// Get transaction receipt for a given transaction hash
    ///
    /// This allows you to check the status and details of a transaction
    /// after it has been submitted.
    ///
    /// # Arguments
    /// * `tx_hash` - Transaction hash returned from buy/sell operations
    ///
    /// # Returns
    /// * `TransactionResult` - Complete transaction details including status, gas used, and logs
    ///
    /// # Example
    /// ```rust,ignore
    /// let tx_hash = core.buy(buy_params, router).await?;
    /// let receipt = core.get_receipt(tx_hash).await?;
    /// println!("Transaction status: {}", receipt.status);
    /// println!("Gas used: {:?}", receipt.gas_used);
    /// ```
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

    // Lens utility functions (wrapped for convenience)

    /// Get available buy tokens and required MON amount
    pub async fn available_buy_tokens(&self, token: Address) -> Result<(U256, U256)> {
        self.lens.available_buy_tokens(token).await
    }

    /// Check if token is locked
    pub async fn is_locked(&self, token: Address) -> Result<bool> {
        self.lens.is_locked(token).await
    }

    /// Check if token is graduated (listed on DEX)
    pub async fn is_graduated(&self, token: Address) -> Result<bool> {
        self.lens.is_graduated(token).await
    }

    /// Get initial buy amount out for token creation
    ///
    /// Calculate how many tokens you'll receive when creating a new token
    /// with a given MON amount (typically used during token creation).
    pub async fn get_initial_buy_amount_out(&self, amount_in: U256) -> Result<U256> {
        self.lens.get_initial_buy_amount_out(amount_in).await
    }

    /// Get deploy fee for token creation
    pub async fn get_deploy_fee(&self) -> Result<U256> {
        self.bonding_curve_router.get_deploy_fee().await
    }

    /// Get bonding curve progress percentage
    ///
    /// Returns the progress of the bonding curve towards graduation (0-10000 = 0-100%)
    pub async fn get_progress(&self, token: Address) -> Result<U256> {
        self.lens.get_progress(token).await
    }

    // Access to individual routers (advanced usage)
    pub fn bonding_curve_router(&self) -> &BondingCurveRouter<DynProvider> {
        &self.bonding_curve_router
    }

    pub fn dex_router(&self) -> &DexRouter<DynProvider> {
        &self.dex_router
    }

    pub fn lens(&self) -> &Lens<DynProvider> {
        &self.lens
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

    /// Estimate gas for trading operations using the unified gas estimation system
    ///
    /// This is a convenience method that wraps the standalone estimate_gas function
    /// and automatically provides the provider and handles the common use case.
    ///
    /// # Example
    /// ```rust,ignore
    /// use nadfun_sdk::{Core, GasEstimationParams};
    ///
    /// let params = GasEstimationParams::Buy {
    ///     token,
    ///     amount_in: mon_amount,
    ///     amount_out_min: min_tokens,
    ///     to: wallet,
    ///     deadline,
    /// };
    ///
    /// let estimated_gas = core.estimate_gas(&router, params).await?;
    /// let gas_with_buffer = estimated_gas * 120 / 100; // Add 20% buffer
    /// ```
    pub async fn estimate_gas(&self, router: &Router, params: GasEstimationParams) -> Result<u64> {
        estimate_gas(self.provider.clone(), router, params).await
    }

    /// Create a new token using the complete token creation flow
    ///
    /// This function handles the entire token creation process:
    /// 1. Download image from URI and upload to metadata server
    /// 2. Create metadata on server
    /// 3. Get salt value from server
    /// 4. Execute create transaction on bonding curve
    ///
    /// # Arguments
    /// * `params` - Token creation parameters including metadata and transaction details
    /// * `api_client` - ApiClient for API access (with optional API key for higher rate limits)
    ///
    /// # Returns
    /// * `TokenCreationResult` - Contains token address, metadata URI, image URI, salt, and transaction hash
    ///
    /// # Example
    /// ```rust,ignore
    /// use nadfun_sdk::{Core, CreateTokenParams, ApiClient};
    /// use alloy::primitives::utils::parse_ether;
    ///
    /// // Create API client (with optional API key)
    /// let api = ApiClient::new().with_api_key("your-api-key".to_string());
    ///
    /// let params = CreateTokenParams {
    ///     name: "My Token".to_string(),
    ///     symbol: "MTK".to_string(),
    ///     description: "My awesome token".to_string(),
    ///     image_uri: "https://example.com/image.png".to_string(),
    ///     website: Some("https://example.com".to_string()),
    ///     twitter: Some("@mytoken".to_string()),
    ///     telegram: Some("@mytokenchat".to_string()),
    ///     creator_address: wallet_address,
    ///     amount_out: parse_ether("1000000")?,
    ///     value: parse_ether("1.5")?, // 1.5 MON
    /// };
    ///
    /// let result = core.create_token(params, &api).await?;
    /// println!("Token created at: {}", result.token_address);
    /// ```
    pub async fn create_token(
        &self,
        params: CreateTokenParams,
        api_client: &ApiClient,
    ) -> Result<TokenCreationResult> {
        // Step 1-3: Prepare token creation (image upload, metadata, salt, token_address)
        let (metadata_uri, image_uri, salt, token_address_str, is_nsfw) =
            api_client.prepare_token_creation(&params).await?;

        // Parse token address
        let token_address: Address = token_address_str.parse()?;

        // Get deploy fee
        let deploy_fee = self.get_deploy_fee().await?;
        let total_value = params.value + deploy_fee;

        // Step 4: Execute create transaction on bonding curve - returns tx_hash immediately
        let tx_hash = self
            .bonding_curve_router
            .create(
                params.name.clone(),
                params.symbol.clone(),
                metadata_uri.clone(),
                params.amount_out,
                salt,
                params.action_id, // Actor type (CapricornActor or AmplifyActor)
                total_value,      // Initial buy amount + deploy fee
                None,             // gas_limit (auto-estimate)
                None,             // gas_price (use network default)
                None,             // nonce (auto-increment)
            )
            .await?;

        Ok(TokenCreationResult {
            token_address,
            metadata_uri,
            image_uri,
            salt: format!("0x{}", hex::encode(salt)),
            transaction_hash: tx_hash,
            is_nsfw, // Return is_nsfw status from server
        })
    }

    /// Claim creator reward from CreatorTreasury for a single token
    ///
    /// Claims accumulated trading fees for a token you created.
    /// The wMON reward is automatically converted to native MON.
    ///
    /// # Arguments
    /// * `params` - Claim parameters including token, amount, and merkle proof
    ///
    /// # Returns
    /// * Transaction hash
    ///
    /// # Example
    /// ```rust,ignore
    /// use nadfun_sdk::{Core, CreatorClaimParams, TokenCreationClient};
    ///
    /// // Get created tokens and their reward info from API
    /// let client = TokenCreationClient::new();
    /// let response = client.get_created_tokens(wallet, 1, 10).await?;
    ///
    /// // Build claim params for a claimable token
    /// if let Some(params) = TokenCreationClient::build_claim_params(&response.tokens[0]) {
    ///     let tx_hash = core.claim_creator_reward(params).await?;
    ///     println!("Claimed reward, tx: {}", tx_hash);
    /// }
    /// ```
    pub async fn claim_creator_reward(&self, params: CreatorClaimParams) -> Result<B256> {
        let treasury_address: Address = get_creator_treasury().parse()?;
        let creator = CreatorClient::new(treasury_address, self.provider.clone());
        creator.claim(params).await
    }

    /// Claim creator rewards for multiple tokens in a single transaction
    ///
    /// More gas efficient than calling claim_creator_reward() multiple times.
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = TokenCreationClient::new();
    /// let response = client.get_created_tokens(wallet, 1, 10).await?;
    ///
    /// if let Some(batch_params) = TokenCreationClient::build_batch_claim_params(&response.tokens) {
    ///     let tx_hash = core.claim_creator_rewards_batch(batch_params).await?;
    ///     println!("Batch claimed rewards, tx: {}", tx_hash);
    /// }
    /// ```
    pub async fn claim_creator_rewards_batch(
        &self,
        params: CreatorBatchClaimParams,
    ) -> Result<B256> {
        let treasury_address: Address = get_creator_treasury().parse()?;
        let creator = CreatorClient::new(treasury_address, self.provider.clone());
        creator.claim_batch(params).await
    }
}
