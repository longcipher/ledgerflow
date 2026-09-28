//! Signing types for warrants and proofs.

use std::fmt::{self, Display};

use ed25519_dalek::{Signature, Signer as _, VerifyingKey};
use serde::{Deserialize, Serialize};

use super::hex_encode;

/// Supported signer algorithms for warrants and proofs.
///
/// The EVM-family variants enable native wallet integration and EIP-8004
/// interop:
///
/// - [`SigningAlgorithm::Secp256k1`]: strict (low-s) ECDSA over `SHA-256(message)`.
///   `SignerRef::public_key` is the 33-byte compressed SEC1 encoding.
/// - [`SigningAlgorithm::EthPersonalSign`]: EIP-191 `personal_sign` semantics. The verification
///   preimage is `keccak256("\x19Ethereum Signed Message:\n" + len(message) + message)`.
///   `SignerRef::public_key` is either the 33-byte compressed pubkey or a 20-byte Ethereum address
///   claim.
/// - [`SigningAlgorithm::EthTypedData`]: EIP-712 semantics. The `message` passed to verification
///   MUST already be the 32-byte typed-data digest (`keccak256(domainSeparator || structHash)`).
///   Key conventions match [`SigningAlgorithm::EthPersonalSign`].
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[non_exhaustive]
pub enum SigningAlgorithm {
    #[default]
    Ed25519,
    Secp256k1,
    EthPersonalSign,
    EthTypedData,
}

impl SigningAlgorithm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::Secp256k1 => "secp256k1",
            Self::EthPersonalSign => "eth_personal_sign",
            Self::EthTypedData => "eth_typed_data",
        }
    }

    /// Returns `true` for the secp256k1/EVM family of algorithms.
    #[must_use]
    pub const fn is_secp256k1_family(self) -> bool {
        matches!(self, Self::Secp256k1 | Self::EthPersonalSign | Self::EthTypedData)
    }
}

impl Display for SigningAlgorithm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Public signer identity used for warrant issuance and proof verification.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SignerRef {
    pub alg: SigningAlgorithm,
    #[serde(with = "serde_bytes")]
    pub public_key: Vec<u8>,
    pub key_id: Option<String>,
}

impl SignerRef {
    #[must_use]
    pub const fn new(alg: SigningAlgorithm, public_key: Vec<u8>) -> Self {
        Self { alg, public_key, key_id: None }
    }

    #[must_use]
    pub fn with_key_id(mut self, key_id: String) -> Self {
        self.key_id = Some(key_id);
        self
    }
}

/// Ed25519 signing key pair for warrant issuance, proof creation, and approvals.
#[derive(Clone)]
pub struct SigningKeyPair {
    signing_key: ed25519_dalek::SigningKey,
}

impl fmt::Debug for SigningKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKeyPair")
            .field("public_key_hex", &hex_encode(&self.signing_key.verifying_key().to_bytes()))
            .finish()
    }
}

impl SigningKeyPair {
    /// Creates a key pair from raw Ed25519 secret key bytes.
    #[must_use]
    pub fn from_bytes(secret_key: &[u8; 32]) -> Self {
        Self { signing_key: ed25519_dalek::SigningKey::from_bytes(secret_key) }
    }

    /// Returns the public key bytes.
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Creates a `SignerRef` from this key pair.
    #[must_use]
    pub fn signer_ref(&self) -> SignerRef {
        SignerRef::new(SigningAlgorithm::Ed25519, self.public_key_bytes().to_vec())
    }

    /// Signs a message, producing a [`SignatureEnvelope`].
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> SignatureEnvelope {
        let signature = self.signing_key.sign(message);
        SignatureEnvelope { alg: SigningAlgorithm::Ed25519, value: signature.to_bytes().to_vec() }
    }
}

/// Signature container for warrants, proofs, and approvals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureEnvelope {
    pub alg: SigningAlgorithm,
    pub value: Vec<u8>,
}

impl SignatureEnvelope {
    /// Verifies this signature against a signer and message using **strict**
    /// verification semantics for the envelope's algorithm.
    ///
    /// - Ed25519: `verify_strict` (rejects non-canonical signatures).
    /// - Secp256k1: low-s ECDSA over `SHA-256(message)`.
    /// - EthPersonalSign / EthTypedData: EIP-191 recovery with low-s enforcement; see
    ///   [`SigningAlgorithm`] for key conventions.
    pub fn verify_strict(&self, signer: &SignerRef, message: &[u8]) -> bool {
        if self.alg != signer.alg {
            return false;
        }
        match self.alg {
            SigningAlgorithm::Ed25519 => self.verify_ed25519_strict(signer, message),
            SigningAlgorithm::Secp256k1 |
            SigningAlgorithm::EthPersonalSign |
            SigningAlgorithm::EthTypedData => {
                crate::crypto::verify_secp256k1_family(self.alg, signer, message, &self.value)
            }
        }
    }

    /// The original strict Ed25519 verification path.
    fn verify_ed25519_strict(&self, signer: &SignerRef, message: &[u8]) -> bool {
        let Ok(pk_array) = <&[u8; 32]>::try_from(signer.public_key.as_slice()) else {
            return false;
        };
        let Ok(sig_array) = <&[u8; 64]>::try_from(self.value.as_slice()) else {
            return false;
        };
        let Ok(verifying_key) = VerifyingKey::from_bytes(pk_array) else {
            return false;
        };
        let signature = Signature::from_bytes(sig_array);
        verifying_key.verify_strict(message, &signature).is_ok()
    }
}

impl Serialize for SignatureEnvelope {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("SignatureEnvelope", 2)?;
        state.serialize_field("alg", &self.alg)?;
        state.serialize_field("value", &serde_bytes::ByteBuf::from(self.value.clone()))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SignatureEnvelope {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Inner {
            alg: SigningAlgorithm,
            value: serde_bytes::ByteBuf,
        }
        let inner = Inner::deserialize(deserializer)?;
        Ok(Self { alg: inner.alg, value: inner.value.into_vec() })
    }
}
