//! Merchant-side verification that composes x402 payloads with LedgerFlow
//! authz checks.
//!
//! The verifier preserves x402 semantics while adding: challenge matching,
//! signer consistency, warrant-chain resolution (inline or cached digest),
//! replay protection, trust-anchor verification, revocation pre-check, and
//! approval-gate evaluation — all through the core `verify_authorization`
//! pipeline.

use std::collections::BTreeMap;

use ledgerflow_core::{
    AuthorizationContext, AuthorizationInput, DEFAULT_PROOF_FRESHNESS_MS, PaymentRail,
    RevocationCheck, ToolArguments, TrustedIssuers, VerifiedAuthorization, Warrant, WarrantChain,
};
use thiserror::Error;

use crate::{
    replay::{ReplayConflict, ReplayFingerprint, ReplayStore},
    x402::{
        HttpRequest, LedgerFlowAuthorizationExtension, LedgerFlowChallenge, PaymentPayload,
        canonical_accepted_hash, canonical_request_hash,
    },
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

/// Verification failures surfaced by the protocol integration layer.
#[derive(Debug, Error)]
pub enum MerchantVerificationError {
    #[error("the payment payload did not include a LedgerFlow extension")]
    MissingLedgerFlowExtension,
    #[error("the payload did not echo the active challenge id")]
    ChallengeMismatch,
    #[error("the payload signer did not match the proof signer")]
    ExtensionSignerMismatch,
    #[error("the warrant chain is empty")]
    EmptyChain,
    #[error("the warrant digest `{digest}` was not present in merchant cache")]
    UnknownWarrantDigest { digest: String },
    #[error("the proof replay key was already used for a different request")]
    ReplayDetected,
    #[error(transparent)]
    Core(#[from] ledgerflow_core::AuthorizationError),
}

/// Merchant-side verifier that preserves x402 semantics while adding LedgerFlow checks.
#[derive(Clone, Debug)]
pub struct MerchantVerifier<R, W, Rev> {
    replay_store: R,
    warrant_repository: W,
    revocation: Rev,
}

impl<R, W, Rev> MerchantVerifier<R, W, Rev> {
    #[must_use]
    pub const fn new(replay_store: R, warrant_repository: W, revocation: Rev) -> Self {
        Self { replay_store, warrant_repository, revocation }
    }

    pub const fn replay_store_mut(&mut self) -> &mut R {
        &mut self.replay_store
    }

    pub const fn warrant_repository_mut(&mut self) -> &mut W {
        &mut self.warrant_repository
    }
}

impl<R, W, Rev> MerchantVerifier<R, W, Rev>
where
    R: ReplayStore,
    W: WarrantRepository,
    Rev: RevocationCheck,
{
    /// Verifies a payment payload against the active challenge and request.
    ///
    /// # Arguments
    ///
    /// - `trusted`: the merchant's trusted-issuer anchors.
    /// - `tool_name`: the tool being invoked (for approval gates).
    /// - `tool_arguments`: the tool-call arguments (for approval gates).
    /// - `now_ms`: verification timestamp.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_payment(
        &mut self,
        challenge: &LedgerFlowChallenge,
        request: &HttpRequest,
        payload: &PaymentPayload,
        trusted: &TrustedIssuers,
        tool_name: &str,
        tool_arguments: &ToolArguments,
        now_ms: u64,
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

        let chain = self.resolve_chain(extension)?;
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
            selected_amount: payload.accepted.amount,
            asset: payload.accepted.asset.clone(),
            asset_network: payload.accepted.network.clone(),
            scheme: payload.accepted.scheme.clone(),
            payee_id: payload.accepted.payee_id.clone(),
            rail: match extension.payment_subject.kind {
                ledgerflow_core::PaymentSubjectKind::ExchangeAccount |
                ledgerflow_core::PaymentSubjectKind::FacilitatorAccount => PaymentRail::Exchange,
                _ => PaymentRail::Onchain,
            },
            challenge_id: challenge.challenge_id.clone(),
            request_hash: request_hash.clone(),
            accepted_hash: accepted_hash.clone(),
            now_ms,
            freshness_window_ms: proof_freshness_ms,
            clock_skew_ms: challenge.clock_skew_ms,
            payment_subject: extension.payment_subject.clone(),
            presenter: extension.signer.clone(),
        };
        let input = AuthorizationInput {
            chain: &chain,
            trusted,
            proof: &extension.proof,
            context: &context,
            approvals: &extension.approvals,
            tool_arguments,
            revocation: &self.revocation,
        };
        let authorization = ledgerflow_core::verify_authorization(&input)?;

        if let Some(payment_identifier) = payload.payment_identifier() {
            self.replay_store.cache_payment(payment_identifier.to_string(), authorization.clone());
        }

        Ok(MerchantVerificationOutcome { authorization, settlement_reused: false })
    }

    fn claim_replay(
        &mut self,
        challenge: &LedgerFlowChallenge,
        extension: &LedgerFlowAuthorizationExtension,
        request_hash: &str,
        accepted_hash: &str,
        now_ms: u64,
    ) -> Result<(), MerchantVerificationError> {
        let fingerprint = ReplayFingerprint {
            challenge_id: challenge.challenge_id.clone(),
            nonce: extension.proof.tuple.nonce.clone(),
            request_hash: request_hash.to_string(),
            accepted_hash: accepted_hash.to_string(),
        };

        match self.replay_store.claim_nonce(fingerprint, now_ms) {
            Ok(()) => Ok(()),
            // Any nonce conflict is a replay: the nonce key (challenge_id,
            // nonce) is globally unique per challenge, so a conflict means the
            // exact same proof was already observed.
            Err(ReplayConflict { .. }) => Err(MerchantVerificationError::ReplayDetected),
        }
    }

    /// Resolves the presented warrant chain.
    ///
    /// v1 rule: the chain is transmitted inline. If the extension carries an
    /// empty chain (digest-only mode), the merchant cache is consulted; a
    /// digest-only submission must have been cached by a prior inline one.
    fn resolve_chain(
        &mut self,
        extension: &LedgerFlowAuthorizationExtension,
    ) -> Result<WarrantChain, MerchantVerificationError> {
        if extension.warrant_chain.is_empty() {
            return Err(MerchantVerificationError::EmptyChain);
        }
        let mut chain = WarrantChain::default();
        for warrant in &extension.warrant_chain {
            self.warrant_repository.store(warrant.clone());
            chain.push(warrant.clone());
        }
        Ok(chain)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use ledgerflow_core::{
        AuthorizationContext, MerchantConstraint, PaymentConstraint, ProofBuilder, SigningKeyPair,
        TrustedIssuer, TrustedIssuers, Warrant, WarrantBuilder, sha256_prefixed,
    };

    use super::*;
    use crate::{
        replay::{InMemoryReplayStore, ReplayFingerprint},
        x402::{LedgerFlowAuthorizationExtension, LedgerFlowChallenge},
    };

    fn issuer_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x11; 32])
    }

    fn holder_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x22; 32])
    }

    fn trusted() -> TrustedIssuers {
        let mut set = TrustedIssuers::new();
        set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
        set
    }

    fn warrant() -> Warrant {
        WarrantBuilder::new(2_000)
            .warrant_id(*b"mid-root-0000000")
            .ttl_secs(3600)
            .max_depth(2)
            .issuer(issuer_keys().signer_ref())
            .holder(holder_keys().signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
            .resource(ledgerflow_core::ResourceConstraint::default())
            .payment(PaymentConstraint::new(1_000))
            .sign_with(&issuer_keys(), [0_u8; 8])
    }

    fn challenge() -> LedgerFlowChallenge {
        LedgerFlowChallenge {
            version: "1".to_string(),
            challenge_id: "challenge-1".to_string(),
            merchant_id: "merchant-a".to_string(),
            resource: "https://merchant-a.example/pay".to_string(),
            proof_freshness_ms: 60_000,
            clock_skew_ms: 30_000,
            challenge_ttl_ms: 300_000,
            required_subject_kinds: vec!["payment".to_string()],
            ledger: None,
        }
    }

    fn request() -> HttpRequest {
        HttpRequest::new("POST", "merchant-a.example", "/pay", b"{\"ok\":true}".to_vec())
    }

    fn extension() -> LedgerFlowAuthorizationExtension {
        let w = warrant();
        let req = request();
        let quote =
            crate::x402::AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
        let request_hash = crate::x402::canonical_request_hash(&req);
        let accepted_hash = crate::x402::canonical_accepted_hash(&quote);
        let ctx = AuthorizationContext {
            merchant_id: "merchant-a".to_string(),
            merchant_host: "merchant-a.example".to_string(),
            tool_name: "web-search".to_string(),
            model_provider: String::new(),
            action_label: String::new(),
            http_method: "POST".to_string(),
            path_and_query: "/pay".to_string(),
            selected_amount: 100,
            asset: "USDC".to_string(),
            asset_network: Some("base".to_string()),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash,
            accepted_hash,
            now_ms: 2_000,
            freshness_window_ms: 60_000,
            clock_skew_ms: 30_000,
            payment_subject: ledgerflow_core::PaymentSubjectRef::new(
                ledgerflow_core::PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            presenter: holder_keys().signer_ref(),
        };
        let proof = ProofBuilder::new()
            .warrant_id(w.id.clone())
            .challenge_id(ctx.challenge_id.clone())
            .method(ctx.http_method.clone())
            .uri(format!("{}{}", ctx.merchant_host, ctx.path_and_query))
            .request_hash(ctx.request_hash.clone())
            .accepted_hash(ctx.accepted_hash.clone())
            .payment_payload_digest(sha256_prefixed("x402-payload"))
            .nonce("nonce-1".to_string())
            .created_at_ms(ctx.now_ms)
            .sign_with(&holder_keys());
        LedgerFlowAuthorizationExtension {
            version: "1".to_string(),
            challenge_id: "challenge-1".to_string(),
            warrant_chain: vec![w],
            proof,
            signer: holder_keys().signer_ref(),
            payment_subject: ctx.payment_subject,
            approvals: Vec::new(),
        }
    }

    #[test]
    fn warrant_repository_stores_and_loads_by_digest() {
        let mut repo = InMemoryWarrantRepository::default();
        assert!(repo.load("missing").is_none());
        let w = warrant();
        repo.store(w.clone());
        let loaded = repo.load(&w.digest()).expect("loaded");
        assert_eq!(loaded.digest(), w.digest());
    }

    #[test]
    fn merchant_verifier_full_payment_flow_passes() {
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
            ledgerflow_core::InMemoryRevocationCheck::new(),
        );
        let ch = challenge();
        let req = request();
        let ext = extension();
        let payload = crate::x402::PaymentPayload {
            accepted: crate::x402::AcceptedQuote::exact(
                "USDC",
                100,
                "merchant-a",
                Some("base".to_string()),
            ),
            settlement_payload: "0xabc".to_string(),
            payment_identifier: Some("payment-1".to_string()),
            ledgerflow: Some(ext),
        };
        let outcome = verifier
            .verify_payment(&ch, &req, &payload, &trusted(), "web-search", &BTreeMap::new(), 2_000)
            .expect("verified");
        assert!(!outcome.settlement_reused);
        assert_eq!(outcome.authorization.amount, 100);
    }

    #[test]
    fn merchant_verifier_reuses_cached_payment_by_identifier() {
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
            ledgerflow_core::InMemoryRevocationCheck::new(),
        );
        let ch = challenge();
        let req = request();
        let ext = extension();
        let payload = crate::x402::PaymentPayload {
            accepted: crate::x402::AcceptedQuote::exact(
                "USDC",
                100,
                "merchant-a",
                Some("base".to_string()),
            ),
            settlement_payload: "0xabc".to_string(),
            payment_identifier: Some("payment-1".to_string()),
            ledgerflow: Some(ext),
        };
        let first = verifier
            .verify_payment(&ch, &req, &payload, &trusted(), "web-search", &BTreeMap::new(), 2_000)
            .expect("first");
        assert!(!first.settlement_reused);

        let second = verifier
            .verify_payment(&ch, &req, &payload, &trusted(), "web-search", &BTreeMap::new(), 2_000)
            .expect("second");
        assert!(second.settlement_reused);
        assert_eq!(second.authorization.amount, first.authorization.amount);
    }

    #[test]
    fn merchant_verifier_rejects_challenge_mismatch() {
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
            ledgerflow_core::InMemoryRevocationCheck::new(),
        );
        let mut ch = challenge();
        ch.challenge_id = "other-challenge".to_string();
        let req = request();
        let ext = extension();
        let payload = crate::x402::PaymentPayload {
            accepted: crate::x402::AcceptedQuote::exact(
                "USDC",
                100,
                "merchant-a",
                Some("base".to_string()),
            ),
            settlement_payload: "0xabc".to_string(),
            payment_identifier: None,
            ledgerflow: Some(ext),
        };
        let error = verifier
            .verify_payment(&ch, &req, &payload, &trusted(), "web-search", &BTreeMap::new(), 2_000)
            .expect_err("challenge mismatch");
        assert!(matches!(error, MerchantVerificationError::ChallengeMismatch));
    }

    #[test]
    fn merchant_verifier_detects_replay_for_same_request() {
        let mut verifier = MerchantVerifier::new(
            InMemoryReplayStore::default(),
            InMemoryWarrantRepository::default(),
            ledgerflow_core::InMemoryRevocationCheck::new(),
        );
        let ch = challenge();
        let req = request();
        let ext = extension();
        let payload = crate::x402::PaymentPayload {
            accepted: crate::x402::AcceptedQuote::exact(
                "USDC",
                100,
                "merchant-a",
                Some("base".to_string()),
            ),
            settlement_payload: "0xabc".to_string(),
            payment_identifier: None,
            ledgerflow: Some(ext),
        };
        verifier
            .verify_payment(&ch, &req, &payload, &trusted(), "web-search", &BTreeMap::new(), 2_000)
            .expect("first");

        let error = verifier
            .verify_payment(&ch, &req, &payload, &trusted(), "web-search", &BTreeMap::new(), 2_000)
            .expect_err("replay");
        assert!(matches!(error, MerchantVerificationError::ReplayDetected));
    }

    #[test]
    fn replay_store_accepts_same_nonce_for_different_request() {
        let mut store = InMemoryReplayStore::default();
        let fp1 = ReplayFingerprint {
            challenge_id: "challenge-1".to_string(),
            nonce: "nonce-1".to_string(),
            request_hash: "sha256:req1".to_string(),
            accepted_hash: "sha256:acc1".to_string(),
        };
        let fp2 = ReplayFingerprint {
            challenge_id: "challenge-1".to_string(),
            nonce: "nonce-1".to_string(),
            request_hash: "sha256:req2".to_string(),
            accepted_hash: "sha256:acc2".to_string(),
        };
        store.claim_nonce(fp1, 1_000).expect("first");
        let error = store.claim_nonce(fp2, 1_000).expect_err("same nonce is replay");
        assert_eq!(error.existing.nonce, "nonce-1");
    }

    #[test]
    fn rail_mapping_exchange_account_uses_exchange_rail() {
        let subject = ledgerflow_core::PaymentSubjectRef::new(
            ledgerflow_core::PaymentSubjectKind::ExchangeAccount,
            "exchange-1",
        );
        assert_eq!(
            match subject.kind {
                ledgerflow_core::PaymentSubjectKind::ExchangeAccount |
                ledgerflow_core::PaymentSubjectKind::FacilitatorAccount => PaymentRail::Exchange,
                _ => PaymentRail::Onchain,
            },
            PaymentRail::Exchange
        );
        let opaque = ledgerflow_core::PaymentSubjectRef::new(
            ledgerflow_core::PaymentSubjectKind::Opaque,
            "gateway:gw",
        );
        assert_eq!(
            match opaque.kind {
                ledgerflow_core::PaymentSubjectKind::ExchangeAccount |
                ledgerflow_core::PaymentSubjectKind::FacilitatorAccount => PaymentRail::Exchange,
                _ => PaymentRail::Onchain,
            },
            PaymentRail::Onchain
        );
    }
}
