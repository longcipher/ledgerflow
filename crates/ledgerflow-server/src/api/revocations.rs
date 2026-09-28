//! Revocation endpoint.

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{ApiError, ApiResponse};
use crate::state::AppState;

/// Revoke request body.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct RevokeRequest {
    /// Hex-encoded 16-byte warrant id.
    pub warrant_id: Option<String>,
    /// Hex-encoded 32-byte holder public key.
    pub holder_public_key: Option<String>,
}

/// Revokes a warrant or holder (tenant-scoped).
#[utoipa::path(
    post,
    path = "/v1/revocations",
    request_body = RevokeRequest,
    responses(
        (status = 200, description = "Revocation recorded"),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized")
    )
)]
pub async fn revoke(
    State(state): State<AppState>,
    ctx: crate::saas::SaaSContext,
    Json(request): Json<RevokeRequest>,
) -> Result<Json<ApiResponse<String>>, ApiError> {
    // Tenant-scoped revocation (design §10.2): a tenant can only revoke within
    // its own namespace, never another tenant's warrants/holders.
    let tenant = &ctx.tenant_id;
    if let Some(warrant_id) = &request.warrant_id {
        let bytes: [u8; 16] = super::decode_hex(warrant_id)
            .ok_or_else(|| ApiError::BadRequest("warrant_id must be 16-byte hex".to_string()))?;
        state
            .revocation_store
            .revoke_warrant_for(tenant, &bytes)
            .map_err(|error| ApiError::Internal(error.to_string()))?;
        if let Err(error) = state.webhook.emit(crate::webhook::WebhookEvent::WarrantRevoked {
            tenant_id: tenant.clone(),
            warrant_id: warrant_id.clone(),
        }) {
            tracing::error!("failed to emit webhook event: {error}");
        }
        return Ok(Json(ApiResponse::ok(format!("warrant {warrant_id} revoked"))));
    }
    if let Some(holder_key) = &request.holder_public_key {
        let bytes: [u8; 32] = super::decode_hex(holder_key).ok_or_else(|| {
            ApiError::BadRequest("holder_public_key must be 32-byte hex".to_string())
        })?;
        let holder = ledgerflow_core::SignerRef::new(
            ledgerflow_core::SigningAlgorithm::Ed25519,
            bytes.to_vec(),
        );
        state
            .revocation_store
            .revoke_holder_for(tenant, &holder)
            .map_err(|error| ApiError::Internal(error.to_string()))?;
        return Ok(Json(ApiResponse::ok(format!("holder {holder_key} revoked"))));
    }
    Err(ApiError::BadRequest("provide warrant_id or holder_public_key".to_string()))
}
