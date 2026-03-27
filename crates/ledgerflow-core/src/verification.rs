//! Type-safe verification pipeline with extension traits for ergonomic APIs.
//!
//! # Design Patterns
//!
//! ## TypeState Verification Pipeline
//!
//! [`VerificationPipeline`] replaces the previous wrapper struct with a
//! **compile-time-enforced step order**. Each `.verify_*()` call consumes
//! the current state and returns the next. Skipping a step or calling steps
//! out of order is a **compile error**.
//!
//! ```ignore
//! use ledgerflow_core::verification::VerificationPipeline;
//!
//! let result = VerificationPipeline::new(&warrant, &proof, &context)
//!     .verify_warrant_signature()?    // Unverified          -> WarrantVerified
//!     .verify_time_bounds()?          // WarrantVerified     -> TimeValidated
//!     .verify_proof_signature()?      // TimeValidated       -> ProofSignatureVerified
//!     .verify_bindings()?             // ProofSignatureVerified -> BindingsVerified
//!     .verify_freshness()?            // BindingsVerified    -> FreshnessVerified
//!     .verify_subject_and_delegation()? // FreshnessVerified -> SubjectVerified
//!     .verify_constraints()?          // SubjectVerified     -> FullyVerified
//!     .complete();                    // FullyVerified       -> VerifiedAuthorization
//! ```
//!
//! ## Extension Traits
//!
//! [`WarrantExt`], [`ProofExt`], and [`Digestible`] add verification and
//! digest methods to existing types without modifying them.
//!
//! ## Newtype Wrappers
//!
//! [`VerifiedWarrant`] and [`VerifiedProof`] provide compile-time proof that
//! cryptographic verification was performed. They can only be constructed
//! through the extension trait methods.

use std::marker::PhantomData;

use crate::{
    constraint,
    error::{AuthorizationError, Result},
    warrant::{AuthorizationContext, Proof, VerifiedAuthorization, Warrant, verify_authorization},
};

// ============================================================================
// Newtype Wrappers
// ============================================================================

/// A warrant that has been cryptographically verified.
///
/// This newtype can only be constructed through [`WarrantExt::verify_signature`],
/// providing compile-time proof that verification was performed.
#[derive(Clone, Debug)]
pub struct VerifiedWarrant(pub(crate) Warrant);

impl VerifiedWarrant {
    /// Extracts the inner warrant.
    ///
    /// Prefer working with `VerifiedWarrant` to maintain compile-time guarantees.
    #[must_use]
    pub fn into_inner(self) -> Warrant {
        self.0
    }

    /// Borrows the inner warrant.
    #[must_use]
    pub const fn as_inner(&self) -> &Warrant {
        &self.0
    }
}

/// A proof that has been cryptographically verified against a warrant.
///
/// Can only be constructed through [`ProofExt::verify_against`].
#[derive(Clone, Debug)]
pub struct VerifiedProof(pub(crate) Proof);

impl VerifiedProof {
    /// Extracts the inner proof.
    #[must_use]
    pub fn into_inner(self) -> Proof {
        self.0
    }

    /// Borrows the inner proof.
    #[must_use]
    pub const fn as_inner(&self) -> &Proof {
        &self.0
    }
}

// ============================================================================
// Extension Traits
// ============================================================================

/// Extension trait that adds verification methods to [`Warrant`].
///
/// # Design Pattern: Extension Trait
///
/// We add new functionality to an existing type without modifying it.
/// This is Rust's type-safe answer to "monkey patching".
pub trait WarrantExt {
    /// Verifies the warrant's cryptographic signature.
    ///
    /// Returns a [`VerifiedWarrant`] on success, which can only be constructed
    /// through this method. This provides compile-time proof of verification.
    fn verify_signature(&self) -> Result<VerifiedWarrant>;

    /// Checks if the warrant is currently valid based on time bounds.
    fn is_valid_at(&self, now_ms: u64) -> Result<()>;

    /// Checks if the warrant allows a specific merchant.
    fn allows_merchant(&self, merchant_id: &str, merchant_host: &str) -> Result<()>;
}

