//! Audit endpoint.

use axum::{Json, extract::State};

use super::ApiResponse;
use crate::{state::AppState, webhook::WebhookEvent};

/// Returns the tenant-scoped audit event stream.
#[utoipa::path(
    get,
    path = "/v1/audit",
    responses((status = 200, description = "Audit events"))
)]
pub async fn audit(
    State(state): State<AppState>,
    ctx: crate::saas::SaaSContext,
) -> Json<ApiResponse<Vec<String>>> {
    // Tenant-scoped audit (design §10.2): a tenant only sees its own events.
    let tenant = &ctx.tenant_id;
    let events = state
        .webhook
        .buffered()
        .into_iter()
        .filter(|event| event.tenant_id() == tenant)
        .map(|event| match event {
            WebhookEvent::WarrantIssued { warrant_id, .. } => {
                format!("warrant_issued:{warrant_id}")
            }
            WebhookEvent::WarrantRevoked { warrant_id, .. } => {
                format!("warrant_revoked:{warrant_id}")
            }
            WebhookEvent::PaymentSettled { transaction_id, amount, .. } => {
                format!("payment_settled:{transaction_id}:{amount}")
            }
            WebhookEvent::ApprovalRequested { request_hash, .. } => {
                format!("approval_requested:{request_hash}")
            }
        })
        .collect();
    Json(ApiResponse::ok(events))
}
