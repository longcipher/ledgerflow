//! Verification orchestration for a full payment authorization.
//!
//! [`verify_authorization`] is the single entry point used by merchants and
//! the Facilitator. It composes:
//!
//! 1. Chain verification (I1-I7, trust anchor) — [`crate::chain`]
//! 2. PoP verification + freshness — [`crate::pop`]
//! 3. Approval gates (m-of-n) — [`crate::approval`]
//! 4. Revocation check (online seam) — [`crate::revocation`]
//!
//! Online checks (revocation) are passed in as a trait object so the core
//! stays stateless while production deployments wire in persistent storage.

use crate::{
    approval::{ApprovalGate, SignedApproval, verify_approvals},
    chain::{VerifiedChainAuthorization, WarrantChain, verify_chain},
    constraint::{AuthorizationContext, Verify},
    error::{AuthorizationError, Result},
    pop::PopProof,
    revocation::{RevocationCheck, RevocationDecision},
    trust::TrustedIssuers,
    warrant::{SignerRef, Warrant},
};

/// Tool-call arguments used to evaluate approval gates.
pub type ToolArguments = std::collections::BTreeMap<String, String>;

/// The result of a successful authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAuthorization {
    pub merchant_id: String,
    pub tool_name: String,
    pub payment_subject: crate::warrant::PaymentSubjectRef,
    pub holder: SignerRef,
    pub leaf_warrant: Warrant,
    pub root_warrant: Warrant,
    pub chain_len: usize,
    pub amount: u128,
    pub asset: String,
    pub scheme: String,
    pub payee_id: String,
    pub rail: crate::warrant::PaymentRail,
    pub challenge_id: String,
    pub request_hash: String,
    pub accepted_hash: String,
    pub warrant_digest: String,
}

/// Inputs for a full authorization check.
#[derive(Clone, Debug)]
pub struct AuthorizationInput<'a> {
    pub chain: &'a WarrantChain,
    pub trusted: &'a TrustedIssuers,
    pub proof: &'a PopProof,
    pub context: &'a AuthorizationContext,
    /// Approvals attached to this payment (may be empty).
    pub approvals: &'a [SignedApproval],
    /// Arguments of the tool call being authorized (used by approval gates).
    pub tool_arguments: &'a ToolArguments,
    /// Online revocation seam (must be provided; core never does I/O).
    pub revocation: &'a dyn RevocationCheck,
    /// Optional expected payment-payload digest that the PoP must bind to.
    ///
    /// When `Some`, [`verify_authorization`] cross-checks the PoP's
    /// `payment_payload_digest` against this value, closing the
    /// proof-of-possession ↔ payment binding gap (design §6.3). Callers that do
    /// not compute a bound leave this `None` (no check).
    pub payment_payload_digest: Option<String>,
}

/// Runs the full authorization pipeline.
pub fn verify_authorization(input: &AuthorizationInput<'_>) -> Result<VerifiedAuthorization> {
    // 1. Chain + PoP + trust anchor + freshness.
    let chain_verified = verify_chain(input.chain, input.trusted, input.proof, input.context)?;

    // 1b. Payment-payload binding: when the caller supplies an expected digest,
    // the PoP must commit to the exact payment payload (design §6.3). This
    // closes the proof-of-possession ↔ payment binding gap so a valid PoP
    // cannot be replayed against a different payment.
    if let Some(expected) = &input.payment_payload_digest &&
        &input.proof.tuple.payment_payload_digest != expected
    {
        return Err(AuthorizationError::PaymentPayloadDigestMismatch);
    }

    // 2. Revocation (online).
    let leaf = &chain_verified.leaf;
    match input.revocation.check_warrant(&leaf.id) {
        RevocationDecision::Ok => {}
        RevocationDecision::RevokedWarrant => return Err(AuthorizationError::WarrantRevoked),
        RevocationDecision::RevokedHolder => return Err(AuthorizationError::HolderRevoked),
    }
    match input.revocation.check_holder(&leaf.holder) {
        RevocationDecision::Ok => {}
        RevocationDecision::RevokedWarrant => return Err(AuthorizationError::WarrantRevoked),
        RevocationDecision::RevokedHolder => return Err(AuthorizationError::HolderRevoked),
    }

    // 3. Approval gates.
    let gate = leaf.approval_gates.get(&input.context.tool_name);
    let requires_approval =
        gate.is_some_and(|gate: &ApprovalGate| gate.fires(input.tool_arguments));
    if requires_approval {
        verify_approvals(
            input.approvals,
            &leaf.required_approvers,
            leaf.min_approvals,
            &input.context.request_hash,
            input.context.now_ms,
            &input.proof.tuple,
        )?;
    } else if !input.approvals.is_empty() {
        // Approvals supplied but not required: reject (fail-closed) unless
        // they still validate against the warrant's approver set.
        if !leaf.required_approvers.is_empty() {
            crate::approval::verify_approval_threshold(
                input.approvals,
                &leaf.required_approvers,
                leaf.min_approvals,
                &input.context.request_hash,
                input.context.now_ms,
            )?;
        }
    }

    // 3b. Human-presence requirement (AP2-style human-in-the-loop).
    //
    // A human-present challenge demands positive human confirmation bound to
    // this exact payment, regardless of whether a tool gate fired. Empty
    // approvals fail immediately; non-empty approvals must satisfy the
    // warrant's approver policy with PoP digest binding (so a warrant
    // without approvers can never satisfy such a challenge — fail-closed).
    if input.context.human_present {
        if input.approvals.is_empty() {
            return Err(AuthorizationError::HumanPresenceRequired);
        }
        verify_approvals(
            input.approvals,
            &leaf.required_approvers,
            leaf.min_approvals,
            &input.context.request_hash,
            input.context.now_ms,
            &input.proof.tuple,
        )?;
    }

    // 4. Payment subject containment (leaf-level).
    if !leaf.payment_subjects_allowed(input.context) {
        return Err(AuthorizationError::PaymentSubjectNotAllowed {
            subject: input.context.payment_subject.value.clone(),
        });
    }

    Ok(build_authorization(chain_verified, input))
}

