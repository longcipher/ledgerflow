#[path = "config.rs"]
pub mod config;
#[path = "handlers.rs"]
pub mod handlers;
use axum::{
    http::Method,
    routing::{get, post},
    Extension, Router,
};
use tower_http::{cors, trace::TraceLayer};
use x402_rs::facilitator_local::FacilitatorLocal;

/// Build the Axum app with all routes and layers.
pub fn build_app(facilitator: FacilitatorLocal) -> Router {
    Router::new()
        .route("/", get(|| async { "ledgerflow-facilitator ok" }))
        .route("/verify", get(handlers::get_verify_info))
        .route("/verify", post(handlers::post_verify))
        .route("/settle", get(handlers::get_settle_info))
        .route("/settle", post(handlers::post_settle))
        .route("/supported", get(handlers::get_supported))
        .layer(Extension(facilitator))
        .layer(TraceLayer::new_for_http())
        .layer(
            cors::CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(cors::Any),
        )
}
