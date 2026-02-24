use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    adapters::{
        AdapterDescriptor, AdapterRegistry, EvmAdapter, EvmAdapterConfig, OffchainAdapter,
        OffchainAdapterConfig, OffchainBackendConfig, PaymentAdapter,
    },
    service::FacilitatorService,
};

fn default_enabled() -> bool {
    true
}

fn default_x402_version() -> u8 {
    2
}

fn default_scheme() -> String {
    "exact".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub host: Option<String>,
    pub port: Option<u16>,

    /// Maximum requests per second (global). `None` or `0` = unlimited.
    pub rate_limit_per_second: Option<u64>,

    #[serde(default)]
    pub adapters: Vec<AdapterConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AdapterConfig {
    Offchain(OffchainAdapterInput),
    Evm(EvmAdapterInput),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffchainAdapterInput {
    pub id: String,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default = "default_x402_version")]
    pub x402_version: u8,

    #[serde(default = "default_scheme")]
    pub scheme: String,

    #[serde(default)]
    pub networks: Vec<String>,

    #[serde(default)]
    pub signers: Vec<String>,

    pub backend: OffchainBackendConfig,
}

impl OffchainAdapterInput {
    fn into_runtime(self) -> Result<OffchainAdapterConfig, eyre::Error> {
        let network_patterns = if self.networks.is_empty() {
            vec!["offchain:*".parse()?]
        } else {
            self.networks
                .iter()
                .map(|pattern| pattern.parse())
                .collect::<Result<Vec<_>, _>>()?
        };

        let descriptor = AdapterDescriptor {
            id: self.id,
            x402_version: self.x402_version,
            scheme: self.scheme,
            networks: network_patterns,
        };

        Ok(OffchainAdapterConfig {
            descriptor,
            backend: self.backend,
            signers: self.signers,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmAdapterInput {
    pub id: String,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default = "default_x402_version")]
    pub x402_version: u8,

    #[serde(default = "default_scheme")]
    pub scheme: String,

    /// CAIP-2 network patterns (e.g. `"eip155:84532"`, `"eip155:*"`).
    #[serde(default)]
    pub networks: Vec<String>,

    /// JSON-RPC endpoint URL.
    pub rpc_url: String,

    /// Numeric chain ID (e.g. `84532`).
    pub chain_id: u64,

    /// Environment variable name holding the hex-encoded signer private key.
    /// Required for settlement; verify-only if absent.
    #[serde(default)]
    pub signer_key_env: Option<String>,

    /// Facilitator signer addresses reported in `/supported`.
    #[serde(default)]
    pub signers: Vec<String>,
}

impl EvmAdapterInput {
    fn into_runtime(self) -> Result<EvmAdapterConfig, eyre::Error> {
        let network_patterns = if self.networks.is_empty() {
            vec![format!("eip155:{}", self.chain_id).parse()?]
        } else {
            self.networks
                .iter()
                .map(|pattern| pattern.parse())
                .collect::<Result<Vec<_>, _>>()?
        };

        let descriptor = AdapterDescriptor {
            id: self.id,
            x402_version: self.x402_version,
            scheme: self.scheme,
            networks: network_patterns,
        };

        // Resolve signer key from environment variable if configured.
        let signer_key = self
            .signer_key_env
            .as_deref()
            .and_then(|env_name| std::env::var(env_name).ok());

        Ok(EvmAdapterConfig {
            descriptor,
            rpc_url: self.rpc_url,
            chain_id: self.chain_id,
            signer_key,
            signers: self.signers,
        })
    }
}

pub fn build_service(config: &ServerConfig) -> eyre::Result<FacilitatorService> {
    let mut adapters: Vec<Arc<dyn PaymentAdapter>> = Vec::new();

    for adapter in &config.adapters {
        match adapter {
            AdapterConfig::Offchain(offchain) if offchain.enabled => {
                let runtime = offchain.clone().into_runtime()?;
                info!(id = %runtime.descriptor.id, "registering offchain adapter");
                let adapter = OffchainAdapter::try_new(runtime)?;
                adapters.push(Arc::new(adapter));
            }
            AdapterConfig::Offchain(_) => {}
            AdapterConfig::Evm(evm) if evm.enabled => {
                let runtime = evm.clone().into_runtime()?;
                let has_signer = runtime.signer_key.is_some();
                info!(
                    id = %runtime.descriptor.id,
                    chain_id = %runtime.chain_id,
                    settle_enabled = has_signer,
                    "registering EVM adapter"
                );
                let adapter = EvmAdapter::try_new(runtime)?;
                adapters.push(Arc::new(adapter));
            }
            AdapterConfig::Evm(_) => {}
        }
    }

    if adapters.is_empty() {
        info!("no adapters configured, registering default mock offchain adapter");
        let default = OffchainAdapter::try_new(OffchainAdapterConfig {
            descriptor: AdapterDescriptor {
                id: "default-mock-offchain".to_string(),
                x402_version: 2,
                scheme: "exact".to_string(),
                networks: vec!["offchain:*".parse()?],
            },
            backend: OffchainBackendConfig::Mock {
                payer: "cex:user:default".to_string(),
                transaction_prefix: "offchain-tx".to_string(),
            },
            signers: vec!["offchain-facilitator".to_string()],
        })?;
        adapters.push(Arc::new(default));
    }

    info!(count = adapters.len(), "adapter registry ready");
    let registry = AdapterRegistry::new(adapters);
    Ok(FacilitatorService::new(registry))
}

pub fn load_config(path: Option<&str>) -> eyre::Result<ServerConfig> {
    let mut builder = config::Config::builder();

    if let Some(path) = path {
        builder = builder.add_source(config::File::with_name(path));
    } else if std::path::Path::new("config.toml").exists() {
        builder = builder.add_source(config::File::with_name("config.toml"));
    }

    builder =
        builder.add_source(config::Environment::with_prefix("LEDGERFLOW_FAC").separator("__"));

    let settings = builder.build()?;
    Ok(settings.try_deserialize::<ServerConfig>()?)
}
