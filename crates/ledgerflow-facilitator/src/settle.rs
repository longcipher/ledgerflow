//! `/settle` orchestration with **atomic re-verification**.
//!
//! The verify step is only a pre-check. Settlement MUST re-run the full
//! authorization pipeline atomically before touching any rail, closing the
//! verify→settle TOCTOU window (design §8.1).

use std::{collections::BTreeMap, sync::Arc};

use ledgerflow_core::{
    AuthorizationContext, AuthorizationError, PopProof, RevocationCheck, TrustedIssuers,
    VerifiedAuthorization, WarrantChain, bind_authorization, verify_chain_with_resolver,
};
use parking_lot::Mutex;

use crate::{
    outcome::SettlementOutcome,
    rails::RailAdapter,
    reputation::ReputationReporter,
    routing::RoutingError,
    subject::{PaymentSubjectResolver, ResolvedSubject},
};

/// Inputs to the settle orchestration.
#[derive(Clone, Debug)]
pub struct SettleRequest<'a> {
    /// The verified authorization from `/verify`.
    pub authorization: &'a VerifiedAuthorization,
    /// The presented chain, re-verified in full at settle time.
    pub chain: &'a WarrantChain,
    /// The presented PoP.
    pub proof: &'a PopProof,
    /// The request context.
    pub context: &'a AuthorizationContext,
    /// Trusted root issuers used for the settle-time chain re-verification.
    pub trusted: &'a TrustedIssuers,
    /// Settlement timestamp (unix milliseconds).
    pub now_ms: u64,
}

/// Process-local nonce claims: one PoP may settle at most once while its
/// freshness window is open. Persistent idempotency remains the rail's job.
#[derive(Clone, Debug, Default)]
struct SettlementReplayGuard {
    claims: Arc<Mutex<BTreeMap<(String, String), u64>>>,
}

impl SettlementReplayGuard {
    fn claim(
        &self,
        challenge_id: &str,
        nonce: &str,
        now_ms: u64,
        ttl_ms: u64,
    ) -> Result<(), AuthorizationError> {
        let key = (challenge_id.to_string(), nonce.to_string());
        let mut claims = self.claims.lock();
        if let Some(created_at_ms) = claims.get(&key) &&
            now_ms.saturating_sub(*created_at_ms) < ttl_ms
        {
            return Err(AuthorizationError::ReplayDetected);
        }
        claims.insert(key, now_ms);
        Ok(())
    }
}

/// Settlement service that atomically re-verifies and routes to a rail.
#[derive(Clone)]
pub struct SettlementService<R, P, A> {
    pub revocation: R,
    pub resolver: P,
    pub adapters: Vec<A>,
    /// Optional EIP-8004 reputation reporter invoked after successful
    /// settlement. Reporting never affects settlement outcomes.
    pub reputation: Option<ReputationReporter>,
    replay: SettlementReplayGuard,
}

