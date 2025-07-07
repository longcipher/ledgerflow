use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DepositEvent {
    pub id: Option<i64>,
    pub chain_name: String,
    pub contract_address: String,
    pub order_id: String,
    pub sender: String,
    pub amount: String,
    pub transaction_hash: String,
    pub block_number: i64,
    pub log_index: i64,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub processed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChainState {
    pub chain_name: String,
    pub contract_address: String,
    pub last_scanned_block: i64,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct ParsedDepositEvent {
    pub order_id: [u8; 32],
    pub sender: Address,
    pub amount: U256,
    pub transaction_hash: String,
    pub block_number: u64,
    pub log_index: u64,
}
