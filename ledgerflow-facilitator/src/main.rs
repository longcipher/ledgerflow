use std::net::SocketAddr;

use clap::Parser;
use color_eyre::Result;
use dotenvy::dotenv;
use ledgerflow_facilitator::{
    AppConfig, build_app,
    config::{build_service, load_config},
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "ledgerflow-facilitator",
    version,
    about = "Chain-agnostic x402 v2 facilitator with offchain/CEX adapter support"
)]
struct Args {
    /// Path to facilitator config file.
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    dotenv().ok();

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();
    let cfg = load_config(args.config.as_deref())?;
    let service = build_service(&cfg)?;

    let app_config = AppConfig {
        rate_limit_per_second: cfg.rate_limit_per_second,
    };
    let app = build_app(service, app_config);

    let host = cfg.host.unwrap_or_else(|| "0.0.0.0".to_string());
    let port = cfg.port.unwrap_or(3402);

    let ip: std::net::IpAddr = host
        .parse()
        .map_err(|_| eyre::eyre!("invalid host in config: {host}"))?;

    let addr: SocketAddr = (ip, port).into();
    tracing::info!(%addr, "starting ledgerflow facilitator");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
