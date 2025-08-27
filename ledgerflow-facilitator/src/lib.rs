pub mod config;
pub mod facilitators;
pub mod handlers;
pub mod types;

use axum::{
    Extension, Router,
    http::Method,
    routing::{get, post},
};
use tower_http::{cors, trace::TraceLayer};

use crate::facilitators::Facilitator;

/// Build the Axum app with all routes and layers.
pub fn build_app<F: Facilitator + Clone + 'static>(facilitator: F) -> Router {
    Router::new()
        .route("/", get(|| async { "ledgerflow-facilitator ok" }))
        .route("/verify", get(handlers::get_verify_info))
        .route("/verify", post(handlers::post_verify::<F>))
        .route("/settle", get(handlers::get_settle_info))
        .route("/settle", post(handlers::post_settle::<F>))
        .route("/supported", get(handlers::get_supported::<F>))
        .layer(Extension(facilitator))
        .layer(TraceLayer::new_for_http())
        .layer(
            cors::CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(cors::Any),
        )
}
