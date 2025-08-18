use alloy::primitives::{Address, B256, U256};

#[derive(Debug, Clone)]
pub enum Router {
    Dex(Address),
    BondingCurve(Address),
}

impl Router {
    pub fn address(&self) -> Address {
        match self {
            Router::Dex(addr) => *addr,
            Router::BondingCurve(addr) => *addr,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuyParams {
    pub token: Address,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub to: Address,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u128>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SellParams {
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub token: Address,
    pub to: Address,
    pub deadline: U256,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u128>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SellPermitParams {
    pub amount_in: U256,        // Amount of tokens to sell
    pub amount_out_min: U256,   // Minimum amount of MON to receive
    pub amount_allowance: U256, // amount for the permit
    pub token: Address,         // Address of the token to sell
    pub to: Address,            // Address to receive the MON
    pub deadline: U256,         // Timestamp after which the transaction will revert
    pub v: u8,                  // v part of the signature
    pub r: B256,                // r part of the signature
    pub s: B256,                // s part of the signature
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u128>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CurveState {
    pub real_mon_reserve: U256,
    pub real_token_reserve: U256,
    pub virtual_mon_reserve: U256,
    pub virtual_token_reserve: U256,
    pub k: U256,
    pub target_token_amount: U256,
    pub init_virtual_mon_reserve: U256,
    pub init_virtual_token_reserve: U256,
}

#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub address: Address,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: U256,
}

#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub metadata: TokenMetadata,
    pub balance: U256,
    pub nonce: U256,
}

#[derive(Debug, Clone)]
pub struct AllowanceStatus {
    pub allowance: U256,
    pub balance: U256,
    pub is_sufficient: bool,
    pub is_unlimited: bool,
}

#[derive(Debug)]
pub struct TransactionResult {
    pub transaction_hash: B256,
    pub block_number: Option<u64>,
    pub gas_used: Option<U256>,
    pub status: bool,
    pub logs: Vec<alloy::rpc::types::Log>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, B256, U256};

    #[test]
    fn test_buy_params_creation() {
        let token: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let to: Address = "0x9876543210987654321098765432109876543210"
            .parse()
            .unwrap();

        let params = BuyParams {
            token,
            amount_in: U256::from(1000000000000000000u64), // 1 ETH
            amount_out_min: U256::from(1000),
            to,
            deadline: U256::from(1000000000u64),
            gas_limit: Some(21000), // Standard gas for transfer
            gas_price: Some(20000000000), // 20 gwei
            nonce: Some(42),
        };

        assert_eq!(params.token, token);
        assert_eq!(params.amount_in, U256::from(1000000000000000000u64));
        assert_eq!(params.to, to);
        assert_eq!(params.gas_limit, Some(21000));
        assert_eq!(params.gas_price, Some(20000000000));
        assert_eq!(params.nonce, Some(42));
    }

    #[test]
    fn test_sell_params_creation() {
        let token: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let to: Address = "0x9876543210987654321098765432109876543210"
            .parse()
            .unwrap();

        let params = SellParams {
            amount_in: U256::from(1000000000000000000u64),
            amount_out_min: U256::from(0),
            token,
            to,
            deadline: U256::from(1000000000u64),
            gas_limit: Some(25000), // Slightly higher gas for sell
            gas_price: Some(15000000000), // 15 gwei
            nonce: None,
        };

        assert_eq!(params.token, token);
        assert_eq!(params.amount_in, U256::from(1000000000000000000u64));
        assert_eq!(params.amount_out_min, U256::from(0));
        assert_eq!(params.gas_limit, Some(25000));
        assert_eq!(params.gas_price, Some(15000000000));
        assert_eq!(params.nonce, None);
    }

    #[test]
    fn test_sell_permit_params_creation() {
        let token: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let to: Address = "0x9876543210987654321098765432109876543210"
            .parse()
            .unwrap();

        let params = SellPermitParams {
            amount_in: U256::from(1000000000000000000u64),
            amount_out_min: U256::from(0),
            amount_allowance: U256::from(1000000000000000000u64),
            token,
            to,
            deadline: U256::from(1000000000u64),
            v: 27,
            r: B256::ZERO,
            s: B256::ZERO,
            gas_limit: Some(30000), // Test gas amount
            gas_price: Some(25000000000), // 25 gwei
            nonce: Some(100),
        };

        assert_eq!(params.token, token);
        assert_eq!(params.v, 27);
        assert_eq!(params.r, B256::ZERO);
        assert_eq!(params.gas_limit, Some(30000));
        assert_eq!(params.gas_price, Some(25000000000));
        assert_eq!(params.nonce, Some(100));
    }

    #[test]
    fn test_curve_state_creation() {
        let curve_state = CurveState {
            real_mon_reserve: U256::from(1000000),
            real_token_reserve: U256::from(2000000),
            virtual_mon_reserve: U256::from(3000000),
            virtual_token_reserve: U256::from(4000000),
            k: U256::from(5000000),
            target_token_amount: U256::from(6000000),
            init_virtual_mon_reserve: U256::from(7000000),
            init_virtual_token_reserve: U256::from(8000000),
        };

        assert_eq!(curve_state.real_mon_reserve, U256::from(1000000));
        assert_eq!(curve_state.k, U256::from(5000000));
    }

    #[test]
    fn test_transaction_result_creation() {
        let tx_result = TransactionResult {
            transaction_hash: B256::ZERO,
            block_number: Some(12345),
            gas_used: Some(U256::from(21000)),
            status: true,
            logs: vec![],
        };

        assert_eq!(tx_result.block_number, Some(12345));
        assert_eq!(tx_result.status, true);
        assert_eq!(tx_result.gas_used, Some(U256::from(21000)));
    }
}
