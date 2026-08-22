//! secp256k1 / Ethereum-family signature support.
//!
//! This module implements the strict verification and signing primitives for
//! the secp256k1 family of [`SigningAlgorithm`] variants:
//!
//! - **Secp256k1**: low-s ECDSA over `SHA-256(message)`. The secp256k1 backend normalizes/enforces
//!   low-`s`, so malleable (high-`s`) signatures are rejected — mirroring the Ed25519 strict
//!   semantics used elsewhere in the protocol.
//! - **EthPersonalSign**: EIP-191 `personal_sign`. The signed preimage is `keccak256("\x19Ethereum
//!   Signed Message:\n" + decimal_len + message)`; the signer is identified by recovering the
//!   public key from the `(r, s, recovery_id)` triple.
//! - **EthTypedData**: EIP-712. Callers pass the already-computed 32-byte typed-data digest
//!   (`keccak256(domainSeparator || structHash)`) as the `message`; recovery proceeds identically.
//!
//! Key conventions (see [`crate::warrant::SignerRef`]):
//!
//! - 33-byte compressed SEC1 public keys are compared byte-for-byte against the recovered key.
//! - 20-byte public keys are interpreted as **Ethereum address claims**; the recovered key's
//!   derived address (`keccak256(X||Y)[12..32]`) must match.

use k256::{
    ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey, signature::hazmat::PrehashVerifier},
    elliptic_curve::sec1::ToSec1Point as _,
};
use sha2::{Digest as _, Sha256};
use tiny_keccak::{Hasher, Keccak};

use crate::warrant::{SignatureEnvelope, SignerRef, SigningAlgorithm};

/// The secp256k1 group order `n`
/// (`FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141`).
#[cfg(test)]
const SECP256K1_ORDER: [u8; 32] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
    0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41,
];

/// Half the group order `(n - 1) / 2`; signatures with `s > n/2` are
/// non-canonical (malleable) and rejected by strict paths.
const SECP256K1_HALF_ORDER: [u8; 32] = [
    0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46, 0x68, 0x1B, 0x20, 0xA0,
];

/// Computes Keccak-256 (the original Ethereum padding, NOT NIST SHA3-256).
#[must_use]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(data);
    let mut out = [0_u8; 32];
    hasher.finalize(&mut out);
    out
}

/// Computes the EIP-191 personal-message hash:
/// `keccak256("\x19Ethereum Signed Message:\n" + len(message) + message)`.
#[must_use]
pub fn eip191_message_hash(message: &[u8]) -> [u8; 32] {
    let mut prefixed = Vec::with_capacity(28 + message.len());
    prefixed.extend_from_slice(b"\x19Ethereum Signed Message:\n");
    prefixed.extend_from_slice(message.len().to_string().as_bytes());
    prefixed.extend_from_slice(message);
    keccak256(&prefixed)
}

/// Computes the EIP-191 hash of a fixed 32-byte digest:
/// `keccak256("\x19Ethereum Signed Message:\n32" || digest)`.
///
/// This mirrors Solidity's `ECDSA.toEthSignedMessageHash(bytes32)` and is the
/// convention EIP-8004 `FeedbackAuth` signatures cover.
#[must_use]
pub fn eip191_hash_of_bytes32(digest: &[u8; 32]) -> [u8; 32] {
    let mut prefixed = Vec::with_capacity(28 + 32);
    prefixed.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
    prefixed.extend_from_slice(digest);
    keccak256(&prefixed)
}

/// Derives the 20-byte Ethereum address of a compressed SEC1 public key:
/// the last 20 bytes of `keccak256(X || Y)` over the uncompressed 64-byte
/// coordinate pair.
#[must_use]
pub fn ethereum_address_from_compressed_pubkey(compressed: &[u8; 33]) -> Option<[u8; 20]> {
    let public_key = k256::PublicKey::from_sec1_bytes(compressed).ok()?;
    let point = public_key.to_sec1_point(false);
    let coordinates = point.as_bytes().get(1..65)?;
    let digest = keccak256(coordinates);
    let mut address = [0_u8; 20];
    address.copy_from_slice(digest.get(12..32)?);
    Some(address)
}

/// Returns `true` when `s` is within the canonical low range
/// (`s <= n/2`).
fn is_low_s(s: &[u8; 32]) -> bool {
    s <= &SECP256K1_HALF_ORDER
}

