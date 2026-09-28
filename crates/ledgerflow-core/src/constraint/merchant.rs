//! Merchant allowlist constraint.

use serde::{Deserialize, Serialize};

/// Merchant allowlist constraint (exact ids and/or host suffixes).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct MerchantConstraint {
    pub merchant_ids: Vec<String>,
    pub host_suffixes: Vec<String>,
}

impl MerchantConstraint {
    #[must_use]
    pub const fn new() -> Self {
        Self { merchant_ids: Vec::new(), host_suffixes: Vec::new() }
    }

    #[must_use]
    pub fn with_ids(ids: impl IntoIterator<Item = String>) -> Self {
        Self { merchant_ids: ids.into_iter().collect(), host_suffixes: Vec::new() }
    }

    #[must_use]
    pub fn with_host_suffixes(suffixes: impl IntoIterator<Item = String>) -> Self {
        Self { merchant_ids: Vec::new(), host_suffixes: suffixes.into_iter().collect() }
    }

    /// Returns `true` when this constraint is satisfied by the context.
    pub fn allows(&self, merchant_id: &str, merchant_host: &str) -> bool {
        let id_ok =
            self.merchant_ids.is_empty() || self.merchant_ids.iter().any(|id| id == merchant_id);
        let host_ok = self.host_suffixes.is_empty() ||
            self.host_suffixes.iter().any(|suffix| merchant_host.ends_with(suffix));
        id_ok && host_ok
    }
}
