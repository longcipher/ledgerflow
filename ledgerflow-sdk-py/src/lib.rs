//! Python bindings for the LedgerFlow SDK.
//!
//! Wraps [`ledgerflow_sdk_rs`] types and the HTTP client so they can be used
//! from Python via PyO3. Built with `maturin` for wheel distribution.

use ledgerflow_sdk_rs::{error::SdkError, models};
use pyo3::{exceptions::PyRuntimeError, prelude::*};

// ---------------------------------------------------------------------------
// Error conversion
// ---------------------------------------------------------------------------

/// Map [`SdkError`] to a Python `RuntimeError`.
fn sdk_err(err: SdkError) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

// ---------------------------------------------------------------------------
// OrderStatus
// ---------------------------------------------------------------------------

/// Payment-order status (mirrors the Rust enum as a string-friendly class).
#[pyclass(name = "OrderStatus", eq, eq_int, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyOrderStatus {
    Pending = 0,
    Deposited = 1,
    Completed = 2,
    Failed = 3,
    Cancelled = 4,
}

#[pymethods]
impl PyOrderStatus {
    fn __str__(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Deposited => "deposited",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn __repr__(&self) -> String {
        format!("OrderStatus.{}", self.__str__())
    }
}

impl From<models::OrderStatus> for PyOrderStatus {
    fn from(s: models::OrderStatus) -> Self {
        match s {
            models::OrderStatus::Pending => Self::Pending,
            models::OrderStatus::Deposited => Self::Deposited,
            models::OrderStatus::Completed => Self::Completed,
            models::OrderStatus::Failed => Self::Failed,
            models::OrderStatus::Cancelled => Self::Cancelled,
        }
    }
}

// ---------------------------------------------------------------------------
// Request wrappers
// ---------------------------------------------------------------------------

/// Parameters for creating a new payment order.
#[pyclass(name = "CreateOrderRequest", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyCreateOrderRequest {
    pub account_id: i64,
    pub amount: Option<String>,
    pub token_address: Option<String>,
    pub chain_id: Option<i64>,
    pub broker_id: Option<String>,
}

#[pymethods]
impl PyCreateOrderRequest {
    #[new]
    #[pyo3(signature = (account_id, amount=None, token_address=None, chain_id=None, broker_id=None))]
    pub fn new(
        account_id: i64,
        amount: Option<String>,
        token_address: Option<String>,
        chain_id: Option<i64>,
        broker_id: Option<String>,
    ) -> Self {
        Self {
            account_id,
            amount,
            token_address,
            chain_id,
            broker_id,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "CreateOrderRequest(account_id={}, amount={:?}, token_address={:?}, chain_id={:?}, broker_id={:?})",
            self.account_id, self.amount, self.token_address, self.chain_id, self.broker_id,
        )
    }
}

impl From<&PyCreateOrderRequest> for models::CreateOrderRequest {
    fn from(py: &PyCreateOrderRequest) -> Self {
        Self {
            account_id: py.account_id,
            amount: py.amount.clone(),
            token_address: py.token_address.clone(),
            chain_id: py.chain_id,
            broker_id: py.broker_id.clone(),
        }
    }
}

/// Parameters for registering a new account.
#[pyclass(name = "RegisterAccountRequest", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRegisterAccountRequest {
    pub username: String,
    pub email: String,
    pub telegram_id: i64,
    pub evm_pk: String,
    pub is_admin: Option<bool>,
}

#[pymethods]
impl PyRegisterAccountRequest {
    #[new]
    #[pyo3(signature = (username, email, telegram_id, evm_pk, is_admin=None))]
    pub fn new(
        username: String,
        email: String,
        telegram_id: i64,
        evm_pk: String,
        is_admin: Option<bool>,
    ) -> Self {
        Self {
            username,
            email,
            telegram_id,
            evm_pk,
            is_admin,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "RegisterAccountRequest(username={:?}, email={:?}, telegram_id={}, evm_pk={:?}, is_admin={:?})",
            self.username, self.email, self.telegram_id, self.evm_pk, self.is_admin,
        )
    }
}

impl From<&PyRegisterAccountRequest> for models::RegisterAccountRequest {
    fn from(py: &PyRegisterAccountRequest) -> Self {
        Self {
            username: py.username.clone(),
            email: py.email.clone(),
            telegram_id: py.telegram_id,
            evm_pk: py.evm_pk.clone(),
            is_admin: py.is_admin,
        }
    }
}

// ---------------------------------------------------------------------------
// Response wrappers
// ---------------------------------------------------------------------------

/// Response from creating an order.
#[pyclass(name = "CreateOrderResponse", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyCreateOrderResponse {
    pub order_id: String,
    pub amount: Option<String>,
    pub token_address: Option<String>,
    pub chain_id: Option<i64>,
    pub status: PyOrderStatus,
    /// ISO 8601 timestamp string.
    pub created_at: String,
}

#[pymethods]
impl PyCreateOrderResponse {
    fn __repr__(&self) -> String {
        format!(
            "CreateOrderResponse(order_id={:?}, status={:?}, created_at={:?})",
            self.order_id,
            self.status.__str__(),
            self.created_at,
        )
    }
}

