pub mod adapters;
pub mod config;
pub mod handlers;
pub mod service;

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{Method, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use tower_http::{cors, trace::TraceLayer};

use crate::service::FacilitatorService;

/// Maximum request body size (1 MB). Far exceeds any valid x402 payload.
const DEFAULT_BODY_LIMIT: usize = 1_048_576;

/// Application-level config consumed by [`build_app`].
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    /// Maximum requests per second (global). `None` or `0` = unlimited.
    pub rate_limit_per_second: Option<u64>,
}

pub fn build_app(service: FacilitatorService, app_config: AppConfig) -> Router {
    let router = Router::new()
        .route("/", get(|| async { "ledgerflow-facilitator ok" }))
        .route("/verify", get(handlers::get_verify_info))
        .route("/verify", post(handlers::post_verify))
        .route("/settle", get(handlers::get_settle_info))
        .route("/settle", post(handlers::post_settle))
        .route("/supported", get(handlers::get_supported))
        .route("/health", get(handlers::get_health))
        .with_state(service)
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT))
        .layer(TraceLayer::new_for_http())
        .layer(
            cors::CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(cors::Any),
        );

    // Apply fixed-window rate limiting when configured.
    match app_config.rate_limit_per_second {
        Some(rps) if rps > 0 => {
            let counter = Arc::new(AtomicU64::new(0));

            // Background task resets the counter every second.
            let counter_reset = counter.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    counter_reset.store(0, Ordering::Relaxed);
                }
            });

            router.route_layer(middleware::from_fn(
                move |req: axum::extract::Request, next: middleware::Next| {
                    let counter = counter.clone();
                    async move {
                        if counter.fetch_add(1, Ordering::Relaxed) >= rps {
                            StatusCode::TOO_MANY_REQUESTS.into_response()
                        } else {
                            next.run(req).await.into_response()
                        }
                    }
                },
            ))
        }
        _ => router,
    }
}
