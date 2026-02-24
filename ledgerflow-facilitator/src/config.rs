use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    adapters::{
        AdapterDescriptor, AdapterRegistry, OffchainAdapter, OffchainAdapterConfig,
        OffchainBackendConfig, PaymentAdapter,
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

    #[serde(default)]
    pub adapters: Vec<AdapterConfig>,

    #[serde(default)]
    pub offchain_adapters: Vec<OffchainAdapterInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AdapterConfig {
    Offchain(OffchainAdapterInput),
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

pub fn build_service(config: &ServerConfig) -> eyre::Result<FacilitatorService> {
    let mut adapters: Vec<Arc<dyn PaymentAdapter>> = Vec::new();

    for adapter in &config.adapters {
        match adapter {
            AdapterConfig::Offchain(offchain) if offchain.enabled => {
                let runtime = offchain.clone().into_runtime()?;
                let adapter = OffchainAdapter::try_new(runtime)?;
                adapters.push(Arc::new(adapter));
            }
            AdapterConfig::Offchain(_) => {}
        }
    }

    for offchain in &config.offchain_adapters {
        if !offchain.enabled {
            continue;
        }
        let runtime = offchain.clone().into_runtime()?;
        let adapter = OffchainAdapter::try_new(runtime)?;
        adapters.push(Arc::new(adapter));
    }

    if adapters.is_empty() {
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
