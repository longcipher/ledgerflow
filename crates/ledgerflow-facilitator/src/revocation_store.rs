//! Persistent revocation store (the online security commitment).
//!
//! Revocation MUST survive restarts in production (design §6.6). This module
//! provides a file-backed store using JSON Lines: every revocation is
//! appended and flushed, and a new instance reloads prior records.
//!
//! The in-memory variant is only permitted for demonstrations and must be
//! explicitly acknowledged by the operator (e.g. `--insecure-revoc-memory`).

use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use ledgerflow_core::{
    RevocationCheck, RevocationDecision, SignerRef,
};

/// A revocation record (JSON Lines).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RevocationRecord {
    Warrant { id_hex: String },
    Holder { key_hex: String },
}

/// File-backed, restart-safe revocation store.
///
/// The type is cheap to clone (it shares the underlying storage through
/// `Arc`), so multiple services can hold handles to the same store.
#[derive(Clone, Debug)]
pub struct FileRevocationStore {
    inner: std::sync::Arc<FileRevocationStoreInner>,
}

/// Inner storage shared by clones.
#[derive(Debug)]
struct FileRevocationStoreInner {
    path: PathBuf,
    revoked_warrants: Mutex<HashSet<Vec<u8>>>,
    revoked_holders: Mutex<HashSet<Vec<u8>>>,
}

impl FileRevocationStore {
    /// Opens (and loads) a revocation store at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RevocationStoreError> {
        let path = path.as_ref().to_path_buf();
        let mut revoked_warrants = HashSet::new();
        let mut revoked_holders = HashSet::new();

        if path.exists() {
            let file = File::open(&path).map_err(RevocationStoreError::Io)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.map_err(RevocationStoreError::Io)?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let record: RevocationRecord = serde_json::from_str(trimmed)
                    .map_err(|error| RevocationStoreError::Corrupt(error.to_string()))?;
                match record {
                    RevocationRecord::Warrant { id_hex } => {
                        revoked_warrants.insert(hex_decode(&id_hex).map_err(|()| {
                            RevocationStoreError::Corrupt("invalid warrant id hex".to_string())
                        })?);
                    }
                    RevocationRecord::Holder { key_hex } => {
                        revoked_holders.insert(hex_decode(&key_hex).map_err(|()| {
                            RevocationStoreError::Corrupt("invalid holder key hex".to_string())
                        })?);
                    }
                }
            }
        }

        Ok(Self {
            inner: std::sync::Arc::new(FileRevocationStoreInner {
                path,
                revoked_warrants: Mutex::new(revoked_warrants),
                revoked_holders: Mutex::new(revoked_holders),
            }),
        })
    }

    /// Revokes a warrant by id (persisted immediately).
    pub fn revoke_warrant(&self, warrant_id: &[u8]) -> Result<(), RevocationStoreError> {
        let record = RevocationRecord::Warrant { id_hex: hex_encode(warrant_id) };
        self.append(&record)?;
        if let Ok(mut set) = self.inner.revoked_warrants.lock() {
            set.insert(warrant_id.to_vec());
        }
        Ok(())
    }

    /// Revokes a holder key (persisted immediately).
    pub fn revoke_holder(&self, holder: &SignerRef) -> Result<(), RevocationStoreError> {
        let record = RevocationRecord::Holder { key_hex: hex_encode(&holder.public_key) };
        self.append(&record)?;
        if let Ok(mut set) = self.inner.revoked_holders.lock() {
            set.insert(holder.public_key.clone());
        }
        Ok(())
    }

    fn append(&self, record: &RevocationRecord) -> Result<(), RevocationStoreError> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.inner.path)
            .map_err(RevocationStoreError::Io)?;
        let line = serde_json::to_string(record)
            .map_err(|error| RevocationStoreError::Corrupt(error.to_string()))?;
        writeln!(file, "{line}").map_err(RevocationStoreError::Io)?;
        file.flush().map_err(RevocationStoreError::Io)?;
        file.sync_all().map_err(RevocationStoreError::Io)?;
        Ok(())
    }
}

impl RevocationCheck for FileRevocationStore {
    fn check_warrant(&self, warrant_id: &[u8]) -> RevocationDecision {
        let revoked = self
            .inner
            .revoked_warrants
            .lock()
            .is_ok_and(|set| set.contains(warrant_id));
        if revoked {
            RevocationDecision::RevokedWarrant
        } else {
            RevocationDecision::Ok
        }
    }

