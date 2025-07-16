use std::path::PathBuf;

use clap::Parser;
use eyre::Result;
use tracing::{info, warn};

mod config;
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

    info!("🚀 Starting LedgerFlow Aptos Indexer");
    info!("📋 Configuration file: {}", args.config.display());

    // Load configuration
    let config = Config::from_file(&args.config).await?;
    info!(
        "✅ Loaded configuration for contract: {}",
        config.contract_address
    );

    // Log configuration details
    info!(
        "📄 Contract Address: {}, Starting Version: {}",
        config.contract_address, config.starting_version
    );
    info!(
        "🗄️  Database URL: {}",
        &config.postgres_config.connection_string[..20]
    );

    // Start the processor
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
