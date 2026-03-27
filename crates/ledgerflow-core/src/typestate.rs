//! TypeState-driven warrant builder with compile-time validation.
//!
//! This module demonstrates extreme compile-time safety by encoding warrant
//! construction states in the type system. Invalid states are impossible to represent.

use std::marker::PhantomData;

use crate::warrant::{
    AudienceScope, Constraint, DelegationPolicy, PaymentSubjectRef, SignatureEnvelope, SignerRef,
    SigningKeyPair, Warrant, WarrantMetadata,
};

// ============================================================================
// TypeState Markers: Zero-sized types that exist only at compile time
// ============================================================================

/// Marker: Warrant has no issuer configured yet
#[derive(Debug)]
pub struct NoIssuer;
/// Marker: Warrant has an issuer configured
#[derive(Debug)]
pub struct HasIssuer;
/// Marker: Warrant has no subject signer configured yet
#[derive(Debug)]
pub struct NoSubject;
/// Marker: Warrant has a subject signer configured
#[derive(Debug)]
pub struct HasSubject;
/// Marker: Warrant has no payment subjects configured yet
#[derive(Debug)]
pub struct NoPaymentSubjects;
/// Marker: Warrant has payment subjects configured
#[derive(Debug)]
pub struct HasPaymentSubjects;
/// Marker: Warrant is unsigned
#[derive(Debug)]
pub struct Unsigned;
/// Marker: Warrant is signed and ready
#[derive(Debug)]
pub struct Signed;

// ============================================================================
// Typed Warrant Builder: State transitions enforced at compile time
// ============================================================================

/// Type-safe warrant builder that enforces construction order at compile time.
///
/// The builder uses phantom types to track which fields have been set. The compiler
/// prevents you from building a warrant until all required fields are provided,
/// and prevents you from setting the same field twice.
///
/// # Design Pattern: TypeState
///
/// Each builder method consumes `self` and returns a new builder with updated
/// type parameters. This is a zero-cost abstraction - the phantom types disappear
/// at compile time, leaving only the data.
///
/// # Example
///
/// ```ignore
/// let warrant = WarrantBuilder::new()
///     .version(WARRANT_VERSION_V1)
///     .warrant_id("warrant-123")
///     .issuer(issuer_keys.signer_ref())  // Transitions NoIssuer -> HasIssuer
///     .subject_signer(agent_keys.signer_ref())  // Transitions NoSubject -> HasSubject
///     .add_payment_subject(subject)  // Transitions NoPaymentSubjects -> HasPaymentSubjects
///     .audience(AudienceScope::Any)
///     .not_before_ms(1000)
///     .expires_at_ms(5000)
///     .delegation(DelegationPolicy { can_delegate: false, max_depth: 0 })
///     .sign_with(&issuer_keys);  // Only available when all required fields are set
/// ```
pub struct WarrantBuilder<I, S, P, Sig> {
    version: u16,
    warrant_id: String,
    issuer: Option<SignerRef>,
    subject_signer: Option<SignerRef>,
    payment_subjects: Vec<PaymentSubjectRef>,
    audience: AudienceScope,
    not_before_ms: u64,
    expires_at_ms: u64,
    delegation: DelegationPolicy,
    constraints: Vec<Constraint>,
    metadata: WarrantMetadata,
    signature: Option<SignatureEnvelope>,
    _marker: PhantomData<(I, S, P, Sig)>,
}

impl<I, S, P, Sig> std::fmt::Debug for WarrantBuilder<I, S, P, Sig> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarrantBuilder")
            .field("version", &self.version)
            .field("warrant_id", &self.warrant_id)
            .finish_non_exhaustive()
    }
}

impl WarrantBuilder<NoIssuer, NoSubject, NoPaymentSubjects, Unsigned> {
    /// Creates a new warrant builder with sensible defaults.
    ///
    /// All required fields must be set before the warrant can be signed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: crate::warrant::WARRANT_VERSION_V1,
            warrant_id: String::new(),
            issuer: None,
            subject_signer: None,
            payment_subjects: Vec::new(),
            audience: AudienceScope::Any,
            not_before_ms: 0,
            expires_at_ms: 0,
            delegation: DelegationPolicy { can_delegate: false, max_depth: 0 },
            constraints: Vec::new(),
            metadata: WarrantMetadata::default(),
            signature: None,
            _marker: PhantomData,
        }
    }
}

impl Default for WarrantBuilder<NoIssuer, NoSubject, NoPaymentSubjects, Unsigned> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Builder Methods: Available at all states
// ============================================================================

impl<I, S, P, Sig> WarrantBuilder<I, S, P, Sig> {
    /// Sets the warrant schema version.
    #[must_use]
    pub const fn version(mut self, version: u16) -> Self {
        self.version = version;
        self
    }

    /// Sets the unique warrant identifier.
    #[must_use]
    pub fn warrant_id(mut self, warrant_id: impl Into<String>) -> Self {
        self.warrant_id = warrant_id.into();
        self
    }

    /// Sets the audience scope for this warrant.
    #[must_use]
    pub fn audience(mut self, audience: AudienceScope) -> Self {
        self.audience = audience;
        self
    }

    /// Sets the not-before timestamp in milliseconds.
    #[must_use]
    pub const fn not_before_ms(mut self, not_before_ms: u64) -> Self {
        self.not_before_ms = not_before_ms;
        self
    }

    /// Sets the expiration timestamp in milliseconds.
    #[must_use]
    pub const fn expires_at_ms(mut self, expires_at_ms: u64) -> Self {
        self.expires_at_ms = expires_at_ms;
        self
    }

