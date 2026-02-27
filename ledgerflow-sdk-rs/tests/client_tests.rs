//! Integration tests for [`LedgerFlowClient`] using `wiremock` mock server.
//!
//! Each test spins up an isolated HTTP mock server, mounts the expected
//! request/response pair, then exercises the client method and verifies
//! the returned value (or error variant).

use ledgerflow_sdk_rs::{
    client::LedgerFlowClient,
    error::SdkError,
    models::{CreateOrderRequest, OrderStatus},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

// ---------------------------------------------------------------------------
// Helper: fixed ISO-8601 timestamp used across all mock payloads
// ---------------------------------------------------------------------------

const FIXED_TS: &str = "2025-06-15T12:00:00Z";
const FIXED_TS2: &str = "2025-06-15T13:30:00Z";

// =========================================================================
// Success-path tests
// =========================================================================

#[tokio::test]
async fn test_create_order() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "order_id": "0xabc123",
        "amount": "100.50",
        "token_address": "0xUSDC",
        "chain_id": 137,
        "status": "pending",
        "created_at": FIXED_TS,
    });

    Mock::given(method("POST"))
        .and(path("/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let req = CreateOrderRequest {
        account_id: 42,
        amount: "100.50".into(),
        token_address: "0xUSDC".into(),
        chain_id: 137,
        broker_id: None,
    };

    let resp = client
        .create_order(&req)
        .await
        .expect("create_order should succeed");

    assert_eq!(resp.order_id, "0xabc123");
    assert_eq!(resp.amount, "100.50");
    assert_eq!(resp.token_address, "0xUSDC");
    assert_eq!(resp.chain_id, 137);
    assert_eq!(resp.status, OrderStatus::Pending);
}

