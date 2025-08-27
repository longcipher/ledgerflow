//! Common facilitator traits and per-chain implementations.

pub mod evm_facilirator;
pub mod sui_facilitator;

use crate::types::{SettleRequest, SettleResponse, VerifyRequest, VerifyResponse};

/// Error types that can occur during payment verification or settlement.
#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    /// Payment scheme does not match expected scheme
    #[error("Incompatible scheme: expected {expected}, got {actual}")]
    IncompatibleScheme { expected: String, actual: String },

    /// Payment network does not match expected network  
    #[error("Incompatible network: expected {expected}, got {actual}")]
    IncompatibleNetwork { expected: String, actual: String },

    /// Payment receivers do not match expected receivers
    #[error("Incompatible receivers: expected {expected}, got {actual}")]
    IncompatibleReceivers { expected: String, actual: String },

    /// Invalid signature provided
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    /// Invalid timing constraints
    #[error("Invalid timing: {0}")]
    InvalidTiming(String),

    /// Payment amount is insufficient
    #[error("Insufficient payment amount")]
    InsufficientAmount,

    /// Insufficient funds for payment
    #[error("Insufficient funds for payment")]
    InsufficientFunds,

    /// Unsupported network
    #[error("Unsupported network: {0}")]
    UnsupportedNetwork(String),

    /// Invalid contract call
    #[error("Invalid contract call: {0}")]
    InvalidContractCall(String),

    /// Invalid address format
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// Clock-related errors  
    #[error("Clock error: {0}")]
    ClockError(String),

    /// Sui blockchain errors
    #[error("Sui error: {0}")]
    SuiError(String),

    /// Intent signing errors
    #[error("Intent signing error: {0}")]
    IntentSigningError(String),

    /// Transaction execution errors
    #[error("Transaction execution error: {0}")]
    TransactionExecutionError(String),
}

/// The main trait for payment verification and settlement.
///
/// This trait abstracts over different blockchain implementations,
/// allowing the same x402 protocol to work across multiple chains.
#[async_trait::async_trait]
pub trait Facilitator: Send + Sync {
    /// Verify that a payment payload is valid according to the requirements.
    ///
    /// This method should:
    /// - Validate the signature
    /// - Check timing constraints
    /// - Verify the payment amount meets requirements
    /// - Ensure the payer has sufficient funds
    /// - Validate network compatibility
    ///
    /// Returns `Ok(VerifyResponse::Valid)` if verification succeeds,
    /// or `Ok(VerifyResponse::Invalid)` if verification fails for a known reason,
    /// or `Err(PaymentError)` for unexpected errors.
    async fn verify(&self, request: &VerifyRequest) -> Result<VerifyResponse, PaymentError>;

    /// Settle a payment on-chain.
    ///
    /// This method should:
    /// - First verify the payment (same checks as `verify`)
    /// - Submit the transaction to the blockchain
    /// - Wait for confirmation
    /// - Return the transaction hash and status
    ///
    /// Returns `Ok(SettleResponse)` with success status and transaction info,
    /// or `Err(PaymentError)` for any failures.
    async fn settle(&self, request: &SettleRequest) -> Result<SettleResponse, PaymentError>;

    /// Get the supported networks for this facilitator.
    fn supported_networks(&self) -> Vec<crate::types::Network>;
}
