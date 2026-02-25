//! LedgerFlow API request/response models.
//!
//! These types mirror the JSON shapes from the Balancer API and serve as
//! the single source of truth for all SDK bindings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Status of a payment order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Deposited,
    Completed,
    Failed,
    Cancelled,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Body for `POST /orders`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub account_id: i64,
    pub amount: Option<String>,
    pub token_address: Option<String>,
    pub chain_id: Option<i64>,
    pub broker_id: Option<String>,
}

/// Body for `POST /accounts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAccountRequest {
    pub username: String,
    pub email: String,
    pub telegram_id: i64,
    pub evm_pk: String,
    pub is_admin: Option<bool>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Response from `POST /orders`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub order_id: String,
    pub amount: Option<String>,
    pub token_address: Option<String>,
    pub chain_id: Option<i64>,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}

/// Full order representation returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Account balance summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub account_id: i64,
    pub total_balance: String,
    pub completed_orders_count: u32,
}

/// Response from `POST /accounts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAccountResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub telegram_id: Option<i64>,
    pub evm_address: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Full account representation returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub telegram_id: Option<i64>,
    pub evm_address: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Paginated list of orders (admin endpoint).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminOrdersResponse {
    pub orders: Vec<OrderResponse>,
    pub total_count: u32,
}

/// Health-check response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub service: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    /// Helper: serialise to JSON then deserialise back, returning the
    /// round-tripped value.
    fn round_trip<T>(val: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug,
    {
        let json = serde_json::to_string(val).expect("serialise should succeed");
        serde_json::from_str(&json).expect("deserialise should succeed")
    }

    #[test]
    fn models_order_status_round_trip() {
        for status in [
            OrderStatus::Pending,
            OrderStatus::Deposited,
            OrderStatus::Completed,
            OrderStatus::Failed,
            OrderStatus::Cancelled,
        ] {
            let rt = round_trip(&status);
            assert_eq!(rt, status);
        }
    }

    #[test]
    fn models_order_status_lowercase_serialization() {
        let json = serde_json::to_string(&OrderStatus::Pending).expect("serialise");
        assert_eq!(json, "\"pending\"");
        let json = serde_json::to_string(&OrderStatus::Completed).expect("serialise");
        assert_eq!(json, "\"completed\"");
    }

    #[test]
    fn models_create_order_request_round_trip() {
        let req = CreateOrderRequest {
            account_id: 42,
            amount: Some("100.50".into()),
            token_address: Some("0xA0b8...".into()),
            chain_id: Some(1),
            broker_id: Some("broker-1".into()),
        };
        let rt = round_trip(&req);
        assert_eq!(rt.account_id, req.account_id);
        assert_eq!(rt.amount, req.amount);
    }

    #[test]
    fn models_register_account_request_round_trip() {
        let req = RegisterAccountRequest {
            username: "alice".into(),
            email: "alice@example.com".into(),
            telegram_id: 123456,
            evm_pk: "0xdeadbeef".into(),
            is_admin: Some(false),
        };
        let rt = round_trip(&req);
        assert_eq!(rt.username, req.username);
        assert_eq!(rt.telegram_id, req.telegram_id);
    }

    #[test]
    fn models_create_order_response_round_trip() {
        let resp = CreateOrderResponse {
            order_id: "0xabc123".into(),
            amount: Some("50.00".into()),
            token_address: Some("0xUSDC".into()),
            chain_id: Some(137),
            status: OrderStatus::Pending,
            created_at: Utc::now(),
        };
        let rt = round_trip(&resp);
        assert_eq!(rt.order_id, resp.order_id);
        assert_eq!(rt.status, OrderStatus::Pending);
    }

    #[test]
    fn models_order_response_round_trip() {
        let resp = OrderResponse {
            order_id: "0xorder1".into(),
            account_id: 7,
            amount: "200.00".into(),
            token_address: "0xUSDC".into(),
            chain_id: 1,
            status: OrderStatus::Completed,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            transaction_hash: Some("0xtx123".into()),
        };
        let rt = round_trip(&resp);
        assert_eq!(rt.order_id, resp.order_id);
        assert_eq!(rt.transaction_hash, resp.transaction_hash);
    }

    #[test]
    fn models_balance_response_round_trip() {
        let resp = BalanceResponse {
            account_id: 1,
            total_balance: "999.99".into(),
            completed_orders_count: 10,
        };
        let rt = round_trip(&resp);
        assert_eq!(rt.total_balance, resp.total_balance);
        assert_eq!(rt.completed_orders_count, 10);
    }

    #[test]
    fn models_register_account_response_round_trip() {
        let resp = RegisterAccountResponse {
            id: 1,
            username: "bob".into(),
            email: Some("bob@example.com".into()),
            telegram_id: Some(654321),
            evm_address: Some("0xBob".into()),
            is_admin: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let rt = round_trip(&resp);
        assert_eq!(rt.id, resp.id);
        assert_eq!(rt.is_admin, false);
    }

    #[test]
    fn models_account_response_round_trip() {
        let resp = AccountResponse {
            id: 2,
            username: "carol".into(),
            email: None,
            telegram_id: None,
            evm_address: None,
            is_admin: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let rt = round_trip(&resp);
        assert_eq!(rt.username, "carol");
        assert!(rt.email.is_none());
    }

    #[test]
    fn models_admin_orders_response_round_trip() {
        let resp = AdminOrdersResponse {
            orders: vec![OrderResponse {
                order_id: "0xord".into(),
                account_id: 1,
                amount: "10".into(),
                token_address: "0xUSDC".into(),
                chain_id: 1,
                status: OrderStatus::Deposited,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                transaction_hash: None,
            }],
            total_count: 1,
        };
        let rt = round_trip(&resp);
        assert_eq!(rt.orders.len(), 1);
        assert_eq!(rt.total_count, 1);
    }

    #[test]
    fn models_health_response_round_trip() {
        let resp = HealthResponse {
            status: "ok".into(),
            timestamp: Utc::now(),
            service: "ledgerflow-balancer".into(),
        };
        let rt = round_trip(&resp);
        assert_eq!(rt.status, "ok");
        assert_eq!(rt.service, resp.service);
    }
}
