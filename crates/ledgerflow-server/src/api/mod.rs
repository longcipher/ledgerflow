//! REST API for the LedgerFlow server.
//!
//! Endpoints (v1):
//!
//! - `GET  /healthz` — liveness.
//! - `POST /v1/warrants` — issue a root warrant (demo issuer).
//! - `POST /v1/revocations` — revoke a warrant or holder.
//! - `GET  /v1/settlements/{transaction_id}` — idempotent settlement query.
//! - `GET  /v1/audit` — buffered webhook/audit events.

use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::OpenApi;

mod audit;
mod health;
mod revocations;
mod settlements;
mod warrants;

pub use audit::audit;
pub use health::health;
pub use revocations::{RevokeRequest, revoke};
pub use settlements::query_settlement;
pub use warrants::{IssueWarrantRequest, IssueWarrantResponse, issue_warrant};

/// OpenAPI document for the LedgerFlow server REST API (design §10.3).
#[derive(OpenApi)]
#[openapi(
    paths(
        health::health,
        warrants::issue_warrant,
        revocations::revoke,
        settlements::query_settlement,
        audit::audit
    ),
    components(schemas(IssueWarrantRequest, IssueWarrantResponse, RevokeRequest)),
    info(
        title = "LedgerFlow Server API",
        version = "0.1.0",
        description = "REST API for warrant issuance, revocation, settlement query, and audit."
    )
)]
pub struct ApiDoc;

/// Builds the API router.
pub fn router() -> Router<crate::state::AppState> {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/warrants", post(issue_warrant))
        .route("/v1/revocations", post(revoke))
        .route("/v1/settlements/{transaction_id}", get(query_settlement))
        .route("/v1/audit", get(audit))
        .merge(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui")
                .url("/openapi.json", ApiDoc::openapi()),
        )
}

/// API response envelope.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    #[must_use]
    pub const fn ok(data: T) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }

    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self { ok: false, data: None, error: Some(message.into()) }
    }
}

/// API errors (mapped to HTTP status codes).
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, axum::Json(ApiResponse::<()>::error(self.to_string()))).into_response()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Maximum per-charge cap the server will issue (base units). Requests above
/// this ceiling are rejected (fail-closed). Tunable per deployment.
const MAX_PER_CHARGE_CAP: u128 = u128::MAX / 1_000_000;

/// Generates a cryptographically random nonce for warrant issuance.
fn random_bytes() -> [u8; 8] {
    rand::random()
}

/// Decodes a hex string into exactly `N` bytes (delegates to core).
fn decode_hex<const N: usize>(hex: &str) -> Option<[u8; N]> {
    ledgerflow_core::hex_decode_fixed(hex)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use axum::response::IntoResponse;

    use super::*;

    #[test]
    fn api_error_maps_to_http_status() {
        let cases = [
            (ApiError::BadRequest("x".to_string()), axum::http::StatusCode::BAD_REQUEST),
            (ApiError::Unauthorized, axum::http::StatusCode::UNAUTHORIZED),
            (ApiError::NotFound, axum::http::StatusCode::NOT_FOUND),
            (ApiError::Internal("x".to_string()), axum::http::StatusCode::INTERNAL_SERVER_ERROR),
        ];
        for (error, expected) in cases {
            let response = error.into_response();
            assert_eq!(response.status(), expected);
        }
    }

    #[test]
    fn api_response_ok_and_error_shapes() {
        let ok: ApiResponse<String> = ApiResponse::ok("data".to_string());
        assert!(ok.ok);
        assert_eq!(ok.data.as_deref(), Some("data"));
        assert!(ok.error.is_none());

        let err: ApiResponse<()> = ApiResponse::error("boom");
        assert!(!err.ok);
        assert!(err.data.is_none());
        assert_eq!(err.error.as_deref(), Some("boom"));
    }

    #[test]
    fn decode_hex_validates_length_and_content() {
        let bytes: [u8; 2] = decode_hex("abcd").expect("valid");
        assert_eq!(bytes, [0xAB, 0xCD]);
        assert!(decode_hex::<2>("abc").is_none()); // odd length
        assert!(decode_hex::<2>("abcde").is_none()); // wrong length
        assert!(decode_hex::<2>("zzzz").is_none()); // invalid hex
        assert!(decode_hex::<32>("aa").is_none()); // wrong N
    }
}
