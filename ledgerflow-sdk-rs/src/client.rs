// LedgerFlow API client implementation

#[cfg(feature = "native")]
mod native {
    use std::time::Duration;

    use crate::{
        error::SdkError,
        models::{
            AccountResponse, AdminOrdersResponse, BalanceResponse, CreateOrderRequest,
            CreateOrderResponse, HealthResponse, OrderResponse, RegisterAccountRequest,
            RegisterAccountResponse,
        },
    };

    /// HTTP client for the LedgerFlow Balancer API.
    ///
    /// All methods are async and return `Result<T, SdkError>`.
    /// The client is only available on native (non-WASM) targets
    /// behind the `native` feature flag.
    #[derive(Clone)]
    pub struct LedgerFlowClient {
        base_url: String,
        http: hpx::Client,
    }

    impl std::fmt::Debug for LedgerFlowClient {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("LedgerFlowClient")
                .field("base_url", &self.base_url)
                .finish_non_exhaustive()
        }
    }

    impl LedgerFlowClient {
        /// Create a new client pointing at the given balancer base URL.
        ///
        /// A 30-second timeout is applied to every request.
        ///
        /// # Errors
        ///
        /// Returns [`SdkError::Network`] if the underlying HTTP client
        /// cannot be constructed.
        pub fn new(base_url: &str) -> Result<Self, SdkError> {
            let http = hpx::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| SdkError::Network(e.to_string()))?;

            Ok(Self {
                base_url: base_url.trim_end_matches('/').to_string(),
                http,
            })
        }

        // -----------------------------------------------------------------
        // Orders
        // -----------------------------------------------------------------

        /// Create a new payment order (`POST /orders`).
        pub async fn create_order(
            &self,
            request: &CreateOrderRequest,
        ) -> Result<CreateOrderResponse, SdkError> {
            let url = format!("{}/orders", self.base_url);
            let response = self.http.post(&url).json(request).send().await?;
            Self::handle_response(response).await
        }

        /// Fetch a single order by its ID (`GET /orders/{order_id}`).
        pub async fn get_order(&self, order_id: &str) -> Result<OrderResponse, SdkError> {
            let url = format!("{}/orders/{}", self.base_url, order_id);
            let response = self.http.get(&url).send().await?;
            Self::handle_response(response).await
        }

        // -----------------------------------------------------------------
        // Accounts
        // -----------------------------------------------------------------

        /// Register a new account (`POST /register`).
        pub async fn register_account(
            &self,
            request: &RegisterAccountRequest,
        ) -> Result<RegisterAccountResponse, SdkError> {
            let url = format!("{}/register", self.base_url);
            let response = self.http.post(&url).json(request).send().await?;
            Self::handle_response(response).await
        }

        /// Look up an account by username (`GET /accounts/username/{username}`).
        pub async fn get_account_by_username(
            &self,
            username: &str,
        ) -> Result<AccountResponse, SdkError> {
            let url = format!("{}/accounts/username/{}", self.base_url, username);
            let response = self.http.get(&url).send().await?;
            Self::handle_response(response).await
        }

        /// Look up an account by email (`GET /accounts/email/{email}`).
        pub async fn get_account_by_email(&self, email: &str) -> Result<AccountResponse, SdkError> {
            let url = format!("{}/accounts/email/{}", self.base_url, email);
            let response = self.http.get(&url).send().await?;
            Self::handle_response(response).await
        }

        /// Look up an account by Telegram ID (`GET /accounts/telegram/{telegram_id}`).
        pub async fn get_account_by_telegram_id(
            &self,
            telegram_id: i64,
        ) -> Result<AccountResponse, SdkError> {
            let url = format!("{}/accounts/telegram/{}", self.base_url, telegram_id);
            let response = self.http.get(&url).send().await?;
            Self::handle_response(response).await
        }

        // -----------------------------------------------------------------
        // Balance
        // -----------------------------------------------------------------

        /// Fetch the balance for an account (`GET /accounts/{account_id}/balance`).
        pub async fn get_balance(&self, account_id: i64) -> Result<BalanceResponse, SdkError> {
            let url = format!("{}/accounts/{}/balance", self.base_url, account_id);
            let response = self.http.get(&url).send().await?;
            Self::handle_response(response).await
        }

        // -----------------------------------------------------------------
        // Admin
        // -----------------------------------------------------------------

        /// List pending orders with pagination (`GET /admin/orders`).
        pub async fn list_pending_orders(
            &self,
            limit: Option<u32>,
            offset: Option<u32>,
        ) -> Result<AdminOrdersResponse, SdkError> {
            let mut url = format!("{}/admin/orders", self.base_url);

            let mut params: Vec<String> = Vec::new();
            if let Some(l) = limit {
                params.push(format!("limit={l}"));
            }
            if let Some(o) = offset {
                params.push(format!("offset={o}"));
            }
            if !params.is_empty() {
                url.push('?');
                url.push_str(&params.join("&"));
            }

            let response = self.http.get(&url).send().await?;
            Self::handle_response(response).await
        }

        // -----------------------------------------------------------------
        // Health
        // -----------------------------------------------------------------

        /// Check service health (`GET /health`).
        pub async fn health_check(&self) -> Result<HealthResponse, SdkError> {
            let url = format!("{}/health", self.base_url);
            let response = self.http.get(&url).send().await?;
            Self::handle_response(response).await
        }

        // -----------------------------------------------------------------
        // Internal helpers
        // -----------------------------------------------------------------

        /// Inspect the HTTP response status and either deserialise the body
        /// on success or return an [`SdkError::Http`] on failure.
        async fn handle_response<T: serde::de::DeserializeOwned>(
            response: hpx::Response,
        ) -> Result<T, SdkError> {
            if response.status().is_success() {
                response
                    .json::<T>()
                    .await
                    .map_err(|e| SdkError::Deserialization(e.to_string()))
            } else {
                let status = response.status().as_u16();
                let message = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(SdkError::Http { status, message })
            }
        }
    }
}

#[cfg(feature = "native")]
pub use native::LedgerFlowClient;

#[cfg(feature = "wasm")]
pub use crate::wasm_client::LedgerFlowClient;
