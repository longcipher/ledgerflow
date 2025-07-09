use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: i64,
    pub order_id: String,
    pub account_id: i64,
    pub broker_id: String,
    pub amount: String, // Using String to handle arbitrary precision
    pub token_address: String,
    pub chain_id: i64, // Chain identifier for cross-chain support
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub transaction_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "order_status", rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Deposited,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub telegram_id: Option<i64>,
    pub evm_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Balance {
    pub id: i64,
    pub account_id: i64,
    pub balance: String, // Using String to handle arbitrary precision
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub account_id: i64,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    pub broker_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub order_id: String,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    pub vault_address: String,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub account_id: i64,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub transaction_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub account_id: i64,
    pub total_balance: String,
    pub completed_orders_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminOrdersResponse {
    pub orders: Vec<OrderResponse>,
    pub total_count: u32,
}
