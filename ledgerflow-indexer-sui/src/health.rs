use std::sync::Arc;

use axum::{Router, extract::State, http::StatusCode, response::Json, routing::get};
use eyre::Result;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::database::Database;

#[derive(Clone)]
pub struct HealthState {
    pub database: Arc<Database>,
}

/// Start the health check server
pub async fn start_health_server(port: u16, database: Arc<Database>) -> Result<()> {
    let state = HealthState { database };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    info!("🏥 Starting health check server on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Basic health check endpoint
async fn health_check() -> Result<Json<Value>, StatusCode> {
    let response = json!({
        "status": "healthy",
        "service": "ledgerflow-indexer-sui",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    Ok(Json(response))
}

/// Readiness check that includes database connectivity
async fn readiness_check(State(state): State<HealthState>) -> Result<Json<Value>, StatusCode> {
    // Check database connectivity
    match state.database.health_check().await {
        Ok(()) => {
            let response = json!({
                "status": "ready",
                "service": "ledgerflow-indexer-sui",
                "database": "connected",
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            Ok(Json(response))
        }
        Err(e) => {
            warn!("Database health check failed: {:?}", e);
            let _response = json!({
                "status": "not_ready",
                "service": "ledgerflow-indexer-sui",
                "database": "disconnected",
                "error": e.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}