/// Splits a 65-byte recoverable signature into `(r, s, recovery_id)` with v
/// normalized from {27, 28} to {0, 1}.
fn split_recoverable(signature: &[u8]) -> Option<([u8; 32], [u8; 32], RecoveryId)> {
    if signature.len() != 65 {
        return None;
    }
    let mut r = [0_u8; 32];
    let mut s = [0_u8; 32];
    r.copy_from_slice(signature.get(..32)?);
    s.copy_from_slice(signature.get(32..64)?);
    let v_byte = *signature.last()?;
    let rec_id = match v_byte {
        0 | 1 => RecoveryId::from_byte(v_byte)?,
        27 | 28 => RecoveryId::from_byte(v_byte - 27)?,
        _ => return None,
    };
    Some((r, s, rec_id))
}

/// Recovers the compressed SEC1 public key from a recoverable signature over
/// a 32-byte prehash. Rejects high-`s` (non-canonical) signatures outright.
fn recover_compressed(prehash: &[u8; 32], signature: &[u8]) -> Option<[u8; 33]> {
    let (r_bytes, s_bytes, rec_id) = split_recoverable(signature)?;
    if !is_low_s(&s_bytes) {
        return None;
    }
    let mut raw = [0_u8; 64];
    raw[..32].copy_from_slice(&r_bytes);
    raw[32..].copy_from_slice(&s_bytes);
    let signature = Signature::from_slice(&raw).ok()?;
    let recovered = VerifyingKey::recover_from_prehash(prehash, &signature, rec_id).ok()?;
    let encoded = recovered.to_sec1_point(true);
    let mut compressed = [0_u8; 33];
    compressed.copy_from_slice(encoded.as_bytes().get(..33)?);
    Some(compressed)
}

/// Matches a recovered compressed public key against a signer reference.
///
/// A 33-byte claim compares the compressed key directly; a 20-byte claim is
/// an Ethereum address comparison.
fn recovered_key_matches(signer: &SignerRef, recovered: &[u8; 33]) -> bool {
    match signer.public_key.len() {
        33 => signer.public_key.as_slice() == recovered,
        20 => {
            let Ok(expected) = <&[u8; 20]>::try_from(signer.public_key.as_slice()) else {
                return false;
            };
            let Some(actual) = ethereum_address_from_compressed_pubkey(recovered) else {
                return false;
            };
            expected == &actual
        }
        _ => false,
    }
}

/// Verifies a secp256k1-family signature according to the algorithm's
/// preimage convention. Used by
/// [`SignatureEnvelope::verify_strict`](crate::warrant::SignatureEnvelope::verify_strict).
pub(crate) fn verify_secp256k1_family(
    alg: SigningAlgorithm,
    signer: &SignerRef,
    message: &[u8],
    signature_value: &[u8],
) -> bool {
    match alg {
        SigningAlgorithm::Secp256k1 => {
            // Strict path: verify directly against the claimed compressed key
            // (low-s enforced by the backend via NORMALIZE_S).
            if signer.public_key.len() != 33 {
                return false;
            }
            let Ok(verifying_key) = VerifyingKey::from_sec1_bytes(&signer.public_key) else {
                return false;
            };
            let Ok(signature) = Signature::from_slice(signature_value) else {
                return false;
            };
            let digest = Sha256::digest(message);
            verifying_key.verify_prehash(&digest, &signature).is_ok()
        }
        SigningAlgorithm::EthPersonalSign => {
            let digest = eip191_message_hash(message);
            verify_recoverable(signer, &digest, signature_value)
        }
        SigningAlgorithm::EthTypedData => {
            let Ok(digest) = <&[u8; 32]>::try_from(message) else {
                return false;
            };
            verify_recoverable(signer, digest, signature_value)
        }
        SigningAlgorithm::Ed25519 => false,
    }
}

/// Recovery-based verification shared by EthPersonalSign / EthTypedData.
fn verify_recoverable(signer: &SignerRef, digest: &[u8; 32], signature_value: &[u8]) -> bool {
    let Some(recovered) = recover_compressed(digest, signature_value) else {
        return false;
    };
    recovered_key_matches(signer, &recovered)
}

