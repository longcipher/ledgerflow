//! Signed Revocation List (SRL): auditable, anti-rollback revocation
//! propagation.
//!
//! The SRL is the multi-node revocation primitive (design §6.6, roadmap item
//! in v0.1, now implemented for SaaS deployments). A control plane signs a
//! list of revocations; verifier nodes fetch the latest list and apply it to
//! their local `RevocationCheck`. The list is:
//!
//! - **additive**: entries are never removed from the current list (revocations are permanent for
//!   the warrant's lifetime);
//! - **anti-rollback**: the `version` is a strictly increasing monotone counter; a verifier MUST
//!   reject a list whose version is not greater than the highest it has already applied.
//!
//! The signature covers `SRL_SIGN_DOMAIN || version || encoded_entries`, so
//! entries and version cannot be swapped in or replayed across lists.

use serde::{Deserialize, Serialize};

use crate::{
    error::{AuthorizationError, Result, WireError, WireResult},
    warrant::{CborCodec, SignerRef, SigningKeyPair, sha256_prefixed},
};

/// Domain-separation prefix for SRL signatures.
pub const SRL_SIGN_DOMAIN: &[u8] = b"ledgerflow-srl-v1";

/// A single revocation entry.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SrlEntry {
    /// A warrant (by 16-byte id) is revoked.
    Warrant {
        /// Hex-encoded 16-byte warrant id.
        id_hex: String,
    },
    /// A holder public key is revoked (all its warrants invalid).
    Holder {
        /// Hex-encoded 32-byte public key.
        key_hex: String,
    },
}

/// A signed, versioned revocation list.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedRevocationList {
    /// Monotone version (must increase on each new list).
    pub version: u64,
    /// All revocation entries (additive across versions).
    pub entries: Vec<SrlEntry>,
    /// The control-plane signer.
    pub signer: SignerRef,
    /// Signature over `SRL_SIGN_DOMAIN || version || entries_cbor`.
    pub signature: Vec<u8>,
}

impl SignedRevocationList {
    /// Creates and signs a new SRL.
    #[must_use]
    pub fn sign(version: u64, entries: Vec<SrlEntry>, control_keys: &SigningKeyPair) -> Self {
        let preimage = preimage(version, &entries);
        Self {
            version,
            entries,
            signer: control_keys.signer_ref(),
            signature: control_keys.sign(&preimage).value,
        }
    }

    /// Verifies the SRL signature against the control-plane signer.
    #[must_use]
    pub fn verify_signature(&self, signer: &SignerRef) -> bool {
        if self.signer != *signer {
            return false;
        }
        let envelope = crate::warrant::SignatureEnvelope {
            alg: crate::warrant::SigningAlgorithm::Ed25519,
            value: self.signature.clone(),
        };
        envelope.verify_strict(signer, &preimage(self.version, &self.entries))
    }

    /// Returns a canonical digest of the list (for audit records).
    #[must_use]
    pub fn digest(&self) -> String {
        #[allow(clippy::expect_used)]
        let bytes = self.encode_cbor().expect("srl serialization is infallible");
        sha256_prefixed(bytes)
    }
}

/// Computes the domain-separated signing preimage of an SRL.
fn preimage(version: u64, entries: &[SrlEntry]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SRL_SIGN_DOMAIN.len() + 16 + entries.len() * 64);
    bytes.extend_from_slice(SRL_SIGN_DOMAIN);
    bytes.extend_from_slice(&version.to_be_bytes());
    // Deterministic encoding: entries are encoded in sorted order so the
    // preimage is canonical regardless of insertion order.
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| match e {
        SrlEntry::Warrant { id_hex } => (0_u8, id_hex.clone()),
        SrlEntry::Holder { key_hex } => (1_u8, key_hex.clone()),
    });
    for entry in sorted {
        #[allow(clippy::expect_used)]
        ciborium::ser::into_writer(&entry, &mut bytes)
            .expect("srl entry serialization is infallible");
    }
    bytes
}

impl CborCodec for SignedRevocationList {}

/// Incremental SRL application state: tracks the highest applied version and
/// the union of applied entries.
///
/// This is the pure-domain counterpart of the verifier node's local
/// revocation store: a node applies an SRL by advancing this state, then
/// feeding the entries into its persistent `RevocationCheck`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SrlState {
    /// Highest SRL version applied.
    pub applied_version: u64,
    /// All entries seen so far (deduplicated).
    pub entries: Vec<SrlEntry>,
}

impl SrlState {
    /// Creates an empty SRL state.
    #[must_use]
    pub const fn new() -> Self {
        Self { applied_version: 0, entries: Vec::new() }
    }

