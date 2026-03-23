//! Pure LedgerFlow authorization domain types.

#![allow(missing_docs)]

pub mod error;
pub mod warrant;

pub use crate::{
    error::{AuthorizationError, Result, WireError, WireResult},
    warrant::{
        AmountLimit, AssetRef, AudienceScope, AuthorizationContext, Constraint,
        DEFAULT_PROOF_FRESHNESS_MS, DelegationPolicy, MAX_WARRANT_CBOR_BYTES, MerchantConstraint,
        PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef, PeriodLimit, Proof,
        ResourceConstraint, SignatureEnvelope, SignerRef, SigningAlgorithm, SponsorshipConstraint,
        ToolConstraint, VerifiedAuthorization, WARRANT_VERSION_V1, Warrant, WarrantMetadata,
        sha256_prefixed, verify_authorization,
    },
};
