//! `/settle` orchestration with **atomic re-verification**.
//!
//! The verify step is only a pre-check. Settlement MUST re-verify revocation,
//! TTL, PoP freshness, and the payment cap atomically before touching any
//! rail, closing the verify→settle TOCTOU window (design §8.1).

use ledgerflow_core::{
    AuthorizationContext, AuthorizationError, PopProof, RevocationCheck, VerifiedAuthorization,
    WarrantChain, verify_freshness,
};

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
    /// The presented chain (leaf re-verified for TTL/revocation).
    pub chain: &'a WarrantChain,
    /// The presented PoP.
    pub proof: &'a PopProof,
    /// The request context.
    pub context: &'a AuthorizationContext,
    /// Verification timestamp (unix milliseconds).
    pub now_ms: u64,
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
}

impl<R, P, A> SettlementService<R, P, A>
where
    R: RevocationCheck,
    P: PaymentSubjectResolver,
    A: RailAdapter,
{
    /// Creates a new settlement service without reputation reporting.
    #[must_use]
    pub const fn new(revocation: R, resolver: P, adapters: Vec<A>) -> Self {
        Self { revocation, resolver, adapters, reputation: None }
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
        // 1. Atomic re-verify: revocation + TTL + PoP freshness + amount cap.
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

        // 3. Final revocation re-check immediately before settlement.
        //
        // Design §8.1 requires the settlement action and the final revocation
        // check to be as close to atomic as the store allows. `reverify` above
        // already checked revocation; we re-check once more right before
        // touching the rail to shrink the verify→settle TOCTOU window. The
        // residual window (a revocation landing between this check and the
        // rail call) is documented as a deployment concern: production should
        // hold a settlement lease or perform the check inside the rail's
        // transaction when the rail supports it.
        if let Err(error) = self.reverify(request) {
            return SettlementOutcome::failed(error.to_string());
        }

        // 4. Settle.
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
        let leaf = request.chain.leaf().ok_or(AuthorizationError::EmptyChain)?;
        // Revocation (online, persistent).
        self.revocation.verify(&leaf.id, &leaf.holder)?;
        // TTL.
        let now_secs = request.now_ms / 1000;
        if leaf.expires_at < now_secs {
            return Err(AuthorizationError::WarrantExpired { expires_at: leaf.expires_at });
        }
        // PoP freshness.
        verify_freshness(
            request.proof,
            request.now_ms,
            request.context.freshness_window_ms,
            request.context.clock_skew_ms,
        )?;
        // Amount cap.
        if request.authorization.amount > leaf.payment.max_per_charge {
            return Err(AuthorizationError::PaymentAmountExceeded {
                amount: request.authorization.amount,
                limit: leaf.payment.max_per_charge,
            });
        }
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
        let mut warrant = ledgerflow_core::WarrantBuilder::new(1_000)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
            .resource(ResourceConstraint::default())
            .payment(PaymentConstraint::new(1_000))
            .sign_with(&issuer, [0_u8; 8]);
        if with_agent_ref {
            warrant.extensions.insert(
                ledgerflow_core::agent_identity::AGENT_ID_EXTENSION_KEY.to_string(),
                b"eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/22".to_vec(),
            );
        }
        warrant
    }

    fn authorization(with_agent_ref: bool) -> VerifiedAuthorization {
        VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            holder: SigningKeyPair::from_bytes(&[0x92; 32]).signer_ref(),
            leaf_warrant: warrant(with_agent_ref),
            root_warrant: warrant(false),
            chain_len: 1,
            amount: 100,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            warrant_digest: "sha256:w".to_string(),
        }
    }

    fn request<'a>(
        authorization: &'a VerifiedAuthorization,
        chain: &'a WarrantChain,
        proof: &'a PopProof,
        context: &'a AuthorizationContext,
    ) -> SettleRequest<'a> {
        SettleRequest { authorization, chain, proof, context, now_ms: 5_000 }
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
        let chain = WarrantChain::single(leaf);
        let proof = sample_proof();
        let context = sample_context();
        let outcome = service.settle(&request(&authorization, &chain, &proof, &context));
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
        let chain = WarrantChain::single(leaf);
        let proof = sample_proof();
        let context = sample_context();
        let outcome = service.settle(&request(&authorization, &chain, &proof, &context));
        assert_eq!(outcome.status, crate::outcome::SettlementStatus::Failed);
        assert!(sink.0.lock().expect("lock").is_empty());
    }

    // Minimal PoP/context fixtures; settle re-verification only checks
    // freshness bounds and the amount cap.
    fn sample_proof() -> PopProof {
        use ledgerflow_core::{PopTuple, ProofBuilder};
        let holder = SigningKeyPair::from_bytes(&[0x92; 32]);
        let tuple = PopTuple {
            warrant_id: vec![0_u8; 16],
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
