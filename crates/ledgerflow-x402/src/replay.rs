//! Replay protection and idempotency helpers for merchant verification.

use std::collections::BTreeMap;

use ledgerflow_core::VerifiedAuthorization;

/// Uniquely identifies a proof submission for replay detection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayFingerprint {
    pub challenge_id: String,
    pub nonce: String,
    pub request_hash: String,
    pub accepted_hash: String,
}

impl ReplayFingerprint {
    #[must_use]
    pub fn key(&self) -> (String, String) {
        (self.challenge_id.clone(), self.nonce.clone())
    }
}

/// A nonce claim with a creation timestamp for TTL-based expiry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonceClaim {
    pub fingerprint: ReplayFingerprint,
    pub created_at_ms: u64,
}

/// Replay conflict returned when a nonce was already observed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayConflict {
    pub existing: ReplayFingerprint,
}

/// Storage seam for nonce-based replay protection and payment-id idempotency.
pub trait ReplayStore {
    fn claim_nonce(
        &mut self,
        fingerprint: ReplayFingerprint,
        now_ms: u64,
    ) -> std::result::Result<(), ReplayConflict>;
    fn cached_payment(&self, payment_identifier: &str) -> Option<VerifiedAuthorization>;
    fn cache_payment(&mut self, payment_identifier: String, authorization: VerifiedAuthorization);
}

const DEFAULT_TTL_MS: u64 = 300_000;

/// In-memory replay/idempotency store used by tests and local flows.
#[derive(Clone, Debug)]
pub struct InMemoryReplayStore {
    nonce_claims: BTreeMap<(String, String), NonceClaim>,
    payment_results: BTreeMap<String, VerifiedAuthorization>,
    ttl_ms: u64,
}

impl InMemoryReplayStore {
    /// Create a new store with the specified TTL in milliseconds.
    #[must_use]
    pub const fn with_ttl(ttl_ms: u64) -> Self {
        Self { nonce_claims: BTreeMap::new(), payment_results: BTreeMap::new(), ttl_ms }
    }
}

impl Default for InMemoryReplayStore {
    fn default() -> Self {
        Self::with_ttl(DEFAULT_TTL_MS)
    }
}

impl ReplayStore for InMemoryReplayStore {
    fn claim_nonce(
        &mut self,
        fingerprint: ReplayFingerprint,
        now_ms: u64,
    ) -> std::result::Result<(), ReplayConflict> {
        self.nonce_claims
            .retain(|_, claim| now_ms.saturating_sub(claim.created_at_ms) < self.ttl_ms);

        let key = fingerprint.key();
        if let Some(existing) = self.nonce_claims.get(&key) {
            return Err(ReplayConflict { existing: existing.fingerprint.clone() });
        }

        self.nonce_claims.insert(key, NonceClaim { fingerprint, created_at_ms: now_ms });
        Ok(())
    }

    fn cached_payment(&self, payment_identifier: &str) -> Option<VerifiedAuthorization> {
        self.payment_results.get(payment_identifier).cloned()
    }

    fn cache_payment(&mut self, payment_identifier: String, authorization: VerifiedAuthorization) {
        self.payment_results.insert(payment_identifier, authorization);
    }
}
