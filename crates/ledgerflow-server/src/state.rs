//! Application state shared by handlers.

use ledgerflow_core::{
    RevocationCheck, SignerRef, SigningKeyPair, TrustedIssuers,
};
use ledgerflow_facilitator::{
    DefaultSubjectResolver, EvmRailAdapter, FileRevocationStore, SettlementRegistry,
    SettlementService, VerificationService,
};

/// Application state.
#[derive(Clone)]
pub struct AppState {
    pub config: crate::config::ServerConfig,
    pub saas: crate::saas::SaasAuthExtractor,
    pub verification: VerificationService<FileRevocationStore>,
    pub settlement: SettlementService<FileRevocationStore, DefaultSubjectResolver, EvmRailAdapter>,
    pub registry: SettlementRegistry,
    pub trusted: TrustedIssuers,
    pub webhook: crate::webhook::WebhookSender,
}

impl AppState {
    /// Creates a new application state (used by the binary and tests).
    pub fn new(
        config: crate::config::ServerConfig,
        revocation_path: &std::path::Path,
        trusted: TrustedIssuers,
    ) -> Result<Self, ServerStateError> {
        let revocation = FileRevocationStore::open(revocation_path)?;
        let settlement = SettlementService::new(
            revocation.clone(),
            DefaultSubjectResolver,
            vec![EvmRailAdapter],
        );
        Ok(Self {
            saas: crate::saas::SaasAuthExtractor {
                mode: config.saas.mode,
                service_token: config.saas.service_token.clone(),
                standalone_tenant: config.saas.tenant_id.clone(),
            },
            verification: VerificationService::new(revocation),
            settlement,
            registry: SettlementRegistry::new(),
            trusted,
            webhook: crate::webhook::WebhookSender::disabled(),
            config,
        })
    }
}

/// State construction failures.
#[derive(Debug, thiserror::Error)]
pub enum ServerStateError {
    #[error("failed to open the revocation store: {0}")]
    Revocation(#[from] ledgerflow_facilitator::RevocationStoreError),
    #[error("invalid issuer configuration: {0}")]
    Issuer(String),
}

/// Demo state builder used by tests and the CLI.
pub struct NewAppState;

impl NewAppState {
    /// Builds a state with a demo issuer key pair and a temp revocation store.
    pub fn demo() -> Result<AppState, ServerStateError> {
        let config = crate::config::ServerConfig::from_env()
            .map_err(|error| ServerStateError::Issuer(error.to_string()))?;
        let issuer = SigningKeyPair::from_bytes(&[1_u8; 32]);
        let mut trusted = TrustedIssuers::new();
        trusted.add(ledgerflow_core::TrustedIssuer::new(
            "issuer-1".to_string(),
            issuer.signer_ref(),
        ));
        // Use a process-unique directory so parallel tests do not share a
        // revocation file.
        let dir = std::env::temp_dir().join(format!(
            "ledgerflow-server-demo-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir)
            .map_err(|error| ServerStateError::Issuer(format!("cannot create demo dir: {error}")))?;
        AppState::new(config, &dir.join("revocations.jsonl"), trusted)
    }
}

// Keep the trait import used by the state's bounds referenced so consumers
// can rely on it without an explicit import.
#[allow(dead_code)]
pub(crate) fn _trait_bounds_used(_check: &dyn RevocationCheck, _signer: &SignerRef) {}
