use std::path::Path;

use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Health check port for the indexer service
    pub health_check_port: u16,
    /// Server configuration for the indexer processor
    pub server_config: ServerConfig,
    /// Contract address to monitor on Aptos
    pub contract_address: String,
    /// Starting version/sequence number to index from
    pub starting_version: u64,
    /// PostgreSQL configuration
    pub postgres_config: PostgresConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Transaction stream configuration
    pub transaction_stream_config: TransactionStreamConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStreamConfig {
    /// Indexer gRPC data service address
    pub indexer_grpc_data_service_address: String,
    /// Authentication token for the indexer service
    pub auth_token: String,
    /// Request name header for identification
    pub request_name_header: String,
    /// Starting version for transaction processing
    pub starting_version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    /// PostgreSQL connection string
    pub connection_string: String,
}

impl Config {
    /// Load configuration from a YAML file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
