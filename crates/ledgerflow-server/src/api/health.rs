//! Health endpoint.

use axum::Json;

use super::ApiResponse;

/// Liveness check.
#[utoipa::path(
    get,
    path = "/healthz",
    responses((status = 200, description = "Service is healthy"))
)]
pub async fn health() -> Json<ApiResponse<String>> {
    Json(ApiResponse::ok("ok".to_string()))
}
