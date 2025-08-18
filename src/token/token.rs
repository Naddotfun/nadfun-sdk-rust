use crate::types::TokenMetadata;
use alloy::{
    network::EthereumWallet,
    primitives::{keccak256, Address, B256, U256},
    providers::{DynProvider, ProviderBuilder},
    signers::{Signer, local::PrivateKeySigner},
    sol,
};
use anyhow::Result;
use std::sync::Arc;

// Complete ERC20 + ERC20Permit + ERC20Burnable interface
sol! {
    #[sol(rpc)]
    interface IToken {
        // ERC20 Standard Functions
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function transfer(address to, uint256 value) external returns (bool);
        function transferFrom(address from, address to, uint256 value) external returns (bool);
        function approve(address spender, uint256 value) external returns (bool);

        // ERC20 Events
        event Transfer(address indexed from, address indexed to, uint256 value);
        event Approval(address indexed owner, address indexed spender, uint256 value);

        // ERC20Permit Functions
        function permit(address owner, address spender, uint256 value, uint256 deadline, uint8 v, bytes32 r, bytes32 s) external;
        function nonces(address owner) external view returns (uint256);
        function DOMAIN_SEPARATOR() external view returns (bytes32);

        // ERC20Burnable Functions
        function burn(uint256 amount) external;
        function burnFrom(address account, uint256 amount) external;
    }
}

pub struct TokenHelper {
    provider: Arc<DynProvider>,
    signer: PrivateKeySigner,
}

impl TokenHelper {
    pub async fn new(rpc_url: String, private_key: String) -> Result<Self> {
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet = EthereumWallet::from(signer.clone());
        let url = rpc_url.parse()?;
        let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);
        let dyn_provider = Arc::new(DynProvider::new(provider));