    /// Sets the delegation policy.
    #[must_use]
    pub const fn delegation(mut self, delegation: DelegationPolicy) -> Self {
        self.delegation = delegation;
        self
    }

    /// Adds a constraint to the warrant.
    #[must_use]
    pub fn add_constraint(mut self, constraint: Constraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Sets all constraints at once, replacing any existing constraints.
    #[must_use]
    pub fn constraints(mut self, constraints: Vec<Constraint>) -> Self {
        self.constraints = constraints;
        self
    }

    /// Sets the warrant metadata.
    #[must_use]
    pub fn metadata(mut self, metadata: WarrantMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

// ============================================================================
// State Transition Methods: Change the type state
// ============================================================================

impl<S, P, Sig> WarrantBuilder<NoIssuer, S, P, Sig> {
    /// Sets the issuer signer reference.
    ///
    /// This method is only available when the issuer has not been set yet.
    /// After calling this, the builder transitions to `HasIssuer` state.
    #[must_use]
    pub fn issuer(mut self, issuer: SignerRef) -> WarrantBuilder<HasIssuer, S, P, Sig> {
        self.issuer = Some(issuer);
        WarrantBuilder {
            version: self.version,
            warrant_id: self.warrant_id,
            issuer: self.issuer,
            subject_signer: self.subject_signer,
            payment_subjects: self.payment_subjects,
            audience: self.audience,
            not_before_ms: self.not_before_ms,
            expires_at_ms: self.expires_at_ms,
            delegation: self.delegation,
            constraints: self.constraints,
            metadata: self.metadata,
            signature: self.signature,
            _marker: PhantomData,
        }
    }
}

impl<I, P, Sig> WarrantBuilder<I, NoSubject, P, Sig> {
    /// Sets the subject signer reference.
    ///
    /// This method is only available when the subject has not been set yet.
    /// After calling this, the builder transitions to `HasSubject` state.
    #[must_use]
    pub fn subject_signer(
        mut self,
        subject_signer: SignerRef,
    ) -> WarrantBuilder<I, HasSubject, P, Sig> {
        self.subject_signer = Some(subject_signer);
        WarrantBuilder {
            version: self.version,
            warrant_id: self.warrant_id,
            issuer: self.issuer,
            subject_signer: self.subject_signer,
            payment_subjects: self.payment_subjects,
            audience: self.audience,
            not_before_ms: self.not_before_ms,
            expires_at_ms: self.expires_at_ms,
            delegation: self.delegation,
            constraints: self.constraints,
            metadata: self.metadata,
            signature: self.signature,
            _marker: PhantomData,
        }
    }
}

impl<I, S, Sig> WarrantBuilder<I, S, NoPaymentSubjects, Sig> {
    /// Adds the first payment subject.
    ///
    /// This method is only available when no payment subjects have been added yet.
    /// After calling this, the builder transitions to `HasPaymentSubjects` state.
    #[must_use]
    pub fn add_payment_subject(
        mut self,
        subject: PaymentSubjectRef,
    ) -> WarrantBuilder<I, S, HasPaymentSubjects, Sig> {
        self.payment_subjects.push(subject);
        WarrantBuilder {
            version: self.version,
            warrant_id: self.warrant_id,
            issuer: self.issuer,
            subject_signer: self.subject_signer,
            payment_subjects: self.payment_subjects,
            audience: self.audience,
            not_before_ms: self.not_before_ms,
            expires_at_ms: self.expires_at_ms,
            delegation: self.delegation,
            constraints: self.constraints,
            metadata: self.metadata,
            signature: self.signature,
            _marker: PhantomData,
        }
    }
}

impl<I, S, Sig> WarrantBuilder<I, S, HasPaymentSubjects, Sig> {
    /// Adds an additional payment subject.
    ///
    /// This method is only available after at least one payment subject has been added.
    #[must_use]
    pub fn add_payment_subject(mut self, subject: PaymentSubjectRef) -> Self {
        self.payment_subjects.push(subject);
        self
    }
}

// ============================================================================
// Terminal Method: Only available when all required fields are set
// ============================================================================

impl WarrantBuilder<HasIssuer, HasSubject, HasPaymentSubjects, Unsigned> {
    /// Signs the warrant with the issuer's signing key pair.
    ///
    /// This method is only available when:
    /// - Issuer has been set (HasIssuer)
    /// - Subject signer has been set (HasSubject)
    /// - At least one payment subject has been added (HasPaymentSubjects)
    /// - The warrant is not yet signed (Unsigned)
    ///
    /// The compiler prevents calling this method if any required field is missing.
    #[must_use]
    #[expect(clippy::expect_used, reason = "typestate guarantees Options are Some at this point")]
    pub fn sign_with(self, issuer_keys: &SigningKeyPair) -> Warrant {
        let mut warrant = Warrant {
            version: self.version,
            warrant_id: self.warrant_id,
            issuer: self.issuer.expect("issuer must be set"),
            subject_signer: self.subject_signer.expect("subject must be set"),
            payment_subjects: self.payment_subjects,
            audience: self.audience,
            not_before_ms: self.not_before_ms,
            expires_at_ms: self.expires_at_ms,
            delegation: self.delegation,
            constraints: self.constraints,
            metadata: self.metadata,
            signature: SignatureEnvelope {
                alg: crate::warrant::SigningAlgorithm::Ed25519,
                value: vec![],
            },
        };
        warrant.signature = issuer_keys.sign(warrant.canonical_unsigned_payload().as_bytes());
        warrant
    }
}