/// secp256k1 signing key pair for EVM-native warrant issuance, proofs, and
/// approvals.
#[derive(Clone)]
pub struct Secp256k1KeyPair {
    signing_key: SigningKey,
}

impl std::fmt::Debug for Secp256k1KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Only expose the address; never the secret key material.
        f.debug_struct("Secp256k1KeyPair")
            .field("address", &crate::warrant::hex_encode_bytes(&self.ethereum_address()))
            .finish()
    }
}

impl Secp256k1KeyPair {
    /// Creates a key pair from raw secret key bytes.
    ///
    /// # Errors
    /// Returns an error when `secret_key` is not a valid scalar for the
    /// secp256k1 group (i.e. zero or >= the group order).
    pub fn from_bytes(secret_key: &[u8; 32]) -> Result<Self, crate::error::WireError> {
        let signing_key = SigningKey::from_slice(secret_key).map_err(|error| {
            crate::error::WireError::Serialization(format!("invalid secp256k1 key: {error}"))
        })?;
        Ok(Self { signing_key })
    }

    /// Returns the 33-byte compressed SEC1 public key.
    #[must_use]
    pub fn public_key_compressed(&self) -> [u8; 33] {
        let encoded = self.signing_key.verifying_key().to_sec1_point(true);
        let mut compressed = [0_u8; 33];
        let bytes = encoded.as_bytes();
        let copy_len = bytes.len().min(33);
        compressed[..copy_len].copy_from_slice(&bytes[..copy_len]);
        compressed
    }

    /// Derives the 20-byte Ethereum address of this key pair.
    #[must_use]
    pub fn ethereum_address(&self) -> [u8; 20] {
        ethereum_address_from_compressed_pubkey(&self.public_key_compressed()).unwrap_or([0_u8; 20])
    }

    /// Builds a `SignerRef` carrying the compressed public key.
    #[must_use]
    pub fn signer_ref(&self, alg: SigningAlgorithm) -> SignerRef {
        SignerRef::new(alg, self.public_key_compressed().to_vec())
    }

    /// Signs `SHA-256(message)` producing a low-s [`SignatureEnvelope`] with
    /// algorithm [`SigningAlgorithm::Secp256k1`] (64-byte `r || s`).
    #[must_use]
    pub fn sign_message_sha256(&self, message: &[u8]) -> SignatureEnvelope {
        let digest = Sha256::digest(message);
        // The backend applies low-S normalization before returning.
        let (signature, _) = self.signing_key.sign_prehash_recoverable(&digest);
        SignatureEnvelope { alg: SigningAlgorithm::Secp256k1, value: signature.to_bytes().to_vec() }
    }

    /// Signs a message under EIP-191 `personal_sign` semantics, producing a
    /// 65-byte `r || s || v` envelope (v ∈ {27, 28}) with algorithm
    /// [`SigningAlgorithm::EthPersonalSign`].
    #[must_use]
    pub fn sign_eth_personal(&self, message: &[u8]) -> SignatureEnvelope {
        let digest = eip191_message_hash(message);
        self.sign_digest_eth_style(digest, SigningAlgorithm::EthPersonalSign)
    }

    /// Signs a 32-byte EIP-712 typed-data digest, producing a 65-byte
    /// `r || s || v` envelope with algorithm
    /// [`SigningAlgorithm::EthTypedData`].
    #[must_use]
    pub fn sign_eth_typed_data_digest(&self, digest: &[u8; 32]) -> SignatureEnvelope {
        self.sign_digest_eth_style(*digest, SigningAlgorithm::EthTypedData)
    }

