use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: Uuid,
    pub order_id: String,
    pub account_id: String,
    pub broker_id: String,
    pub amount: String, // Using String to handle arbitrary precision
    pub token_address: String,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub transaction_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "order_status", rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: Uuid,
    pub account_id: String,
    pub email: Option<String>,
    pub telegram_id: Option<String>,
    pub evm_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub account_id: String,
    pub amount: String,
    pub token_address: String,
    pub broker_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub order_id: String,
    pub amount: String,
    pub token_address: String,
    pub vault_address: String,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub account_id: String,
    pub amount: String,
    pub token_address: String,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub transaction_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub account_id: String,
    pub total_balance: String,
    pub completed_orders_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdminOrdersResponse {
    pub orders: Vec<OrderResponse>,
    pub total_count: u32,
}
