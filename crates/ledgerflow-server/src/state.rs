//! Application state shared by handlers.

use std::sync::Arc;

use ledgerflow_core::{RevocationCheck, SignerRef, SigningKeyPair, TrustedIssuers};
use ledgerflow_facilitator::{
    DefaultSubjectResolver, EvmRailAdapter, FileRevocationStore, SettlementRegistry,
    SettlementService, SharedRailAdapter, SolanaRailAdapter, VerificationService,
};

/// Application state.
#[derive(Clone)]
pub struct AppState {
    pub config: crate::config::ServerConfig,
    pub saas: crate::saas::SaasAuthExtractor,
    pub verification: VerificationService<FileRevocationStore>,
    pub settlement:
        SettlementService<FileRevocationStore, DefaultSubjectResolver, SharedRailAdapter>,
    pub registry: SettlementRegistry,
    pub trusted: TrustedIssuers,
    /// The issuer signing key used to issue warrants. Never a predictable demo
    /// key: it is loaded from configuration (fail-fast when absent).
    pub issuer_key: SigningKeyPair,
    /// The revocation store, exposed for tenant-scoped admin operations
    /// (design §10.2).
    pub revocation_store: FileRevocationStore,
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
            vec![
                Arc::new(EvmRailAdapter) as SharedRailAdapter,
                Arc::new(SolanaRailAdapter) as SharedRailAdapter,
            ],
        );
        // The issuer key is mandatory; `NewAppState::demo` supplies a test key,
        // but production construction must provide a real key via config.
        let issuer_key = load_issuer_key(&config)?;
        let webhook = match &config.webhook_url {
            Some(url) => crate::webhook::WebhookSender::with_delivery(url.clone()),
            None => crate::webhook::WebhookSender::disabled(),
        };
        Ok(Self {
            saas: crate::saas::SaasAuthExtractor {
                mode: config.saas.mode,
                service_token: config.saas.service_token.clone(),
                standalone_tenant: config.saas.tenant_id.clone(),
            },
            verification: VerificationService::new(revocation.clone()),
            settlement,
            registry: SettlementRegistry::new(),
            trusted,
            issuer_key,
            revocation_store: revocation,
            webhook,
            config,
        })
    }
}

/// Loads the issuer signing key from configuration.
///
/// Fails when `LEDGERFLOW_ISSUER_KEY` was not configured (the server must never
/// default to a predictable demo key — design §6.8).
fn load_issuer_key(
    config: &crate::config::ServerConfig,
) -> Result<SigningKeyPair, ServerStateError> {
    let hex = config
        .issuer_key_hex
        .as_deref()
        .ok_or_else(|| ServerStateError::Issuer("issuer key not configured".to_string()))?;
    let bytes = decode_hex::<32>(hex)
        .ok_or_else(|| ServerStateError::Issuer("issuer key must be 32-byte hex".to_string()))?;
    Ok(SigningKeyPair::from_bytes(&bytes))
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
///
/// Uses an explicit demo issuer key (hex of `[1u8; 32]`). This is **test-only**;
/// production construction loads a real key from configuration and fails fast
/// when absent (design §6.8).
pub struct NewAppState;

impl NewAppState {
    /// Builds a state with a demo issuer key pair and a temp revocation store.
    pub fn demo() -> Result<AppState, ServerStateError> {
        let config = crate::config::ServerConfig {
            bind_addr: "127.0.0.1:8080".to_string(),
            saas: crate::config::SaasConfig {
                mode: crate::config::SaasMode::Standalone,
                service_token: None,
                tenant_id: "default".to_string(),
            },
            // Demo issuer key (hex of 32 `0x01` bytes). Test-only.
            issuer_key_hex: Some(hex_encode(&[1_u8; 32])),
            webhook_url: None,
        };
        let issuer = SigningKeyPair::from_bytes(&[1_u8; 32]);
        let mut trusted = TrustedIssuers::new();
        trusted
            .add(ledgerflow_core::TrustedIssuer::new("issuer-1".to_string(), issuer.signer_ref()));
        // Use a process-unique directory so parallel tests do not share a
        // revocation file.
        let dir =
            std::env::temp_dir().join(format!("ledgerflow-server-demo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|error| {
            ServerStateError::Issuer(format!("cannot create demo dir: {error}"))
        })?;
        AppState::new(config, &dir.join("revocations.jsonl"), trusted)
    }
}

/// Decodes a hex string into exactly `N` bytes.
fn decode_hex<const N: usize>(hex: &str) -> Option<[u8; N]> {
    let hex = hex.trim();
    if hex.len() != N * 2 {
        return None;
    }
    let mut out = [0_u8; N];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(out)
}

/// Encodes bytes as a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0F) as usize] as char);
    }
    encoded
}

