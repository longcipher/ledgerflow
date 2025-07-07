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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    info!("Starting LedgerFlow Indexer");

    // Load configuration
    let config = Config::from_file(&args.config).await?;
    info!("Loaded configuration for {} chains", config.chains.len());

    // Initialize database
    let database = Database::new(&config.database.url).await?;
    info!("Connected to database");

    // Run migrations
    database.migrate().await?;
    info!("Database migrations completed");

    // Create and start indexer
    let indexer = Indexer::new(config, database).await?;
    info!("Indexer initialized");

    // Start indexing
    indexer.start().await?;

    Ok(())
}
