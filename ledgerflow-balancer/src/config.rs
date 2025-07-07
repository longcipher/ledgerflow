use std::fs;

use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub server: ServerConfig,
    pub business: BusinessConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessConfig {
    pub max_pending_orders_per_account: u32,
    pub broker_id: String,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "postgresql://localhost/ledgerflow".to_string(),
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
            },
            business: BusinessConfig {
                max_pending_orders_per_account: 2,
                broker_id: "ledgerflow-vault".to_string(),
            },
        }
    }
}
