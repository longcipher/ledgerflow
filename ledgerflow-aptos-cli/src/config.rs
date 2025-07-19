use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// CLI configuration for connecting to Aptos network and managing accounts
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

/// Network configuration for Aptos blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// REST API URL for the Aptos node
    #[serde(default = "default_node_url")]
    pub node_url: String,
    /// Chain ID (1 for mainnet, 2 for testnet, etc.)
    #[serde(default = "default_chain_id")]
    pub chain_id: u8,
    /// Optional faucet URL for testnet/devnet
    pub faucet_url: Option<String>,
}

/// Account configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountConfig {
    /// Private key in hex format (with or without 0x prefix)
    pub private_key: String,
    /// Optional account address override (derived from private key if not provided)
    pub address: Option<String>,
}

/// Transaction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionConfig {
    /// Maximum gas amount for transactions
    #[serde(default = "default_max_gas")]
    pub max_gas: u64,
    /// Gas unit price in octas (if not specified, will be estimated)
    pub gas_unit_price: Option<u64>,
    /// Transaction expiration timeout in seconds
    #[serde(default = "default_expiration_secs")]
    pub expiration_secs: u64,
    /// Whether to wait for transaction completion
    #[serde(default = "default_wait_for_transaction")]
    pub wait_for_transaction: bool,
}

/// Vault contract configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Address where the payment vault contract is deployed
    pub contract_address: String,
    /// Module name (usually "payment_vault")
    #[serde(default = "default_module_name")]
    pub module_name: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            node_url: default_node_url(),
            chain_id: default_chain_id(),
            faucet_url: Some(default_faucet_url()),
        }
    }
}

impl Default for AccountConfig {
    fn default() -> Self {
        Self {
            private_key: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            address: None,
        }
    }
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            max_gas: default_max_gas(),
            gas_unit_price: None,
            expiration_secs: default_expiration_secs(),
            wait_for_transaction: default_wait_for_transaction(),
        }
    }
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            contract_address: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            module_name: default_module_name(),
        }
    }
}

fn default_node_url() -> String {
    "https://api.devnet.aptoslabs.com/v1".to_string()
}

fn default_chain_id() -> u8 {
    4 // Devnet
}

fn default_faucet_url() -> String {
    "https://faucet.devnet.aptoslabs.com".to_string()
}

fn default_max_gas() -> u64 {
    100_000
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
