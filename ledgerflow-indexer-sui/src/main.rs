use std::path::PathBuf;

use clap::Parser;
use eyre::Result;
use tracing::{info, warn};

mod config;
mod database;
mod health;
mod models;
mod processor;

use config::Config;

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

    info!("🚀 Starting LedgerFlow Sui Indexer");
    info!("📋 Configuration file: {}", args.config.display());

    // Load configuration
    let config = Config::from_file(&args.config).await?;
    info!(
        "✅ Loaded configuration for package: {}",
        config.contract.package_id
    );

    // Log configuration details
    info!(
        "📄 Package ID: {}, Module: {}, Starting Checkpoint: {}",
        config.contract.package_id, config.contract.module_name, config.indexer.starting_checkpoint
    );
    info!(
        "🗄️  Database URL: {}",
        &config.database.connection_string[..30]
    );

    // Start the indexer processor
    match processor::run_indexer(config).await {
        Ok(()) => {
            info!("✅ Indexer completed successfully");
            Ok(())
        }
        Err(e) => {
            warn!("❌ Indexer encountered an error: {:?}", e);
            Err(e)
        }
    }
}
