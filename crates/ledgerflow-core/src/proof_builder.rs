//! TypeState-driven proof builder with compile-time validation.
//!
//! Mirrors the [`WarrantBuilder`](crate::typestate::WarrantBuilder) pattern:
//! every required field must be supplied before the proof can be signed.
//! Missing fields are a **compile-time** error, not a runtime panic.
//!
//! # Design Pattern: TypeState
//!
//! Four phantom type parameters (`C`, `W`, `A`, `R`) track whether the
//! challenge-id, warrant-digest, accepted-hash, and request-hash have been
//! set. The terminal `sign_with` method is only callable when all four
//! state parameters are in their "set" variant.
//!
//! # Example
//!
//! ```ignore
//! use ledgerflow_core::proof_builder::ProofBuilder;
//!
//! let proof = ProofBuilder::new()
//!     .challenge_id("challenge-1")       // C: NoChallenge -> HasChallenge
//!     .warrant_digest(warrant.digest())  // W: NoWarrantDigest -> HasWarrantDigest
//!     .accepted_hash(accepted_hash)      // A: NoAcceptedHash -> HasAcceptedHash
//!     .request_hash(request_hash)        // R: NoRequestHash -> HasRequestHash
//!     .created_at_ms(2_000)
//!     .nonce("nonce-1")
//!     .sign_with(&agent_keys);           // Only available when C, W, A, R are all "set"
//! ```

use std::marker::PhantomData;

use crate::warrant::{Proof, SignatureEnvelope, SigningAlgorithm, SigningKeyPair};

// ---------------------------------------------------------------------------
// TypeState markers
// ---------------------------------------------------------------------------

/// Challenge-id has **not** been set yet.
#[derive(Debug)]
pub struct NoChallenge;
/// Challenge-id **has** been set.
#[derive(Debug)]
pub struct HasChallenge;

/// Warrant-digest has **not** been set yet.
#[derive(Debug)]
pub struct NoWarrantDigest;
/// Warrant-digest **has** been set.
#[derive(Debug)]
pub struct HasWarrantDigest;

/// Accepted-hash has **not** been set yet.
#[derive(Debug)]
pub struct NoAcceptedHash;
/// Accepted-hash **has** been set.
#[derive(Debug)]
pub struct HasAcceptedHash;

/// Request-hash has **not** been set yet.
#[derive(Debug)]
pub struct NoRequestHash;
/// Request-hash **has** been set.
#[derive(Debug)]
pub struct HasRequestHash;

// ---------------------------------------------------------------------------
// Builder struct
// ---------------------------------------------------------------------------

/// Compile-time-safe proof builder.
///
/// The four type parameters `C`, `W`, `A`, `R` are phantom markers that
/// prevent calling [`sign_with`](Self::sign_with) until every required
/// binding field has been supplied.
pub struct ProofBuilder<C, W, A, R> {
    challenge_id: Option<String>,
    warrant_digest: Option<String>,
    accepted_hash: Option<String>,
    request_hash: Option<String>,
    created_at_ms: u64,
    nonce: String,
    _marker: PhantomData<(C, W, A, R)>,
}

impl<C, W, A, R> std::fmt::Debug for ProofBuilder<C, W, A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProofBuilder")
            .field("created_at_ms", &self.created_at_ms)
            .field("nonce", &self.nonce)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// Initial state
// ---------------------------------------------------------------------------

impl ProofBuilder<NoChallenge, NoWarrantDigest, NoAcceptedHash, NoRequestHash> {
    /// Creates a new proof builder with zeroed defaults.
    ///
    /// All binding fields must be set before calling
    /// [`sign_with`](Self::sign_with).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            challenge_id: None,
            warrant_digest: None,
            accepted_hash: None,
            request_hash: None,
            created_at_ms: 0,
            nonce: String::new(),
            _marker: PhantomData,
        }
    }
}

impl Default for ProofBuilder<NoChallenge, NoWarrantDigest, NoAcceptedHash, NoRequestHash> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Methods available at every state
// ---------------------------------------------------------------------------

impl<C, W, A, R> ProofBuilder<C, W, A, R> {
    /// Sets the proof creation timestamp (milliseconds since epoch).
    #[must_use]
    pub const fn created_at_ms(mut self, created_at_ms: u64) -> Self {
        self.created_at_ms = created_at_ms;
        self
    }

    /// Sets the replay nonce.
    #[must_use]
    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = nonce.into();
        self
    }
}

// ---------------------------------------------------------------------------
// State transitions
// ---------------------------------------------------------------------------

impl<W, A, R> ProofBuilder<NoChallenge, W, A, R> {
    /// Binds the proof to a challenge id.
    ///
    /// **Compile-time guard:** calling this a second time is a type error.
    #[must_use]
    pub fn challenge_id(
        mut self,
        challenge_id: impl Into<String>,
    ) -> ProofBuilder<HasChallenge, W, A, R> {
        self.challenge_id = Some(challenge_id.into());
        ProofBuilder {
            challenge_id: self.challenge_id,
            warrant_digest: self.warrant_digest,
            accepted_hash: self.accepted_hash,
            request_hash: self.request_hash,
            created_at_ms: self.created_at_ms,
            nonce: self.nonce,
            _marker: PhantomData,
        }
    }
}