impl WarrantExt for Warrant {
    fn verify_signature(&self) -> Result<VerifiedWarrant> {
        let message = self.canonical_unsigned_payload();
        if self.signature.verify(&self.issuer, message.as_bytes()) {
            Ok(VerifiedWarrant(self.clone()))
        } else {
            Err(AuthorizationError::InvalidWarrantSignature)
        }
    }

    fn is_valid_at(&self, now_ms: u64) -> Result<()> {
        if self.not_before_ms > now_ms {
            return Err(AuthorizationError::WarrantNotYetValid { now_ms });
        }
        if self.expires_at_ms < now_ms {
            return Err(AuthorizationError::WarrantExpired { expires_at_ms: self.expires_at_ms });
        }
        Ok(())
    }

    fn allows_merchant(&self, merchant_id: &str, merchant_host: &str) -> Result<()> {
        if self.audience.allows(merchant_id, merchant_host) {
            Ok(())
        } else {
            Err(AuthorizationError::MerchantNotAllowed { merchant_id: merchant_id.to_string() })
        }
    }
}

/// Extension trait that adds verification methods to [`Proof`].
pub trait ProofExt {
    /// Verifies the proof's cryptographic signature against a verified warrant.
    fn verify_against(&self, warrant: &VerifiedWarrant) -> Result<VerifiedProof>;

    /// Checks if the proof is within the freshness window.
    fn is_fresh(&self, now_ms: u64, freshness_window_ms: u64) -> Result<()>;
}

impl ProofExt for Proof {
    fn verify_against(&self, warrant: &VerifiedWarrant) -> Result<VerifiedProof> {
        let message = self.preimage();
        if self.signature.verify(&warrant.0.subject_signer, message.as_bytes()) {
            Ok(VerifiedProof(self.clone()))
        } else {
            Err(AuthorizationError::InvalidProofSignature)
        }
    }

