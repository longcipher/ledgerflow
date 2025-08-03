use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct SuiDepositEvent {
    pub id: i32,
    pub chain_id: String,
    pub package_id: String,
    pub vault_id: String,
    pub payer: String,
    pub order_id: String,
    pub amount: bigdecimal::BigDecimal,
    pub timestamp: i64,
    pub deposit_index: i64,
    pub checkpoint_sequence: i64,
    pub transaction_digest: String,
    pub event_index: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct SuiWithdrawEvent {
    pub id: i32,
    pub chain_id: String,
    pub package_id: String,
    pub vault_id: String,
    pub owner: String,
    pub recipient: String,
    pub amount: bigdecimal::BigDecimal,
    pub timestamp: i64,
    pub checkpoint_sequence: i64,
    pub transaction_digest: String,
    pub event_index: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct SuiOwnershipTransferEvent {
    pub id: i32,
    pub chain_id: String,
    pub package_id: String,
    pub vault_id: String,
    pub previous_owner: String,
    pub new_owner: String,
    pub timestamp: i64,
    pub checkpoint_sequence: i64,
    pub transaction_digest: String,
    pub event_index: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IndexerState {
    pub id: i32,
    pub chain_id: String,
    pub package_id: String,
    pub last_processed_checkpoint: i64,
    pub last_processed_transaction: Option<String>,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

/// Raw deposit event data from Sui blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiDepositEventData {
    pub vault_id: String,
    pub payer: String,
    pub order_id: Vec<u8>,
    pub amount: u64,
    pub timestamp: u64,
    pub deposit_index: u64,
}

/// Raw withdrawal event data from Sui blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiWithdrawEventData {
    pub vault_id: String,
    pub owner: String,
    pub recipient: String,
    pub amount: u64,
    pub timestamp: u64,
}

/// Raw ownership transfer event data from Sui blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiOwnershipTransferEventData {
    pub vault_id: String,
    pub previous_owner: String,
    pub new_owner: String,
    pub timestamp: u64,
}
