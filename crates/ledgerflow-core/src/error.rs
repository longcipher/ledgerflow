//! Error types for LedgerFlow core authorization logic.

use thiserror::Error;

use crate::warrant::PaymentRail;

/// Convenient result alias for LedgerFlow core operations.
pub type Result<T> = std::result::Result<T, AuthorizationError>;

/// Convenient result alias for LedgerFlow wire-format operations.
pub type WireResult<T> = std::result::Result<T, WireError>;

/// Errors returned while validating a warrant and proof against request context.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorizationError {
    #[error("unsupported warrant version {0}")]
    UnsupportedVersion(u16),
    #[error("the warrant signature is invalid")]
    InvalidWarrantSignature,
    #[error("the proof signature is invalid")]
    InvalidProofSignature,
    #[error("merchant `{merchant_id}` is not allowed by the warrant")]
    MerchantNotAllowed { merchant_id: String },
    #[error("tool `{tool_name}` is not allowed by the warrant")]
    ToolNotAllowed { tool_name: String },
    #[error("model provider `{model_provider}` is not allowed by the warrant")]
    ModelProviderNotAllowed { model_provider: String },
    #[error("action label `{action_label}` is not allowed by the warrant")]
    ActionLabelNotAllowed { action_label: String },
    #[error("request method `{method}` is not allowed by the warrant")]
    HttpMethodNotAllowed { method: String },
    #[error("request path `{path}` is not allowed by the warrant")]
    ResourcePathNotAllowed { path: String },
    #[error("selected payment amount {amount} exceeds the warrant limit {limit}")]
    PaymentAmountExceeded { amount: u64, limit: u64 },
    #[error("asset `{asset}` is not allowed by the warrant")]
    AssetNotAllowed { asset: String },
    #[error("scheme `{scheme}` is not allowed by the warrant")]
    SchemeNotAllowed { scheme: String },
    #[error("rail `{rail}` is not allowed by the warrant")]
    RailNotAllowed { rail: PaymentRail },
    #[error("payee `{payee_id}` is not allowed by the warrant")]
    PayeeNotAllowed { payee_id: String },
    #[error("warrant is not yet valid at {now_ms}")]
    WarrantNotYetValid { now_ms: u64 },
    #[error("warrant expired at {expires_at_ms}")]
    WarrantExpired { expires_at_ms: u64 },
    #[error("challenge id did not match the current merchant challenge")]
    ChallengeMismatch,
    #[error("warrant digest did not match the presented warrant")]
    WarrantDigestMismatch,
    #[error("accepted quote binding did not match")]
    AcceptedHashMismatch,
    #[error("request binding did not match")]
    RequestHashMismatch,
    #[error("proof signer did not match the warrant subject signer")]
    SignerMismatch,
    #[error("payment subject `{subject}` is not allowed by the warrant")]
    PaymentSubjectNotAllowed { subject: String },
    #[error("proof is outside the freshness window")]
    ProofOutsideFreshnessWindow,
    #[error("delegation is not allowed for this warrant")]
    DelegationNotAllowed,
    #[error("presented delegation depth {presented} exceeds the allowed depth {allowed}")]
    DelegationDepthExceeded { presented: u8, allowed: u8 },
    #[error("sponsorship is not allowed for this warrant")]
    SponsorshipNotAllowed,
}

/// Errors returned while encoding or decoding LedgerFlow wire payloads.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WireError {
    #[error("payload size {size} exceeds the maximum supported size {max}")]
    PayloadTooLarge { size: usize, max: usize },
    #[error("failed to encode the payload as CBOR: {0}")]
    Serialization(String),
    #[error("failed to decode the payload from CBOR: {0}")]
    Deserialization(String),
}