impl<R, P, A> SettlementService<R, P, A>
where
    R: RevocationCheck,
    P: PaymentSubjectResolver,
    A: RailAdapter,
{
    /// Creates a new settlement service without reputation reporting.
    #[must_use]
    pub fn new(revocation: R, resolver: P, adapters: Vec<A>) -> Self {
        Self {
            revocation,
            resolver,
            adapters,
            reputation: None,
            replay: SettlementReplayGuard::default(),
        }
    }

    /// Attaches an EIP-8004 reputation reporter (builder style).
    ///
    /// The reporter emits feedback through its configured
    /// [`FeedbackSink`] after every successful settlement whose leaf warrant
    /// carries an agent identity claim.
    #[must_use]
    pub fn with_reputation(mut self, reporter: ReputationReporter) -> Self {
        self.reputation = Some(reporter);
        self
    }

    /// Settles a verified authorization after atomic re-verification.
    pub fn settle(&self, request: &SettleRequest<'_>) -> SettlementOutcome {
        // 1. Atomic re-verify before routing to avoid leaking rail mapping for revoked/expired
        //    credentials (design §8.1). The re-verify is as close to the rail call as possible
        //    given routing needs the authorization; the residual window between this check and the
        //    rail's transaction is a deployment concern (lease / in-transaction check).
        if let Err(error) = self.reverify(request) {
            return SettlementOutcome::failed(error.to_string());
        }

        // 2. Route to a rail.
        let resolved = match self.resolve(request.authorization) {
            Ok(subject) => subject,
            Err(error) => return SettlementOutcome::failed(error.to_string()),
        };
        let adapter = match self.adapters.iter().find(|adapter| adapter.supports(&resolved)) {
            Some(adapter) => adapter,
            None => {
                return SettlementOutcome::failed(RoutingError::NoCompatibleRail.to_string());
            }
        };

        // 3. Settle.
        match adapter.settle(request.authorization) {
            Ok(receipt) => {
                if let Some(reporter) = &self.reputation {
                    reporter.report_settlement(request.authorization, &receipt);
                }
                SettlementOutcome::settled(receipt)
            }
            Err(error) => SettlementOutcome::failed(error.to_string()),
        }
    }

    fn reverify(&self, request: &SettleRequest<'_>) -> Result<(), AuthorizationError> {
        // Re-run the whole chain pipeline with the settle clock. A subset of
        // checks lets a stale verify result authorize a different chain,
        // context, amount or identity at the rail.
        let mut settle_context = request.context.clone();
        settle_context.now_ms = request.now_ms;
        let verified = verify_chain_with_resolver(
            request.chain,
            request.trusted,
            None,
            request.proof,
            &settle_context,
        )?;

        // Revocation is an online commitment: every warrant in the chain must
        // still be live, not just the presenting leaf.
        for warrant in &request.chain.warrants {
            self.revocation.verify(&warrant.id, &warrant.holder)?;
        }

        // The object handed to the rail must be the one just verified.
        let rebound = bind_authorization(&verified, &settle_context);
        if &rebound != request.authorization {
            return Err(AuthorizationError::AuthorizationBindingMismatch);
        }

        // One PoP nonce may settle at most once inside its freshness window.
        self.replay.claim(
            &request.proof.tuple.challenge_id,
            &request.proof.tuple.nonce,
            request.now_ms,
            settle_context.freshness_window_ms.saturating_add(settle_context.clock_skew_ms),
        )?;

        Ok(())
    }

    fn resolve(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<ResolvedSubject, RoutingError> {
        Ok(self.resolver.resolve(authorization)?)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Mutex;

    use ledgerflow_core::{
        InMemoryRevocationCheck, MerchantConstraint, PaymentConstraint, PaymentRail,
        PaymentSubjectKind, PaymentSubjectRef, ResourceConstraint, SigningKeyPair, Warrant,
    };

    use super::*;
    use crate::{
        RailKind, SubjectResolutionError,
        rails::{RailQuote, SettlementReceipt, VerificationResult},
        reputation::{FeedbackSink, ReputationReporter},
    };

    struct AcceptAllResolver;

    impl PaymentSubjectResolver for AcceptAllResolver {
        fn resolve(
            &self,
            _authorization: &VerifiedAuthorization,
        ) -> Result<ResolvedSubject, SubjectResolutionError> {
            Ok(ResolvedSubject { rail: RailKind::Evm, value: "0xpayee".to_string() })
        }
    }

    struct ScriptedAdapter {
        fail: bool,
    }

    impl RailAdapter for ScriptedAdapter {
        fn kind(&self) -> RailKind {
            RailKind::Evm
        }

        fn supports(&self, _subject: &ResolvedSubject) -> bool {
            true
        }

        fn quote(
            &self,
            _authorization: &VerifiedAuthorization,
        ) -> Result<RailQuote, crate::rails::RailError> {
            Ok(RailQuote {
                rail: RailKind::Evm,
                estimated_fee: 0,
                estimated_time_ms: 0,
                asset: "USDC".to_string(),
            })
        }

        fn settle(
            &self,
            _authorization: &VerifiedAuthorization,
        ) -> Result<SettlementReceipt, crate::rails::RailError> {
            if self.fail {
                Err(crate::rails::RailError::SettlementFailed("forced".to_string()))
            } else {
                Ok(SettlementReceipt {
                    rail: RailKind::Evm,
                    transaction_id: "0xtx".to_string(),
                    settled_amount: 100,
                    asset: "USDC".to_string(),
                })
            }
        }

        fn verify(
            &self,
            _receipt: &SettlementReceipt,
        ) -> Result<VerificationResult, crate::rails::RailError> {
            Ok(VerificationResult { verified: true, confirmations: 1 })
        }
    }

    struct CaptureSink(Mutex<Vec<crate::reputation::SettlementFeedback>>);

    impl FeedbackSink for CaptureSink {
        fn submit(&self, feedback: &crate::reputation::SettlementFeedback) -> Result<(), String> {
            self.0.lock().expect("lock").push(feedback.clone());
            Ok(())
        }
    }

    fn warrant(with_agent_ref: bool) -> Warrant {
        let issuer = SigningKeyPair::from_bytes(&[0x91; 32]);
        let holder = SigningKeyPair::from_bytes(&[0x92; 32]);
        let builder = ledgerflow_core::WarrantBuilder::new(1_000)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
            .resource(ResourceConstraint::default())
            .payment(PaymentConstraint::new(1_000));
        // Extensions are part of the signed payload, so they must be set
        // before signing. Mutating `warrant.extensions` after `sign_with`
        // would invalidate the signature and fail `verify_signature`.
        if with_agent_ref {
            builder
                .extension(
                    ledgerflow_core::agent_identity::AGENT_ID_EXTENSION_KEY.to_string(),
                    b"eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/22".to_vec(),
                )
                .sign_with(&issuer, [0_u8; 8])
        } else {
            builder.sign_with(&issuer, [0_u8; 8])
        }
    }

    fn trusted() -> TrustedIssuers {
        let mut set = TrustedIssuers::new();
        set.add(ledgerflow_core::TrustedIssuer::new(
            "issuer".to_string(),
            SigningKeyPair::from_bytes(&[0x91; 32]).signer_ref(),
        ));
        set
    }

    fn authorization(with_agent_ref: bool) -> VerifiedAuthorization {
        let leaf = warrant(with_agent_ref);
        VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            holder: leaf.holder.clone(),
            leaf_warrant: leaf.clone(),
            root_warrant: leaf.clone(),
            chain_len: 1,
            amount: 100,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            warrant_digest: leaf.digest(),
        }
    }

    fn request<'a>(
        authorization: &'a VerifiedAuthorization,
        chain: &'a WarrantChain,
        proof: &'a PopProof,
        context: &'a AuthorizationContext,
        trusted: &'a TrustedIssuers,
    ) -> SettleRequest<'a> {
        SettleRequest { authorization, chain, proof, context, trusted, now_ms: 5_000 }
    }

    #[test]
    fn successful_settlement_triggers_reputation_report() {
        let sink = std::sync::Arc::new(CaptureSink(Mutex::new(Vec::new())));
        let service = SettlementService::new(
            InMemoryRevocationCheck::new(),
            AcceptAllResolver,
            vec![ScriptedAdapter { fail: false }],
        )
        .with_reputation(ReputationReporter::new(sink.clone(), true));

        let authorization = authorization(true);
        let leaf = authorization.leaf_warrant.clone();
        let chain = WarrantChain::single(leaf.clone());
        let proof = sample_proof(&leaf);
        let context = sample_context();
        let outcome =
            service.settle(&request(&authorization, &chain, &proof, &context, &trusted()));
        assert_eq!(outcome.status, crate::outcome::SettlementStatus::Settled);
        assert_eq!(sink.0.lock().expect("lock").len(), 1);
    }

    #[test]
    fn failed_settlement_does_not_report() {
        let sink = std::sync::Arc::new(CaptureSink(Mutex::new(Vec::new())));
        let service = SettlementService::new(
            InMemoryRevocationCheck::new(),
            AcceptAllResolver,
            vec![ScriptedAdapter { fail: true }],
        )
        .with_reputation(ReputationReporter::new(sink.clone(), true));

        let authorization = authorization(true);
        let leaf = authorization.leaf_warrant.clone();
        let chain = WarrantChain::single(leaf.clone());
        let proof = sample_proof(&leaf);
        let context = sample_context();
        let outcome =
            service.settle(&request(&authorization, &chain, &proof, &context, &trusted()));
        assert_eq!(outcome.status, crate::outcome::SettlementStatus::Failed);
        assert!(sink.0.lock().expect("lock").is_empty());
    }

    // Minimal PoP/context fixtures; settle re-verification checks
    // freshness bounds, amount cap, PoP signature, and constraints.
    fn sample_proof(leaf: &Warrant) -> PopProof {
        use ledgerflow_core::{PopTuple, ProofBuilder};
        let holder = SigningKeyPair::from_bytes(&[0x92; 32]);
        let tuple = PopTuple {
            warrant_id: leaf.id.to_vec(),
            challenge_id: "challenge-1".to_string(),
            method: "POST".to_string(),
            uri: "merchant-a.example/pay".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            payment_payload_digest: "sha256:pay".to_string(),
            tool_args_digest: None,
            approvals_digest: None,
            nonce: "nonce-1".to_string(),
            created_at_ms: 4_000,
        };
        ProofBuilder::new()
            .warrant_id(tuple.warrant_id.clone())
            .challenge_id(tuple.challenge_id.clone())
            .method(tuple.method.clone())
            .uri(tuple.uri.clone())
            .request_hash(tuple.request_hash.clone())
            .accepted_hash(tuple.accepted_hash.clone())
            .payment_payload_digest(tuple.payment_payload_digest.clone())
            .nonce(tuple.nonce.clone())
            .created_at_ms(tuple.created_at_ms)
            .sign_with(&holder)
    }

    fn sample_context() -> AuthorizationContext {
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
            asset_network: Some("base".to_string()),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            now_ms: 5_000,
            freshness_window_ms: 60_000,
            clock_skew_ms: 30_000,
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            presenter: SigningKeyPair::from_bytes(&[0x92; 32]).signer_ref(),
            human_present: false,
        }
    }
}