    fn is_fresh(&self, now_ms: u64, freshness_window_ms: u64) -> Result<()> {
        let freshest_allowed = self.created_at_ms.saturating_add(freshness_window_ms);
        if self.created_at_ms > now_ms || freshest_allowed < now_ms {
            Err(AuthorizationError::ProofOutsideFreshnessWindow)
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// Digestible: Blanket Implementation
// ============================================================================

/// Adds digest computation to any type that can be canonically serialized.
///
/// # Design Pattern: Blanket Implementation
///
/// We automatically implement a trait for all types satisfying certain bounds,
/// avoiding the need for per-type boilerplate.
pub trait Digestible {
    /// Returns the canonical representation for hashing.
    fn canonical_representation(&self) -> String;

    /// Computes the SHA-256 digest with `sha256:` prefix.
    fn compute_digest(&self) -> String {
        crate::warrant::sha256_prefixed(self.canonical_representation())
    }
}

impl Digestible for Warrant {
    fn canonical_representation(&self) -> String {
        self.canonical_signed_payload()
    }
}

impl Digestible for Proof {
    fn canonical_representation(&self) -> String {
        self.preimage()
    }
}

// ============================================================================
// TypeState Verification Pipeline
// ============================================================================

// -- Marker types (zero-sized, exist only at compile time) --

/// Warrant signature has **not** been verified yet.
#[derive(Debug)]
pub struct Unverified;
/// Warrant signature **has** been verified.
#[derive(Debug)]
pub struct WarrantVerified;
/// Time bounds (not_before / expires_at) **have** been validated.
#[derive(Debug)]
pub struct TimeValidated;
/// Proof signature **has** been verified.
#[derive(Debug)]
pub struct ProofSignatureVerified;
/// Challenge, digest, request-hash, and accepted-hash bindings **have** been checked.
#[derive(Debug)]
pub struct BindingsVerified;
/// Proof freshness window **has** been validated.
#[derive(Debug)]
pub struct FreshnessVerified;
/// Payment subject containment and delegation policy **have** been checked.
#[derive(Debug)]
pub struct SubjectVerified;
/// All constraints **have** been verified.
#[derive(Debug)]
pub struct FullyVerified;

/// Type-state verification pipeline that enforces step ordering at compile time.
///
/// Each `.verify_*()` method consumes the pipeline in its current state and
/// returns the next state. The final [`complete`](Self::complete) method is
/// only callable when every verification step has been performed.
///
/// See the [module-level documentation](self) for usage examples.
pub struct VerificationPipeline<'a, State> {
    warrant: &'a Warrant,
    proof: &'a Proof,
    context: &'a AuthorizationContext,
    _state: PhantomData<State>,
}

impl<State> std::fmt::Debug for VerificationPipeline<'_, State> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerificationPipeline").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, Unverified> {
    /// Creates a new verification pipeline in the `Unverified` state.
    #[must_use]
    pub const fn new(
        warrant: &'a Warrant,
        proof: &'a Proof,
        context: &'a AuthorizationContext,
    ) -> Self {
        Self { warrant, proof, context, _state: PhantomData }
    }
}

// ---------------------------------------------------------------------------
// Step 1: Warrant signature
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, Unverified> {
    /// Verifies the warrant's cryptographic signature.
    pub fn verify_warrant_signature(self) -> Result<VerificationPipeline<'a, WarrantVerified>> {
        if self.warrant.version != crate::warrant::WARRANT_VERSION_V1 {
            return Err(AuthorizationError::UnsupportedVersion(self.warrant.version));
        }
        if !self.warrant.verify_signature() {
            return Err(AuthorizationError::InvalidWarrantSignature);
        }
        Ok(VerificationPipeline {
            warrant: self.warrant,
            proof: self.proof,
            context: self.context,
            _state: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Step 2: Time bounds
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, WarrantVerified> {
    /// Validates the warrant's not-before and expires-at timestamps.
    pub fn verify_time_bounds(self) -> Result<VerificationPipeline<'a, TimeValidated>> {
        if self.warrant.not_before_ms > self.context.now_ms {
            return Err(AuthorizationError::WarrantNotYetValid { now_ms: self.context.now_ms });
        }
        if self.warrant.expires_at_ms < self.context.now_ms {
            return Err(AuthorizationError::WarrantExpired {
                expires_at_ms: self.warrant.expires_at_ms,
            });
        }
        if !self.warrant.audience.allows(&self.context.merchant_id, &self.context.merchant_host) {
            return Err(AuthorizationError::MerchantNotAllowed {
                merchant_id: self.context.merchant_id.clone(),
            });
        }
        Ok(VerificationPipeline {
            warrant: self.warrant,
            proof: self.proof,
            context: self.context,
            _state: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Step 3: Proof signature
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, TimeValidated> {
    /// Verifies the proof's cryptographic signature against the warrant subject.
    pub fn verify_proof_signature(
        self,
    ) -> Result<VerificationPipeline<'a, ProofSignatureVerified>> {
        if !self.proof.verify_signature(&self.warrant.subject_signer) {
            return Err(AuthorizationError::InvalidProofSignature);
        }
        Ok(VerificationPipeline {
            warrant: self.warrant,
            proof: self.proof,
            context: self.context,
            _state: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Step 4: Bindings (challenge, digest, request hash, accepted hash)
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, ProofSignatureVerified> {
    /// Checks all binding fields: challenge id, warrant digest, request hash,
    /// accepted hash, and signer consistency.
    pub fn verify_bindings(self) -> Result<VerificationPipeline<'a, BindingsVerified>> {
        if self.proof.challenge_id != self.context.challenge_id {
            return Err(AuthorizationError::ChallengeMismatch);
        }
        if self.proof.warrant_digest != self.warrant.digest() {
            return Err(AuthorizationError::WarrantDigestMismatch);
        }
        if self.proof.accepted_hash != self.context.accepted_hash {
            return Err(AuthorizationError::AcceptedHashMismatch);
        }
        if self.proof.request_hash != self.context.request_hash {
            return Err(AuthorizationError::RequestHashMismatch);
        }
        if self.proof.signer_key != self.warrant.subject_signer.public_key {
            return Err(AuthorizationError::SignerMismatch);
        }
        Ok(VerificationPipeline {
            warrant: self.warrant,
            proof: self.proof,
            context: self.context,
            _state: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Step 5: Freshness
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, BindingsVerified> {
    /// Validates that the proof is within the freshness window.
    pub const fn verify_freshness(self) -> Result<VerificationPipeline<'a, FreshnessVerified>> {
        let freshest_allowed =
            self.proof.created_at_ms.saturating_add(self.context.freshness_window_ms);
        if self.proof.created_at_ms > self.context.now_ms || freshest_allowed < self.context.now_ms
        {
            return Err(AuthorizationError::ProofOutsideFreshnessWindow);
        }
        Ok(VerificationPipeline {
            warrant: self.warrant,
            proof: self.proof,
            context: self.context,
            _state: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Step 6: Subject + Delegation
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, FreshnessVerified> {
    /// Checks payment subject containment and delegation policy.
    pub fn verify_subject_and_delegation(
        self,
    ) -> Result<VerificationPipeline<'a, SubjectVerified>> {
        if !self.warrant.payment_subjects.contains(&self.context.payment_subject) {
            return Err(AuthorizationError::PaymentSubjectNotAllowed {
                subject: self.context.payment_subject.value.clone(),
            });
        }
        if self.context.presented_delegation_depth > 0 && !self.warrant.delegation.can_delegate {
            return Err(AuthorizationError::DelegationNotAllowed);
        }
        if self.context.presented_delegation_depth > self.warrant.delegation.max_depth {
            return Err(AuthorizationError::DelegationDepthExceeded {
                presented: self.context.presented_delegation_depth,
                allowed: self.warrant.delegation.max_depth,
            });
        }
        Ok(VerificationPipeline {
            warrant: self.warrant,
            proof: self.proof,
            context: self.context,
            _state: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Step 7: Constraints
// ---------------------------------------------------------------------------

impl<'a> VerificationPipeline<'a, SubjectVerified> {
    /// Verifies all warrant constraints using the [`Verify`](crate::constraint::Verify) trait.
    pub fn verify_constraints(self) -> Result<VerificationPipeline<'a, FullyVerified>> {
        constraint::verify_all(&self.warrant.constraints, self.context)?;
        Ok(VerificationPipeline {
            warrant: self.warrant,
            proof: self.proof,
            context: self.context,
            _state: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Terminal: produce the result
// ---------------------------------------------------------------------------

impl VerificationPipeline<'_, FullyVerified> {
    /// Completes the pipeline and returns the verified authorization.
    ///
    /// This method is **only callable** when all seven verification steps have
    /// been performed. The compiler prevents calling it on any intermediate state.
    #[must_use]
    pub fn complete(self) -> VerifiedAuthorization {
        VerifiedAuthorization {
            merchant_id: self.context.merchant_id.clone(),
            tool_name: self.context.tool_name.clone(),
            payment_subject: self.context.payment_subject.clone(),
            payer: self.warrant.subject_signer.clone(),
            warrant_digest: self.warrant.digest(),
            accepted_hash: self.context.accepted_hash.clone(),
            request_hash: self.context.request_hash.clone(),
            amount: self.context.selected_quote_amount,
            asset: self.context.asset.clone(),
            scheme: self.context.scheme.clone(),
            payee_id: self.context.payee_id.clone(),
            rail: self.context.rail,
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience: run all steps at once
// ---------------------------------------------------------------------------

impl VerificationPipeline<'_, Unverified> {
    /// Runs the entire verification pipeline in one call.
    ///
    /// Prefer the step-by-step API for better error diagnostics.
    pub fn verify_all(self) -> Result<VerifiedAuthorization> {
        verify_authorization(self.warrant, self.proof, self.context)
    }
}

#[cfg(test)]
mod tests {
    use super::{Digestible, ProofExt, WarrantExt};
    use crate::warrant::{Proof, SigningKeyPair, Warrant};

    fn issuer_keys() -> SigningKeyPair {
        let secret: [u8; 32] = *b"issuer-secret-key-32-bytes-long!";
        SigningKeyPair::from_bytes(&secret)
    }

    fn agent_keys() -> SigningKeyPair {
        let secret: [u8; 32] = *b"agent-secret-key--32-bytes-long!";
        SigningKeyPair::from_bytes(&secret)
    }

    fn sample_warrant() -> Warrant {
        let issuer = issuer_keys();
        let agent = agent_keys();
        use crate::warrant::{
            AmountLimit, AssetRef, AudienceScope, Constraint, DelegationPolicy, PaymentConstraint,
            PaymentRail, PaymentSubjectKind, PaymentSubjectRef, WARRANT_VERSION_V1,
            WarrantMetadata,
        };
        Warrant {
            version: WARRANT_VERSION_V1,
            warrant_id: "test".to_string(),
            issuer: issuer.signer_ref(),
            subject_signer: agent.signer_ref(),
            payment_subjects: vec![PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc",
            )],
            audience: AudienceScope::Any,
            not_before_ms: 0,
            expires_at_ms: 10_000,
            delegation: DelegationPolicy { can_delegate: false, max_depth: 0 },
            constraints: vec![Constraint::Payment(PaymentConstraint {
                max_per_request: AmountLimit { amount: 100 },
                period_limit: None,
                allowed_assets: vec![AssetRef::new("USDC", None)],
                allowed_rails: vec![PaymentRail::Onchain],
                allowed_schemes: vec!["exact".to_string()],
                payee_ids: vec![],
            })],
            metadata: WarrantMetadata::default(),
            signature: issuer.sign(b"placeholder"),
        }
        .sign_with(&issuer)
    }

    #[test]
    fn warrant_ext_verifies_signature() {
        let warrant = sample_warrant();
        // Use UFCS to call the WarrantExt trait method, not the inherent bool method.
        let verified = WarrantExt::verify_signature(&warrant).expect("valid signature");
        assert_eq!(verified.as_inner().warrant_id, "test");
    }

    #[test]
    fn proof_ext_verifies_against_warrant() {
        let warrant = sample_warrant();
        let verified_warrant = WarrantExt::verify_signature(&warrant).expect("valid");
        let proof = Proof::new_signed(
            "challenge-1",
            warrant.digest(),
            "sha256:accepted",
            "sha256:request",
            1_000,
            "nonce-1",
            &agent_keys(),
        );
        let _verified_proof = proof.verify_against(&verified_warrant).expect("valid proof");
    }

    #[test]
    fn digestible_produces_stable_output() {
        let warrant = sample_warrant();
        let d1 = warrant.compute_digest();
        let d2 = warrant.compute_digest();
        assert_eq!(d1, d2);
        assert!(d1.starts_with("sha256:"));
    }

    #[test]
    fn pipeline_typestate_enforces_order() {
        use super::VerificationPipeline;

        let warrant = sample_warrant();
        let ctx = crate::warrant::AuthorizationContext {
            merchant_id: "m".to_string(),
            merchant_host: "m.example".to_string(),
            tool_name: "t".to_string(),
            model_provider: String::new(),
            action_label: String::new(),
            http_method: "GET".to_string(),
            path_and_query: "/p".to_string(),
            selected_quote_amount: 10,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "p".to_string(),
            rail: crate::warrant::PaymentRail::Onchain,
            challenge_id: "c1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            now_ms: 1_000,
            freshness_window_ms: 60_000,
            presented_delegation_depth: 0,
            payment_subject: crate::warrant::PaymentSubjectRef::new(
                crate::warrant::PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc",
            ),
        };
        let proof = Proof::new_signed(
            ctx.challenge_id.clone(),
            warrant.digest(),
            ctx.accepted_hash.clone(),
            ctx.request_hash.clone(),
            ctx.now_ms,
            "nonce-1",
            &agent_keys(),
        );

        // Full pipeline: must go through all steps in order
        let _result = VerificationPipeline::new(&warrant, &proof, &ctx)
            .verify_warrant_signature()
            .expect("sig")
            .verify_time_bounds()
            .expect("time")
            .verify_proof_signature()
            .expect("proof sig")
            .verify_bindings()
            .expect("bindings")
            .verify_freshness()
            .expect("freshness")
            .verify_subject_and_delegation()
            .expect("subject")
            .verify_constraints()
            .expect("constraints")
            .complete();
    }
}
