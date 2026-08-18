//! Embedded in-process signer over a raw `SigningKeyPair`.
//!
//! Used by agents (PoP signing), control planes (warrant issuance), and the
//! demo CLI. This is the simplest [`WalletSigner`] implementation.

use ledgerflow_core::{SignatureEnvelope, SignerRef, SigningKeyPair};

use crate::{
    error::WalletError,
    signer::{
        SignDomain, SignPaymentRequest, SignRequest, SignResult, SignedPayment, WalletDescriptor,
        WalletSigner,
    },
};

/// In-process signer backed by a [`SigningKeyPair`].
#[derive(Clone, Debug)]
pub struct EmbeddedSigner {
    keypair: SigningKeyPair,
    descriptor: WalletDescriptor,
}

impl EmbeddedSigner {
    /// Creates an embedded signer from a key pair.
    #[must_use]
    pub fn new(keypair: SigningKeyPair) -> Self {
        let descriptor = WalletDescriptor {
            name: "embedded".to_string(),
            algorithms: vec![ledgerflow_core::SigningAlgorithm::Ed25519],
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        Self { keypair, descriptor }
    }

    /// Creates an embedded signer from raw Ed25519 secret bytes.
    #[must_use]
    pub fn from_bytes(secret: &[u8; 32]) -> Self {
        Self::new(SigningKeyPair::from_bytes(secret))
    }

    /// Returns the underlying key pair.
    #[must_use]
    pub const fn keypair(&self) -> &SigningKeyPair {
        &self.keypair
    }
}

impl WalletSigner for EmbeddedSigner {
    fn descriptor(&self) -> WalletDescriptor {
        self.descriptor.clone()
    }

    fn sign(&self, request: &SignRequest) -> Result<SignResult, WalletError> {
        // Verify the requested key matches (when a key was specified).
        if let Some(expected) = &request.key {
            let actual = self.keypair.signer_ref();
            if actual.public_key != expected.public_key {
                return Err(WalletError::NoMatchingKey);
            }
        }
        let signature: SignatureEnvelope = self.keypair.sign(&request.message);
        Ok(SignResult { signer: self.keypair.signer_ref(), signature })
    }

    fn keys(&self) -> Result<Vec<SignerRef>, WalletError> {
        Ok(vec![self.keypair.signer_ref()])
    }

    fn sign_payment(&self, request: &SignPaymentRequest) -> Result<SignedPayment, WalletError> {
        // Demo-grade transaction: deterministic canonical string + signature.
        let canonical = format!(
            "signed:{}:{}:{}:{}:{}",
            request.chain_id,
            request.asset,
            request.amount,
            request.payee,
            request.nonce.as_deref().unwrap_or("-")
        );
        let signature = self.keypair.sign(canonical.as_bytes());
        let raw_transaction = format!("{canonical}:{}", hex_encode(&signature.value));
        Ok(SignedPayment { signer: self.keypair.signer_ref(), raw_transaction, tx_hash: None })
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0F) as usize] as char);
    }
    encoded
}

impl SignDomain {
    /// Convenience: returns `true` for domains this signer supports.
    #[must_use]
    pub const fn supported_by_embedded(self) -> bool {
        matches!(self, Self::Warrant | Self::Proof | Self::Approval | Self::Payment)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn sign_domains_are_distinct_and_prefixed() {
        assert_eq!(SignDomain::Warrant.as_domain_bytes(), b"ledgerflow-wallet-warrant");
        assert_eq!(SignDomain::Proof.as_domain_bytes(), b"ledgerflow-wallet-proof");
        assert_eq!(SignDomain::Approval.as_domain_bytes(), b"ledgerflow-wallet-approval");
        assert_eq!(SignDomain::Payment.as_domain_bytes(), b"ledgerflow-wallet-payment");
        assert_ne!(SignDomain::Warrant.as_domain_bytes(), SignDomain::Proof.as_domain_bytes());
        assert_ne!(SignDomain::Proof.as_domain_bytes(), SignDomain::Approval.as_domain_bytes());
        assert_ne!(SignDomain::Approval.as_domain_bytes(), SignDomain::Payment.as_domain_bytes());
    }

    #[test]
    fn all_domains_are_supported_by_embedded() {
        for domain in
            [SignDomain::Warrant, SignDomain::Proof, SignDomain::Approval, SignDomain::Payment]
        {
            assert!(domain.supported_by_embedded(), "{domain:?} should be supported");
        }
    }

    #[test]
    fn hex_encode_is_lowercase_and_deterministic() {
        assert_eq!(hex_encode(&[0xAB, 0xCD]), "abcd");
        assert_eq!(hex_encode(&[0, 255]), "00ff");
        assert_eq!(hex_encode(&[1, 2, 3]), "010203");
        assert_eq!(hex_encode(&[]), "");
    }
}
