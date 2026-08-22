//! ERC-1271 smart-contract wallet signature verification seam.
//!
//! Contract wallets (ERC-4337 accounts, Safes, ...) cannot produce raw ECDSA
//! signatures. Instead they expose `isValidSignature(bytes32,bytes)` on-chain,
//! returning the magic value [`ERC_1271_MAGIC_VALUE`]. The core crate stays
//! I/O-free: the on-chain call is expressed as the
//! [`ContractSignatureVerifier`] trait and implemented by downstream crates
//! (Facilitator / server) over their chain RPC.
//!
//! Account convention: a signer whose `SignerRef::public_key` is a **20-byte
//! value** with a secp256k1-family algorithm is interpreted as an *on-chain
//! account address claim* rather than a public key (see
//! [`crate::crypto`]). When direct cryptographic verification fails for such
//! a signer and a verifier is available, verification falls back to the
//! contract-wallet check.
//!
//! Hash convention: the `bytes32 hash` handed to the contract is
//! `keccak256(domain-separated LedgerFlow message)`, keeping the account-level
//! digest unambiguous across transports.

use crate::{
    crypto::keccak256,
    warrant::{SignatureEnvelope, SignerRef},
};

/// The ERC-1271 `isValidSignature` success magic value (`0x1626ba7e`).
pub const ERC_1271_MAGIC_VALUE: [u8; 4] = [0x16, 0x26, 0xBA, 0x7E];

/// On-chain ERC-1271 verification seam.
///
/// Implementations perform the `isValidSignature(hash, signature)` call
/// against the account at `account.public_key` (a 20-byte address) and return
/// whether the returned bytes start with [`ERC_1271_MAGIC_VALUE`].
pub trait ContractSignatureVerifier: Send + Sync {
    /// Returns `true` when the on-chain account confirms the signature over
    /// `hash`.
    fn is_valid_signature(&self, account: &SignerRef, hash: [u8; 32], signature: &[u8]) -> bool;
}

/// Returns `true` when this signer reference is shaped like an on-chain
/// account claim (20-byte key in the secp256k1 family).
#[must_use]
pub const fn is_contract_account_claim(signer: &SignerRef) -> bool {
    signer.public_key.len() == 20 && signer.alg.is_secp256k1_family()
}

/// Verifies an envelope against a signer, falling back to the ERC-1271 seam
/// when the signer is a contract-account claim and direct verification fails.
///
/// Verification order:
///
/// 1. Direct strict verification
///    ([`SignatureEnvelope::verify_strict`](crate::warrant::SignatureEnvelope::verify_strict)).
/// 2. If that fails AND the signer is a 20-byte secp256k1-family claim AND a verifier is supplied:
///    `verifier.is_valid_signature(signer, keccak256(message), envelope.value)`.
///
/// Anything else fails closed.
pub fn verify_signature_with(
    envelope: &SignatureEnvelope,
    signer: &SignerRef,
    message: &[u8],
    contract_verifier: Option<&dyn ContractSignatureVerifier>,
) -> bool {
    if envelope.verify_strict(signer, message) {
        return true;
    }
    let Some(verifier) = contract_verifier else {
        return false;
    };
    if !is_contract_account_claim(signer) {
        return false;
    }
    verifier.is_valid_signature(signer, keccak256(message), &envelope.value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::{Secp256k1KeyPair, SigningAlgorithm};

    struct MockVerifier {
        accept: bool,
        calls: AtomicU32,
    }

    impl MockVerifier {
        fn new(accept: bool) -> Self {
            Self { accept, calls: AtomicU32::new(0) }
        }
    }

    impl ContractSignatureVerifier for MockVerifier {
        fn is_valid_signature(
            &self,
            _account: &SignerRef,
            _hash: [u8; 32],
            _signature: &[u8],
        ) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.accept
        }
    }

    fn eth_keys() -> Secp256k1KeyPair {
        Secp256k1KeyPair::from_bytes(&[0x21; 32]).expect("valid key")
    }

    #[test]
    fn magic_value_matches_erc1271() {
        assert_eq!(ERC_1271_MAGIC_VALUE, [0x16, 0x26, 0xBA, 0x7E]);
    }

    #[test]
    fn direct_pass_short_circuits_without_fallback() {
        let keys = eth_keys();
        let signer = keys.signer_ref(SigningAlgorithm::EthPersonalSign);
        let envelope = keys.sign_eth_personal(b"message");
        let mock = MockVerifier::new(false);
        assert!(verify_signature_with(&envelope, &signer, b"message", Some(&mock)));
        assert_eq!(mock.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn fallback_invoked_for_address_claims() {
        let keys = eth_keys();
        // Address-claim signer whose key cannot directly verify (different
        // message than signed): forces the fallback path.
        let claim = SignerRef::new(SigningAlgorithm::EthPersonalSign, vec![0xAB; 20]);
        let envelope = keys.sign_eth_personal(b"signed by someone else");
        let accepting = MockVerifier::new(true);
        assert!(verify_signature_with(&envelope, &claim, b"any", Some(&accepting)));
        assert_eq!(accepting.calls.load(Ordering::SeqCst), 1);

        let rejecting = MockVerifier::new(false);
        assert!(!verify_signature_with(&envelope, &claim, b"any", Some(&rejecting)));
        assert_eq!(rejecting.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fallback_skipped_without_verifier_or_wrong_shape() {
        let keys = eth_keys();
        let claim = SignerRef::new(SigningAlgorithm::EthPersonalSign, vec![0xAB; 20]);
        let envelope = keys.sign_eth_personal(b"m");
        // No verifier supplied.
        assert!(!verify_signature_with(&envelope, &claim, b"m", None));
        // Full-pubkey signer failing direct verification never falls back.
        let full = keys.signer_ref(SigningAlgorithm::EthPersonalSign);
        assert!(!verify_signature_with(&envelope, &full, b"other", Some(&MockVerifier::new(true))));
        // Ed25519 signers are outside the contract-account family.
        let ed = crate::warrant::SigningKeyPair::from_bytes(&[0x31; 32]).signer_ref();
        let ed_envelope = crate::warrant::SigningKeyPair::from_bytes(&[0x32; 32]).sign(b"x");
        assert!(!verify_signature_with(&ed_envelope, &ed, b"x", Some(&MockVerifier::new(true))));
    }

    #[test]
    fn contract_account_shape_detection() {
        let keys = eth_keys();
        let claim = SignerRef::new(SigningAlgorithm::Secp256k1, vec![0xCD; 20]);
        assert!(is_contract_account_claim(&claim));
        assert!(!is_contract_account_claim(&keys.signer_ref(SigningAlgorithm::Secp256k1)));
        let ed20 = crate::warrant::SigningKeyPair::from_bytes(&[0x41; 32]).signer_ref();
        assert!(!is_contract_account_claim(&SignerRef {
            alg: SigningAlgorithm::Ed25519,
            public_key: vec![0_u8; 20],
            key_id: ed20.key_id.clone(),
        }));
    }
}