    fn check_holder(&self, holder: &SignerRef) -> RevocationDecision {
        let revoked = self
            .inner
            .revoked_holders
            .lock()
            .is_ok_and(|set| set.contains(&holder.public_key));
        if revoked {
            RevocationDecision::RevokedHolder
        } else {
            RevocationDecision::Ok
        }
    }
}

/// File revocation store failures.
#[derive(Debug, thiserror::Error)]
pub enum RevocationStoreError {
    #[error("I/O error on the revocation store: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt revocation record: {0}")]
    Corrupt(String),
}

/// An in-memory revocation check for demos and tests.
///
/// NOT restart-safe; use [`FileRevocationStore`] in production. Operators
/// MUST explicitly acknowledge this downgrade (e.g. `--insecure-revoc-memory`).
#[derive(Clone, Debug, Default)]
pub struct InsecureMemoryRevocationStore {
    inner: ledgerflow_core::InMemoryRevocationCheck,
}

impl InsecureMemoryRevocationStore {
    /// Creates an empty in-memory store (explicitly insecure for demos).
    #[must_use]
    pub fn new() -> Self {
        Self { inner: ledgerflow_core::InMemoryRevocationCheck::new() }
    }

    /// Revokes a warrant by id.
    pub fn revoke_warrant(&mut self, warrant_id: &[u8]) {
        self.inner.revoke_warrant(warrant_id);
    }

    /// Revokes a holder key.
    pub fn revoke_holder(&mut self, holder: &SignerRef) {
        self.inner.revoke_holder(holder);
    }
}

impl RevocationCheck for InsecureMemoryRevocationStore {
    fn check_warrant(&self, warrant_id: &[u8]) -> RevocationDecision {
        self.inner.check_warrant(warrant_id)
    }

    fn check_holder(&self, holder: &SignerRef) -> RevocationDecision {
        self.inner.check_holder(holder)
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

fn hex_decode(hex: &str) -> Result<Vec<u8>, ()> {
    if !hex.len().is_multiple_of(2) {
        return Err(());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ()))
        .collect()
}


#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use ledgerflow_core::{SigningAlgorithm, SigningKeyPair};

    fn holder() -> SignerRef {
        SigningKeyPair::from_bytes(&[0x77; 32]).signer_ref()
    }

    #[test]
    fn file_store_persists_holder_revocation_across_restart() {
        let dir = std::env::temp_dir().join(format!("ledgerflow-holder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("revocations.jsonl");
        let _ = std::fs::remove_file(&path);

        {
            let store = FileRevocationStore::open(&path).expect("open");
            store.revoke_holder(&holder()).expect("revoke holder");
        }
        let reloaded = FileRevocationStore::open(&path).expect("reopen");
        assert_eq!(
            reloaded.check_holder(&holder()),
            RevocationDecision::RevokedHolder
        );
        // A different holder is not revoked.
        let other = SigningKeyPair::from_bytes(&[0x78; 32]).signer_ref();
        assert_eq!(reloaded.check_holder(&other), RevocationDecision::Ok);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn file_store_clones_share_revocation_state() {
        let dir = std::env::temp_dir().join(format!("ledgerflow-clone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("revocations.jsonl");
        let _ = std::fs::remove_file(&path);

        let store = FileRevocationStore::open(&path).expect("open");
        let clone = store.clone();
        store.revoke_warrant(&[9_u8; 16]).expect("revoke");
        assert_eq!(clone.check_warrant(&[9_u8; 16]), RevocationDecision::RevokedWarrant);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn file_store_corrupt_record_is_a_hard_error() {
        let dir = std::env::temp_dir().join(format!("ledgerflow-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("revocations.jsonl");
        std::fs::write(&path, b"not-json\n").expect("write corrupt");

        let error = FileRevocationStore::open(&path).expect_err("corrupt is fatal");
        assert!(matches!(error, RevocationStoreError::Corrupt(_)));

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn insecure_memory_store_revokes_warrant_and_holder() {
        let mut store = InsecureMemoryRevocationStore::new();
        assert_eq!(store.check_warrant(&[1_u8; 16]), RevocationDecision::Ok);
        store.revoke_warrant(&[1_u8; 16]);
        assert_eq!(store.check_warrant(&[1_u8; 16]), RevocationDecision::RevokedWarrant);

        store.revoke_holder(&holder());
        assert_eq!(store.check_holder(&holder()), RevocationDecision::RevokedHolder);
    }

    #[test]
    fn signing_algorithm_is_ed25519_by_default() {
        assert_eq!(SigningAlgorithm::Ed25519.as_str(), "ed25519");
    }
}
