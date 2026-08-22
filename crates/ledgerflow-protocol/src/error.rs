//! Errors for the LedgerFlow protocol binding layer.

use ledgerflow_core::AuthorizationError;
use thiserror::Error;

/// Errors surfaced by the protocol binding layer.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("payload size {size} exceeds the maximum supported size {max}")]
    PayloadTooLarge { size: usize, max: usize },
    #[error("failed to encode the payload as CBOR: {0}")]
    Serialization(String),
    #[error("failed to decode the payload from CBOR: {0}")]
    Deserialization(String),
    #[error("invalid base64: {0}")]
    InvalidBase64(String),
    #[error("the warrant chain must not be empty")]
    EmptyChain,
    #[error("the carrier cannot carry {size} bytes (limit {max})")]
    CarrierTooLarge { size: usize, max: usize },
    #[error("invalid LedgerFlow credential: {0}")]
    VcInvalid(String),
    #[error(transparent)]
    Core(#[from] AuthorizationError),
}
