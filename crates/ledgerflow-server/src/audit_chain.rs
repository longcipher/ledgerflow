//! Tamper-evident append-only audit log.
//!
//! This module implements a hash-chained audit log where each entry
//! cryptographically links to the previous one. Any modification of
//! historical entries breaks the chain and is detectable.
//!
//! Design:
//! - Each entry contains: sequence number, timestamp, event data, previous hash
//! - The hash chain: `entry_hash = SHA256(prev_hash || entry_data)`
//! - Verification: recompute the chain and compare hashes

use std::{
    collections::VecDeque,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Maximum number of entries to keep in memory.
const MAX_BUFFERED_ENTRIES: usize = 10_000;

/// A single audit log entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// The event data (JSON-serialized).
    pub event_data: String,
    /// Hash of the previous entry (32 bytes, hex-encoded).
    pub previous_hash: String,
    /// Hash of this entry (32 bytes, hex-encoded).
    pub entry_hash: String,
}

impl AuditEntry {
    /// Computes the hash of this entry's content.
    #[must_use]
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.sequence.to_le_bytes());
        hasher.update(self.timestamp_ms.to_le_bytes());
        hasher.update(self.event_data.as_bytes());
        hasher.update(self.previous_hash.as_bytes());
        hex_encode(&hasher.finalize())
    }

    /// Verifies that the entry hash matches its content.
    #[must_use]
    pub fn verify(&self) -> bool {
        self.entry_hash == self.compute_hash()
    }
}

/// Tamper-evident audit log.
#[derive(Clone, Debug)]
pub struct AuditChain {
    /// Buffered entries (most recent last).
    entries: VecDeque<AuditEntry>,
    /// Next sequence number.
    next_sequence: u64,
    /// Hash of the last entry (empty for genesis).
    last_hash: String,
}

impl Default for AuditChain {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditChain {
    /// Creates a new empty audit chain.
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: VecDeque::new(), next_sequence: 1, last_hash: String::new() }
    }

    /// Appends a new entry to the chain.
    pub fn append(&mut self, event_data: impl Into<String>) -> AuditEntry {
        let event_data = event_data.into();
        let timestamp_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);

        let entry = AuditEntry {
            sequence: self.next_sequence,
            timestamp_ms,
            event_data,
            previous_hash: self.last_hash.clone(),
            entry_hash: String::new(),
        };

        let entry_hash = entry.compute_hash();
        let mut entry = entry;
        entry.entry_hash.clone_from(&entry_hash);

        self.entries.push_back(entry.clone());
        self.last_hash = entry_hash;
        self.next_sequence += 1;

        // Trim old entries if buffer is full
        if self.entries.len() > MAX_BUFFERED_ENTRIES {
            self.entries.pop_front();
        }

        entry
    }

    /// Verifies the integrity of the entire chain.
    #[must_use]
    pub fn verify(&self) -> bool {
        let mut prev_hash = String::new();

        for entry in &self.entries {
            if entry.previous_hash != prev_hash {
                return false;
            }
            if !entry.verify() {
                return false;
            }
            prev_hash.clone_from(&entry.entry_hash);
        }

        true
    }

    /// Returns the number of entries in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all entries (oldest first).
    #[must_use]
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.iter().cloned().collect()
    }

    /// Returns the last entry.
    #[must_use]
    pub fn last(&self) -> Option<AuditEntry> {
        self.entries.back().cloned()
    }

    /// Returns the current chain head hash.
    #[must_use]
    pub fn head_hash(&self) -> &str {
        &self.last_hash
    }

    /// Tamper with an entry (for testing).
    #[cfg(test)]
    pub fn tamper_entry(&mut self, index: usize, new_data: &str) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.event_data = new_data.to_string();
        }
    }
}

/// Hex-encodes bytes (delegates to core).
fn hex_encode(bytes: &[u8]) -> String {
    ledgerflow_core::hex_encode_bytes(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn empty_chain_is_valid() {
        let chain = AuditChain::new();
        assert!(chain.verify());
        assert!(chain.is_empty());
    }

    #[test]
    fn single_entry_chain_is_valid() {
        let mut chain = AuditChain::new();
        chain.append("test event");
        assert!(chain.verify());
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn multiple_entries_chain_is_valid() {
        let mut chain = AuditChain::new();
        for i in 0..10 {
            chain.append(format!("event {i}"));
        }
        assert!(chain.verify());
        assert_eq!(chain.len(), 10);
    }

    #[test]
    fn tampered_entry_breaks_chain() {
        let mut chain = AuditChain::new();
        chain.append("event 1");
        chain.append("event 2");
        chain.append("event 3");

        // Tamper with the second entry
        chain.tamper_entry(1, "tampered");

        // The chain should now fail verification
        assert!(!chain.verify());
    }

    #[test]
    fn entry_hash_is_content_bound() {
        let mut chain = AuditChain::new();
        let entry = chain.append("test");

        // Same content should produce same hash
        let mut chain2 = AuditChain::new();
        let entry2 = chain2.append("test");

        // Hashes should be different because timestamps may differ
        // But each entry's hash should match its own content
        assert!(entry.verify());
        assert!(entry2.verify());
    }

    #[test]
    fn chain_head_hash_tracks_last_entry() {
        let mut chain = AuditChain::new();
        assert_eq!(chain.head_hash(), "");

        chain.append("event 1");
        let head1 = chain.head_hash().to_string();
        assert!(!head1.is_empty());

        chain.append("event 2");
        let head2 = chain.head_hash().to_string();
        assert_ne!(head1, head2);
    }
}
