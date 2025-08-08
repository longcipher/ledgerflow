use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    response::Json,
    routing::{get, post},
};
use clap::Parser;
use eyre::Result;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};

mod config;
mod database;
mod error;
mod handlers;
mod models;
mod services;
mod utils;
mod x402;

use config::Config;
use database::Database;
use error::AppError;
use services::BalanceService;

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
    let filter = std::env::var("RUST_LOG")
        .map(|_| tracing_subscriber::EnvFilter::from_default_env())
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("🚀 LedgerFlow Balancer starting up...");

    let args = Args::parse();

    // Load configuration
    info!("📋 Loading configuration from {}", args.config);
    let config = Arc::new(Config::from_file(&args.config)?);
    info!("✅ Configuration loaded successfully from {}", args.config);

    // Initialize database
    info!("🔗 Connecting to database...");
    let db = Arc::new(Database::new(&config.database_url).await?);
    info!("✅ Database connected successfully");

    let app_state = AppState {
        db: db.clone(),
        config: config.clone(),
    };

    // Spawn background task for processing deposited orders
    info!("🔄 Starting background task for processing deposited orders...");
    let db_clone = db.clone();
    tokio::spawn(async move {
        process_deposited_orders_task(db_clone).await;
    });
    info!("✅ Background task started successfully");

    // Build application
    info!("🏗️ Building application routes...");
    let app = Router::new()
        .route("/health", get(health_check))
        // x402 endpoints
        .route("/x402/supported", get(x402::supported))
        .route("/x402/verify", post(x402::verify))
        .route("/x402/settle", post(x402::settle))
        .route("/register", post(handlers::register_account))
        .route(
            "/accounts/username/{username}",
            get(handlers::get_account_by_username),
        )
        .route(
            "/accounts/email/{email}",
            get(handlers::get_account_by_email),
        )
        .route(
            "/accounts/telegram/{telegram_id}",
            get(handlers::get_account_by_telegram_id),
        )
        .route("/orders", post(handlers::create_order))
        .route("/orders/{order_id}", get(handlers::get_order))
        .route("/accounts/{account_id}/balance", get(handlers::get_balance))
        .route("/admin/orders", get(handlers::list_pending_orders))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let bind_address = format!("{}:{}", config.server.host, config.server.port);
    info!("🌐 Binding server to {}", bind_address);
    let listener = TcpListener::bind(&bind_address).await?;

    info!(
        "🎯 LedgerFlow Balancer is ready and listening on {}",
        bind_address
    );
    info!("💡 Available endpoints:");
    info!("   - GET  /health - Health check");
    info!("   - POST /register - Register new account");
    info!("   - GET  /accounts/username/{{username}} - Get account by username");
    info!("   - GET  /accounts/email/{{email}} - Get account by email");
    info!("   - GET  /accounts/telegram/{{telegram_id}} - Get account by telegram ID");
    info!("   - POST /orders - Create new order");
    info!("   - GET  /orders/{{order_id}} - Get order by ID");
    info!("   - GET  /accounts/{{account_id}}/balance - Get account balance");
    info!("   - GET  /admin/orders - List pending orders");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> Result<Json<serde_json::Value>, AppError> {
    info!("🏥 Health check requested");
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "ledgerflow-balancer"
    })))
}

/// Background task to process deposited orders
async fn process_deposited_orders_task(db: Arc<Database>) {
    info!("🔄 Background task: Starting deposited orders processing loop");
    let balance_service = BalanceService::new((*db).clone());
    let mut interval = tokio::time::interval(Duration::from_secs(5)); // Check every 5 seconds

    loop {
        interval.tick().await;

        match balance_service.process_deposited_orders().await {
            Ok(count) => {
                if count > 0 {
                    info!(
                        "✅ Background task: Successfully processed {} deposited orders",
                        count
                    );
                } else {
                    info!("⏸️ Background task: No deposited orders to process");
                }
            }
            Err(e) => {
                error!(
                    "❌ Background task: Error processing deposited orders: {}",
                    e
                );
            }
        }
    }
}
