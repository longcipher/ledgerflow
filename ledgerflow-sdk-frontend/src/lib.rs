//! LedgerFlow SDK for browsers — WASM bindings.
//!
//! Thin wrapper around [`ledgerflow_sdk_rs::wasm_client::LedgerFlowClient`]
//! exposed to JavaScript via `wasm-bindgen`.  Request/response objects are
//! passed as plain JS values and converted with `serde-wasm-bindgen`.
//!
//! The entire module is gated on `target_arch = "wasm32"` so that the crate
//! can be included in the native Cargo workspace without pulling in
//! wasm32-incompatible dependencies.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

// Re-export the inner WASM client and models for convenience.
use ledgerflow_sdk_rs::models;
use ledgerflow_sdk_rs::wasm_client;

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// JavaScript-facing LedgerFlow API client.
///
/// ```js
/// const client = new LedgerFlowClient("https://api.ledgerflow.dev");
/// const order  = await client.createOrder({ account_id: 1, amount: "10.00" });
/// ```
#[wasm_bindgen]
pub struct LedgerFlowClient {
    inner: wasm_client::LedgerFlowClient,
}

#[wasm_bindgen]
impl LedgerFlowClient {
    /// Create a new client pointing at the given balancer base URL.
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: &str) -> Result<LedgerFlowClient, JsError> {
        let inner = wasm_client::LedgerFlowClient::new(base_url)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self { inner })
    }

    // -----------------------------------------------------------------
    // Orders
    // -----------------------------------------------------------------

    /// Create a new payment order.
    ///
    /// Accepts a plain JS object matching `CreateOrderRequest`.
    #[wasm_bindgen(js_name = "createOrder")]
    pub async fn create_order(&self, request: JsValue) -> Result<JsValue, JsError> {
        let req: models::CreateOrderRequest =
            serde_wasm_bindgen::from_value(request).map_err(|e| JsError::new(&e.to_string()))?;
        let resp = self
            .inner
            .create_order(&req)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Fetch a single order by its ID.
    #[wasm_bindgen(js_name = "getOrder")]
    pub async fn get_order(&self, order_id: &str) -> Result<JsValue, JsError> {
        let resp = self
            .inner
            .get_order(order_id)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    // -----------------------------------------------------------------
    // Accounts
    // -----------------------------------------------------------------

    /// Register a new account.
    ///
    /// Accepts a plain JS object matching `RegisterAccountRequest`.
    #[wasm_bindgen(js_name = "registerAccount")]
    pub async fn register_account(&self, request: JsValue) -> Result<JsValue, JsError> {
        let req: models::RegisterAccountRequest =
            serde_wasm_bindgen::from_value(request).map_err(|e| JsError::new(&e.to_string()))?;
        let resp = self
            .inner
            .register_account(&req)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Look up an account by username.
    #[wasm_bindgen(js_name = "getAccountByUsername")]
    pub async fn get_account_by_username(&self, username: &str) -> Result<JsValue, JsError> {
        let resp = self
            .inner
            .get_account_by_username(username)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Look up an account by email.
    #[wasm_bindgen(js_name = "getAccountByEmail")]
    pub async fn get_account_by_email(&self, email: &str) -> Result<JsValue, JsError> {
        let resp = self
            .inner
            .get_account_by_email(email)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Look up an account by Telegram ID.
    ///
    /// Accepts `f64` because wasm-bindgen does not support `i64` directly.
    /// Safe for all realistic Telegram IDs (< 2^53).
    #[wasm_bindgen(js_name = "getAccountByTelegramId")]
    pub async fn get_account_by_telegram_id(
        &self,
        telegram_id: f64,
    ) -> Result<JsValue, JsError> {
        let tid = telegram_id as i64;
        let resp = self
            .inner
            .get_account_by_telegram_id(tid)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    // -----------------------------------------------------------------
    // Balance
    // -----------------------------------------------------------------

    /// Fetch the balance for an account.
    ///
    /// Accepts `f64` because wasm-bindgen does not support `i64` directly.
    #[wasm_bindgen(js_name = "getBalance")]
    pub async fn get_balance(&self, account_id: f64) -> Result<JsValue, JsError> {
        let aid = account_id as i64;
        let resp = self
            .inner
            .get_balance(aid)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    // -----------------------------------------------------------------
    // Admin
    // -----------------------------------------------------------------

    /// List pending orders with optional pagination.
    ///
    /// Both `limit` and `offset` use `f64` for wasm-bindgen compat.
    /// Pass `NaN` or a negative number to omit a parameter.
    #[wasm_bindgen(js_name = "listPendingOrders")]
    pub async fn list_pending_orders(
        &self,
        limit: Option<f64>,
        offset: Option<f64>,
    ) -> Result<JsValue, JsError> {
        let limit = limit.and_then(|v| if v.is_finite() && v >= 0.0 { Some(v as u32) } else { None });
        let offset =
            offset.and_then(|v| if v.is_finite() && v >= 0.0 { Some(v as u32) } else { None });
        let resp = self
            .inner
            .list_pending_orders(limit, offset)
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }

    // -----------------------------------------------------------------
    // Health
    // -----------------------------------------------------------------

    /// Check service health.
    #[wasm_bindgen(js_name = "healthCheck")]
    pub async fn health_check(&self) -> Result<JsValue, JsError> {
        let resp = self
            .inner
            .health_check()
            .await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&resp).map_err(|e| JsError::new(&e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Standalone utilities
// ---------------------------------------------------------------------------

/// Compute a deterministic order ID from broker/account/sequence.
///
/// Uses `f64` for the numeric parameters (wasm-bindgen i64 limitation).
#[wasm_bindgen(js_name = "generateOrderId")]
pub fn generate_order_id(broker_id: &str, account_id: f64, order_id_num: f64) -> String {
    ledgerflow_sdk_rs::utils::generate_order_id(broker_id, account_id as i64, order_id_num as i64)
}
