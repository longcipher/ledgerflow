//! The [`WalletSigner`] capability interface.

use ledgerflow_core::{SignatureEnvelope, SignerRef, SigningAlgorithm};

use crate::error::WalletError;

/// Wallet identifier.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WalletDescriptor {
    /// Wallet name / URI (e.g. `embedded`, `local-rpc`, `walletconnect:...`).
    pub name: String,
    /// Signing algorithms the wallet supports.
    pub algorithms: Vec<SigningAlgorithm>,
    /// Wallet software version.
    pub version: String,
}

/// Signing domain (domain separation; prevents cross-purpose replay).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SignDomain {
    /// Warrant issuance (control plane / human key).
    Warrant,
    /// Proof-of-possession (agent key).
    Proof,
    /// m-of-n approval.
    Approval,
    /// Onchain payment transaction.
    Payment,
}

impl SignDomain {
    /// Domain-separation prefix bytes for this domain.
    #[must_use]
    pub const fn as_domain_bytes(self) -> &'static [u8] {
        match self {
            Self::Warrant => b"ledgerflow-wallet-warrant",
            Self::Proof => b"ledgerflow-wallet-proof",
            Self::Approval => b"ledgerflow-wallet-approval",
            Self::Payment => b"ledgerflow-wallet-payment",
        }
    }
}

/// A signing request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignRequest {
    pub domain: SignDomain,
    /// Message to sign (already domain-separated by the caller where required).
    pub message: Vec<u8>,
    /// Optional key selection (by public key or key id).
    pub key: Option<SignerRef>,
}

/// A signing result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignResult {
    pub signer: SignerRef,
    pub signature: SignatureEnvelope,
}

/// Onchain payment signing request (used by rail schemes).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignPaymentRequest {
    /// CAIP-2 chain id.
    pub chain_id: String,
    /// CAIP-19 asset id.
    pub asset: String,
    /// Amount in base units.
    pub amount: u128,
    /// Payee address.
    pub payee: String,
    pub nonce: Option<String>,
}

/// Onchain payment signing result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedPayment {
    pub signer: SignerRef,
    /// Signed raw transaction (hex/base58 string).
    pub raw_transaction: String,
    pub tx_hash: Option<String>,
}

/// Wallet capability interface.
///
/// Implementations MUST be `Send + Sync`. The interface is synchronous: the
/// underlying operations are local signing or short-lived transport calls.
pub trait WalletSigner: Send + Sync {
    fn descriptor(&self) -> WalletDescriptor;

    /// Signs an arbitrary message.
    fn sign(&self, request: &SignRequest) -> Result<SignResult, WalletError>;

    /// Lists the keys available in this wallet.
    fn keys(&self) -> Result<Vec<SignerRef>, WalletError>;

    /// Signs an onchain payment transaction.
    fn sign_payment(&self, request: &SignPaymentRequest) -> Result<SignedPayment, WalletError>;
}
