//! Merchant-side verification that composes x402 payloads with LedgerFlow authz.

use std::collections::BTreeMap;

use ledgerflow_core::{
    AuthorizationContext, AuthorizationError, DEFAULT_PROOF_FRESHNESS_MS, VerifiedAuthorization,
    Warrant, verify_authorization,
};
use thiserror::Error;

use crate::{
    extension::{
        HttpRequest, LedgerFlowChallenge, PaymentPayload, WarrantTransport,
        canonical_accepted_hash, canonical_request_hash,
    },
    replay::{ReplayConflict, ReplayFingerprint, ReplayStore},
};

/// Repository seam for cached warrants keyed by digest.
pub trait WarrantRepository {
    fn load(&self, digest: &str) -> Option<Warrant>;
    fn store(&mut self, warrant: Warrant);
}

/// In-memory warrant repository for tests and local development flows.
#[derive(Clone, Debug, Default)]
pub struct InMemoryWarrantRepository {
    warrants: BTreeMap<String, Warrant>,
}

impl WarrantRepository for InMemoryWarrantRepository {
    fn load(&self, digest: &str) -> Option<Warrant> {
        self.warrants.get(digest).cloned()
    }

    fn store(&mut self, warrant: Warrant) {
        self.warrants.insert(warrant.digest(), warrant);
    }
}

/// Result of merchant verification, including whether settlement work was reused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MerchantVerificationOutcome {
    pub authorization: VerifiedAuthorization,
    pub settlement_reused: bool,
}

/// Verification failures surfaced by the x402 integration layer.
#[derive(Debug, Error)]
pub enum MerchantVerificationError {
    #[error("the payment payload did not include a LedgerFlow extension")]
    MissingLedgerFlowExtension,
    #[error("the payload did not echo the active challenge id")]
    ChallengeMismatch,
    #[error("the payload signer did not match the proof signer")]
    ExtensionSignerMismatch,
    #[error("the warrant digest did not match the inline warrant")]
    WarrantTransportMismatch,
    #[error("the warrant digest `{digest}` was not present in merchant cache")]
    UnknownWarrantDigest { digest: String },
    #[error("the proof replay key was already used for a different request")]
    ReplayDetected,
    #[error(transparent)]
    Core(#[from] AuthorizationError),
}

/// Merchant-side verifier that preserves x402 semantics while adding LedgerFlow checks.
#[derive(Clone, Debug)]
pub struct MerchantVerifier<R, W> {
    replay_store: R,
    warrant_repository: W,
}

impl<R, W> MerchantVerifier<R, W> {
    #[must_use]
    pub const fn new(replay_store: R, warrant_repository: W) -> Self {
        Self { replay_store, warrant_repository }
    }

    pub const fn replay_store_mut(&mut self) -> &mut R {
        &mut self.replay_store
    }

