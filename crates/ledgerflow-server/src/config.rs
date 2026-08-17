//! Server configuration with fail-fast semantics (design §5.3).
//!
//! - An absent `[saas]` section is an **explicit default** to standalone.
//! - A present-but-invalid `[saas]` section (bad mode, missing service
//!   token, missing tenant) is a **startup error** — never a silent
//!   downgrade.

/// SaaS deployment mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaasMode {
    /// Single-tenant, no gateway (default).
    Standalone,
    /// Multi-tenant behind a gateway that injects internal headers.
    Saas,
}

/// The `[saas]` configuration section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaasConfig {
    pub mode: SaasMode,
    /// Shared service token shared with the gateway (required in `saas` mode).
    pub service_token: Option<String>,
    /// Tenant id used in standalone mode (defaults to `default`).
    pub tenant_id: String,
}

/// Full server configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub saas: SaasConfig,
}

impl ServerConfig {
    /// Loads configuration from environment variables with fail-fast
    /// validation.
    ///
    /// Recognized variables:
    /// - `LEDGERFLOW_BIND` (default `127.0.0.1:8080`)
    /// - `LEDGERFLOW_SAAS_MODE` (`standalone` | `saas`; absent = standalone)
    /// - `LEDGERFLOW_SERVICE_TOKEN` (required when mode is `saas`)
    /// - `LEDGERFLOW_TENANT_ID` (default `default`)
    ///
    /// Invalid `saas` mode or a missing service token in `saas` mode is a
    /// hard error (fail-fast).
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr = std::env::var("LEDGERFLOW_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        let mode_raw = std::env::var("LEDGERFLOW_SAAS_MODE").ok();
        let mode = match mode_raw.as_deref() {
            None | Some("standalone") => SaasMode::Standalone,
            Some("saas") => SaasMode::Saas,
            Some(other) => {
                return Err(ConfigError::InvalidMode {
                    mode: other.to_string(),
                    valid: "standalone|saas".to_string(),
                });
            }
        };
        let service_token = std::env::var("LEDGERFLOW_SERVICE_TOKEN").ok();
        if mode == SaasMode::Saas && service_token.as_deref().is_none_or(|s| s.is_empty()) {
            return Err(ConfigError::MissingServiceToken);
        }
        let tenant_id =
            std::env::var("LEDGERFLOW_TENANT_ID").unwrap_or_else(|_| "default".to_string());
        Ok(Self {
            bind_addr,
            saas: SaasConfig { mode, service_token, tenant_id },
        })
    }
}

/// Configuration failures (all are startup-fatal).
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid LEDGERFLOW_SAAS_MODE `{mode}` (valid: {valid})")]
    InvalidMode { mode: String, valid: String },
    #[error("LEDGERFLOW_SERVICE_TOKEN is required when LEDGERFLOW_SAAS_MODE=saas")]
    MissingServiceToken,
}
