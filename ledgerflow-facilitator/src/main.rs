//! LedgerFlow x402 Facilitator server for Sui blockchain
//!
//! Exposes HTTP endpoints compatible with the x402 spec:
//! - GET  /verify   (info)
//! - POST /verify   (verification)
//! - GET  /settle   (info)
//! - POST /settle   (settlement)
//! - GET  /supported

use std::net::SocketAddr;

use clap::Parser;
use color_eyre::Result;
use dotenvy::dotenv;
use ledgerflow_facilitator::{
    build_app,
    config::{ServerConfig, load_config},
    facilitators::{self, Facilitator},
};
use tower_http::trace::TraceLayer;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    dotenv().ok();

    // CLI + config
    #[derive(Parser, Debug)]
    #[command(
        name = "ledgerflow-facilitator",
        version,
        about = "x402 Facilitator for LedgerFlow on Sui blockchain"
    )]
    struct Args {
        /// Path to config YAML
        #[arg(short, long)]
        config: Option<String>,
    }
    let args = Args::parse();
    let cfg: ServerConfig = load_config(args.config.as_deref())?;

    // logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Apply env from config after logging setup
    cfg.apply_env();

    // Build Sui facilitator from environment
    let facilitator = facilitators::sui_facilitator::SuiFacilitator::from_env()
        .await
        .map_err(|e| eyre::eyre!("Failed to create Sui facilitator: {}", e))?;

    let supported_networks = facilitator.supported_networks();
    tracing::info!(
        networks = ?supported_networks,
        "Sui facilitator initialized with networks"
    );

    if supported_networks.is_empty() {
        tracing::warn!("No Sui networks configured or connected. Running in offline mode.");
    }

    let app = build_app(facilitator).layer(TraceLayer::new_for_http());

    // Listen host/port from config with fallbacks
    let host = cfg.host.unwrap_or_else(|| "0.0.0.0".to_string());
    let port = cfg.port.unwrap_or(9001);

    let ip: std::net::IpAddr = match host.parse::<std::net::IpAddr>() {
        Ok(ip) => ip,
        Err(err) => {
            tracing::error!(%host, %err, "Invalid HOST in config; expected an IP address");
            return Err(eyre::eyre!("invalid HOST: {}", host));
        }
    };
    let addr: SocketAddr = (ip, port).into();
    tracing::info!(%addr, "Starting ledgerflow-facilitator for Sui");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Successfully bound to {}, starting HTTP server", addr);
    axum::serve(listener, app).await?;
    tracing::info!("Server shut down gracefully");
    Ok(())
}