    pub const fn warrant_repository_mut(&mut self) -> &mut W {
        &mut self.warrant_repository
    }
}

impl<R, W> MerchantVerifier<R, W>
where
    R: ReplayStore,
    W: WarrantRepository,
{
    pub fn verify_payment(
        &mut self,
        challenge: &LedgerFlowChallenge,
        request: &HttpRequest,
        payload: &PaymentPayload,
        tool_name: &str,
        now_ms: u64,
        presented_delegation_depth: u8,
    ) -> Result<MerchantVerificationOutcome, MerchantVerificationError> {
        let Some(extension) = &payload.ledgerflow else {
            return Err(MerchantVerificationError::MissingLedgerFlowExtension);
        };

        if extension.challenge_id != challenge.challenge_id {
            return Err(MerchantVerificationError::ChallengeMismatch);
        }

        if extension.signer.public_key != extension.proof.signer_key {
            return Err(MerchantVerificationError::ExtensionSignerMismatch);
        }

        if let Some(payment_identifier) = payload.payment_identifier() &&
            let Some(authorization) = self.replay_store.cached_payment(payment_identifier)
        {
            return Ok(MerchantVerificationOutcome { authorization, settlement_reused: true });
        }

        let accepted_hash = canonical_accepted_hash(&payload.accepted);
        let request_hash = canonical_request_hash(request);

        self.claim_replay(challenge, extension, &request_hash, &accepted_hash, now_ms)?;

        let warrant = self.resolve_warrant(&extension.warrant)?;
        let proof_freshness_ms = if challenge.proof_freshness_ms == 0 {
            DEFAULT_PROOF_FRESHNESS_MS
        } else {
            challenge.proof_freshness_ms
        };
        let context = AuthorizationContext {
            merchant_id: challenge.merchant_id.clone(),
            merchant_host: request.authority.clone(),
            tool_name: tool_name.to_string(),
            model_provider: String::new(),
            action_label: String::new(),
            http_method: request.method.clone(),
            path_and_query: request.path_and_query.clone(),
            selected_quote_amount: payload.accepted.amount,
            asset: payload.accepted.asset.clone(),
            scheme: payload.accepted.scheme.clone(),
            payee_id: payload.accepted.payee_id.clone(),
            rail: match extension.payment_subject.kind {
                ledgerflow_core::PaymentSubjectKind::ExchangeAccount => {
                    ledgerflow_core::PaymentRail::Exchange
                }
                ledgerflow_core::PaymentSubjectKind::FacilitatorAccount => {
                    ledgerflow_core::PaymentRail::Exchange
                }
                _ => ledgerflow_core::PaymentRail::Onchain,
            },
            challenge_id: challenge.challenge_id.clone(),
            request_hash,
            accepted_hash,
            now_ms,
            freshness_window_ms: proof_freshness_ms,
            presented_delegation_depth,
            payment_subject: extension.payment_subject.clone(),
        };
        let authorization = verify_authorization(&warrant, &extension.proof, &context)?;

        if let Some(payment_identifier) = payload.payment_identifier() {
            self.replay_store.cache_payment(payment_identifier.to_string(), authorization.clone());
        }

        Ok(MerchantVerificationOutcome { authorization, settlement_reused: false })
    }

    fn claim_replay(
        &mut self,
        challenge: &LedgerFlowChallenge,
        extension: &crate::extension::LedgerFlowAuthorizationExtension,
        request_hash: &str,
        accepted_hash: &str,
        now_ms: u64,
    ) -> Result<(), MerchantVerificationError> {
        let fingerprint = ReplayFingerprint {
            challenge_id: challenge.challenge_id.clone(),
            nonce: extension.proof.nonce.clone(),
            request_hash: request_hash.to_string(),
            accepted_hash: accepted_hash.to_string(),
        };

        match self.replay_store.claim_nonce(fingerprint, now_ms) {
            Ok(()) => Ok(()),
            Err(ReplayConflict { existing })
                if existing.request_hash == request_hash &&
                    existing.accepted_hash == accepted_hash =>
            {
                Err(MerchantVerificationError::ReplayDetected)
            }
            Err(ReplayConflict { .. }) => Err(MerchantVerificationError::ReplayDetected),
        }
    }

    fn resolve_warrant(
        &mut self,
        transport: &WarrantTransport,
    ) -> Result<Warrant, MerchantVerificationError> {
        match &transport.inline {
            Some(inline) => {
                if transport.digest != inline.digest() {
                    return Err(MerchantVerificationError::WarrantTransportMismatch);
                }
                self.warrant_repository.store(inline.clone());
                Ok(inline.clone())
            }
            None => self.warrant_repository.load(&transport.digest).ok_or_else(|| {
                MerchantVerificationError::UnknownWarrantDigest { digest: transport.digest.clone() }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use ledgerflow_core::{
        AmountLimit, AssetRef, AudienceScope, Constraint, DelegationPolicy, MerchantConstraint,
        PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef, ResourceConstraint,
        SigningKeyPair, SponsorshipConstraint, ToolConstraint, WARRANT_VERSION_V1, Warrant,
        WarrantMetadata,
    };

    fn issuer_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[1u8; 32])
    }

    fn agent_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[2u8; 32])
    }

    use crate::{
        extension::{
            AcceptedQuote, HttpRequest, LedgerFlowChallenge, PaymentPayloadSeed, WarrantTransport,
            build_payment_payload, merchant_payment_required,
        },
        middleware::{InMemoryWarrantRepository, MerchantVerificationError, MerchantVerifier},
        replay::InMemoryReplayStore,
    };

    fn sample_warrant(payment_subject: PaymentSubjectRef, rail: PaymentRail) -> Warrant {
        let issuer = issuer_keys();
        let agent = agent_keys();
        Warrant {
            version: WARRANT_VERSION_V1,
            warrant_id: "warrant-1".to_string(),
            issuer: issuer.signer_ref(),
            subject_signer: agent.signer_ref(),
            payment_subjects: vec![payment_subject],
            audience: AudienceScope::MerchantIds(vec!["merchant-a".to_string()]),
            not_before_ms: 1_000,
            expires_at_ms: 10_000,
            delegation: DelegationPolicy { can_delegate: true, max_depth: 1 },
            constraints: vec![
                Constraint::Merchant(MerchantConstraint {
                    merchant_ids: vec!["merchant-a".to_string()],
                    host_suffixes: vec![],
                }),
                Constraint::Resource(ResourceConstraint {
                    http_methods: vec!["POST".to_string()],
                    path_prefixes: vec!["/pay".to_string()],
                }),
                Constraint::Tool(ToolConstraint {
                    tool_names: vec!["web-search".to_string()],
                    model_providers: vec![],
                    action_labels: vec![],
                }),
                Constraint::Payment(PaymentConstraint {
                    max_per_request: AmountLimit { amount: 200 },
                    period_limit: None,
                    allowed_assets: vec![AssetRef::new("USDC", Some("base".to_string()))],
                    allowed_rails: vec![rail],
                    allowed_schemes: vec!["exact".to_string()],
                    payee_ids: vec!["merchant-a".to_string()],
                }),
                Constraint::Sponsorship(SponsorshipConstraint {
                    allow_sponsored_execution: false,
                    sponsor_ids: vec![],
                }),
            ],
            metadata: WarrantMetadata::default(),
            signature: issuer.sign(b"placeholder"),
        }
        .sign_with(&issuer)
    }

    fn challenge(accepted: AcceptedQuote) -> LedgerFlowChallenge {
        merchant_payment_required("challenge-1", "merchant-a", "/pay", vec![accepted], 60_000)
            .ledgerflow
            .expect("challenge")
    }

    #[test]
    fn accepts_a_valid_inline_warrant() {
        let accepted = AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()));
        let request =
            HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec());
        let subject =
            PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123");
        let challenge = challenge(accepted.clone());
        let payload = build_payment_payload(
            &challenge,
            &request,
            accepted,
            WarrantTransport::inline(sample_warrant(subject.clone(), PaymentRail::Onchain)),
            PaymentPayloadSeed {
                payment_subject: subject,
                signer: agent_keys(),
                created_at_ms: 2_000,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-1".to_string()),
            },
        );
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
        );

        let outcome = verifier
            .verify_payment(&challenge, &request, &payload, "web-search", 2_000, 1)
            .expect("valid");

        assert_eq!(outcome.authorization.merchant_id, "merchant-a");
        assert!(!outcome.settlement_reused);
    }

    #[test]
    fn rejects_replay_when_the_same_nonce_is_reused_for_a_different_request() {
        let accepted = AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()));
        let challenge = challenge(accepted.clone());
        let subject =
            PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123");
        let warrant = sample_warrant(subject.clone(), PaymentRail::Onchain);
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
        );

        let first_request = HttpRequest::new("POST", "merchant-a.example", "/pay", b"one".to_vec());
        let first_payload = build_payment_payload(
            &challenge,
            &first_request,
            accepted.clone(),
            WarrantTransport::inline(warrant.clone()),
            PaymentPayloadSeed {
                payment_subject: subject.clone(),
                signer: agent_keys(),
                created_at_ms: 2_000,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-1".to_string()),
            },
        );
        verifier
            .verify_payment(&challenge, &first_request, &first_payload, "web-search", 2_000, 1)
            .expect("first request");

        let second_request =
            HttpRequest::new("POST", "merchant-a.example", "/pay", b"two".to_vec());
        let second_payload = build_payment_payload(
            &challenge,
            &second_request,
            accepted,
            WarrantTransport::inline(warrant),
            PaymentPayloadSeed {
                payment_subject: subject,
                signer: agent_keys(),
                created_at_ms: 2_100,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-2".to_string()),
            },
        );

        let error = verifier
            .verify_payment(&challenge, &second_request, &second_payload, "web-search", 2_100, 1)
            .expect_err("replay");

        assert!(matches!(error, MerchantVerificationError::ReplayDetected));
    }

    #[test]
    fn returns_the_cached_result_for_the_same_payment_identifier() {
        let accepted = AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()));
        let request =
            HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec());
        let subject =
            PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123");
        let challenge = challenge(accepted.clone());
        let warrant = sample_warrant(subject.clone(), PaymentRail::Onchain);
        let payload = build_payment_payload(
            &challenge,
            &request,
            accepted,
            WarrantTransport::inline(warrant),
            PaymentPayloadSeed {
                payment_subject: subject,
                signer: agent_keys(),
                created_at_ms: 2_000,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-1".to_string()),
            },
        );
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
        );

        verifier
            .verify_payment(&challenge, &request, &payload, "web-search", 2_000, 1)
            .expect("first");
        let second = verifier
            .verify_payment(&challenge, &request, &payload, "web-search", 2_500, 1)
            .expect("cached");

        assert!(second.settlement_reused);
    }

    #[test]
    fn loads_a_cached_warrant_by_digest_after_inline_submission() {
        let accepted = AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()));
        let request =
            HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec());
        let subject =
            PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123");
        let challenge = challenge(accepted.clone());
        let warrant = sample_warrant(subject.clone(), PaymentRail::Onchain);
        let warrant_digest = warrant.digest();
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
        );
        let inline_payload = build_payment_payload(
            &challenge,
            &request,
            accepted.clone(),
            WarrantTransport::inline(warrant),
            PaymentPayloadSeed {
                payment_subject: subject.clone(),
                signer: agent_keys(),
                created_at_ms: 2_000,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-1".to_string()),
            },
        );
        verifier
            .verify_payment(&challenge, &request, &inline_payload, "web-search", 2_000, 1)
            .expect("inline");

        let digest_payload = build_payment_payload(
            &challenge,
            &request,
            accepted,
            WarrantTransport::digest_ref(warrant_digest),
            PaymentPayloadSeed {
                payment_subject: subject,
                signer: agent_keys(),
                created_at_ms: 2_100,
                nonce: "nonce-2".to_string(),
                payment_identifier: Some("payment-2".to_string()),
            },
        );
        let outcome = verifier
            .verify_payment(&challenge, &request, &digest_payload, "web-search", 2_100, 1)
            .expect("digest");

        assert_eq!(outcome.authorization.tool_name, "web-search");
    }

    #[test]
    fn rejects_an_unknown_warrant_digest_reference() {
        let accepted = AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()));
        let request =
            HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec());
        let subject =
            PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123");
        let challenge = challenge(accepted.clone());
        let payload = build_payment_payload(
            &challenge,
            &request,
            accepted,
            WarrantTransport::digest_ref("sha256:missing"),
            PaymentPayloadSeed {
                payment_subject: subject,
                signer: agent_keys(),
                created_at_ms: 2_000,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-1".to_string()),
            },
        );
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
        );

        let error = verifier
            .verify_payment(&challenge, &request, &payload, "web-search", 2_000, 1)
            .expect_err("missing digest");

        assert!(matches!(error, MerchantVerificationError::UnknownWarrantDigest { .. }));
    }
}
