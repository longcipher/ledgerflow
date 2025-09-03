//! Axum handlers for x402 facilitator endpoints.

use axum::{Extension, Json, http::StatusCode, response::IntoResponse};
use tracing::instrument;

use crate::{
    facilitators::{Facilitator, PaymentError},
    types::{
        ErrorResponse, FacilitatorErrorReason, MixedAddress, Scheme, SettleRequest, SettleResponse,
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
pub async fn get_supported<F: Facilitator>(
    Extension(facilitator): Extension<F>,
) -> impl IntoResponse {
    tracing::info!("GET /supported endpoint called");
    let mut kinds = Vec::new();

    for network in facilitator.supported_networks() {
        kinds.push(SupportedPaymentKind {
            x402_version: X402Version::V1,
            scheme: Scheme::Exact,
            network,
            extra: None,
        });
    }

    (StatusCode::OK, Json(kinds))
}

#[instrument(skip_all)]
pub async fn post_verify<F: Facilitator>(
    Extension(facilitator): Extension<F>,
    Json(body): Json<VerifyRequest>,
) -> impl IntoResponse {
    // Extract payer from payload for error responses
    let payer: Option<MixedAddress> = match &body.payment_payload.payload {
        crate::types::ExactPaymentPayload::Sui(sui_payload) => {
            Some(sui_payload.authorization.from.into())
        }
        crate::types::ExactPaymentPayload::Evm(evm_payload) => {
            Some(evm_payload.authorization.from.into())
        }
    };

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

            let make_invalid_response = |reason: FacilitatorErrorReason| {
                (StatusCode::OK, Json(VerifyResponse::invalid(payer, reason))).into_response()
            };

            match error {
                PaymentError::IncompatibleScheme { .. }
                | PaymentError::IncompatibleReceivers { .. }
                | PaymentError::InvalidSignature(_)
                | PaymentError::InvalidTiming(_)
                | PaymentError::InsufficientAmount => {
                    make_invalid_response(FacilitatorErrorReason::InvalidScheme)
                }
                PaymentError::IncompatibleNetwork { .. } | PaymentError::UnsupportedNetwork(_) => {
                    make_invalid_response(FacilitatorErrorReason::InvalidNetwork)
                }
                PaymentError::InsufficientFunds => {
                    make_invalid_response(FacilitatorErrorReason::InsufficientFunds)
                }
                PaymentError::IntentSigningError(_) => {
                    make_invalid_response(FacilitatorErrorReason::InvalidSignature)
                }
                PaymentError::InvalidContractCall(_)
                | PaymentError::InvalidAddress(_)
                | PaymentError::ClockError(_)
                | PaymentError::SuiError(_)
                | PaymentError::TransactionExecutionError(_) => bad_request,
            }
        }
    }
}

#[instrument(skip_all)]
pub async fn post_settle<F: Facilitator>(
    Extension(facilitator): Extension<F>,
    Json(body): Json<SettleRequest>,
) -> impl IntoResponse {
    // Extract payer and network from payload for error responses
    let (payer, network) = match &body.payment_payload.payload {
        crate::types::ExactPaymentPayload::Sui(sui_payload) => (
            sui_payload.authorization.from.into(),
            body.payment_payload.network,
        ),
        crate::types::ExactPaymentPayload::Evm(evm_payload) => (
            evm_payload.authorization.from.into(),
            body.payment_payload.network,
        ),
    };

    match facilitator.settle(&body).await {
        Ok(settle_response) => (StatusCode::OK, Json(settle_response)).into_response(),
        Err(error) => {
            tracing::warn!(error = ?error, "Settlement failed");

            let _bad_request = (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid request".to_string(),
                }),
            )
                .into_response();

            let make_settle_error = |reason: FacilitatorErrorReason| {
                (
                    StatusCode::OK,
                    Json(SettleResponse {
                        success: false,
                        error_reason: Some(reason),
                        payer,
                        transaction: None,
                        network,
                    }),
                )
                    .into_response()
            };

            match error {
                PaymentError::IncompatibleScheme { .. }
                | PaymentError::IncompatibleReceivers { .. }
                | PaymentError::InvalidSignature(_)
                | PaymentError::InvalidTiming(_)
                | PaymentError::InsufficientAmount => {
                    make_settle_error(FacilitatorErrorReason::InvalidScheme)
                }
                PaymentError::IncompatibleNetwork { .. } | PaymentError::UnsupportedNetwork(_) => {
                    make_settle_error(FacilitatorErrorReason::InvalidNetwork)
                }
                PaymentError::InsufficientFunds => {
                    make_settle_error(FacilitatorErrorReason::InsufficientFunds)
                }
                PaymentError::IntentSigningError(_) => {
                    make_settle_error(FacilitatorErrorReason::InvalidSignature)
                }
                PaymentError::InvalidContractCall(_)
                | PaymentError::InvalidAddress(_)
                | PaymentError::ClockError(_)
                | PaymentError::SuiError(_)
                | PaymentError::TransactionExecutionError(_) => {
                    make_settle_error(FacilitatorErrorReason::UnexpectedSettleError)
                }
            }
        }
    }
}
