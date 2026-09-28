//! Settlement query endpoint.

use axum::{
    Json,
    extract::{Path, State},
};

use super::{ApiError, ApiResponse};
use crate::state::AppState;

/// Queries an idempotent settlement by transaction id.
#[utoipa::path(
    get,
    path = "/v1/settlements/{transaction_id}",
    params(("transaction_id" = String, Path, description = "Settlement transaction id")),
    responses(
        (status = 200, description = "Settlement status"),
        (status = 404, description = "Not found")
    )
)]
pub async fn query_settlement(
    State(state): State<AppState>,
    Path(transaction_id): Path<String>,
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
