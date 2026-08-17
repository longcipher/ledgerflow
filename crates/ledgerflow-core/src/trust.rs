//! Trusted-issuer anchors (trust model).
//!
//! Every verifier (merchant or Facilitator) configures a set of trusted
//! issuer public keys. The **root** warrant of any presented chain must have
//! been issued by one of these trusted issuers, otherwise the chain is
//! rejected (fail-closed). Key rotation is supported via `key_id`: a rotated
//! issuer simply replaces its entry in the set.

use serde::{Deserialize, Serialize};

use crate::{
    error::{AuthorizationError, Result},
    warrant::{SignerRef, Warrant},
};

/// A single trusted issuer entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrustedIssuer {
    /// Machine-readable key id (used for rotation and audit).
    pub key_id: String,
    /// The issuer signer reference.
    pub issuer: SignerRef,
}

impl TrustedIssuer {
    #[must_use]
    pub const fn new(key_id: String, issuer: SignerRef) -> Self {
        Self { key_id, issuer }
    }
}

/// The set of trusted issuer anchors for a verifier.
///
/// The default (empty) set is **fail-closed**: every chain is rejected until
/// at least one trusted issuer is configured.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrustedIssuers {
    pub issuers: Vec<TrustedIssuer>,
}

impl TrustedIssuers {
    /// Creates an empty (fail-closed) trust set.
    #[must_use]
    pub const fn new() -> Self {
        Self { issuers: Vec::new() }
    }

    /// Adds a trusted issuer.
    pub fn add(&mut self, issuer: TrustedIssuer) {
        self.issuers.push(issuer);
    }

    /// Returns `true` when the signer is trusted.
    #[must_use]
    pub fn contains(&self, signer: &SignerRef) -> bool {
        self.issuers.iter().any(|entry| {
            entry.issuer.alg == signer.alg && entry.issuer.public_key == signer.public_key
        })
    }

    /// Returns `true` when the set is non-empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.issuers.is_empty()
    }

    /// Verifies that the root warrant's issuer is trusted.
    ///
    /// Fails with [`AuthorizationError::UntrustedIssuer`] when the root
    /// issuer is not in the set (including when the set is empty).
    pub fn verify_root(&self, root: &Warrant) -> Result<()> {
        if self.contains(&root.issuer) {
            Ok(())
        } else {
            Err(AuthorizationError::UntrustedIssuer {
                key_id: root.issuer.key_id.clone().unwrap_or_default(),
            })
        }
    }
}
