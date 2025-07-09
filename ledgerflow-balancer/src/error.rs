use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] eyre::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Order not found: {0}")]
    OrderNotFound(String),

    #[error("Account not found: {0}")]
    AccountNotFound(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Too many pending orders for account: {0}")]
    TooManyPendingOrders(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            AppError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Configuration error"),
            AppError::InvalidInput(_) => (StatusCode::BAD_REQUEST, "Invalid input"),
            AppError::OrderNotFound(_) => (StatusCode::NOT_FOUND, "Order not found"),
            AppError::AccountNotFound(_) => (StatusCode::NOT_FOUND, "Account not found"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "Not found"),
            AppError::TooManyPendingOrders(_) => {
                (StatusCode::BAD_REQUEST, "Too many pending orders")
            }
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };

        let body = Json(json!({
            "error": error_message,
            "message": self.to_string(),
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}
