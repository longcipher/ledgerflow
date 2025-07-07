#![allow(unused)]

use eyre::Report;
use teloxide::RequestError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Telegram API error: {0}")]
    Telegram(#[from] RequestError),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Wallet error: {0}")]
    Wallet(String),

    #[error("Balancer API error: {0}")]
    BalancerApi(String),

    #[error("Blockchain error: {0}")]
    Blockchain(String),

    #[error("User input error: {0}")]
    UserInput(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("QR code generation error: {0}")]
    QrCode(#[from] qrcode::types::QrError),

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("General error: {0}")]
    General(#[from] Report),
}

pub type BotResult<T> = Result<T, BotError>;
