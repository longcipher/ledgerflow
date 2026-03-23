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
    ) -> std::result::Result<(), ReplayConflict>;
    fn cached_payment(&self, payment_identifier: &str) -> Option<VerifiedAuthorization>;
    fn cache_payment(&mut self, payment_identifier: String, authorization: VerifiedAuthorization);
}

/// In-memory replay/idempotency store used by tests and local flows.
#[derive(Clone, Debug, Default)]
pub struct InMemoryReplayStore {
    nonce_claims: BTreeMap<(String, String), ReplayFingerprint>,
    payment_results: BTreeMap<String, VerifiedAuthorization>,
}

impl ReplayStore for InMemoryReplayStore {
    fn claim_nonce(
        &mut self,
        fingerprint: ReplayFingerprint,
    ) -> std::result::Result<(), ReplayConflict> {
        let key = fingerprint.key();
        if let Some(existing) = self.nonce_claims.get(&key) {
            return Err(ReplayConflict { existing: existing.clone() });
        }

        self.nonce_claims.insert(key, fingerprint);
        Ok(())
    }

    fn cached_payment(&self, payment_identifier: &str) -> Option<VerifiedAuthorization> {
        self.payment_results.get(payment_identifier).cloned()
    }

    fn cache_payment(&mut self, payment_identifier: String, authorization: VerifiedAuthorization) {
        self.payment_results.insert(payment_identifier, authorization);
    }
}
