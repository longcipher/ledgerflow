//! Builder for constructing and signing PoP proofs.
//!
//! Provides a fluent, type-safe way to assemble a [`PopTuple`] and sign it.

use crate::{
    pop::{PopProof, PopTuple},
    warrant::SigningKeyPair,
};

/// A fluent builder for [`PopProof`].
#[derive(Clone, Debug, Default)]
pub struct ProofBuilder {
    warrant_id: Vec<u8>,
    challenge_id: String,
    method: String,
    uri: String,
    request_hash: String,
    accepted_hash: String,
    payment_payload_digest: String,
    approvals_digest: Option<String>,
    nonce: String,
    created_at_ms: u64,
}

impl ProofBuilder {
    /// Creates a new proof builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            warrant_id: Vec::new(),
            challenge_id: String::new(),
            method: String::new(),
            uri: String::new(),
            request_hash: String::new(),
            accepted_hash: String::new(),
            payment_payload_digest: String::new(),
            approvals_digest: None,
            nonce: String::new(),
            created_at_ms: 0,
        }
    }

    /// Sets the 16-byte warrant id the proof binds to.
    #[must_use]
    pub fn warrant_id(mut self, warrant_id: impl Into<Vec<u8>>) -> Self {
        self.warrant_id = warrant_id.into();
        self
    }

    /// Sets the challenge id.
    #[must_use]
    pub fn challenge_id(mut self, challenge_id: impl Into<String>) -> Self {
        self.challenge_id = challenge_id.into();
        self
    }

    /// Sets the HTTP method.
    #[must_use]
    pub fn method(mut self, method: impl Into<String>) -> Self {
        self.method = method.into();
        self
    }

    /// Sets the request URI.
    #[must_use]
    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = uri.into();
        self
    }

    /// Sets the canonical request hash.
    #[must_use]
    pub fn request_hash(mut self, request_hash: impl Into<String>) -> Self {
        self.request_hash = request_hash.into();
        self
    }

    /// Sets the canonical accepted-quote hash.
    #[must_use]
    pub fn accepted_hash(mut self, accepted_hash: impl Into<String>) -> Self {
        self.accepted_hash = accepted_hash.into();
        self
    }

    /// Sets the payment payload digest.
    #[must_use]
    pub fn payment_payload_digest(mut self, payment_payload_digest: impl Into<String>) -> Self {
        self.payment_payload_digest = payment_payload_digest.into();
        self
    }

    /// Sets the approvals digest.
    #[must_use]
    pub fn approvals_digest(mut self, approvals_digest: impl Into<String>) -> Self {
        self.approvals_digest = Some(approvals_digest.into());
        self
    }

    /// Sets the nonce.
    #[must_use]
    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = nonce.into();
        self
    }

    /// Sets the creation timestamp (unix milliseconds).
    #[must_use]
    pub const fn created_at_ms(mut self, created_at_ms: u64) -> Self {
        self.created_at_ms = created_at_ms;
        self
    }

    /// Signs the proof with the holder's key pair.
    #[must_use]
    pub fn sign_with(self, signer_keys: &SigningKeyPair) -> PopProof {
        let tuple = PopTuple {
            warrant_id: self.warrant_id,
            challenge_id: self.challenge_id,
            method: self.method,
            uri: self.uri,
            request_hash: self.request_hash,
            accepted_hash: self.accepted_hash,
            payment_payload_digest: self.payment_payload_digest,
            approvals_digest: self.approvals_digest,
            nonce: self.nonce,
            created_at_ms: self.created_at_ms,
        };
        PopProof::new_signed(tuple, signer_keys)
    }
}
