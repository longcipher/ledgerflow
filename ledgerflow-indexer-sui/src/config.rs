use std::path::Path;

use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Health check port for the indexer service
    pub health_check_port: u16,
    /// Sui network configuration
    pub network: NetworkConfig,
    /// Contract/package configuration to monitor
    pub contract: ContractConfig,
    /// Indexer behavior configuration
    pub indexer: IndexerConfig,
    /// Database configuration
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Sui Full node RPC URL
    pub rpc_url: String,
    /// WebSocket URL for event subscription
    pub ws_url: Option<String>,
    /// Network name (devnet, testnet, mainnet, localnet)
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractConfig {
    /// Package ID of the payment vault contract
    pub package_id: String,
    /// Module name within the package (typically "payment_vault")
    pub module_name: String,
    /// Event struct name to monitor (typically "DepositReceived")
    pub deposit_event_type: String,
    /// Event struct name for withdrawals (typically "WithdrawCompleted")
    pub withdraw_event_type: String,
    /// Event struct name for ownership transfers (typically "OwnershipTransferred")  
    pub ownership_transfer_event_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    /// Starting checkpoint sequence number to index from (0 for from beginning)
    pub starting_checkpoint: u64,
    /// Number of checkpoints to process in each batch
    pub checkpoint_batch_size: u64,
    /// Delay between processing batches (milliseconds)
    pub processing_delay_ms: u64,
    /// Maximum number of retries for failed operations
    pub max_retries: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// PostgreSQL connection string
    pub connection_string: String,
    /// Maximum number of database connections in the pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
}

impl Config {
    /// Load configuration from a YAML file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
