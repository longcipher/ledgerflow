//! Transport bindings for LedgerFlow authorization.
//!
//! This crate binds the transport-agnostic `ledgerflow-core` authorization
//! types to concrete protocols:
//!
//! - [`x402`]: x402 v2 extensions (challenge + payment payload).
//! - [`mpp`]: MPP Payment HTTP authentication scheme parameters.
//! - [`middleware`]: merchant-side verification (trust anchor, revocation, replay, approvals).
//! - [`replay`]: nonce replay protection and payment-id idempotency.
//! - [`carrier`]: transport carrier size policy.

#![allow(missing_docs)]

pub mod carrier;
pub mod error;
pub mod middleware;
pub mod mpp;
pub mod replay;
pub mod wire;
pub mod x402;

pub use crate::{
    carrier::{LedgerFlowCarrier, MAX_HEADER_CBOR_BYTES},
    error::ProtocolError,
    middleware::{
        InMemoryWarrantRepository, MerchantVerificationError, MerchantVerificationOutcome,
        MerchantVerifier, WarrantRepository,
    },
    mpp::{
        LEDGERFLOW_PARAM, SlimAuthorization, decode_authorization_param, decode_challenge_param,
        encode_authorization_param, encode_challenge_param,
    },
    replay::{InMemoryReplayStore, NonceClaim, ReplayConflict, ReplayFingerprint, ReplayStore},
    x402::{
        AcceptedQuote, HttpRequest, LEDGERFLOW_EXTENSION_VERSION, LedgerFlowAuthorizationExtension,
        LedgerFlowChallenge, MAX_LEDGERFLOW_EXTENSION_BYTES, PaymentPayload, PaymentPayloadSeed,
        PaymentRequiredResponse, build_payment_payload, canonical_accepted_hash,
        canonical_request_hash, merchant_payment_required,
    },
};
