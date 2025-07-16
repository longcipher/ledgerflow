mod config;
mod database;
mod indexer;
mod types;

use std::path::PathBuf;

use clap::Parser;
use config::Config;
use database::Database;
use eyre::Result;
use indexer::Indexer;
use tracing::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let filter = std::env::var("RUST_LOG")
        .map(|_| tracing_subscriber::EnvFilter::from_default_env())
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();

    info!("🚀 Starting LedgerFlow Indexer");
    info!("📋 Configuration file: {}", args.config.display());

    // Load configuration
    let config = Config::from_file(&args.config).await?;
    info!("✅ Loaded configuration for {} chains", config.chains.len());

    // Log chain configurations
    for chain in &config.chains {
        info!(
            "⛓️  Chain: {} (ID: {}) - RPC: {}",
            chain.name, chain.chain_id, chain.rpc_http
        );
        info!(
            "   📄 Contract: {}, Start Block: {}",
            chain.payment_vault_contract, chain.start_block
        );
    }

    // Initialize database
    let database = Database::new(&config.database.url).await?;
    info!("✅ Connected to database successfully");

    // Create and start indexer
    let indexer = Indexer::new(config, database).await?;
    info!("✅ Indexer initialized successfully");

    info!("🔥 Starting indexing process...");
    // Start indexing
    indexer.start().await?;

    info!("🛑 Indexer stopped");
    Ok(())
}
