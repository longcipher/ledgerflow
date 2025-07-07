use std::sync::Arc;

use axum::{
    Router,
    response::Json,
    routing::{get, post},
};
use clap::Parser;
use eyre::Result;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

mod config;
mod database;
mod error;
mod handlers;
mod models;
mod services;
mod utils;

use config::Config;
use database::Database;
use error::AppError;

#[derive(Parser)]
#[command(name = "ledgerflow-balancer")]
#[command(about = "LedgerFlow Balancer - Backend service for payment management")]
struct Args {
    #[arg(short, long, default_value = "config.yaml")]
    config: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Load configuration
    let config = Arc::new(Config::from_file(&args.config)?);
    info!("Configuration loaded from {}", args.config);

    // Initialize database
    let db = Arc::new(Database::new(&config.database_url).await?);
    info!("Database connected");

    // Run migrations
    db.migrate().await?;
    info!("Database migrations completed");

    let app_state = AppState {
        db,
        config: config.clone(),
    };

    // Build application
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/orders", post(handlers::create_order))
        .route("/orders/:order_id", get(handlers::get_order))
        .route("/accounts/:account_id/balance", get(handlers::get_balance))
        .route("/admin/orders", get(handlers::list_pending_orders))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let bind_address = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&bind_address).await?;

    info!("Starting server on {}", bind_address);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "ledgerflow-balancer"
    })))
}
