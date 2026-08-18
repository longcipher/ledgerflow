//! SRL synchronization: applying control-plane revocation lists to a local
//! persistent store (multi-node SaaS revocation).
//!
//! In a multi-verifier deployment, revocation originates at a control plane
//! and must propagate to every verifier node. This module bridges the pure
//! [`SignedRevocationList`] / [`SrlState`] domain (in `ledgerflow-core`) with
//! the persistent [`FileRevocationStore`] used at verification time:
//!
//! - [`SrlSync`] tracks the highest applied version (anti-rollback) and the union of entries.
//! - [`SrlSync::apply`] validates signature + version, then persists the new entries into the store
//!   (each entry becomes an ordinary revocation record, so it survives restarts through the
//!   existing JSON-Lines store).
//!
//! The HTTP transport of SRLs (a control-plane endpoint that verifiers poll)
//! is a deployment concern left to `ledgerflow-server` (P4); this module owns
//! the application semantics.

use ledgerflow_core::{RevocationCheck, SignedRevocationList, SignerRef, SrlEntry, SrlState};

use crate::revocation_store::{FileRevocationStore, RevocationStoreError};

/// Bridges SRL application onto a persistent revocation store.
///
/// Cheap to clone: the state and the store are both shared internally.
#[derive(Clone, Debug)]
pub struct SrlSync {
    state: std::sync::Arc<std::sync::Mutex<SrlState>>,
    store: FileRevocationStore,
    /// The trusted control-plane signer that SRLs must verify against.
    trusted_control_plane: SignerRef,
}

impl SrlSync {
    /// Creates an SRL sync bound to a store and a trusted control-plane
    /// signer.
    #[must_use]
    pub fn new(store: FileRevocationStore, trusted_control_plane: SignerRef) -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(SrlState::new())),
            store,
            trusted_control_plane,
        }
    }

    /// Returns the highest SRL version applied so far.
    #[must_use]
    pub fn applied_version(&self) -> u64 {
        self.state.lock().map_or(0, |s| s.applied_version)
    }

    /// Applies a signed SRL: verifies the signature, enforces anti-rollback,
    /// and persists every new entry into the store.
    pub fn apply(&self, list: &SignedRevocationList) -> Result<(), SrlSyncError> {
        let mut state = self.state.lock().map_err(|_| SrlSyncError::Poisoned)?;
        // Validate before mutating anything.
        state.apply(list, &self.trusted_control_plane).map_err(SrlSyncError::Core)?;

        // Persist entries that are new to the store. Persistence failures are
        // surfaced but do NOT advance the applied version (so the node retries
        // on the next poll and never silently skips a revocation).
        let mut store_updated = false;
        for entry in &list.entries {
            match entry {
                SrlEntry::Warrant { id_hex } => {
                    let id = hex_decode(id_hex).ok_or_else(|| {
                        SrlSyncError::Store(RevocationStoreError::Corrupt(format!(
                            "invalid warrant id hex `{id_hex}`"
                        )))
                    })?;
                    if self.store.check_warrant(&id) == ledgerflow_core::RevocationDecision::Ok {
                        self.store.revoke_warrant(&id)?;
                        store_updated = true;
                    }
                }
                SrlEntry::Holder { key_hex } => {
                    let key = hex_decode(key_hex).ok_or_else(|| {
                        SrlSyncError::Store(RevocationStoreError::Corrupt(format!(
                            "invalid holder key hex `{key_hex}`"
                        )))
                    })?;
                    let holder = SignerRef::new(ledgerflow_core::SigningAlgorithm::Ed25519, key);
                    if self.store.check_holder(&holder) == ledgerflow_core::RevocationDecision::Ok {
                        self.store.revoke_holder(&holder)?;
                        store_updated = true;
                    }
                }
            }
        }

        // Only now advance the version (after all entries persisted).
        if store_updated || !list.entries.is_empty() {
            state.applied_version = list.version;
        }
        Ok(())
    }
}

/// SRL sync failures.
#[derive(Debug, thiserror::Error)]
pub enum SrlSyncError {
    #[error("the SRL state lock is poisoned")]
    Poisoned,
    #[error("SRL validation failed: {0}")]
    Core(#[from] ledgerflow_core::AuthorizationError),
    #[error("revocation store failure: {0}")]
    Store(#[from] RevocationStoreError),
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok()).collect()
}
