use std::path::Path;

use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub database_url: String,
    pub telegram: TelegramConfig,
    pub balancer: BalancerConfig,
    pub blockchain: BlockchainConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BalancerConfig {
    pub base_url: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockchainConfig {
    pub rpc_url: String,
    pub payment_vault_address: String,
    pub chain_id: u64,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "postgresql://localhost:5432/ledgerflow".to_string(),
            telegram: TelegramConfig {
                bot_token: "YOUR_BOT_TOKEN_HERE".to_string(),
                webhook_url: None,
            },
            balancer: BalancerConfig {
                base_url: "http://localhost:3000".to_string(),
                timeout_seconds: 30,
            },
            blockchain: BlockchainConfig {
                rpc_url: "https://sepolia.unichain.org".to_string(),
                payment_vault_address: "0x0000000000000000000000000000000000000000".to_string(),
                chain_id: 1301,
            },
        }
    }
}
