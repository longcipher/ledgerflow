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

    // The trust anchor is derived from the configured issuer key (fail-fast
    // guarantees it is present; never a predictable demo key — design §6.8).
    let issuer_key_hex = config
        .issuer_key_hex
        .clone()
        .ok_or_else(|| eyre::eyre!("LEDGERFLOW_ISSUER_KEY is required (fail-fast)"))?;
    let issuer_key_bytes = hex_decode(&issuer_key_hex)
        .ok_or_else(|| eyre::eyre!("LEDGERFLOW_ISSUER_KEY must be 32-byte hex"))?;
    let issuer_key = ledgerflow_core::SigningKeyPair::from_bytes(&issuer_key_bytes);

    let state = AppState::new(config.clone(), &cli.revocation_store, {
        let mut trusted = ledgerflow_core::TrustedIssuers::new();
        trusted.add(ledgerflow_core::TrustedIssuer::new(
            "issuer-1".to_string(),
            issuer_key.signer_ref(),
        ));
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

/// Decodes a 32-byte hex string into bytes.
fn hex_decode(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(out)
}
