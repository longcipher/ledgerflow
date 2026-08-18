//! LedgerFlow server binary.

#![allow(clippy::print_stdout)]

use clap::Parser;
use eyre::{Result, WrapErr};
use ledgerflow_server::{AppState, config::ServerConfig, saas::saas_auth_middleware};

#[derive(Debug, Parser)]
#[command(name = "ledgerflow-server", version, about = "LedgerFlow deployment server")]
struct Cli {
    /// Path to the revocation store (JSON Lines file).
    #[arg(long, default_value = "./data/revocations.jsonl")]
    revocation_store: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = ServerConfig::from_env().wrap_err("invalid configuration (fail-fast)")?;
    println!("ledgerflow-server: bind={} mode={:?}", config.bind_addr, config.saas.mode);

    let state = AppState::new(config.clone(), &cli.revocation_store, {
        let mut trusted = ledgerflow_core::TrustedIssuers::new();
        let issuer = ledgerflow_core::SigningKeyPair::from_bytes(&[1_u8; 32]);
        trusted
            .add(ledgerflow_core::TrustedIssuer::new("issuer-1".to_string(), issuer.signer_ref()));
        trusted
    })
    .wrap_err("failed to initialize application state")?;

    let saas_extractor = state.saas.clone();
    let app = ledgerflow_server::api::router()
        .with_state(state)
        .layer(axum::middleware::from_fn_with_state(saas_extractor, saas_auth_middleware));

    let listener =
        tokio::net::TcpListener::bind(&config.bind_addr).await.wrap_err("failed to bind")?;
    println!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await.wrap_err("server error")
}
