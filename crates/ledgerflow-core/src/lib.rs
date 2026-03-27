//! Pure LedgerFlow authorization domain types.

#![allow(missing_docs)]

pub mod constraint;
pub mod error;
pub mod proof_builder;
pub mod typestate;
pub mod verification;
pub mod warrant;

pub use crate::{
    constraint::{Verify, verify_all as verify_all_constraints},
    error::{AuthorizationError, Result, WireError, WireResult},
    proof_builder::ProofBuilder,
    typestate::WarrantBuilder,
    verification::{
        Digestible, FullyVerified, ProofExt, VerificationPipeline, VerifiedProof, VerifiedWarrant,
        WarrantExt,
    },
    warrant::{
        AmountLimit, AssetRef, AudienceScope, AuthorizationContext, CborCodec, Constraint,
        DEFAULT_PROOF_FRESHNESS_MS, DelegationPolicy, MAX_WARRANT_CBOR_BYTES, MerchantConstraint,
        PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef, PeriodLimit, Proof,
        ResourceConstraint, SignatureEnvelope, SignerRef, SigningAlgorithm, SigningKeyPair,
        SponsorshipConstraint, ToolConstraint, VerifiedAuthorization, WARRANT_VERSION_V1, Warrant,
        WarrantMetadata, sha256_prefixed, verify_authorization,
    },
};
