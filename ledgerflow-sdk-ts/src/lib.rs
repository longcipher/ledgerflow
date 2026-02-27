//! Node.js/TypeScript bindings for the LedgerFlow SDK.
//!
//! Wraps [`ledgerflow_sdk_rs`] types and the HTTP client using napi-rs
//! so they can be consumed from Node.js / TypeScript as a native addon.

use ledgerflow_sdk_rs::models;
use napi_derive::napi;

// ---------------------------------------------------------------------------
// Helper: SdkError → napi::Error
// ---------------------------------------------------------------------------

fn sdk_err(err: ledgerflow_sdk_rs::error::SdkError) -> napi::Error {
    napi::Error::from_reason(err.to_string())
}

/// Convert [`models::OrderStatus`] to its lowercase string representation.
fn order_status_to_string(status: &models::OrderStatus) -> String {
    match status {
        models::OrderStatus::Pending => "pending".to_string(),
        models::OrderStatus::Deposited => "deposited".to_string(),
        models::OrderStatus::Completed => "completed".to_string(),
        models::OrderStatus::Failed => "failed".to_string(),
        models::OrderStatus::Cancelled => "cancelled".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Request types (plain JS objects via #[napi(object)])
// ---------------------------------------------------------------------------

/// Parameters for creating a new payment order.
#[napi(object)]
pub struct CreateOrderRequest {
    pub account_id: i64,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    pub broker_id: Option<String>,
}

/// Parameters for registering a new account.
#[napi(object)]
pub struct RegisterAccountRequest {
    pub username: String,
    pub email: String,
    pub telegram_id: i64,
    pub evm_address: String,
}

// ---------------------------------------------------------------------------
// Response types (plain JS objects via #[napi(object)])
// ---------------------------------------------------------------------------

/// Response from creating an order.
#[napi(object)]
pub struct CreateOrderResponse {
    pub order_id: String,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    /// Order status as lowercase string (`"pending"`, `"completed"`, …).
    pub status: String,
    /// ISO 8601 timestamp string.
    pub created_at: String,
}

impl From<models::CreateOrderResponse> for CreateOrderResponse {
    fn from(r: models::CreateOrderResponse) -> Self {
        Self {
            order_id: r.order_id,
            amount: r.amount,
            token_address: r.token_address,
            chain_id: r.chain_id,
            status: order_status_to_string(&r.status),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

/// Full order representation returned by the API.
#[napi(object)]
pub struct OrderResponse {
    pub order_id: String,
    pub account_id: i64,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    /// Order status as lowercase string.
    pub status: String,
    /// ISO 8601 timestamp string.
    pub created_at: String,
    /// ISO 8601 timestamp string.
    pub updated_at: String,
    pub transaction_hash: Option<String>,
}

impl From<models::OrderResponse> for OrderResponse {
    fn from(r: models::OrderResponse) -> Self {
        Self {
            order_id: r.order_id,
            account_id: r.account_id,
            amount: r.amount,
            token_address: r.token_address,
            chain_id: r.chain_id,
            status: order_status_to_string(&r.status),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
            transaction_hash: r.transaction_hash,
        }
    }
}

/// Account balance summary.
#[napi(object)]
pub struct BalanceResponse {
    pub account_id: i64,
    pub total_balance: String,
    pub completed_orders_count: u32,
}

impl From<models::BalanceResponse> for BalanceResponse {
    fn from(r: models::BalanceResponse) -> Self {
        Self {
            account_id: r.account_id,
            total_balance: r.total_balance,
            completed_orders_count: r.completed_orders_count,
        }
    }
}

/// Response from registering an account.
#[napi(object)]
pub struct RegisterAccountResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub telegram_id: Option<i64>,
    pub evm_address: Option<String>,
    pub api_token: Option<String>,
    pub is_admin: bool,
    /// ISO 8601 timestamp string.
    pub created_at: String,
    /// ISO 8601 timestamp string.
    pub updated_at: String,
}

impl From<models::RegisterAccountResponse> for RegisterAccountResponse {
    fn from(r: models::RegisterAccountResponse) -> Self {
        Self {
            id: r.id,
            username: r.username,
            email: r.email,
            telegram_id: r.telegram_id,
            evm_address: r.evm_address,
            api_token: r.api_token,
            is_admin: r.is_admin,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

/// Full account representation.
#[napi(object)]
pub struct AccountResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub telegram_id: Option<i64>,
    pub evm_address: Option<String>,
    pub is_admin: bool,
    /// ISO 8601 timestamp string.
    pub created_at: String,
    /// ISO 8601 timestamp string.
    pub updated_at: String,
}

impl From<models::AccountResponse> for AccountResponse {
    fn from(r: models::AccountResponse) -> Self {
        Self {
            id: r.id,
            username: r.username,
            email: r.email,
            telegram_id: r.telegram_id,
            evm_address: r.evm_address,
            is_admin: r.is_admin,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

/// Paginated list of orders (admin endpoint).
#[napi(object)]
pub struct AdminOrdersResponse {
    pub orders: Vec<OrderResponse>,
    pub total_count: u32,
}

impl From<models::AdminOrdersResponse> for AdminOrdersResponse {
    fn from(r: models::AdminOrdersResponse) -> Self {
        Self {
            orders: r.orders.into_iter().map(OrderResponse::from).collect(),
            total_count: r.total_count,
        }
    }
}

/// Health-check response.
#[napi(object)]
pub struct HealthResponse {
    pub status: String,
    /// ISO 8601 timestamp string.
    pub timestamp: String,
    pub service: String,
}

impl From<models::HealthResponse> for HealthResponse {
    fn from(r: models::HealthResponse) -> Self {
        Self {
            status: r.status,
            timestamp: r.timestamp.to_rfc3339(),
            service: r.service,
        }
    }
}

// ---------------------------------------------------------------------------
// Client class
// ---------------------------------------------------------------------------

/// HTTP client for the LedgerFlow Balancer API.
///
/// Wraps the Rust SDK client and exposes async methods that return
/// JavaScript `Promise`s via napi-rs.
#[napi]
pub struct LedgerFlowClient {
    inner: ledgerflow_sdk_rs::client::LedgerFlowClient,
}

#[napi]
impl LedgerFlowClient {
    /// Create a new client pointing at the given balancer base URL.
    #[napi(constructor)]
    pub fn new(base_url: String) -> napi::Result<Self> {
        let inner = ledgerflow_sdk_rs::client::LedgerFlowClient::new(&base_url).map_err(sdk_err)?;
        Ok(Self { inner })
    }

    // -- Orders ------------------------------------------------------------

    /// Create a new payment order.
    ///
    /// Returns a `Promise<CreateOrderResponse>`.
    #[napi]
    pub async fn create_order(
        &self,
        request: CreateOrderRequest,
    ) -> napi::Result<CreateOrderResponse> {
        let sdk_req = models::CreateOrderRequest {
            account_id: request.account_id,
            amount: request.amount,
            token_address: request.token_address,
            chain_id: request.chain_id,
            broker_id: request.broker_id,
        };
        let resp = self.inner.create_order(&sdk_req).await.map_err(sdk_err)?;
        Ok(CreateOrderResponse::from(resp))
    }

    /// Fetch a single order by its ID.
    ///
    /// Returns a `Promise<OrderResponse>`.
    #[napi]
    pub async fn get_order(&self, order_id: String) -> napi::Result<OrderResponse> {
        let resp = self.inner.get_order(&order_id).await.map_err(sdk_err)?;
        Ok(OrderResponse::from(resp))
    }

    // -- Accounts ----------------------------------------------------------

    /// Register a new account.
    ///
    /// Returns a `Promise<RegisterAccountResponse>`.
    #[napi]
    pub async fn register_account(
        &self,
        request: RegisterAccountRequest,
    ) -> napi::Result<RegisterAccountResponse> {
        let sdk_req = models::RegisterAccountRequest {
            username: request.username,
            email: request.email,
            telegram_id: request.telegram_id,
            evm_address: request.evm_address,
        };
        let resp = self
            .inner
            .register_account(&sdk_req)
            .await
            .map_err(sdk_err)?;
        Ok(RegisterAccountResponse::from(resp))
    }

    /// Look up an account by username.
    ///
    /// Returns a `Promise<AccountResponse>`.
    #[napi]
    pub async fn get_account_by_username(&self, username: String) -> napi::Result<AccountResponse> {
        let resp = self
            .inner
            .get_account_by_username(&username)
            .await
            .map_err(sdk_err)?;
        Ok(AccountResponse::from(resp))
    }

    /// Look up an account by email.
    ///
    /// Returns a `Promise<AccountResponse>`.
    #[napi]
    pub async fn get_account_by_email(&self, email: String) -> napi::Result<AccountResponse> {
        let resp = self
            .inner
            .get_account_by_email(&email)
            .await
            .map_err(sdk_err)?;
        Ok(AccountResponse::from(resp))
    }

    /// Look up an account by Telegram ID.
    ///
    /// Returns a `Promise<AccountResponse>`.
    #[napi]
    pub async fn get_account_by_telegram_id(
        &self,
        telegram_id: i64,
    ) -> napi::Result<AccountResponse> {
        let resp = self
            .inner
            .get_account_by_telegram_id(telegram_id)
            .await
            .map_err(sdk_err)?;
        Ok(AccountResponse::from(resp))
    }

    // -- Balance -----------------------------------------------------------

    /// Fetch the balance for an account.
    ///
    /// Returns a `Promise<BalanceResponse>`.
    #[napi]
    pub async fn get_balance(&self, account_id: i64) -> napi::Result<BalanceResponse> {
        let resp = self.inner.get_balance(account_id).await.map_err(sdk_err)?;
        Ok(BalanceResponse::from(resp))
    }

    // -- Admin -------------------------------------------------------------

    /// List pending orders with optional pagination.
    ///
    /// Returns a `Promise<AdminOrdersResponse>`.
    #[napi]
    pub async fn list_pending_orders(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> napi::Result<AdminOrdersResponse> {
        let resp = self
            .inner
            .list_pending_orders(limit, offset)
            .await
            .map_err(sdk_err)?;
        Ok(AdminOrdersResponse::from(resp))
    }

    // -- Health ------------------------------------------------------------

    /// Check service health.
    ///
    /// Returns a `Promise<HealthResponse>`.
    #[napi]
    pub async fn health_check(&self) -> napi::Result<HealthResponse> {
        let resp = self.inner.health_check().await.map_err(sdk_err)?;
        Ok(HealthResponse::from(resp))
    }
}

// ---------------------------------------------------------------------------
// Standalone functions
// ---------------------------------------------------------------------------

/// Generate a unique order ID using keccak256 (matches on-chain logic).
///
/// Replicates `abi.encodePacked(broker_id, account_id, order_id_num)` and
/// hashes with Keccak-256, returning a 64-character hex string.
#[napi]
pub fn generate_order_id(broker_id: String, account_id: i64, order_id_num: i64) -> String {
    ledgerflow_sdk_rs::utils::generate_order_id(&broker_id, account_id, order_id_num)
}