impl<C, A, R> ProofBuilder<C, NoWarrantDigest, A, R> {
    /// Binds the proof to a warrant digest.
    #[must_use]
    pub fn warrant_digest(
        mut self,
        warrant_digest: impl Into<String>,
    ) -> ProofBuilder<C, HasWarrantDigest, A, R> {
        self.warrant_digest = Some(warrant_digest.into());
        ProofBuilder {
            challenge_id: self.challenge_id,
            warrant_digest: self.warrant_digest,
            accepted_hash: self.accepted_hash,
            request_hash: self.request_hash,
            created_at_ms: self.created_at_ms,
            nonce: self.nonce,
            _marker: PhantomData,
        }
    }
}

impl<C, W, R> ProofBuilder<C, W, NoAcceptedHash, R> {
    /// Binds the proof to an accepted-quote hash.
    #[must_use]
    pub fn accepted_hash(
        mut self,
        accepted_hash: impl Into<String>,
    ) -> ProofBuilder<C, W, HasAcceptedHash, R> {
        self.accepted_hash = Some(accepted_hash.into());
        ProofBuilder {
            challenge_id: self.challenge_id,
            warrant_digest: self.warrant_digest,
            accepted_hash: self.accepted_hash,
            request_hash: self.request_hash,
            created_at_ms: self.created_at_ms,
            nonce: self.nonce,
            _marker: PhantomData,
        }
    }
}

impl<C, W, A> ProofBuilder<C, W, A, NoRequestHash> {
    /// Binds the proof to a request hash.
    #[must_use]
    pub fn request_hash(
        mut self,
        request_hash: impl Into<String>,
    ) -> ProofBuilder<C, W, A, HasRequestHash> {
        self.request_hash = Some(request_hash.into());
        ProofBuilder {
            challenge_id: self.challenge_id,
            warrant_digest: self.warrant_digest,
            accepted_hash: self.accepted_hash,
            request_hash: self.request_hash,
            created_at_ms: self.created_at_ms,
            nonce: self.nonce,
            _marker: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Terminal: sign is only available when all four binding fields are set
// ---------------------------------------------------------------------------

impl ProofBuilder<HasChallenge, HasWarrantDigest, HasAcceptedHash, HasRequestHash> {
    /// Signs the proof with the agent's key pair.
    ///
    /// This method is **only available** when all four binding fields have
    /// been set. The compiler will reject calls if any field is missing —
    /// no runtime `expect` / `unwrap` is involved.
    #[must_use]
    #[expect(
        clippy::expect_used,
        reason = "typestate guarantees all Options are Some at this point"
    )]
    pub fn sign_with(self, signer_keys: &SigningKeyPair) -> Proof {
        // SAFETY: typestate guarantees all Options are Some.
        let challenge_id = self.challenge_id.expect("typestate: HasChallenge");
        let warrant_digest = self.warrant_digest.expect("typestate: HasWarrantDigest");
        let accepted_hash = self.accepted_hash.expect("typestate: HasAcceptedHash");
        let request_hash = self.request_hash.expect("typestate: HasRequestHash");

        let signer_key = signer_keys.public_key_bytes().to_vec();

        let mut proof = Proof {
            challenge_id,
            warrant_digest,
            accepted_hash,
            request_hash,
            created_at_ms: self.created_at_ms,
            nonce: self.nonce,
            signer_key,
            signature: SignatureEnvelope { alg: SigningAlgorithm::Ed25519, value: vec![] },
        };
        proof.signature = signer_keys.sign(proof.preimage().as_bytes());
        proof
    }
}

#[cfg(test)]
mod tests {
    use super::ProofBuilder;
    use crate::warrant::SigningKeyPair;

    fn agent_keys() -> SigningKeyPair {
        let secret: [u8; 32] = *b"agent-secret-key--32-bytes-long!";
        SigningKeyPair::from_bytes(&secret)
    }

    #[test]
    fn builder_produces_a_signable_proof_when_all_fields_set() {
        let keys = agent_keys();
        let proof = ProofBuilder::new()
            .challenge_id("c1")
            .warrant_digest("sha256:abc")
            .accepted_hash("sha256:def")
            .request_hash("sha256:ghi")
            .created_at_ms(1_000)
            .nonce("n1")
            .sign_with(&keys);

        assert_eq!(proof.challenge_id, "c1");
        assert_eq!(proof.warrant_digest, "sha256:abc");
        assert!(proof.signature.verify(&keys.signer_ref(), proof.preimage().as_bytes()));
    }

    #[test]
    fn builder_signature_matches_new_signed_output() {
        let keys = agent_keys();
        let from_builder = ProofBuilder::new()
            .challenge_id("c1")
            .warrant_digest("sha256:abc")
            .accepted_hash("sha256:def")
            .request_hash("sha256:ghi")
            .created_at_ms(1_000)
            .nonce("n1")
            .sign_with(&keys);

        let from_new_signed = crate::warrant::Proof::new_signed(
            "c1",
            "sha256:abc",
            "sha256:def",
            "sha256:ghi",
            1_000,
            "n1",
            &keys,
        );

        assert_eq!(from_builder.signature, from_new_signed.signature);
        assert_eq!(from_builder.preimage(), from_new_signed.preimage());
    }
}
