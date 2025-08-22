use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    // RPC URLs supported by x402-rs provider cache
    pub rpc_url_base: Option<String>,
    pub rpc_url_base_sepolia: Option<String>,
    pub rpc_url_avalanche_fuji: Option<String>,
    pub rpc_url_avalanche: Option<String>,
    pub rpc_url_xdc: Option<String>,

    // signing
    pub signer_type: Option<String>, // e.g., "private-key"
    pub private_key: Option<String>, // 0x...
}

impl ServerConfig {
    pub fn apply_env(&self) {
        if let Some(v) = &self.host {
            std::env::set_var("HOST", v);
        }
        if let Some(v) = &self.port {
            std::env::set_var("PORT", v.to_string());
        }
        if let Some(v) = &self.rpc_url_base {
            std::env::set_var("RPC_URL_BASE", v);
            // Also set commonly used alias expected by some tooling
            std::env::set_var("BASE_MAINNET_RPC_URL", v);
        }
        if let Some(v) = &self.rpc_url_base_sepolia {
            std::env::set_var("RPC_URL_BASE_SEPOLIA", v);
            std::env::set_var("BASE_SEPOLIA_RPC_URL", v);
        }
        if let Some(v) = &self.rpc_url_avalanche_fuji {
            std::env::set_var("RPC_URL_AVALANCHE_FUJI", v);
            std::env::set_var("AVALANCHE_FUJI_RPC_URL", v);
        }
        if let Some(v) = &self.rpc_url_avalanche {
            std::env::set_var("RPC_URL_AVALANCHE", v);
            std::env::set_var("AVALANCHE_MAINNET_RPC_URL", v);
        }
        if let Some(v) = &self.rpc_url_xdc {
            std::env::set_var("RPC_URL_XDC", v);
            std::env::set_var("XDC_MAINNET_RPC_URL", v);
        }
        if let Some(v) = &self.signer_type {
            std::env::set_var("SIGNER_TYPE", v);
        }
        if let Some(v) = &self.private_key {
            std::env::set_var("PRIVATE_KEY", v);
        }
    }
}

pub fn load_config(path: Option<&str>) -> eyre::Result<ServerConfig> {
    let mut builder = config::Config::builder();
    if let Some(path) = path {
        builder = builder.add_source(config::File::with_name(path));
    } else {
        // try default config.yaml in crate dir if present
        if std::path::Path::new("config.yaml").exists() {
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
