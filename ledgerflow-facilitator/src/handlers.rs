//! Axum handlers wrapping x402-rs facilitator logic.

use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use tracing::instrument;
use x402_rs::{
    facilitator::Facilitator as _,
    facilitator_local::{FacilitatorLocal, PaymentError},
    network::Network,
    types::{
        ErrorResponse, FacilitatorErrorReason, Scheme, SettleRequest, SettleResponse,
        SupportedPaymentKind, VerifyRequest, VerifyResponse, X402Version,
    },
};

#[instrument(skip_all)]
pub async fn get_verify_info() -> impl IntoResponse {
    Json(serde_json::json!({
        "endpoint": "/verify",
        "description": "POST to verify x402 payments",
        "body": {
            "paymentPayload": "PaymentPayload",
            "paymentRequirements": "PaymentRequirements",
        }
    }))
}

#[instrument(skip_all)]
pub async fn get_settle_info() -> impl IntoResponse {
    Json(serde_json::json!({
        "endpoint": "/settle",
        "description": "POST to settle x402 payments",
        "body": {
            "paymentPayload": "PaymentPayload",
            "paymentRequirements": "PaymentRequirements",
        }
    }))
}

#[instrument(skip_all)]
pub async fn get_supported() -> impl IntoResponse {
    let mut kinds = Vec::with_capacity(Network::variants().len());
    for network in Network::variants() {
        kinds.push(SupportedPaymentKind {
            x402_version: X402Version::V1,
            scheme: Scheme::Exact,
            network: *network,
        })
    }
    (StatusCode::OK, Json(kinds))
}

#[instrument(skip_all)]
pub async fn post_verify(
    Extension(facilitator): Extension<FacilitatorLocal>,
    Json(body): Json<VerifyRequest>,
) -> impl IntoResponse {
    let payload = &body.payment_payload;
    let payer = &payload.payload.authorization.from;

    match facilitator.verify(&body).await {
        Ok(valid_response) => (StatusCode::OK, Json(valid_response)).into_response(),
        Err(error) => {
            tracing::warn!(error = ?error, "Verification failed");
            let bad_request = (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid request".to_string(),
                }),
            )
                .into_response();

            let invalid_schema = (
                StatusCode::OK,
                Json(VerifyResponse::invalid(
                    *payer,
                    FacilitatorErrorReason::InvalidScheme,
                )),
            )
                .into_response();

            match error {
                PaymentError::IncompatibleScheme { .. }
                | PaymentError::IncompatibleNetwork { .. }
                | PaymentError::IncompatibleReceivers { .. }
                | PaymentError::InvalidSignature(_)
                | PaymentError::InvalidTiming(_)
                | PaymentError::InsufficientValue => invalid_schema,
                PaymentError::UnsupportedNetwork(_) => (
                    StatusCode::OK,
                    Json(VerifyResponse::invalid(
                        *payer,
                        FacilitatorErrorReason::InvalidNetwork,
                    )),
                )
                    .into_response(),
                PaymentError::InvalidContractCall(_)
                | PaymentError::InvalidAddress(_)
                | PaymentError::ClockError(_) => bad_request,
                PaymentError::InsufficientFunds => (
                    StatusCode::OK,
                    Json(VerifyResponse::invalid(
                        *payer,
                        FacilitatorErrorReason::InsufficientFunds,
                    )),
                )
                    .into_response(),
            }
        }
    }
}

#[instrument(skip_all)]
pub async fn post_settle(
    Extension(facilitator): Extension<FacilitatorLocal>,
    Json(body): Json<SettleRequest>,
) -> impl IntoResponse {
    let payer = &body.payment_payload.payload.authorization.from;
    let network = &body.payment_payload.network;
    match facilitator.settle(&body).await {
        Ok(valid_response) => (StatusCode::OK, Json(valid_response)).into_response(),
        Err(error) => {
            tracing::warn!(error = ?error, "Settlement failed");
            let bad_request = (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid request".to_string(),
                }),
            )
                .into_response();

            let invalid_schema = (
                StatusCode::OK,
                Json(SettleResponse {
                    success: false,
                    error_reason: Some(FacilitatorErrorReason::InvalidScheme),
                    payer: (*payer).into(),
                    transaction: None,
                    network: *network,
                }),
            )
                .into_response();

            match error {
                PaymentError::IncompatibleScheme { .. }
                | PaymentError::IncompatibleNetwork { .. }
                | PaymentError::IncompatibleReceivers { .. }
                | PaymentError::InvalidSignature(_)
                | PaymentError::InvalidTiming(_)
                | PaymentError::InsufficientValue => invalid_schema,
                PaymentError::InvalidContractCall(_)
                | PaymentError::InvalidAddress(_)
                | PaymentError::UnsupportedNetwork(_)
                | PaymentError::ClockError(_) => bad_request,
                PaymentError::InsufficientFunds => (
                    StatusCode::BAD_REQUEST,
                    Json(SettleResponse {
                        success: false,
                        error_reason: Some(FacilitatorErrorReason::InsufficientFunds),
                        payer: (*payer).into(),
                        transaction: None,
                        network: *network,
                    }),
                )
                    .into_response(),
            }
        }
    }
}
