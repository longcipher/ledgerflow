pub mod adapters;
pub mod config;
pub mod handlers;
pub mod service;

use axum::{
    Router,
    http::Method,
    routing::{get, post},
};
use tower_http::{cors, trace::TraceLayer};

use crate::service::FacilitatorService;

pub fn build_app(service: FacilitatorService) -> Router {
    Router::new()
        .route("/", get(|| async { "ledgerflow-facilitator ok" }))
        .route("/verify", get(handlers::get_verify_info))
        .route("/verify", post(handlers::post_verify))
        .route("/settle", get(handlers::get_settle_info))
        .route("/settle", post(handlers::post_settle))
        .route("/supported", get(handlers::get_supported))
        .route("/health", get(handlers::get_health))
        .with_state(service)
        .layer(TraceLayer::new_for_http())
        .layer(
            cors::CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(cors::Any),
        )
}