impl From<models::CreateOrderResponse> for PyCreateOrderResponse {
    fn from(r: models::CreateOrderResponse) -> Self {
        Self {
            order_id: r.order_id,
            amount: r.amount,
            token_address: r.token_address,
            chain_id: r.chain_id,
            status: r.status.into(),
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

/// Full order representation.
#[pyclass(name = "OrderResponse", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyOrderResponse {
    pub order_id: String,
    pub account_id: i64,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    pub status: PyOrderStatus,
    /// ISO 8601 timestamp string.
    pub created_at: String,
    /// ISO 8601 timestamp string.
    pub updated_at: String,
    pub transaction_hash: Option<String>,
}

#[pymethods]
impl PyOrderResponse {
    fn __repr__(&self) -> String {
        format!(
            "OrderResponse(order_id={:?}, account_id={}, status={:?})",
            self.order_id,
            self.account_id,
            self.status.__str__(),
        )
    }
}

impl From<models::OrderResponse> for PyOrderResponse {
    fn from(r: models::OrderResponse) -> Self {
        Self {
            order_id: r.order_id,
            account_id: r.account_id,
            amount: r.amount,
            token_address: r.token_address,
            chain_id: r.chain_id,
            status: r.status.into(),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
            transaction_hash: r.transaction_hash,
        }
    }
}

/// Account balance summary.
#[pyclass(name = "BalanceResponse", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyBalanceResponse {
    pub account_id: i64,
    pub total_balance: String,
    pub completed_orders_count: u32,
}

#[pymethods]
impl PyBalanceResponse {
    fn __repr__(&self) -> String {
        format!(
            "BalanceResponse(account_id={}, total_balance={:?}, completed_orders_count={})",
            self.account_id, self.total_balance, self.completed_orders_count,
        )
    }
}

impl From<models::BalanceResponse> for PyBalanceResponse {
    fn from(r: models::BalanceResponse) -> Self {
        Self {
            account_id: r.account_id,
            total_balance: r.total_balance,
            completed_orders_count: r.completed_orders_count,
        }
    }
}

/// Response from registering an account.
#[pyclass(name = "RegisterAccountResponse", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyRegisterAccountResponse {
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

#[pymethods]
impl PyRegisterAccountResponse {
    fn __repr__(&self) -> String {
        format!(
            "RegisterAccountResponse(id={}, username={:?}, is_admin={})",
            self.id, self.username, self.is_admin,
        )
    }
}

impl From<models::RegisterAccountResponse> for PyRegisterAccountResponse {
    fn from(r: models::RegisterAccountResponse) -> Self {
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

/// Full account representation.
#[pyclass(name = "AccountResponse", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAccountResponse {
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

#[pymethods]
impl PyAccountResponse {
    fn __repr__(&self) -> String {
        format!(
            "AccountResponse(id={}, username={:?}, is_admin={})",
            self.id, self.username, self.is_admin,
        )
    }
}

impl From<models::AccountResponse> for PyAccountResponse {
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
#[pyclass(name = "AdminOrdersResponse", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyAdminOrdersResponse {
    pub orders: Vec<PyOrderResponse>,
    pub total_count: u32,
}

#[pymethods]
impl PyAdminOrdersResponse {
    fn __repr__(&self) -> String {
        format!(
            "AdminOrdersResponse(total_count={}, orders_len={})",
            self.total_count,
            self.orders.len(),
        )
    }
}

impl From<models::AdminOrdersResponse> for PyAdminOrdersResponse {
    fn from(r: models::AdminOrdersResponse) -> Self {
        Self {
            orders: r.orders.into_iter().map(PyOrderResponse::from).collect(),
            total_count: r.total_count,
        }
    }
}

/// Health-check response.
#[pyclass(name = "HealthResponse", get_all, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyHealthResponse {
    pub status: String,
    /// ISO 8601 timestamp string.
    pub timestamp: String,
    pub service: String,
}

#[pymethods]
impl PyHealthResponse {
    fn __repr__(&self) -> String {
        format!(
            "HealthResponse(status={:?}, service={:?})",
            self.status, self.service,
        )
    }
}

impl From<models::HealthResponse> for PyHealthResponse {
    fn from(r: models::HealthResponse) -> Self {
        Self {
            status: r.status,
            timestamp: r.timestamp.to_rfc3339(),
            service: r.service,
        }
    }
}

// ---------------------------------------------------------------------------
// Client wrapper
// ---------------------------------------------------------------------------

/// HTTP client for the LedgerFlow Balancer API.
///
/// Internally owns a Tokio runtime so that Python callers can invoke
/// async Rust methods synchronously.
#[pyclass(name = "LedgerFlowClient")]
pub struct PyLedgerFlowClient {
    inner: ledgerflow_sdk_rs::client::LedgerFlowClient,
    rt: tokio::runtime::Runtime,
}

#[pymethods]
impl PyLedgerFlowClient {
    /// Create a new client pointing at the given balancer base URL.
    #[new]
    pub fn new(base_url: &str) -> PyResult<Self> {
        let inner = ledgerflow_sdk_rs::client::LedgerFlowClient::new(base_url).map_err(sdk_err)?;
        let rt =
            tokio::runtime::Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Self { inner, rt })
    }

