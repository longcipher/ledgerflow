use clap::{Parser, Subcommand};
use eyre::Result;
use teloxide::{prelude::*, Bot};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod bot;
mod config;
mod database;
mod error;
mod handlers;
mod models;
mod services;
mod wallet;

use crate::{config::Config, database::Database};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Telegram bot
    Start {
        /// Path to configuration file
        #[arg(short, long, default_value = "config.yaml")]
        config: String,
    },
    /// Generate a new wallet
    GenerateWallet,
    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize color_eyre for better error reports
    color_eyre::install()?;

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    match cli.command {
        Commands::Start { config } => {
            info!("Starting LedgerFlow Telegram Bot");
            start_bot(config).await?;
        }
        Commands::GenerateWallet => {
            let wallet = wallet::generate_wallet().await?;
            println!("Generated new wallet:");
            println!("Address: {}", wallet.address);
            println!("Private Key: {} (Keep this secure!)", wallet.private_key);
        }
        Commands::Version => {
            println!("LedgerFlow Bot v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

async fn start_bot(config_path: String) -> Result<()> {
    // Load configuration
    let config = Config::from_file(&config_path)?;

    // Initialize database connection
    let database = Database::new(&config.database_url).await?;

    // Run database migrations
    database.migrate().await?;

    // Initialize Telegram bot
    let bot = Bot::new(&config.telegram.bot_token);

    info!("Bot started successfully!");

    // Create the handler with shared state
    let handler = handlers::create_handler(bot.clone(), database, config).await?;

    // Start the bot dispatcher
    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
