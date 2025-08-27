use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub host: Option<String>,
    pub port: Option<u16>,

    // Sui gRPC URLs
    pub sui_mainnet_grpc_url: Option<String>,
    pub sui_testnet_grpc_url: Option<String>,
    pub sui_devnet_grpc_url: Option<String>,

    // Sui package IDs for USDC and Vault contracts
    pub sui_mainnet_usdc_package_id: Option<String>,
    pub sui_testnet_usdc_package_id: Option<String>,
    pub sui_devnet_usdc_package_id: Option<String>,
    pub sui_mainnet_vault_package_id: Option<String>,
    pub sui_testnet_vault_package_id: Option<String>,
    pub sui_devnet_vault_package_id: Option<String>,

    // EVM RPC URLs
    pub base_sepolia_rpc_url: Option<String>,
    pub base_mainnet_rpc_url: Option<String>,
    pub xdc_mainnet_rpc_url: Option<String>,
    pub avalanche_fuji_rpc_url: Option<String>,
    pub avalanche_mainnet_rpc_url: Option<String>,

    // EVM USDC contract addresses
    pub base_sepolia_usdc_address: Option<String>,
    pub base_mainnet_usdc_address: Option<String>,
    pub xdc_mainnet_usdc_address: Option<String>,
    pub avalanche_fuji_usdc_address: Option<String>,
    pub avalanche_mainnet_usdc_address: Option<String>,

    // EVM PaymentVault contract addresses
    pub base_sepolia_vault_address: Option<String>,
    pub base_mainnet_vault_address: Option<String>,
    pub xdc_mainnet_vault_address: Option<String>,
    pub avalanche_fuji_vault_address: Option<String>,
    pub avalanche_mainnet_vault_address: Option<String>,

    // Signing (for settlement transactions)
    pub signer_type: Option<String>, // e.g., "private-key"
    pub private_key: Option<String>, // Private key for transaction signing

    // Transaction settings
    pub gas_budget: Option<u64>,    // Sui gas budget in MIST
    pub evm_gas_limit: Option<u64>, // EVM gas limit
    pub evm_gas_price: Option<u64>, // EVM gas price in wei
}