    /// Applies a signed SRL.
    ///
    /// Fails when the list's version is not strictly greater than the already
    /// applied version (anti-rollback), or when the signature does not verify
    /// against the trusted control-plane signer.
    pub fn apply(&mut self, list: &SignedRevocationList, trusted_signer: &SignerRef) -> Result<()> {
        if list.version <= self.applied_version {
            return Err(AuthorizationError::SrlVersionRegression {
                presented: list.version,
                applied: self.applied_version,
            });
        }
        if !list.verify_signature(trusted_signer) {
            return Err(AuthorizationError::InvalidSrlSignature);
        }
        for entry in &list.entries {
            if !self.entries.contains(entry) {
                self.entries.push(entry.clone());
            }
        }
        self.applied_version = list.version;
        Ok(())
    }

    /// Checks whether a warrant is revoked per the applied SRL.
    #[must_use]
    pub fn is_warrant_revoked(&self, warrant_id: &[u8]) -> bool {
        let id_hex = crate::warrant::hex_encode_bytes(warrant_id);
        self.entries.iter().any(|e| matches!(e, SrlEntry::Warrant { id_hex: e } if e == &id_hex))
    }

    /// Checks whether a holder key is revoked per the applied SRL.
    #[must_use]
    pub fn is_holder_revoked(&self, holder: &SignerRef) -> bool {
        let key_hex = crate::warrant::hex_encode_bytes(&holder.public_key);
        self.entries.iter().any(|e| matches!(e, SrlEntry::Holder { key_hex: e } if e == &key_hex))
    }

    /// Serializes the entries for wire transmission (as a new SRL body).
    pub fn encode_entries(&self) -> WireResult<Vec<u8>> {
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&self.entries, &mut bytes)
            .map_err(|error| WireError::Serialization(error.to_string()))?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn control_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x2A; 32])
    }

    fn sample_list(version: u64) -> SignedRevocationList {
        SignedRevocationList::sign(
            version,
            vec![SrlEntry::Warrant { id_hex: "aabb".to_string() }],
            &control_keys(),
        )
    }

    #[test]
    fn digest_is_prefixed_and_content_bound() {
        let list = sample_list(1);
        assert!(list.digest().starts_with("sha256:"));
        // Same content → same digest; changed content → different digest.
        assert_eq!(list.digest(), sample_list(1).digest());
        let other = SignedRevocationList::sign(
            1,
            vec![SrlEntry::Warrant { id_hex: "ccdd".to_string() }],
            &control_keys(),
        );
        assert_ne!(list.digest(), other.digest());
    }

    #[test]
    fn signature_binds_version_and_entries() {
        let keys = control_keys();
        let list = sample_list(1);
        let signer = keys.signer_ref();
        assert!(list.verify_signature(&signer));

        // Tampering with the version invalidates the signature.
        let mut bumped = list.clone();
        bumped.version = 2;
        assert!(!bumped.verify_signature(&signer));

        // Tampering with the entries invalidates the signature.
        let mut extra = list.clone();
        extra.entries.push(SrlEntry::Holder { key_hex: "ff00".to_string() });
        assert!(!extra.verify_signature(&signer));

        // A different signer fails.
        let stranger = SigningKeyPair::from_bytes(&[0x2B; 32]).signer_ref();
        assert!(!list.verify_signature(&stranger));
    }

    #[test]
    fn holder_revocation_queries_reflect_state() {
        let holder = SigningKeyPair::from_bytes(&[0x2C; 32]);
        let state = SrlState::new();
        // Fresh state revokes nobody.
        assert!(!state.is_holder_revoked(&holder.signer_ref()));

        let mut state = SrlState::new();
        let list = SignedRevocationList::sign(
            1,
            vec![SrlEntry::Holder {
                key_hex: crate::warrant::hex_encode_bytes(&holder.public_key_bytes()),
            }],
            &control_keys(),
        );
        state.apply(&list, &control_keys().signer_ref()).expect("apply");
        assert!(state.is_holder_revoked(&holder.signer_ref()));
        // An unrelated holder stays clean.
        let other = SigningKeyPair::from_bytes(&[0x2D; 32]);
        assert!(!state.is_holder_revoked(&other.signer_ref()));
    }

    #[test]
    fn apply_rejects_rollback_and_bad_signatures() {
        let mut state = SrlState::new();
        let v2 = sample_list(2);
        state.apply(&v2, &control_keys().signer_ref()).expect("v2");
        // Same or lower version is rejected.
        let error =
            state.apply(&sample_list(1), &control_keys().signer_ref()).expect_err("rollback");
        assert!(matches!(error, AuthorizationError::SrlVersionRegression { .. }));

        // Forged signer rejected.
        let forged =
            SignedRevocationList::sign(3, Vec::new(), &SigningKeyPair::from_bytes(&[0x2E; 32]));
        let error = state.apply(&forged, &control_keys().signer_ref()).expect_err("forged");
        assert_eq!(error, AuthorizationError::InvalidSrlSignature);
    }
}