#[tokio::test]
async fn test_get_order() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "order_id": "abc123",
        "account_id": 7,
        "amount": "200.00",
        "token_address": "0xUSDC",
        "chain_id": 1,
        "status": "completed",
        "created_at": FIXED_TS,
        "updated_at": FIXED_TS2,
        "transaction_hash": "0xtxhash999",
    });

    Mock::given(method("GET"))
        .and(path("/orders/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .get_order("abc123")
        .await
        .expect("get_order should succeed");

    assert_eq!(resp.order_id, "abc123");
    assert_eq!(resp.account_id, 7);
    assert_eq!(resp.amount, "200.00");
    assert_eq!(resp.token_address, "0xUSDC");
    assert_eq!(resp.chain_id, 1);
    assert_eq!(resp.status, OrderStatus::Completed);
    assert_eq!(resp.transaction_hash.as_deref(), Some("0xtxhash999"));
}

#[tokio::test]
async fn test_get_order_not_found() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "error": "Order not found",
        "message": "Order not found: nonexistent",
        "status": 404,
    });

    Mock::given(method("GET"))
        .and(path("/orders/nonexistent"))
        .respond_with(ResponseTemplate::new(404).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let err = client
        .get_order("nonexistent")
        .await
        .expect_err("get_order should fail with 404");

    match err {
        SdkError::Http { status, message } => {
            assert_eq!(status, 404);
            assert!(
                message.contains("Order not found"),
                "message should contain 'Order not found', got: {message}",
            );
        }
        other => panic!("expected SdkError::Http, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_server_error_500() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let err = client
        .health_check()
        .await
        .expect_err("health_check should fail with 500");

    match err {
        SdkError::Http { status, message } => {
            assert_eq!(status, 500);
            assert!(
                message.contains("Internal Server Error"),
                "message should contain error text, got: {message}",
            );
        }
        other => panic!("expected SdkError::Http, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_register_account() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "id": 42,
        "username": "alice",
        "email": "alice@example.com",
        "telegram_id": 123456,
        "evm_address": "0xAlice",
        "is_admin": false,
        "created_at": FIXED_TS,
        "updated_at": FIXED_TS,
    });

    Mock::given(method("POST"))
        .and(path("/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let req = ledgerflow_sdk_rs::models::RegisterAccountRequest {
        username: "alice".into(),
        email: "alice@example.com".into(),
        telegram_id: 123456,
        evm_address: "0x00000000000000000000000000000000000000AA".into(),
    };

    let resp = client
        .register_account(&req)
        .await
        .expect("register_account should succeed");

    assert_eq!(resp.id, 42);
    assert_eq!(resp.username, "alice");
    assert_eq!(resp.email.as_deref(), Some("alice@example.com"));
    assert_eq!(resp.telegram_id, Some(123456));
    assert_eq!(resp.evm_address.as_deref(), Some("0xAlice"));
    assert!(!resp.is_admin);
}

#[tokio::test]
async fn test_get_account_by_username() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "id": 10,
        "username": "testuser",
        "email": "test@example.com",
        "telegram_id": 999,
        "evm_address": "0xTest",
        "is_admin": false,
        "created_at": FIXED_TS,
        "updated_at": FIXED_TS2,
    });

    Mock::given(method("GET"))
        .and(path("/accounts/username/testuser"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .get_account_by_username("testuser")
        .await
        .expect("get_account_by_username should succeed");

    assert_eq!(resp.id, 10);
    assert_eq!(resp.username, "testuser");
    assert_eq!(resp.email.as_deref(), Some("test@example.com"));
    assert_eq!(resp.telegram_id, Some(999));
}

#[tokio::test]
async fn test_get_account_by_email() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "id": 11,
        "username": "emailuser",
        "email": "user@example.com",
        "telegram_id": null,
        "evm_address": null,
        "is_admin": true,
        "created_at": FIXED_TS,
        "updated_at": FIXED_TS,
    });

    Mock::given(method("GET"))
        .and(path("/accounts/email/user@example.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .get_account_by_email("user@example.com")
        .await
        .expect("get_account_by_email should succeed");

    assert_eq!(resp.id, 11);
    assert_eq!(resp.username, "emailuser");
    assert!(resp.is_admin);
    assert!(resp.telegram_id.is_none());
    assert!(resp.evm_address.is_none());
}

#[tokio::test]
async fn test_get_account_by_telegram_id() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "id": 12,
        "username": "tguser",
        "email": null,
        "telegram_id": 777888,
        "evm_address": "0xTg",
        "is_admin": false,
        "created_at": FIXED_TS,
        "updated_at": FIXED_TS,
    });

    Mock::given(method("GET"))
        .and(path("/accounts/telegram/777888"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .get_account_by_telegram_id(777888)
        .await
        .expect("get_account_by_telegram_id should succeed");

    assert_eq!(resp.id, 12);
    assert_eq!(resp.username, "tguser");
    assert_eq!(resp.telegram_id, Some(777888));
}

#[tokio::test]
async fn test_get_balance() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "account_id": 1,
        "total_balance": "1500.75",
        "completed_orders_count": 42,
    });

    Mock::given(method("GET"))
        .and(path("/accounts/1/balance"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .get_balance(1)
        .await
        .expect("get_balance should succeed");

    assert_eq!(resp.account_id, 1);
    assert_eq!(resp.total_balance, "1500.75");
    assert_eq!(resp.completed_orders_count, 42);
}

#[tokio::test]
async fn test_list_pending_orders() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "orders": [
            {
                "order_id": "ord-1",
                "account_id": 1,
                "amount": "50.00",
                "token_address": "0xUSDC",
                "chain_id": 1,
                "status": "pending",
                "created_at": FIXED_TS,
                "updated_at": FIXED_TS,
                "transaction_hash": null,
            },
            {
                "order_id": "ord-2",
                "account_id": 2,
                "amount": "75.00",
                "token_address": "0xUSDT",
                "chain_id": 137,
                "status": "deposited",
                "created_at": FIXED_TS2,
                "updated_at": FIXED_TS2,
                "transaction_hash": "0xtx456",
            }
        ],
        "total_count": 2,
    });

    Mock::given(method("GET"))
        .and(path("/admin/orders"))
        .and(query_param("limit", "10"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .list_pending_orders(Some(10), Some(0))
        .await
        .expect("list_pending_orders should succeed");

    assert_eq!(resp.total_count, 2);
    assert_eq!(resp.orders.len(), 2);
    assert_eq!(resp.orders[0].order_id, "ord-1");
    assert_eq!(resp.orders[0].status, OrderStatus::Pending);
    assert_eq!(resp.orders[1].order_id, "ord-2");
    assert_eq!(resp.orders[1].status, OrderStatus::Deposited);
    assert_eq!(resp.orders[1].transaction_hash.as_deref(), Some("0xtx456"),);
}

#[tokio::test]
async fn test_list_pending_orders_no_params() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "orders": [],
        "total_count": 0,
    });

    Mock::given(method("GET"))
        .and(path("/admin/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .list_pending_orders(None, None)
        .await
        .expect("list_pending_orders with no params should succeed");

    assert_eq!(resp.total_count, 0);
    assert!(resp.orders.is_empty());
}

#[tokio::test]
async fn test_health_check() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "status": "ok",
        "timestamp": FIXED_TS,
        "service": "ledgerflow-balancer",
    });

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&body))
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let resp = client
        .health_check()
        .await
        .expect("health_check should succeed");

    assert_eq!(resp.status, "ok");
    assert_eq!(resp.service, "ledgerflow-balancer");
}

// =========================================================================
// Error-path tests
// =========================================================================

#[tokio::test]
async fn test_network_error() {
    // Port 1 is privileged and very unlikely to have anything listening.
    let client = LedgerFlowClient::new("http://127.0.0.1:1").expect("client should be created");

    let err = client
        .health_check()
        .await
        .expect_err("health_check should fail with network error");

    match err {
        SdkError::Network(_) => { /* expected */ }
        other => panic!("expected SdkError::Network, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_deserialization_error() {
    let server = MockServer::start().await;

    // Return a 200 with JSON that does not match HealthResponse schema
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"unexpected": true})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = LedgerFlowClient::new(&server.uri()).expect("client should be created");

    let err = client
        .health_check()
        .await
        .expect_err("health_check should fail with deserialization error");

    match err {
        SdkError::Deserialization(msg) => {
            assert!(
                !msg.is_empty(),
                "deserialization error message should not be empty",
            );
        }
        other => panic!("expected SdkError::Deserialization, got: {other:?}"),
    }
}
