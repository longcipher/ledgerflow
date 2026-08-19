//! Server configuration with fail-fast semantics (design §5.3).
//!
//! - An absent `[saas]` section is an **explicit default** to standalone.
//! - A present-but-invalid `[saas]` section (bad mode, missing service token, missing tenant) is a
//!   **startup error** — never a silent downgrade.

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
    /// Hex-encoded Ed25519 issuer signing key used to issue warrants.
    ///
    /// **Required.** A real, secret key must be supplied (e.g.
    /// `LEDGERFLOW_ISSUER_KEY`). Absence is a startup failure: the server must
    /// never fall back to a predictable demo key in production (design §6.8).
    pub issuer_key_hex: Option<String>,
    /// Optional webhook delivery URL (design §10.3). When set, events are
    /// delivered to this endpoint with bounded retry.
    pub webhook_url: Option<String>,
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
    /// - `LEDGERFLOW_ISSUER_KEY` (hex Ed25519 key; required to issue warrants)
    ///
    /// Invalid `saas` mode or a missing service token in `saas` mode is a
    /// hard error (fail-fast). A missing issuer key is also a hard error: the
    /// server must never default to a predictable demo key.
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind_addr =
            std::env::var("LEDGERFLOW_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
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
        let issuer_key_hex = std::env::var("LEDGERFLOW_ISSUER_KEY").ok();
        if issuer_key_hex.as_deref().is_none_or(|s| s.is_empty()) {
            return Err(ConfigError::MissingIssuerKey);
        }
        let webhook_url = std::env::var("LEDGERFLOW_WEBHOOK_URL").ok();
        Ok(Self {
            bind_addr,
            saas: SaasConfig { mode, service_token, tenant_id },
            issuer_key_hex,
            webhook_url,
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
    #[error("LEDGERFLOW_ISSUER_KEY is required to issue warrants (never defaults to a demo key)")]
    MissingIssuerKey,
}
