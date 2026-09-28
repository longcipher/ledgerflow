//! Warrant issuance endpoint.

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, ApiResponse};
use crate::state::AppState;

/// Issue-warrant request body.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct IssueWarrantRequest {
    pub holder_public_key: String,
    pub merchant_id: String,
    pub amount_cap: u128,
    pub ttl_secs: Option<u64>,
}

/// Issue-warrant response body.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct IssueWarrantResponse {
    pub warrant_id: String,
    pub digest: String,
    pub expires_at: u64,
}

/// Issues a root warrant for a holder.
#[utoipa::path(
    post,
    path = "/v1/warrants",
    request_body = IssueWarrantRequest,
    responses(
        (status = 200, description = "Warrant issued", body = IssueWarrantResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn issue_warrant(
    State(state): State<AppState>,
    ctx: crate::saas::SaaSContext,
    Json(request): Json<IssueWarrantRequest>,
) -> Result<Json<ApiResponse<IssueWarrantResponse>>, ApiError> {
    let holder_key_bytes = super::decode_hex(&request.holder_public_key)
        .ok_or_else(|| ApiError::BadRequest("holder_public_key must be 32-byte hex".to_string()))?;
    let holder_key = ledgerflow_core::SigningKeyPair::from_bytes(&holder_key_bytes);
    // Enforce a sane upper bound on the per-charge cap (fail-closed: a request
    // for more than the configured ceiling is rejected rather than silently
    // issued). Design §6.1 hard caps must be enforced server-side.
    if request.amount_cap > super::MAX_PER_CHARGE_CAP {
        return Err(ApiError::BadRequest(format!(
            "amount_cap exceeds the maximum allowed ({})",
            super::MAX_PER_CHARGE_CAP
        )));
    }
    if request.amount_cap == 0 {
        return Err(ApiError::BadRequest("amount_cap must be greater than zero".to_string()));
    }
    if request.merchant_id.is_empty() {
        return Err(ApiError::BadRequest("merchant_id must not be empty".to_string()));
    }
    let issuer_key = state.issuer_key.clone();
    let now_ms = super::now_ms();
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
        .sign_with(&issuer_key, super::random_bytes());

    let warrant_id = warrant.id_hex();
    let digest = warrant.digest();
    let expires_at = warrant.expires_at;
    if let Err(error) = state.webhook.emit(crate::webhook::WebhookEvent::WarrantIssued {
        tenant_id: ctx.tenant_id,
        warrant_id: warrant_id.clone(),
    }) {
        tracing::error!("failed to emit webhook event: {error}");
    }
    Ok(Json(ApiResponse::ok(IssueWarrantResponse { warrant_id, digest, expires_at })))
}
