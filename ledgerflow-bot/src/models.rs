use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub telegram_id: i64,
    pub email: Option<String>,
    pub evm_address: Option<String>,
    pub encrypted_pk: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: i64,
    pub order_id: String,
    pub account_id: i64,
    pub broker_id: String,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub transaction_hash: Option<String>,
    pub notified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub account_id: i64,
    pub amount: String,
    pub token_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub order_id: String,
    pub payment_address: String,
    pub amount: String,
    pub token_address: String,
    pub chain_id: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub account_id: i64,
    pub total_balance: String,
    pub completed_orders_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub address: String,
    pub private_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentDetails {
    pub order_id: String,
    pub payment_address: String,
    pub amount: String,
    pub token_symbol: String,
    pub chain_name: String,
    pub qr_code: Option<String>,
}

// User session state management
#[derive(Debug, Clone, PartialEq)]
pub enum UserState {
    None,
    AwaitingEmail,
    AwaitingUsername(String), // Save email
    AwaitingDepositAmount,
}

#[derive(Debug, Clone)]
pub struct UserSession {
    pub state: UserState,
    #[allow(dead_code)]
    pub temp_email: Option<String>,
}

impl UserSession {
    pub fn new() -> Self {
        Self {
            state: UserState::None,
            temp_email: None,
        }
    }
}

impl Default for UserSession {
    fn default() -> Self {
        Self::new()
    }
}
