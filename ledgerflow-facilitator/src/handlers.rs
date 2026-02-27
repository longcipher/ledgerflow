use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::warn;
use x402_types::proto::{self, v1};

use crate::{adapters::AdapterError, service::FacilitatorService};

pub async fn get_verify_info() -> impl IntoResponse {
    Json(serde_json::json!({
        "endpoint": "/verify",
        "description": "POST v2 x402 payloads for payment verification",
        "body": {
            "x402Version": 2,
            "paymentPayload": "v2 PaymentPayload",
            "paymentRequirements": "v2 PaymentRequirements"
        }
    }))
}

pub async fn get_settle_info() -> impl IntoResponse {
    Json(serde_json::json!({
        "endpoint": "/settle",
        "description": "POST v2 x402 payloads for settlement",
        "body": {
            "x402Version": 2,
            "paymentPayload": "v2 PaymentPayload",
            "paymentRequirements": "v2 PaymentRequirements"
        }
    }))
}

pub async fn get_supported(State(service): State<FacilitatorService>) -> impl IntoResponse {
    let supported = service.supported();
    (StatusCode::OK, Json(supported))
}

pub async fn get_health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

pub async fn post_verify(
    State(service): State<FacilitatorService>,
    Json(body): Json<proto::VerifyRequest>,
) -> impl IntoResponse {
    match service.verify(&body).await {
        Ok(valid_response) => (StatusCode::OK, Json(valid_response)).into_response(),
        Err(crate::service::ServiceError::Adapter(adapter_error)) => {
            warn!(error = %adapter_error, "verify failed");
            verify_error_response(adapter_error)
        }
    }
}

pub async fn post_settle(
    State(service): State<FacilitatorService>,
    Json(body): Json<proto::SettleRequest>,
) -> impl IntoResponse {
    let network_hint = body
        .scheme_handler_slug()
        .map(|slug| slug.chain_id.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    match service.settle(&body).await {
        Ok(settle_response) => (StatusCode::OK, Json(settle_response)).into_response(),
        Err(crate::service::ServiceError::Adapter(adapter_error)) => {
            warn!(error = %adapter_error, network = %network_hint, "settle failed");
            settle_error_response(adapter_error, network_hint)
        }
    }
}

fn verify_error_response(error: AdapterError) -> Response {
    match error {
        AdapterError::Verification(err) => (
            StatusCode::OK,
            Json::<proto::VerifyResponse>(
                v1::VerifyResponse::invalid(None::<String>, err.to_string()).into(),
            ),
        )
            .into_response(),
        AdapterError::InvalidRequest(details) => (
            StatusCode::OK,
            Json::<proto::VerifyResponse>(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {details}"))
                    .into(),
            ),
        )
            .into_response(),
        AdapterError::Upstream(details) => (
            StatusCode::BAD_GATEWAY,
            Json::<proto::VerifyResponse>(
                v1::VerifyResponse::invalid(None::<String>, format!("upstream_error: {details}"))
                    .into(),
            ),
        )
            .into_response(),
    }
}

fn settle_error_response(error: AdapterError, network_hint: String) -> Response {
    match error {
        AdapterError::Verification(err) => (
            StatusCode::OK,
            Json::<proto::SettleResponse>(
                (v1::SettleResponse::Error {
                    reason: err.to_string(),
                    network: network_hint,
                })
                .into(),
            ),
        )
            .into_response(),
        AdapterError::InvalidRequest(details) => (
            StatusCode::OK,
            Json::<proto::SettleResponse>(
                (v1::SettleResponse::Error {
                    reason: format!("invalid_request: {details}"),
                    network: network_hint,
                })
                .into(),
            ),
        )
            .into_response(),
        AdapterError::Upstream(details) => (
            StatusCode::BAD_GATEWAY,
            Json::<proto::SettleResponse>(
                (v1::SettleResponse::Error {
                    reason: format!("upstream_error: {details}"),
                    network: network_hint,
                })
                .into(),
            ),
        )
            .into_response(),
    }
}
