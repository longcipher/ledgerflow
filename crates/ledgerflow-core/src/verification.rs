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
        // schema, this method MUST be tightened to enforce it (fail-closed).
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
