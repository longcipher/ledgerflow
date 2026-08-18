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
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{state::AppState, webhook::WebhookEvent};

/// Builds the API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/warrants", post(issue_warrant))
        .route("/v1/revocations", post(revoke))
        .route("/v1/settlements/{transaction_id}", get(query_settlement))
        .route("/v1/audit", get(audit))
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
        (status, Json(ApiResponse::<()>::error(self.to_string()))).into_response()
    }
}

async fn health() -> Json<ApiResponse<String>> {
    Json(ApiResponse::ok("ok".to_string()))
}

/// Issue-warrant request body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueWarrantRequest {
    pub holder_public_key: String,
    pub merchant_id: String,
    pub amount_cap: u128,
    pub ttl_secs: Option<u64>,
}

/// Issue-warrant response body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueWarrantResponse {
    pub warrant_id: String,
    pub digest: String,
    pub expires_at: u64,
}

async fn issue_warrant(
    State(state): State<AppState>,
    Json(request): Json<IssueWarrantRequest>,
) -> Result<Json<ApiResponse<IssueWarrantResponse>>, ApiError> {
    let holder_key_bytes = decode_hex(&request.holder_public_key)
        .ok_or_else(|| ApiError::BadRequest("holder_public_key must be 32-byte hex".to_string()))?;
    let holder_key = ledgerflow_core::SigningKeyPair::from_bytes(&holder_key_bytes);
    // The demo issuer key is derived from the trusted issuer configured in the
    // state. For simplicity v1 uses a fixed demo key pair shared with the CLI.
    let issuer_key = ledgerflow_core::SigningKeyPair::from_bytes(&[1_u8; 32]);
    let now_ms = now_ms();
    let warrant = ledgerflow_core::WarrantBuilder::new(now_ms)
        .ttl_secs(request.ttl_secs.unwrap_or(ledgerflow_core::DEFAULT_WARRANT_TTL_SECS))
        .max_depth(ledgerflow_core::DEFAULT_MAX_DEPTH)
        .issuer(issuer_key.signer_ref())
        .holder(holder_key.signer_ref())
        .merchant(ledgerflow_core::MerchantConstraint::with_ids(vec![request.merchant_id]))
        .resource(ledgerflow_core::ResourceConstraint {
            http_methods: vec!["POST".to_string()],
            path_prefixes: vec!["/pay".to_string()],
        })
        .payment(ledgerflow_core::PaymentConstraint::new(request.amount_cap))
        .sign_with(&issuer_key, random_bytes());

    let warrant_id = warrant.id_hex();
    let digest = warrant.digest();
    let expires_at = warrant.expires_at;
    state.webhook.emit(WebhookEvent::WarrantIssued { warrant_id: warrant_id.clone() });
    Ok(Json(ApiResponse::ok(IssueWarrantResponse { warrant_id, digest, expires_at })))
}

/// Revoke request body.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeRequest {
    /// Hex-encoded 16-byte warrant id.
    pub warrant_id: Option<String>,
    /// Hex-encoded 32-byte holder public key.
    pub holder_public_key: Option<String>,
}

async fn revoke(
    State(state): State<AppState>,
    Json(request): Json<RevokeRequest>,
) -> Result<Json<ApiResponse<String>>, ApiError> {
    let revocation = &state.verification.revocation;
    if let Some(warrant_id) = &request.warrant_id {
        let bytes: [u8; 16] = decode_hex(warrant_id)
            .ok_or_else(|| ApiError::BadRequest("warrant_id must be 16-byte hex".to_string()))?;
        revocation.revoke_warrant(&bytes).map_err(|error| ApiError::Internal(error.to_string()))?;
        state.webhook.emit(WebhookEvent::WarrantRevoked { warrant_id: warrant_id.clone() });
        return Ok(Json(ApiResponse::ok(format!("warrant {warrant_id} revoked"))));
    }
    if let Some(holder_key) = &request.holder_public_key {
        let bytes: [u8; 32] = decode_hex(holder_key).ok_or_else(|| {
            ApiError::BadRequest("holder_public_key must be 32-byte hex".to_string())
        })?;
        let holder = ledgerflow_core::SignerRef::new(
            ledgerflow_core::SigningAlgorithm::Ed25519,
            bytes.to_vec(),
        );
        revocation.revoke_holder(&holder).map_err(|error| ApiError::Internal(error.to_string()))?;
        return Ok(Json(ApiResponse::ok(format!("holder {holder_key} revoked"))));
    }
    Err(ApiError::BadRequest("provide warrant_id or holder_public_key".to_string()))
}

async fn query_settlement(
    State(state): State<AppState>,
    axum::extract::Path(transaction_id): axum::extract::Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    match state.registry.query(&transaction_id) {
        Some(entry) => {
            let value = serde_json::json!({
                "transaction_id": entry.receipt.transaction_id,
                "status": match entry.status {
                    ledgerflow_facilitator::SettlementStatus::Settled => "settled",
                    ledgerflow_facilitator::SettlementStatus::Pending => "pending",
                    ledgerflow_facilitator::SettlementStatus::Failed => "failed",
                },
                "amount": entry.receipt.settled_amount,
                "asset": entry.receipt.asset,
            });
            Ok(Json(ApiResponse::ok(value)))
        }
        None => Err(ApiError::NotFound),
    }
}

async fn audit(State(state): State<AppState>) -> Json<ApiResponse<Vec<String>>> {
    let events = state
        .webhook
        .buffered()
        .into_iter()
        .map(|event| match event {
            WebhookEvent::WarrantIssued { warrant_id } => format!("warrant_issued:{warrant_id}"),
            WebhookEvent::WarrantRevoked { warrant_id } => format!("warrant_revoked:{warrant_id}"),
            WebhookEvent::PaymentSettled { transaction_id, amount } => {
                format!("payment_settled:{transaction_id}:{amount}")
            }
            WebhookEvent::ApprovalRequested { request_hash } => {
                format!("approval_requested:{request_hash}")
            }
        })
        .collect();
    Json(ApiResponse::ok(events))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn random_bytes() -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    for byte in &mut bytes {
        *byte = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| (d.as_nanos() & 0xFF) as u8);
    }
    bytes
}

/// Decodes a hex string into exactly `N` bytes.
fn decode_hex<const N: usize>(hex: &str) -> Option<[u8; N]> {
    if hex.len() != N * 2 {
        return None;
    }
    let mut out = [0_u8; N];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(out)
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