    // -- Orders ------------------------------------------------------------

    /// Create a new payment order.
    pub fn create_order(&self, request: &PyCreateOrderRequest) -> PyResult<PyCreateOrderResponse> {
        let req = models::CreateOrderRequest::from(request);
        let resp = self
            .rt
            .block_on(self.inner.create_order(&req))
            .map_err(sdk_err)?;
        Ok(PyCreateOrderResponse::from(resp))
    }

    /// Fetch a single order by its ID.
    pub fn get_order(&self, order_id: &str) -> PyResult<PyOrderResponse> {
        let resp = self
            .rt
            .block_on(self.inner.get_order(order_id))
            .map_err(sdk_err)?;
        Ok(PyOrderResponse::from(resp))
    }

    // -- Accounts ----------------------------------------------------------

    /// Register a new account.
    pub fn register_account(
        &self,
        request: &PyRegisterAccountRequest,
    ) -> PyResult<PyRegisterAccountResponse> {
        let req = models::RegisterAccountRequest::from(request);
        let resp = self
            .rt
            .block_on(self.inner.register_account(&req))
            .map_err(sdk_err)?;
        Ok(PyRegisterAccountResponse::from(resp))
    }

    /// Look up an account by username.
    pub fn get_account_by_username(&self, username: &str) -> PyResult<PyAccountResponse> {
        let resp = self
            .rt
            .block_on(self.inner.get_account_by_username(username))
            .map_err(sdk_err)?;
        Ok(PyAccountResponse::from(resp))
    }

    /// Look up an account by email.
    pub fn get_account_by_email(&self, email: &str) -> PyResult<PyAccountResponse> {
        let resp = self
            .rt
            .block_on(self.inner.get_account_by_email(email))
            .map_err(sdk_err)?;
        Ok(PyAccountResponse::from(resp))
    }

    /// Look up an account by Telegram ID.
    pub fn get_account_by_telegram_id(&self, telegram_id: i64) -> PyResult<PyAccountResponse> {
        let resp = self
            .rt
            .block_on(self.inner.get_account_by_telegram_id(telegram_id))
            .map_err(sdk_err)?;
        Ok(PyAccountResponse::from(resp))
    }

    // -- Balance -----------------------------------------------------------

    /// Fetch the balance for an account.
    pub fn get_balance(&self, account_id: i64) -> PyResult<PyBalanceResponse> {
        let resp = self
            .rt
            .block_on(self.inner.get_balance(account_id))
            .map_err(sdk_err)?;
        Ok(PyBalanceResponse::from(resp))
    }

    // -- Admin -------------------------------------------------------------

    /// List pending orders with optional pagination.
    #[pyo3(signature = (limit=None, offset=None))]
    pub fn list_pending_orders(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> PyResult<PyAdminOrdersResponse> {
        let resp = self
            .rt
            .block_on(self.inner.list_pending_orders(limit, offset))
            .map_err(sdk_err)?;
        Ok(PyAdminOrdersResponse::from(resp))
    }

    // -- Health ------------------------------------------------------------

    /// Check service health.
    pub fn health_check(&self) -> PyResult<PyHealthResponse> {
        let resp = self
            .rt
            .block_on(self.inner.health_check())
            .map_err(sdk_err)?;
        Ok(PyHealthResponse::from(resp))
    }

    fn __repr__(&self) -> &'static str {
        "LedgerFlowClient(...)"
    }
}

// ---------------------------------------------------------------------------
// Standalone functions
// ---------------------------------------------------------------------------

/// Generate a unique order ID using keccak256 (matches on-chain logic).
#[pyfunction]
pub fn generate_order_id(broker_id: &str, account_id: i64, order_id_num: i64) -> String {
    ledgerflow_sdk_rs::utils::generate_order_id(broker_id, account_id, order_id_num)
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Root Python module (`import ledgerflow_sdk`).
#[pymodule]
fn ledgerflow_sdk(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Enum
    m.add_class::<PyOrderStatus>()?;

    // Request types
    m.add_class::<PyCreateOrderRequest>()?;
    m.add_class::<PyRegisterAccountRequest>()?;

    // Response types
    m.add_class::<PyCreateOrderResponse>()?;
    m.add_class::<PyOrderResponse>()?;
    m.add_class::<PyBalanceResponse>()?;
    m.add_class::<PyRegisterAccountResponse>()?;
    m.add_class::<PyAccountResponse>()?;
    m.add_class::<PyAdminOrdersResponse>()?;
    m.add_class::<PyHealthResponse>()?;

    // Client
    m.add_class::<PyLedgerFlowClient>()?;

    // Functions
    m.add_function(wrap_pyfunction!(generate_order_id, m)?)?;

    Ok(())
}
