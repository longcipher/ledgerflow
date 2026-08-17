//! Proof-of-possession (PoP) binding tuples.
//!
//! The PoP proves that the presenter controls the private key of the warrant
//! holder, and binds the proof to a specific challenge, request, and payment.
//! The binding tuple is encoded with **CBOR deterministic encoding** (RFC 8949
//! Core Deterministic Encoding) rather than raw HTTP text normalization, which
//! makes cross-implementation verification unambiguous.

use serde::{Deserialize, Serialize};

use crate::{
    error::{AuthorizationError, Result},
    warrant::{SignatureEnvelope, SignerRef, SigningKeyPair, sha256_prefixed},
};

/// Domain-separation prefix for PoP signatures.
pub const POP_SIGN_DOMAIN: &[u8] = b"ledgerflow-pop-v1";

/// The structured binding tuple signed by the holder.
///
/// Every field is bound into the signature, so neither party can swap in a
/// different challenge, request, or payment without invalidating the proof.
/// The tuple is serialized with CBOR deterministic encoding, so field order
/// and integer encodings are canonical across implementations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PopTuple {
    /// 16-byte warrant id (must match the leaf warrant's id).
    #[serde(with = "serde_bytes")]
    pub warrant_id: Vec<u8>,
    /// The merchant-issued challenge id.
    pub challenge_id: String,
    /// HTTP method (uppercase) of the payment request.
    pub method: String,
    /// URI (authority + path + query) of the payment request.
    pub uri: String,
    /// Canonical digest of the request (method + uri + body).
    pub request_hash: String,
    /// Canonical digest of the accepted quote.
    pub accepted_hash: String,
    /// Digest of the scheme-specific payment payload.
    pub payment_payload_digest: String,
    /// Canonical digest of the tool-call arguments (sorted key-value pairs).
    ///
    /// Binding the tool arguments into the PoP defends against confused-deputy
    /// attacks at the tool layer (e.g. MCP calls that never pass through the
    /// HTTP body that `request_hash` covers). It is required for tool-gated
    /// scenarios; HTTP-only callers may leave it empty (which is treated as a
    /// distinct value from any real digest, so omission is unambiguous).
    pub tool_args_digest: Option<String>,
    /// Digest of the approvals array (present when approvals are attached).
    /// Closing the PoP/approvals ambiguity window.
    pub approvals_digest: Option<String>,
    /// Client-generated nonce for replay protection.
    pub nonce: String,
    /// Unix milliseconds when the proof was created.
    pub created_at_ms: u64,
}

impl PopTuple {
    /// CBOR-encodes the tuple deterministically (the signing preimage body).
    #[must_use]
    pub fn encode_cbor(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        #[allow(clippy::expect_used)]
        ciborium::ser::into_writer(self, &mut bytes)
            .expect("pop tuple serialization is infallible");
        bytes
    }

    /// Computes the full domain-separated signing preimage.
    #[must_use]
    pub fn preimage(&self) -> Vec<u8> {
        let mut preimage = Vec::with_capacity(POP_SIGN_DOMAIN.len() + self.encode_cbor().len());
        preimage.extend_from_slice(POP_SIGN_DOMAIN);
        preimage.extend_from_slice(&self.encode_cbor());
        preimage
    }

    /// Computes a canonical digest of the tuple (used for audit records).
    #[must_use]
    pub fn digest(&self) -> String {
        sha256_prefixed(self.encode_cbor())
    }

    /// Produces a digest over a list of signed approvals.
    #[must_use]
    pub fn approvals_digest(approvals: &[crate::approval::SignedApproval]) -> String {
        let mut bytes = Vec::new();
        for approval in approvals {
            bytes.extend_from_slice(&approval.encode_cbor());
        }
        sha256_prefixed(bytes)
    }

    /// Produces a canonical digest over tool-call arguments.
    ///
    /// Arguments are serialized as a sorted list of `(key, value)` pairs
    /// (sorting by key, then by value) so that semantically identical argument
    /// maps hash identically across implementations. Empty argument maps yield
    /// `None` (omission), which is distinct from a real digest of an empty
    /// map — callers that need to bind "no arguments" explicitly should pass
    /// a sentinel argument.
    #[must_use]
    pub fn tool_args_digest(args: &crate::verification::ToolArguments) -> Option<String> {
        if args.is_empty() {
            return None;
        }
        let mut entries: Vec<(&String, &String)> = args.iter().collect();
        entries.sort_by(|(ka, va), (kb, vb)| (ka, va).cmp(&(kb, vb)));
        let mut bytes = Vec::with_capacity(args.len() * 16);
        for (key, value) in entries {
            bytes.extend_from_slice(key.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(value.as_bytes());
            bytes.push(0xFF);
        }
        Some(sha256_prefixed(bytes))
    }
}

/// A signed proof-of-possession.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PopProof {
    /// The binding tuple.
    pub tuple: PopTuple,
    /// Public key of the signer (must match the leaf warrant holder).
    #[serde(with = "serde_bytes")]
    pub signer_key: Vec<u8>,
    /// Signature over `tuple.preimage()`.
    pub signature: SignatureEnvelope,
}