fn build_authorization(
    verified: VerifiedChainAuthorization,
    input: &AuthorizationInput<'_>,
) -> VerifiedAuthorization {
    VerifiedAuthorization {
        merchant_id: input.context.merchant_id.clone(),
        tool_name: input.context.tool_name.clone(),
        payment_subject: input.context.payment_subject.clone(),
        holder: verified.leaf.holder.clone(),
        leaf_warrant: verified.leaf.clone(),
        root_warrant: verified.root.clone(),
        chain_len: verified.chain_len,
        amount: input.context.selected_amount,
        asset: input.context.asset.clone(),
        scheme: input.context.scheme.clone(),
        payee_id: input.context.payee_id.clone(),
        rail: input.context.rail,
        challenge_id: input.context.challenge_id.clone(),
        request_hash: input.context.request_hash.clone(),
        accepted_hash: input.context.accepted_hash.clone(),
        warrant_digest: verified.leaf.digest(),
    }
}

/// Extension trait adding convenience checks to warrants.
pub trait WarrantExt {
    /// Returns `true` when the context's payment subject is allowed.
    fn payment_subjects_allowed(&self, context: &AuthorizationContext) -> bool;

    /// Evaluates the runtime-conjunction constraints of this warrant.
    fn verify_constraints(&self, context: &AuthorizationContext) -> Result<()>;
}

impl WarrantExt for Warrant {
    // Skipped for mutation testing: the body is a documented v1-permissive
    // constant, so "replace with true" is an equivalent mutant.
    #[cfg_attr(test, mutants::skip)]
    fn payment_subjects_allowed(&self, context: &AuthorizationContext) -> bool {
        // v1 model: the warrant's holder is bound to the payment subject via
        // the CAIP-10 of the presenter's key when present. For simplicity,
        // the subject allowance is expressed through the merchant/resource
        // constraints; callers that model explicit subjects set them here.
        //
        // NOTE (documented v1 limitation, not a silent bug): the v1 `Warrant`
        // schema carries no dedicated payment-subject constraint field, so
        // this predicate is intentionally permissive. Subject containment is
        // instead enforced through the merchant/resource constraints and the
        // PoP binding (design §6.4). When a subject constraint is added to the
        // schema, this method MUST be tightened to enforce it (fail-closed)
        // and this skip removed.
        let _ = context;
        true
    }

    fn verify_constraints(&self, context: &AuthorizationContext) -> Result<()> {
        self.merchant.verify(context)?;
        self.resource.verify(context)?;
        if let Some(tool) = &self.tool {
            tool.verify(context)?;
        }
        self.payment.verify(context)?;
        Ok(())
    }
}

