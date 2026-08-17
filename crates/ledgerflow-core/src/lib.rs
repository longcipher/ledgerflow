//! Pure LedgerFlow authorization domain types.
//!
//! This crate is the stateless, transport-agnostic core of LedgerFlow. It
//! contains:
//!
//! - [`warrant`]: the signed capability token (Warrant) and its CBOR codec.
//! - [`chain`]: delegation-chain verification (invariants I1-I7).
//! - [`pop`]: proof-of-possession binding tuples.
//! - [`constraint`]: stateless, decidable constraints.
//! - [`approval`]: m-of-n human approval gates.
//! - [`trust`]: trusted-issuer anchors.
//! - [`revocation`]: the `RevocationCheck` seam (implemented out of crate).
//! - [`verification`]: the type-state verification pipeline.
//! - [`typestate`] / [`proof_builder`]: compile-time-safe builders.
//!
//! The crate performs **no I/O, no networking, no storage**. Online checks
//! (revocation, budget accounting) are expressed as traits implemented by
//! downstream crates.

#![allow(missing_docs)]

pub mod approval;
pub mod chain;
pub mod constraint;
pub mod error;
pub mod pop;
pub mod proof_builder;
pub mod revocation;
pub mod trust;
pub mod typestate;
pub mod verification;
pub mod warrant;

pub use crate::{
    approval::{
        ApprovalGate, ApprovalVerification, SignedApproval, verify_approvals,
        verify_approval_threshold,
    },
    chain::{VerifiedChainAuthorization, WarrantChain, verify_chain, verify_link},
    constraint::{
        AuthorizationContext, Constraint, MerchantConstraint, PaymentConstraint,
        ResourceConstraint, ToolConstraint, Verify, verify_all as verify_all_constraints,
    },
    error::{AuthorizationError, Result, WireError, WireResult},
    pop::{POP_SIGN_DOMAIN, PopProof, PopTuple, verify_freshness},
    proof_builder::ProofBuilder,
    revocation::{InMemoryRevocationCheck, RevocationCheck, RevocationDecision},
    trust::{TrustedIssuer, TrustedIssuers},
    typestate::{DelegatedWarrantBuilder, WarrantBuilder},
    verification::{
        AuthorizationInput, ToolArguments, VerifiedAuthorization, WarrantExt, verify_authorization,
    },
    warrant::{
        AssetRef, CborCodec, DEFAULT_CHALLENGE_TTL_MS, DEFAULT_CLOCK_SKEW_MS, DEFAULT_MAX_DEPTH,
        DEFAULT_PROOF_FRESHNESS_MS, DEFAULT_WARRANT_TTL_SECS, MAX_DELEGATION_DEPTH,
        MAX_WARRANT_CBOR_BYTES, MAX_WARRANT_TTL_SECS, PaymentRail, PaymentSubjectKind,
        PaymentSubjectRef, SignatureEnvelope, SignerRef, SigningAlgorithm, SigningKeyPair,
        WARRANT_SIGN_DOMAIN, WARRANT_VERSION_V1, Warrant, WarrantMetadata, generate_warrant_id,
        sha256_prefixed,
    },
};

#[cfg(test)]
pub(crate) mod test_support;
