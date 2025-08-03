use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// CLI configuration for connecting to Sui network and managing accounts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Network configuration
    pub network: NetworkConfig,
    /// Account configuration
    pub account: AccountConfig,
    /// Transaction configuration
    pub transaction: TransactionConfig,
    /// Vault configuration
    pub vault: VaultConfig,
}

/// Network configuration for Sui blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// RPC URL for the Sui Full node
    #[serde(default = "default_rpc_url")]
    pub rpc_url: String,
    /// WebSocket URL for event subscription (optional)
    pub ws_url: Option<String>,
    /// Network name (devnet, testnet, mainnet, or localnet)
    #[serde(default = "default_network")]
    pub network: String,
}

/// Account configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    /// Private key in hex format (with or without 0x prefix)
    pub private_key: String,
    /// Optional account address override (derived from private key if not provided)
    pub address: Option<String>,
    /// Key scheme (ed25519, secp256k1, secp256r1)
    #[serde(default = "default_key_scheme")]
    pub key_scheme: String,
}

/// Transaction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionConfig {
    /// Gas budget for transactions (in MIST)
    #[serde(default = "default_gas_budget")]
    pub gas_budget: u64,
    /// Gas price (if not specified, will be estimated)
    pub gas_price: Option<u64>,
    /// Transaction expiration timeout in seconds
    #[serde(default = "default_expiration_secs")]
    pub expiration_secs: u64,
    /// Whether to wait for transaction confirmation
    #[serde(default = "default_wait_for_transaction")]
    pub wait_for_transaction: bool,
}

/// Vault contract configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Package ID where the payment vault contract is deployed
    pub package_id: String,
    /// Module name (usually "payment_vault")
    #[serde(default = "default_module_name")]
    pub module_name: String,
    /// Vault object ID (the shared object containing the vault)
    pub vault_object_id: String,
    /// USDC coin type (e.g., "0x2::sui::SUI" for SUI or custom USDC type)
    #[serde(default = "default_usdc_type")]
    pub usdc_type: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            rpc_url: default_rpc_url(),
            ws_url: Some(default_ws_url()),
            network: default_network(),
        }
    }
}

impl Default for AccountConfig {
    fn default() -> Self {
        Self {
            private_key: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            address: None,
            key_scheme: default_key_scheme(),
        }
    }
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            gas_budget: default_gas_budget(),
            gas_price: None,
            expiration_secs: default_expiration_secs(),
            wait_for_transaction: default_wait_for_transaction(),
        }
    }
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            package_id: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            module_name: default_module_name(),
            vault_object_id: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            usdc_type: default_usdc_type(),
        }
    }
}

fn default_rpc_url() -> String {
    "https://fullnode.devnet.sui.io:443".to_string()
}

fn default_ws_url() -> String {
    "wss://fullnode.devnet.sui.io:443".to_string()
}

fn default_network() -> String {
    "devnet".to_string()
}

fn default_key_scheme() -> String {
    "ed25519".to_string()
}

fn default_gas_budget() -> u64 {
    10_000_000 // 0.01 SUI in MIST
}

fn default_expiration_secs() -> u64 {
    600 // 10 minutes
}

fn default_wait_for_transaction() -> bool {
    true
}

fn default_module_name() -> String {
    "payment_vault".to_string()
}

fn default_usdc_type() -> String {
    "0x2::sui::SUI".to_string() // Default to SUI for testing, should be USDC in production
}

impl Config {
    /// Save configuration to file
    pub fn to_file(&self, path: &PathBuf) -> eyre::Result<()> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load configuration using config crate with environment variable support
    pub fn load(config_path: Option<PathBuf>) -> eyre::Result<Self> {
        let mut builder = config::Config::builder();

        // Add file source if provided
        if let Some(path) = config_path {
            builder = builder.add_source(config::File::from(path).required(false));
        }

        // Add environment variables with prefix "LEDGERFLOW_"
        builder = builder.add_source(
            config::Environment::with_prefix("LEDGERFLOW")
                .separator("_")
                .list_separator(","),
        );

        let settings = builder.build()?;
        let config: Config = settings.try_deserialize()?;
        Ok(config)
    }

    /// Create a sample configuration file
    pub fn create_sample(path: &PathBuf) -> eyre::Result<()> {
        let config = Config::default();
        config.to_file(path)?;
        println!("Created sample configuration file at: {}", path.display());
        println!("Please edit the file to configure your account and vault settings.");
        Ok(())
    }
}
