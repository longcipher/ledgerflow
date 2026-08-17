//! Revocation checking (online security commitment).
//!
//! Revocation is a **stateful** security guarantee and therefore lives outside
//! the stateless core: `ledgerflow-core` defines the pure `RevocationCheck`
//! seam, and downstream crates (Facilitator / Server) implement it with
//! persistent storage. Production deployments MUST persist revocation
//! records; in-memory implementations are only permitted for demonstrations
//! and must be explicitly acknowledged (e.g. `--insecure-revoc-memory`).

use crate::{
    error::{AuthorizationError, Result},
    warrant::SignerRef,
};

/// A single revocation decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevocationDecision {
    /// Not revoked.
    Ok,
    /// The warrant itself was revoked.
    RevokedWarrant,
    /// The holder key was revoked (all warrants by this holder are invalid).
    RevokedHolder,
}

impl RevocationDecision {
    /// Returns `true` when the decision permits continued use.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

/// Pure seam for online revocation checks.
///
/// Implementations MUST be persistent in production. A best-effort check that
/// returns `Ok` when the store is unavailable is acceptable only with an
/// explicit availability downgrade (never silently).
pub trait RevocationCheck: std::fmt::Debug {
    /// Checks whether a warrant (by id) is revoked.
    fn check_warrant(&self, warrant_id: &[u8]) -> RevocationDecision;

    /// Checks whether a holder key is revoked.
    fn check_holder(&self, holder: &SignerRef) -> RevocationDecision;

    /// Convenience: runs both checks and returns an error when revoked.
    fn verify(&self, warrant_id: &[u8], holder: &SignerRef) -> Result<()> {
        match self.check_warrant(warrant_id) {
            RevocationDecision::Ok => {}
            RevocationDecision::RevokedWarrant => {
                return Err(AuthorizationError::WarrantRevoked);
            }
            RevocationDecision::RevokedHolder => {
                return Err(AuthorizationError::HolderRevoked);
            }
        }
        match self.check_holder(holder) {
            RevocationDecision::Ok => {}
            RevocationDecision::RevokedWarrant => {
                return Err(AuthorizationError::WarrantRevoked);
            }
            RevocationDecision::RevokedHolder => {
                return Err(AuthorizationError::HolderRevoked);
            }
        }
        Ok(())
    }
}

/// An in-memory revocation check for demonstrations and tests only.
///
/// This intentionally lives in the core crate so that unit tests can exercise
/// the seam without a database; production code should use the persistent
/// implementation in `ledgerflow-facilitator` / `ledgerflow-server`.
#[derive(Clone, Debug, Default)]
pub struct InMemoryRevocationCheck {
    revoked_warrants: std::collections::HashSet<Vec<u8>>,
    revoked_holders: std::collections::HashSet<Vec<u8>>,
}

impl InMemoryRevocationCheck {
    /// Creates an empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            revoked_warrants: std::collections::HashSet::new(),
            revoked_holders: std::collections::HashSet::new(),
        }
    }

    /// Revokes a warrant by id.
    pub fn revoke_warrant(&mut self, warrant_id: &[u8]) {
        self.revoked_warrants.insert(warrant_id.to_vec());
    }

    /// Revokes a holder key.
    pub fn revoke_holder(&mut self, holder: &SignerRef) {
        self.revoked_holders.insert(holder.public_key.clone());
    }
}

impl RevocationCheck for InMemoryRevocationCheck {
    fn check_warrant(&self, warrant_id: &[u8]) -> RevocationDecision {
        if self.revoked_warrants.contains(warrant_id) {
            RevocationDecision::RevokedWarrant
        } else {
            RevocationDecision::Ok
        }
    }

    fn check_holder(&self, holder: &SignerRef) -> RevocationDecision {
        if self.revoked_holders.contains(&holder.public_key) {
            RevocationDecision::RevokedHolder
        } else {
            RevocationDecision::Ok
        }
    }
}


#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::warrant::SigningKeyPair;

    fn holder() -> SignerRef {
        SigningKeyPair::from_bytes(&[0x61; 32]).signer_ref()
    }

    #[test]
    fn decision_is_allowed_only_for_ok() {
        assert!(RevocationDecision::Ok.is_allowed());
        assert!(!RevocationDecision::RevokedWarrant.is_allowed());
        assert!(!RevocationDecision::RevokedHolder.is_allowed());
    }

    #[test]
    fn check_verify_passes_when_not_revoked() {
        let store = InMemoryRevocationCheck::new();
        store.verify(&[1; 16], &holder()).expect("not revoked");
    }

    #[test]
    fn check_verify_rejects_revoked_warrant() {
        let mut store = InMemoryRevocationCheck::new();
        store.revoke_warrant(&[1; 16]);
        let error = store.verify(&[1; 16], &holder()).expect_err("revoked");
        assert_eq!(error, AuthorizationError::WarrantRevoked);
    }

    #[test]
    fn check_verify_rejects_revoked_holder() {
        let mut store = InMemoryRevocationCheck::new();
        store.revoke_holder(&holder());
        let error = store.verify(&[1; 16], &holder()).expect_err("holder revoked");
        assert_eq!(error, AuthorizationError::HolderRevoked);
    }

    #[test]
    fn check_verify_checks_holder_even_if_warrant_ok() {
        let mut store = InMemoryRevocationCheck::new();
        store.revoke_holder(&holder());
        let error = store.verify(&[9; 16], &holder()).expect_err("holder revoked");
        assert_eq!(error, AuthorizationError::HolderRevoked);
    }

    #[test]
    fn in_memory_store_clones_independently() {
        let mut store = InMemoryRevocationCheck::new();
        let clone = store.clone();
        store.revoke_warrant(&[2; 16]);
        // The clone shares the same HashSet (HashSet is not Arc-shared, but
        // clone copies) -- verify the original reflects the mutation.
        assert_eq!(store.check_warrant(&[2; 16]), RevocationDecision::RevokedWarrant);
        assert_eq!(clone.check_warrant(&[2; 16]), RevocationDecision::Ok);
    }
}
