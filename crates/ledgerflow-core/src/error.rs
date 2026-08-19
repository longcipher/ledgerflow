//! Error types for LedgerFlow core authorization logic.

use thiserror::Error;

/// Convenient result alias for LedgerFlow core operations.
pub type Result<T> = std::result::Result<T, AuthorizationError>;

/// Convenient result alias for LedgerFlow wire-format operations.
pub type WireResult<T> = std::result::Result<T, WireError>;

/// Errors returned while validating a warrant chain and proof against request
/// context.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorizationError {
    #[error("unsupported warrant version {0}")]
    UnsupportedVersion(u8),
    #[error("the warrant signature is invalid")]
    InvalidWarrantSignature,
    #[error("the proof signature is invalid")]
    InvalidProofSignature,
    #[error("merchant `{merchant_id}` is not allowed by the warrant")]
    MerchantNotAllowed { merchant_id: String },
    #[error("resource `{method} {path}` is not allowed by the warrant")]
    ResourceNotAllowed { method: String, path: String },
    #[error("tool `{tool_name}` is not allowed by the warrant")]
    ToolNotAllowed { tool_name: String },
    #[error("the payment is not allowed by the warrant")]
    PaymentNotAllowed,
    #[error("selected payment amount {amount} exceeds the warrant limit {limit}")]
    PaymentAmountExceeded { amount: u128, limit: u128 },
    #[error("warrant is not yet valid at {issued_at}")]
    WarrantNotYetValid { issued_at: u64 },
    #[error("warrant expired at {expires_at}")]
    WarrantExpired { expires_at: u64 },
    #[error("challenge id did not match the current merchant challenge")]
    ChallengeMismatch,
    #[error("warrant digest did not match the presented warrant")]
    WarrantDigestMismatch,
    #[error("accepted quote binding did not match")]
    AcceptedHashMismatch,
    #[error("payment payload digest binding did not match")]
    PaymentPayloadDigestMismatch,
    #[error("request binding did not match")]
    RequestHashMismatch,
    #[error("proof signer did not match the warrant holder")]
    SignerMismatch,
    #[error("payment subject `{subject}` is not allowed by the warrant")]
    PaymentSubjectNotAllowed { subject: String },
    #[error("proof is outside the freshness window (created_at={created_at_ms}, now={now_ms})")]
    ProofOutsideFreshnessWindow { created_at_ms: u64, now_ms: u64 },
    #[error("delegation is not allowed for this warrant")]
    DelegationNotAllowed,
    #[error("presented delegation depth {presented} exceeds the allowed depth {allowed}")]
    DelegationDepthExceeded { presented: u8, allowed: u8 },
    #[error("the warrant chain is empty")]
    EmptyChain,
    #[error("warrant `{warrant_id}` appears more than once in the chain (cycle detected)")]
    DuplicateWarrantInChain { warrant_id: String },
    #[error("child issuer does not match parent holder (I1)")]
    DelegationAuthorityMismatch,
    #[error("child depth {actual} does not equal parent depth + 1 (expected {expected}) (I2)")]
    DepthMismatch { expected: u32, actual: u32 },
    #[error("child expires later than parent (I3)")]
    TtlMonotonicityViolation,
    #[error("child amount cap exceeds parent cap (I7)")]
    AmountMonotonicityViolation,
    #[error("child warrant is missing its parent hash (I5)")]
    MissingParentHash,
    #[error("child parent hash does not match parent payload (I5)")]
    ParentHashMismatch,
    #[error("the root issuer is not trusted")]
    UntrustedIssuer { key_id: String },
    #[error("child constraint violates monotonic attenuation on `{dimension}`: {detail}")]
    AttenuationViolation { dimension: String, detail: String },
    #[error("the warrant has been revoked")]
    WarrantRevoked,
    #[error("the holder key has been revoked")]
    HolderRevoked,
    #[error("this action requires human approval")]
    ApprovalRequired,
    #[error("insufficient approvals: got {got}, need {need}")]
    InsufficientApprovals { got: u32, need: u32 },
    #[error("approval request hash does not match the payment request")]
    ApprovalRequestMismatch,
    #[error("approval has expired")]
    ApprovalExpired,
    #[error("approver is not in the required approvers list")]
    ApproverNotAllowed,
    #[error("the approval signature is invalid")]
    InvalidApprovalSignature,
    #[error("the approvals digest does not match the PoP binding")]
    ApprovalsDigestMismatch,
    #[error("unknown warrant extension key `{key}` (extensions are frozen in v1)")]
    UnknownExtension { key: String },
    #[error(
        "SRL version {presented} does not advance the applied version {applied} (anti-rollback)"
    )]
    SrlVersionRegression { presented: u64, applied: u64 },
    #[error("the SRL signature is invalid")]
    InvalidSrlSignature,
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
    #[error("unknown warrant extension key `{key}` (extensions are frozen in v1)")]
    UnknownExtension { key: String },
}