impl PopProof {
    /// Creates a new signed proof using the holder's signing key pair.
    #[must_use]
    pub fn new_signed(tuple: PopTuple, signer_keys: &SigningKeyPair) -> Self {
        let preimage = tuple.preimage();
        Self {
            signature: signer_keys.sign(&preimage),
            tuple,
            signer_key: signer_keys.public_key_bytes().to_vec(),
        }
    }

    /// Verifies the PoP signature against the given signer using **strict**
    /// Ed25519 verification.
    pub fn verify_signature(&self, signer: &SignerRef) -> bool {
        self.signer_key == signer.public_key &&
            self.signature.verify_strict(signer, &self.tuple.preimage())
    }
}

/// Verifies that the proof is within the freshness window with clock-skew
/// tolerance.
///
/// Accepts `|now - created_at| <= freshness_window + skew`.
pub const fn verify_freshness(
    proof: &PopProof,
    now_ms: u64,
    freshness_window_ms: u64,
    clock_skew_ms: u64,
) -> Result<()> {
    let tolerance = freshness_window_ms.saturating_add(clock_skew_ms);
    let elapsed = now_ms.abs_diff(proof.tuple.created_at_ms);
    if elapsed > tolerance {
        return Err(AuthorizationError::ProofOutsideFreshnessWindow {
            created_at_ms: proof.tuple.created_at_ms,
            now_ms,
        });
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::{approval::SignedApproval, warrant::SigningKeyPair};

    fn tuple() -> PopTuple {
        PopTuple {
            warrant_id: vec![1; 16],
            challenge_id: "challenge-1".to_string(),
            method: "POST".to_string(),
            uri: "merchant-a.example/pay".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            payment_payload_digest: "sha256:pay".to_string(),
            tool_args_digest: None,
            approvals_digest: None,
            nonce: "nonce-1".to_string(),
            created_at_ms: 2_000,
        }
    }

    fn signer() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x42; 32])
    }

    #[test]
    fn tuple_preimage_is_domain_separated() {
        let t = tuple();
        let preimage = t.preimage();
        // Domain prefix first.
        assert!(preimage.starts_with(POP_SIGN_DOMAIN));
        // Then the deterministic CBOR payload.
        assert!(preimage.len() > POP_SIGN_DOMAIN.len());
        // The digest is a sha256 of the tuple.
        assert!(t.digest().starts_with("sha256:"));
        assert_eq!(t.digest(), sha256_prefixed(t.encode_cbor()));
    }

    #[test]
    fn tuple_cbor_is_deterministic() {
        let first = tuple().encode_cbor();
        let second = tuple().encode_cbor();
        assert_eq!(first, second);
        assert_ne!(first, [] as [u8; 0]);
    }

    #[test]
    fn approvals_digest_binds_all_approvals() {
        let a = SigningKeyPair::from_bytes(&[0x51; 32]);
        let approval = SignedApproval::sign(
            "sha256:req",
            &a.signer_ref(),
            10_300,
            &a,
        );
        let digest = PopTuple::approvals_digest(std::slice::from_ref(&approval));
        assert!(digest.starts_with("sha256:"));
        // Different approvals yield different digests.
        let other = SignedApproval::sign(
            "sha256:other",
            &a.signer_ref(),
            10_300,
            &a,
        );
        assert_ne!(digest, PopTuple::approvals_digest(std::slice::from_ref(&other)));
    }

    #[test]
    fn proof_verifies_under_matching_signer() {
        let key = signer();
        let proof = PopProof::new_signed(tuple(), &key);
        assert!(proof.verify_signature(&key.signer_ref()));
        // Wrong signer fails.
        let other = SigningKeyPair::from_bytes(&[0x43; 32]);
        assert!(!proof.verify_signature(&other.signer_ref()));
    }

    #[test]
    fn proof_verifies_false_for_wrong_key_field() {
        let key = signer();
        let mut proof = PopProof::new_signed(tuple(), &key);
        proof.signer_key = SigningKeyPair::from_bytes(&[0x44; 32]).public_key_bytes().to_vec();
        assert!(!proof.verify_signature(&key.signer_ref()));
    }

    #[test]
    fn verify_freshness_bounds() {
        let proof = PopProof::new_signed(tuple(), &signer());
        // Exactly at tolerance passes (strict >).
        verify_freshness(&proof, 2_000 + 60_000 + 30_000, 60_000, 30_000).expect("at limit");
        // One ms beyond fails.
        let error =
            verify_freshness(&proof, 2_000 + 60_000 + 30_000 + 1, 60_000, 30_000)
                .expect_err("beyond");
        assert!(matches!(error, AuthorizationError::ProofOutsideFreshnessWindow { .. }));
        // Future proof (created after now) also bounded by abs diff.
        verify_freshness(&proof, 1_000, 60_000, 30_000).expect("skew tolerance");
    }
}