impl ServerConfig {
    pub fn apply_env(&self) {
        if let Some(v) = &self.host {
            unsafe {
                std::env::set_var("HOST", v);
            }
        }
        if let Some(v) = &self.port {
            unsafe {
                std::env::set_var("PORT", v.to_string());
            }
        }

        // Sui gRPC URLs
        if let Some(v) = &self.sui_mainnet_grpc_url {
            unsafe {
                std::env::set_var("SUI_MAINNET_GRPC_URL", v);
            }
        }
        if let Some(v) = &self.sui_testnet_grpc_url {
            unsafe {
                std::env::set_var("SUI_TESTNET_GRPC_URL", v);
            }
        }
        if let Some(v) = &self.sui_devnet_grpc_url {
            unsafe {
                std::env::set_var("SUI_DEVNET_GRPC_URL", v);
            }
        }

        // Sui package IDs
        if let Some(v) = &self.sui_mainnet_usdc_package_id {
            unsafe {
                std::env::set_var("SUI_MAINNET_USDC_PACKAGE_ID", v);
            }
        }
        if let Some(v) = &self.sui_testnet_usdc_package_id {
            unsafe {
                std::env::set_var("SUI_TESTNET_USDC_PACKAGE_ID", v);
            }
        }
        if let Some(v) = &self.sui_devnet_usdc_package_id {
            unsafe {
                std::env::set_var("SUI_DEVNET_USDC_PACKAGE_ID", v);
            }
        }
        if let Some(v) = &self.sui_mainnet_vault_package_id {
            unsafe {
                std::env::set_var("SUI_MAINNET_VAULT_PACKAGE_ID", v);
            }
        }
        if let Some(v) = &self.sui_testnet_vault_package_id {
            unsafe {
                std::env::set_var("SUI_TESTNET_VAULT_PACKAGE_ID", v);
            }
        }
        if let Some(v) = &self.sui_devnet_vault_package_id {
            unsafe {
                std::env::set_var("SUI_DEVNET_VAULT_PACKAGE_ID", v);
            }
        }

        // EVM RPC URLs
        if let Some(v) = &self.base_sepolia_rpc_url {
            unsafe {
                std::env::set_var("BASE_SEPOLIA_RPC_URL", v);
            }
        }
        if let Some(v) = &self.base_mainnet_rpc_url {
            unsafe {
                std::env::set_var("BASE_MAINNET_RPC_URL", v);
            }
        }
        if let Some(v) = &self.xdc_mainnet_rpc_url {
            unsafe {
                std::env::set_var("XDC_MAINNET_RPC_URL", v);
            }
        }
        if let Some(v) = &self.avalanche_fuji_rpc_url {
            unsafe {
                std::env::set_var("AVALANCHE_FUJI_RPC_URL", v);
            }
        }
        if let Some(v) = &self.avalanche_mainnet_rpc_url {
            unsafe {
                std::env::set_var("AVALANCHE_MAINNET_RPC_URL", v);
            }
        }

        // EVM USDC contract addresses
        if let Some(v) = &self.base_sepolia_usdc_address {
            unsafe {
                std::env::set_var("BASE_SEPOLIA_USDC_ADDRESS", v);
            }
        }
        if let Some(v) = &self.base_mainnet_usdc_address {
            unsafe {
                std::env::set_var("BASE_MAINNET_USDC_ADDRESS", v);
            }
        }
        if let Some(v) = &self.xdc_mainnet_usdc_address {
            unsafe {
                std::env::set_var("XDC_MAINNET_USDC_ADDRESS", v);
            }
        }
        if let Some(v) = &self.avalanche_fuji_usdc_address {
            unsafe {
                std::env::set_var("AVALANCHE_FUJI_USDC_ADDRESS", v);
            }
        }
        if let Some(v) = &self.avalanche_mainnet_usdc_address {
            unsafe {
                std::env::set_var("AVALANCHE_MAINNET_USDC_ADDRESS", v);
            }
        }

        // EVM PaymentVault contract addresses
        if let Some(v) = &self.base_sepolia_vault_address {
            unsafe {
                std::env::set_var("BASE_SEPOLIA_VAULT_ADDRESS", v);
            }
        }
        if let Some(v) = &self.base_mainnet_vault_address {
            unsafe {
                std::env::set_var("BASE_MAINNET_VAULT_ADDRESS", v);
            }
        }
        if let Some(v) = &self.xdc_mainnet_vault_address {
            unsafe {
                std::env::set_var("XDC_MAINNET_VAULT_ADDRESS", v);
            }
        }
        if let Some(v) = &self.avalanche_fuji_vault_address {
            unsafe {
                std::env::set_var("AVALANCHE_FUJI_VAULT_ADDRESS", v);
            }
        }
        if let Some(v) = &self.avalanche_mainnet_vault_address {
            unsafe {
                std::env::set_var("AVALANCHE_MAINNET_VAULT_ADDRESS", v);
            }
        }

        // Signing
        if let Some(v) = &self.signer_type {
            unsafe {
                std::env::set_var("SIGNER_TYPE", v);
            }
        }
        if let Some(v) = &self.private_key {
            unsafe {
                std::env::set_var("PRIVATE_KEY", v);
            }
        }

        // Read private keys from environment variables if not set in config
        if self.private_key.is_none()
            && let Ok(env_private_key) = std::env::var("SUI_PRIVATE_KEY")
        {
            unsafe {
                std::env::set_var("PRIVATE_KEY", env_private_key);
            }
        }

        // Set EVM private key from environment
        if let Ok(evm_private_key) = std::env::var("EVM_PRIVATE_KEY") {
            unsafe {
                std::env::set_var("EVM_PRIVATE_KEY", evm_private_key);
            }
        }

        // Transaction settings
        if let Some(v) = &self.gas_budget {
            unsafe {
                std::env::set_var("GAS_BUDGET", v.to_string());
            }
        }
        if let Some(v) = &self.evm_gas_limit {
            unsafe {
                std::env::set_var("EVM_GAS_LIMIT", v.to_string());
            }
        }
        if let Some(v) = &self.evm_gas_price {
            unsafe {
                std::env::set_var("EVM_GAS_PRICE", v.to_string());
            }
        }
    }
}

pub fn load_config(path: Option<&str>) -> eyre::Result<ServerConfig> {
    let mut builder = config::Config::builder();

    if let Some(path) = path {
        // Use the specified config file
        builder = builder.add_source(config::File::with_name(path));
    } else {
        // Try default config files in order of preference: TOML first, then YAML
        if std::path::Path::new("config.toml").exists() {
            builder = builder.add_source(config::File::with_name("config.toml"));
        } else if std::path::Path::new("config.yaml").exists() {
            builder = builder.add_source(config::File::with_name("config.yaml"));
        }
    }

    builder =
        builder.add_source(config::Environment::with_prefix("LEDGERFLOW_FAC").separator("__"));

    // Build the configuration; if it fails (e.g., malformed file), fall back to an empty config
    // and log a warning instead of panicking.
    let cfg = match builder.build() {
        Ok(cfg) => cfg,
        Err(err) => {
            tracing::warn!(error = %err, "Failed to build configuration, using defaults");
            // An empty builder should always succeed; if it doesn't, propagate the error.
            config::Config::builder().build()?
        }
    };

    // Deserialize into our struct; on failure, prefer a safe default with a warning.
    let server_cfg = match cfg.try_deserialize::<ServerConfig>() {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(error = %err, "Failed to deserialize configuration, using defaults");
            ServerConfig::default()
        }
    };

    Ok(server_cfg)
}