    fn sign_digest_eth_style(&self, digest: [u8; 32], alg: SigningAlgorithm) -> SignatureEnvelope {
        // The backend already applied low-S normalization and adjusted the
        // recovery id parity accordingly.
        let (signature, recovery_id) = self.signing_key.sign_prehash_recoverable(&digest);
        let mut value = Vec::with_capacity(65);
        value.extend_from_slice(signature.to_bytes().as_slice());
        value.push(recovery_id.to_byte() + 27);
        SignatureEnvelope { alg, value }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::warrant::SigningKeyPair;

    fn hex(bytes: &[u8]) -> String {
        let mut out = String::new();
        for byte in bytes {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    #[test]
    fn keccak256_empty_matches_known_vector() {
        assert_eq!(
            hex(&keccak256(b"")),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    #[test]
    fn keccak256_abc_matches_known_vector() {
        assert_eq!(
            hex(&keccak256(b"abc")),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
    }

    #[test]
    fn eip191_prefix_is_exact() {
        let hash = eip191_message_hash(b"hello");
        let mut manual = b"\x19Ethereum Signed Message:\n5hello".to_vec();
        assert_eq!(hash, keccak256(&manual));
        // Length prefix is decimal ASCII of the byte length.
        let long = vec![0xAB_u8; 300];
        let hash_long = eip191_message_hash(&long);
        manual = b"\x19Ethereum Signed Message:\n300".to_vec();
        manual.extend_from_slice(&long);
        assert_eq!(hash_long, keccak256(&manual));
    }

    #[test]
    fn order_constants_are_consistent() {
        // half_order must equal floor((n-1)/2): n is odd, so halving (n-1)
        // shifts the big-endian integer right by one bit.
        let mut shifted = SECP256K1_ORDER;
        shifted[31] -= 1; // n - 1
        let mut carry = 0_u16;
        for byte in &mut shifted {
            let value = carry << 8 | u16::from(*byte);
            *byte = (value / 2) as u8;
            carry = value % 2;
        }
        assert_eq!(SECP256K1_HALF_ORDER, shifted);
    }

    #[test]
    fn secp256k1_sha256_roundtrip_and_tamper() {
        let keys = Secp256k1KeyPair::from_bytes(&[0x11; 32]).expect("valid key");
        let signer = keys.signer_ref(SigningAlgorithm::Secp256k1);
        let envelope = keys.sign_message_sha256(b"ledgerflow");
        assert!(envelope.verify_strict(&signer, b"ledgerflow"));
        assert!(!envelope.verify_strict(&signer, b"tampered"));
        let other = Secp256k1KeyPair::from_bytes(&[0x22; 32]).expect("valid key");
        assert!(
            !envelope.verify_strict(&other.signer_ref(SigningAlgorithm::Secp256k1), b"ledgerflow")
        );
    }

    #[test]
    fn eth_personal_sign_roundtrip_with_pubkey_claim() {
        let keys = Secp256k1KeyPair::from_bytes(&[0x33; 32]).expect("valid key");
        let signer = keys.signer_ref(SigningAlgorithm::EthPersonalSign);
        let envelope = keys.sign_eth_personal(b"warrant payload");
        assert!(envelope.verify_strict(&signer, b"warrant payload"));
        assert!(!envelope.verify_strict(&signer, b"other payload"));
    }

    #[test]
    fn eth_personal_sign_accepts_address_claim() {
        let keys = Secp256k1KeyPair::from_bytes(&[0x44; 32]).expect("valid key");
        let address_signer =
            SignerRef::new(SigningAlgorithm::EthPersonalSign, keys.ethereum_address().to_vec());
        let envelope = keys.sign_eth_personal(b"approval request");
        assert_eq!(address_signer.public_key.len(), 20);
        assert!(envelope.verify_strict(&address_signer, b"approval request"));
        // Wrong address claim fails.
        let wrong = SignerRef::new(SigningAlgorithm::EthPersonalSign, vec![0xEE; 20]);
        assert!(!envelope.verify_strict(&wrong, b"approval request"));
    }

    #[test]
    fn eth_personal_sign_accepts_both_v_conventions() {
        let keys = Secp256k1KeyPair::from_bytes(&[0x55; 32]).expect("valid key");
        let signer = keys.signer_ref(SigningAlgorithm::EthPersonalSign);
        let envelope = keys.sign_eth_personal(b"v conventions");
        assert_eq!(envelope.value.len(), 65);
        let v = *envelope.value.last().expect("65 bytes");
        assert!(v == 27 || v == 28);
        // The zero-based encoding of the same signature must also verify.
        let mut zero_based = envelope.clone();
        zero_based.value[64] = v - 27;
        assert!(zero_based.verify_strict(&signer, b"v conventions"));
    }

    #[test]
    fn eth_typed_data_requires_32_byte_digest() {
        let keys = Secp256k1KeyPair::from_bytes(&[0x66; 32]).expect("valid key");
        let signer = keys.signer_ref(SigningAlgorithm::EthTypedData);
        let digest = keccak256(b"domainSeparator||structHash");
        let envelope = keys.sign_eth_typed_data_digest(&digest);
        assert!(envelope.verify_strict(&signer, &digest));
        // Non-32-byte messages are rejected outright.
        assert!(!envelope.verify_strict(&signer, &digest[..31]));
        assert!(!envelope.verify_strict(&signer, b"not a digest"));
        let other_digest = keccak256(b"different");
        assert!(!envelope.verify_strict(&signer, &other_digest));
    }

    #[test]
    fn high_s_signature_is_rejected() {
        let keys = Secp256k1KeyPair::from_bytes(&[0x77; 32]).expect("valid key");
        let signer = keys.signer_ref(SigningAlgorithm::EthPersonalSign);
        let envelope = keys.sign_eth_personal(b"malleability");
        // Build the malleable twin: s' = n - s with flipped recovery parity.
        let mut malleable = envelope.clone();
        let s: [u8; 32] = envelope.value[32..64].try_into().expect("fixed-width slice");
        let mut s_twinned = [0_u8; 32];
        let mut borrow = 0_u16;
        for index in (0..32).rev() {
            let minuend = u16::from(SECP256K1_ORDER[index]);
            let subtrahend = u16::from(s[index]) + borrow;
            borrow = u16::from(minuend < subtrahend);
            s_twinned[index] = (minuend.wrapping_sub(subtrahend)) as u8;
        }
        malleable.value[32..64].copy_from_slice(&s_twinned);
        let v = malleable.value[64];
        malleable.value[64] = if v == 27 { 28 } else { 27 };
        assert_ne!(envelope.value, malleable.value);
        assert!(!malleable.verify_strict(&signer, b"malleability"));
        // The canonical original still verifies.
        assert!(envelope.verify_strict(&signer, b"malleability"));
    }

    #[test]
    fn cross_algorithm_verification_fails() {
        let ed_keys = SigningKeyPair::from_bytes(&[0x88; 32]);
        let eth_keys = Secp256k1KeyPair::from_bytes(&[0x99; 32]).expect("valid key");
        let ed_envelope = ed_keys.sign(b"cross alg");
        let eth_signer = eth_keys.signer_ref(SigningAlgorithm::EthPersonalSign);
        assert!(!ed_envelope.verify_strict(&eth_signer, b"cross alg"));
        let eth_envelope = eth_keys.sign_eth_personal(b"cross alg");
        assert!(!eth_envelope.verify_strict(&ed_keys.signer_ref(), b"cross alg"));
    }

    #[test]
    fn invalid_secret_key_is_rejected() {
        assert!(Secp256k1KeyPair::from_bytes(&[0; 32]).is_err());
        // n itself is not a valid scalar.
        assert!(Secp256k1KeyPair::from_bytes(&SECP256K1_ORDER).is_err());
    }

    #[test]
    fn eip191_bytes32_hash_is_independently_reproducible() {
        let digest = keccak256(b"tuple bytes");
        let hash = eip191_hash_of_bytes32(&digest);
        // Manual composition of the Solidity toEthSignedMessageHash convention.
        let mut manual = b"\x19Ethereum Signed Message:\n32".to_vec();
        manual.extend_from_slice(&digest);
        assert_eq!(hash, keccak256(&manual));
        // Distinct digests must produce distinct hashes (not a constant).
        let other = keccak256(b"other tuple");
        assert_ne!(hash, eip191_hash_of_bytes32(&other));
    }

    #[test]
    fn debug_output_names_the_type_and_address_without_secrets() {
        let keys = Secp256k1KeyPair::from_bytes(&[0xAB; 32]).expect("valid key");
        let rendered = format!("{keys:?}");
        assert!(rendered.contains("Secp256k1KeyPair"));
        assert!(rendered.contains(&hex(&keys.ethereum_address())));
        assert!(!rendered.contains(&hex(&[0xAB; 32])));
    }

    #[test]
    fn address_derivation_is_deterministic() {
        let keys = Secp256k1KeyPair::from_bytes(&[0xAA; 32]).expect("valid key");
        let first = keys.ethereum_address();
        let second = keys.ethereum_address();
        assert_eq!(first, second);
        assert_eq!(
            first,
            ethereum_address_from_compressed_pubkey(&keys.public_key_compressed())
                .expect("derivable")
        );
    }
}
