//! Wallet integration errors.

use thiserror::Error;

use crate::signer::SignDomain;

/// Errors surfaced by wallet integrations.
#[derive(Debug, Error)]
pub enum WalletError {
    #[error("the wallet does not support the signing domain {0:?}")]
    UnsupportedDomain(SignDomain),
    #[error("the wallet has no available key matching the request")]
    NoMatchingKey,
    #[error("the wallet rejected the signing request: {0}")]
    Rejected(String),
    #[error("the wallet is unreachable: {0}")]
    Unreachable(String),
    #[error("transport error: {0}")]
    Transport(String),
    #[error("invalid JSON-RPC payload: {0}")]
    InvalidPayload(String),
}
