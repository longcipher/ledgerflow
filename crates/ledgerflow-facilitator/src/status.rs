//! Settlement status registry (idempotent `/status` queries).

use std::collections::BTreeMap;
use std::sync::Mutex;

use crate::{
    outcome::SettlementStatus,
    rails::SettlementReceipt,
};

/// A single registry entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    pub receipt: SettlementReceipt,
    pub status: SettlementStatus,
}

/// In-memory settlement registry with idempotent lookups.
///
/// The registry is intentionally in-memory in v1; a persistent registry is a
/// deployment concern handled by `ledgerflow-server` (P3).
#[derive(Debug, Default)]
pub struct SettlementRegistry {
    inner: std::sync::Arc<SettlementRegistryInner>,
}

/// Shared registry storage.
#[derive(Debug, Default)]
struct SettlementRegistryInner {
    by_transaction: Mutex<BTreeMap<String, RegistryEntry>>,
    by_warrant: Mutex<BTreeMap<String, Vec<String>>>,
}

impl Clone for SettlementRegistry {
    fn clone(&self) -> Self {
        Self { inner: std::sync::Arc::clone(&self.inner) }
    }
}

impl SettlementRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self { inner: std::sync::Arc::new(SettlementRegistryInner::default()) }
    }

    /// Records a settlement outcome (idempotent by transaction id).
    pub fn record(&self, warrant_digest: &str, receipt: SettlementReceipt, status: SettlementStatus) {
        let transaction_id = receipt.transaction_id.clone();
        if let Ok(mut map) = self.inner.by_transaction.lock() {
            map.insert(transaction_id.clone(), RegistryEntry { receipt, status });
        }
        if let Ok(mut map) = self.inner.by_warrant.lock() {
            let ids = map.entry(warrant_digest.to_string()).or_default();
            if !ids.contains(&transaction_id) {
                ids.push(transaction_id);
            }
        }
    }

    /// Queries a single settlement by transaction id.
    pub fn query(&self, transaction_id: &str) -> Option<RegistryEntry> {
        self.inner
            .by_transaction
            .lock()
            .ok()
            .and_then(|map| map.get(transaction_id).cloned())
    }

    /// Queries all settlements for a warrant digest.
    pub fn query_by_warrant(&self, warrant_digest: &str) -> Vec<RegistryEntry> {
        let transaction_ids = self
            .inner
            .by_warrant
            .lock()
            .ok()
            .and_then(|map| map.get(warrant_digest).cloned())
            .unwrap_or_default();
        let map = self.inner.by_transaction.lock().ok();
        transaction_ids
            .into_iter()
            .filter_map(|id| map.as_ref().and_then(|m| m.get(&id).cloned()))
            .collect()
    }
}