// Keep the trait import used by the state's bounds referenced so consumers
// can rely on it without an explicit import.
#[allow(dead_code)]
pub(crate) fn _trait_bounds_used(_check: &dyn RevocationCheck, _signer: &SignerRef) {}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use ledgerflow_core::{
        AssetRef, AuthorizationContext, MerchantConstraint, PaymentConstraint, PaymentRail,
        PaymentSubjectKind, PaymentSubjectRef, PopProof, ProofBuilder, ResourceConstraint,
        WarrantBuilder, WarrantChain, sha256_prefixed,
    };

    use super::*;

    fn root_warrant(now_ms: u64) -> ledgerflow_core::Warrant {
        let issuer = SigningKeyPair::from_bytes(&[1_u8; 32]);
        let holder = SigningKeyPair::from_bytes(&[2_u8; 32]);
        WarrantBuilder::new(now_ms)
            .warrant_id(*b"solana-state-tst")
            .ttl_secs(60)
            .max_depth(1)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
            .resource(ResourceConstraint {
                http_methods: vec!["POST".to_string()],
                path_prefixes: vec!["/pay".to_string()],
            })
            .payment(
                PaymentConstraint::new(1_000)
                    .with_asset(AssetRef::new("USDC", Some("solana".to_string())))
                    .with_rails(vec![PaymentRail::Onchain])
                    .with_schemes(vec!["exact".to_string()])
                    .with_payees(vec!["merchant-a".to_string()]),
            )
            .sign_with(&issuer, [0_u8; 8])
    }

    fn solana_context(now_ms: u64) -> AuthorizationContext {
        AuthorizationContext {
            merchant_id: "merchant-a".to_string(),
            merchant_host: "merchant-a.example".to_string(),
            tool_name: "web-search".to_string(),
            model_provider: String::new(),
            action_label: String::new(),
            http_method: "POST".to_string(),
            path_and_query: "/pay".to_string(),
            selected_amount: 100,
            asset: "USDC".to_string(),
            asset_network: Some("solana".to_string()),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: sha256_prefixed("POST\nmerchant-a.example\n/pay\nsha256:body"),
            accepted_hash: sha256_prefixed("exact:USDC:100:merchant-a"),
            now_ms,
            freshness_window_ms: 60_000,
            clock_skew_ms: 30_000,
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:solana:mainnet:7vfCXTUXx5Wn4P6m7XJ3e1yK2bXxVmW7nYj1m5X9A1t3",
            ),
            presenter: SigningKeyPair::from_bytes(&[2_u8; 32]).signer_ref(),
        }
    }

    fn proof(warrant: &ledgerflow_core::Warrant, context: &AuthorizationContext) -> PopProof {
        ProofBuilder::new()
            .warrant_id(warrant.id.clone())
            .challenge_id(context.challenge_id.clone())
            .method(context.http_method.clone())
            .uri(format!("{}{}", context.merchant_host, context.path_and_query))
            .request_hash(context.request_hash.clone())
            .accepted_hash(context.accepted_hash.clone())
            .payment_payload_digest(sha256_prefixed("x402-payload"))
            .nonce("nonce-1".to_string())
            .created_at_ms(context.now_ms)
            .sign_with(&SigningKeyPair::from_bytes(&[2_u8; 32]))
    }

    #[test]
    fn app_state_routes_solana_subjects_through_settlement_runtime() {
        let state = NewAppState::demo().expect("demo state");
        let now_ms = 5_000;
        let warrant = root_warrant(now_ms);
        let chain = WarrantChain::single(warrant.clone());
        let context = solana_context(now_ms);
        let proof = proof(&warrant, &context);
        let tool_arguments = std::collections::BTreeMap::new();
        let outcome = state.verification.verify(&ledgerflow_facilitator::VerifyRequest {
            chain: &chain,
            trusted: &state.trusted,
            proof: &proof,
            context: &context,
            approvals: &[],
            tool_arguments: &tool_arguments,
        });
        let authorization = outcome.authorization.expect("authorized");

        let settlement = state.settlement.settle(&ledgerflow_facilitator::SettleRequest {
            authorization: &authorization,
            chain: &chain,
            proof: &proof,
            context: &context,
            now_ms,
        });

        assert_eq!(settlement.status, ledgerflow_facilitator::SettlementStatus::Settled);
        let receipt = settlement.receipt.expect("receipt");
        assert_eq!(receipt.rail, ledgerflow_facilitator::RailKind::Solana);
        assert!(receipt.transaction_id.starts_with("solana-tx-"));
    }
}
