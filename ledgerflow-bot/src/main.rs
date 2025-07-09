use std::error::Error;

use teloxide::prelude::*;
use tracing::{error, info};

mod bot;
mod config;
mod database;
mod error;
mod handlers;
mod models;
mod notification;
mod services;
mod wallet;

use crate::{
    config::Config, database::Database, handlers::create_handler, notification::NotificationService,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize tracing
    let filter = std::env::var("RUST_LOG")
        .map(|_| tracing_subscriber::EnvFilter::from_default_env())
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting LedgerFlow Bot...");

    let config = Config::from_file("config.yaml")?;
    let database = Database::new(&config.database_url).await?;
    let bot = Bot::new(&config.telegram.bot_token);

    // Start notification service
    let notification_service = NotificationService::new(bot.clone(), database.clone());
    tokio::spawn(async move {
        if let Err(e) = notification_service.start_notification_loop().await {
            error!("Notification service error: {}", e);
        }
    });

    info!("Bot started successfully");

    let handler = create_handler(bot.clone(), database, config).await?;

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}