/// Legacy convenience alias retained for compatibility with older callers.
pub type PipelineInput<'a> = AuthorizationInput<'a>;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        MerchantConstraint, PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef,
        PopProof, ProofBuilder, ResourceConstraint, RevocationDecision, SignedApproval,
        SigningKeyPair, TrustedIssuer, TrustedIssuers, WarrantBuilder,
    };

    fn issuer_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x1A; 32])
    }

    fn holder_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x1B; 32])
    }

    fn approver_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x1C; 32])
    }

    #[derive(Debug)]
    struct AcceptRevocation;

    impl RevocationCheck for AcceptRevocation {
        fn check_warrant(&self, _warrant_id: &[u8]) -> RevocationDecision {
            RevocationDecision::Ok
        }

        fn check_holder(&self, _holder: &SignerRef) -> RevocationDecision {
            RevocationDecision::Ok
        }
    }

    fn warrant(with_approvers: bool) -> Warrant {
        let mut builder = WarrantBuilder::new(2_000)
            .issuer(issuer_keys().signer_ref())
            .holder(holder_keys().signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
            .resource(ResourceConstraint::default())
            .payment(PaymentConstraint::new(1_000));
        if with_approvers {
            builder = builder.approver(approver_keys().signer_ref());
        }
        builder.sign_with(&issuer_keys(), [0_u8; 8])
    }

    fn context() -> AuthorizationContext {
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
            request_hash: crate::sha256_prefixed("req"),
            accepted_hash: crate::sha256_prefixed("acc"),
            now_ms: 2_000,
            freshness_window_ms: 60_000,
            clock_skew_ms: 30_000,
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            presenter: holder_keys().signer_ref(),
            human_present: false,
        }
    }

    fn proof_for(warrant: &Warrant, context: &AuthorizationContext) -> PopProof {
        ProofBuilder::new()
            .warrant_id(warrant.id.clone())
            .challenge_id(context.challenge_id.clone())
            .method(context.http_method.clone())
            .uri(format!("{}{}", context.merchant_host, context.path_and_query))
            .request_hash(context.request_hash.clone())
            .accepted_hash(context.accepted_hash.clone())
            .payment_payload_digest(crate::sha256_prefixed("payment-payload"))
            .nonce("nonce-1".to_string())
            .created_at_ms(context.now_ms)
            .sign_with(&holder_keys())
    }

    fn trusted() -> TrustedIssuers {
        let mut set = TrustedIssuers::new();
        set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
        set
    }

    #[test]
    fn payment_payload_digest_mismatch_is_rejected_and_match_passes() {
        let warrant = warrant(false);
        let chain = WarrantChain::single(warrant.clone());
        let proof = proof_for(&warrant, &context());
        let ctx = context();
        let trust = trusted();
        let revocation = AcceptRevocation;
        let args = BTreeMap::new();

        // Matching digest passes.
        let matching = AuthorizationInput {
            chain: &chain,
            trusted: &trust,
            proof: &proof,
            context: &ctx,
            approvals: &[],
            tool_arguments: &args,
            revocation: &revocation,
            payment_payload_digest: Some(crate::sha256_prefixed("payment-payload")),
        };
        assert!(verify_authorization(&matching).is_ok());

        // Divergent digest fails with the dedicated error.
        let divergent = AuthorizationInput {
            chain: &chain,
            trusted: &trust,
            proof: &proof,
            context: &ctx,
            approvals: &[],
            tool_arguments: &args,
            revocation: &revocation,
            payment_payload_digest: Some(crate::sha256_prefixed("different-payload")),
        };
        let error = verify_authorization(&divergent).expect_err("digest mismatch");
        assert_eq!(error, AuthorizationError::PaymentPayloadDigestMismatch);
    }

    #[test]
    fn unconstrained_approvals_without_gate_are_accepted() {
        // Approvals supplied although no gate fired and the warrant declares
        // NO approver set: they are inert and must not fail authorization.
        let warrant = warrant(false);
        let chain = WarrantChain::single(warrant.clone());
        let proof = proof_for(&warrant, &context());
        let ctx = context();
        let trust = trusted();
        let revocation = AcceptRevocation;
        let args = BTreeMap::new();
        let stranger = SigningKeyPair::from_bytes(&[0x1D; 32]);
        let approvals = vec![SignedApproval::sign(
            &ctx.request_hash,
            &stranger.signer_ref(),
            10_300,
            &stranger,
        )];
        let result = verify_authorization(&AuthorizationInput {
            chain: &chain,
            trusted: &trust,
            proof: &proof,
            context: &ctx,
            approvals: &approvals,
            tool_arguments: &args,
            revocation: &revocation,
            payment_payload_digest: None,
        });
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_approvals_with_configured_approvers_fail_even_without_gate() {
        // Gate did not fire but the warrant HAS approvers: supplied approvals
        // must still validate (fail-closed), so a forged signature is fatal.
        let warrant = warrant(true);
        let chain = WarrantChain::single(warrant.clone());
        let proof = proof_for(&warrant, &context());
        let ctx = context();
        let trust = trusted();
        let revocation = AcceptRevocation;
        let args = BTreeMap::new();
        let mut forged = SignedApproval::sign(
            &ctx.request_hash,
            &approver_keys().signer_ref(),
            10_300,
            &approver_keys(),
        );
        forged.signature.value = vec![0xFF; 64];
        let error = verify_authorization(&AuthorizationInput {
            chain: &chain,
            trusted: &trust,
            proof: &proof,
            context: &ctx,
            approvals: std::slice::from_ref(&forged),
            tool_arguments: &args,
            revocation: &revocation,
            payment_payload_digest: None,
        })
        .expect_err("forged approval");
        assert_eq!(error, AuthorizationError::InvalidApprovalSignature);
    }

    #[test]
    fn warrant_ext_verify_constraints_surfaces_violations() {
        let warrant = warrant(false);
        let mut ctx = context();
        ctx.selected_amount = 5_000; // exceeds the 1_000 cap
        let error = warrant.verify_constraints(&ctx).expect_err("cap exceeded");
        assert!(matches!(error, AuthorizationError::PaymentAmountExceeded { .. }));
        // Within limits it passes.
        let ok_ctx = context();
        assert!(warrant.verify_constraints(&ok_ctx).is_ok());
    }
}