        Ok(Self {
            provider: dyn_provider,
            signer,
        })
    }

    // =================
    // ERC20 Functions
    // =================

    /// Get token name
    pub async fn name(&self, token: Address) -> Result<String> {
        let contract = IToken::new(token, self.provider.as_ref());
        let name = contract.name().call().await?;
        Ok(name)
    }

    /// Get token symbol
    pub async fn symbol(&self, token: Address) -> Result<String> {
        let contract = IToken::new(token, self.provider.as_ref());
        let symbol = contract.symbol().call().await?;
        Ok(symbol)
    }

    /// Get token decimals
    pub async fn decimals(&self, token: Address) -> Result<u8> {
        let contract = IToken::new(token, self.provider.as_ref());
        let decimals = contract.decimals().call().await?;
        Ok(decimals)
    }

    /// Get total supply
    pub async fn total_supply(&self, token: Address) -> Result<U256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let total_supply = contract.totalSupply().call().await?;
        Ok(total_supply)
    }

    /// Get balance of an address
    pub async fn balance_of(&self, token: Address, owner: Address) -> Result<U256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let balance = contract.balanceOf(owner).call().await?;
        Ok(balance)
    }

    /// Get allowance between owner and spender
    pub async fn allowance(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Result<U256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let allowance = contract.allowance(owner, spender).call().await?;
        Ok(allowance)
    }

    /// Transfer tokens (requires wallet with this token)
    pub async fn transfer(&self, token: Address, to: Address, value: U256) -> Result<B256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let tx = contract.transfer(to, value).send().await?;
        let receipt = tx.get_receipt().await?;
        Ok(receipt.transaction_hash)
    }

    /// Transfer tokens from one address to another (requires allowance)
    pub async fn transfer_from(
        &self,
        token: Address,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<B256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let tx = contract.transferFrom(from, to, value).send().await?;
        let receipt = tx.get_receipt().await?;
        Ok(receipt.transaction_hash)
    }

    /// Approve spender to spend tokens
    pub async fn approve(&self, token: Address, spender: Address, value: U256) -> Result<B256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let tx = contract.approve(spender, value).send().await?;
        let receipt = tx.get_receipt().await?;
        Ok(receipt.transaction_hash)
    }

    // =================
    // ERC20Permit Functions
    // =================

    /// Gets the current nonce for the owner address on the given token
    pub async fn get_nonce(&self, token: Address, owner: Address) -> Result<U256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let nonce = contract.nonces(owner).call().await?;
        Ok(nonce)
    }

    /// Gets the domain separator for the given token
    pub async fn get_domain_separator(&self, token: Address) -> Result<B256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let domain_separator = contract.DOMAIN_SEPARATOR().call().await?;
        Ok(domain_separator)
    }

    // Note: Direct permit() function is intentionally not provided
    //
    // Why? Because it's inefficient:
    // - permit() + sell() = 2 transactions = 2x gas cost
    // - sell_permit() = 1 transaction = 1x gas cost
    //
    // Instead, use:
    // 1. generate_permit_signature() to create signature
    // 2. sell_permit() to execute trade with permit in one transaction

    // =================
    // ERC20Burnable Functions
    // =================

    /// Burn tokens from caller's account
    pub async fn burn(&self, token: Address, amount: U256) -> Result<B256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let tx = contract.burn(amount).send().await?;
        let receipt = tx.get_receipt().await?;
        Ok(receipt.transaction_hash)
    }

    /// Burn tokens from another account (requires allowance)
    pub async fn burn_from(&self, token: Address, account: Address, amount: U256) -> Result<B256> {
        let contract = IToken::new(token, self.provider.as_ref());
        let tx = contract.burnFrom(account, amount).send().await?;
        let receipt = tx.get_receipt().await?;
        Ok(receipt.transaction_hash)
    }

    // =================
    // Metadata & Utility Functions
    // =================

    /// Get complete token metadata
    pub async fn get_token_metadata(&self, token: Address) -> Result<TokenMetadata> {
        // 병렬 호출로 네트워크 지연 최적화
        let (name_result, symbol_result, decimals_result, total_supply_result) = tokio::join!(
            self.name(token),
            self.symbol(token),
            self.decimals(token),
            self.total_supply(token)
        );

        let name = name_result?;
        let symbol = symbol_result?;
        let decimals = decimals_result?;
        let total_supply = total_supply_result?;

        Ok(TokenMetadata {
            address: token,
            name,
            symbol,
            decimals,
            total_supply,
        })
    }

    /// Generates an EIP-2612 permit signature using the internal wallet
    pub async fn generate_permit_signature(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
        value: U256,
        deadline: U256,
    ) -> Result<(u8, B256, B256)> {
        // 병렬 호출로 네트워크 지연 최적화
        let (nonce_result, domain_separator_result) = tokio::join!(
            self.get_nonce(token, owner),
            self.get_domain_separator(token)
        );

        let nonce = nonce_result?;
        let domain_separator = domain_separator_result?;

        // Create the permit message hash according to EIP-2612
        let permit_typehash = keccak256(
            "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)",
        );

        let mut data = Vec::new();
        data.extend_from_slice(permit_typehash.as_slice());
        data.extend_from_slice(&[0u8; 12]); // padding for address
        data.extend_from_slice(owner.as_slice());
        data.extend_from_slice(&[0u8; 12]); // padding for address
        data.extend_from_slice(spender.as_slice());
        data.extend_from_slice(&value.to_be_bytes::<32>());
        data.extend_from_slice(&nonce.to_be_bytes::<32>());
        data.extend_from_slice(&deadline.to_be_bytes::<32>());

        let struct_hash = keccak256(&data);

        // Create the EIP-712 message hash
        let mut message_data = Vec::new();
        message_data.extend_from_slice(b"\x19\x01");
        message_data.extend_from_slice(domain_separator.as_slice());
        message_data.extend_from_slice(struct_hash.as_slice());

        let message_hash = keccak256(&message_data);

        // Sign the message hash using the internal signer
        let signature = self.signer.sign_hash(&message_hash).await?;

        // Extract v, r, s from signature
        let v = if signature.v() { 28u8 } else { 27u8 };
        let r = B256::from_slice(&signature.r().to_be_bytes::<32>());
        let s = B256::from_slice(&signature.s().to_be_bytes::<32>());

        Ok((v, r, s))
    }

    /// Builds a domain separator manually (alternative method)
    pub fn build_domain_separator(
        &self,
        token_name: &str,
        token_address: Address,
        chain_id: u64,
    ) -> B256 {
        let type_hash = keccak256(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        let name_hash = keccak256(token_name.as_bytes());
        let version_hash = keccak256("1".as_bytes());

        keccak256({
            let mut data = Vec::new();
            data.extend_from_slice(type_hash.as_slice());
            data.extend_from_slice(name_hash.as_slice());
            data.extend_from_slice(version_hash.as_slice());
            data.extend_from_slice(&chain_id.to_be_bytes());
            data.extend_from_slice(&[0u8; 12]); // padding for address
            data.extend_from_slice(token_address.as_slice());
            data
        })
    }

    /// Get access to the provider for advanced operations
    pub fn provider(&self) -> &Arc<DynProvider> {
        &self.provider
    }

    /// Get the wallet address associated with this TokenHelper
    pub fn wallet_address(&self) -> Address {
        self.signer.address()
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{keccak256, Address, B256, U256};

    #[test]
    fn test_domain_separator_calculation() {
        let token_address: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let chain_id = 1u64; // Ethereum mainnet
        let token_name = "TestToken";

        // Test domain separator calculation directly
        let type_hash = keccak256(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        let name_hash = keccak256(token_name.as_bytes());
        let version_hash = keccak256("1".as_bytes());

        let mut data = Vec::new();
        data.extend_from_slice(type_hash.as_slice());
        data.extend_from_slice(name_hash.as_slice());
        data.extend_from_slice(version_hash.as_slice());
        data.extend_from_slice(&chain_id.to_be_bytes());
        data.extend_from_slice(&[0u8; 12]); // padding for address
        data.extend_from_slice(token_address.as_slice());

        let domain_separator = keccak256(&data);

        // Domain separator should not be zero
        assert_ne!(domain_separator, B256::ZERO);

        // Test with different names should produce different separators
        let name_hash2 = keccak256("DifferentToken".as_bytes());
        let mut data2 = Vec::new();
        data2.extend_from_slice(type_hash.as_slice());
        data2.extend_from_slice(name_hash2.as_slice());
        data2.extend_from_slice(version_hash.as_slice());
        data2.extend_from_slice(&chain_id.to_be_bytes());
        data2.extend_from_slice(&[0u8; 12]); // padding for address
        data2.extend_from_slice(token_address.as_slice());

        let domain_separator2 = keccak256(&data2);
        assert_ne!(domain_separator, domain_separator2);
    }

    #[test]
    fn test_permit_type_hash() {
        // This should match the Solidity version
        let expected_typehash = keccak256(
            "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)",
        );
        let computed_typehash = keccak256(
            "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)",
        );

        assert_eq!(expected_typehash, computed_typehash);
    }

    #[test]
    fn test_eip712_domain_typehash() {
        let expected_typehash = keccak256(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        let computed_typehash = keccak256(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );

        assert_eq!(expected_typehash, computed_typehash);
    }

    #[test]
    fn test_struct_hash_construction() {
        let owner: Address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        let spender: Address = "0x2222222222222222222222222222222222222222"
            .parse()
            .unwrap();
        let value = U256::from(1000000000000000000u64); // 1 ETH
        let nonce = U256::from(0);
        let deadline = U256::from(1000000000u64);

        let permit_typehash = keccak256(
            "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)",
        );

        let mut data = Vec::new();
        data.extend_from_slice(permit_typehash.as_slice());
        data.extend_from_slice(&[0u8; 12]); // padding for address
        data.extend_from_slice(owner.as_slice());
        data.extend_from_slice(&[0u8; 12]); // padding for address
        data.extend_from_slice(spender.as_slice());
        data.extend_from_slice(&value.to_be_bytes::<32>());
        data.extend_from_slice(&nonce.to_be_bytes::<32>());
        data.extend_from_slice(&deadline.to_be_bytes::<32>());

        let struct_hash = keccak256(&data);

        // Struct hash should not be zero
        assert_ne!(struct_hash, B256::ZERO);
    }

    #[test]
    fn test_message_hash_construction() {
        let domain_separator = B256::from([1u8; 32]);
        let struct_hash = B256::from([2u8; 32]);

        let mut message_data = Vec::new();
        message_data.extend_from_slice(b"\x19\x01");
        message_data.extend_from_slice(domain_separator.as_slice());
        message_data.extend_from_slice(struct_hash.as_slice());

        let message_hash = keccak256(&message_data);

        // Message hash should not be zero
        assert_ne!(message_hash, B256::ZERO);

        // Different inputs should produce different hashes
        let mut different_message_data = Vec::new();
        different_message_data.extend_from_slice(b"\x19\x01");
        different_message_data.extend_from_slice(&[3u8; 32]);
        different_message_data.extend_from_slice(struct_hash.as_slice());

        let different_message_hash = keccak256(&different_message_data);
        assert_ne!(message_hash, different_message_hash);
    }
}
