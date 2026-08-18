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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use ledgerflow_core::{PaymentSubjectKind, PaymentSubjectRef, SignerRef, SigningAlgorithm};

    use super::*;

    fn fingerprint(challenge: &str, nonce: &str) -> ReplayFingerprint {
        ReplayFingerprint {
            challenge_id: challenge.to_string(),
            nonce: nonce.to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
        }
    }

    #[test]
    fn fingerprint_key_uses_challenge_and_nonce() {
        let fp = fingerprint("c1", "n1");
        assert_eq!(fp.key(), ("c1".to_string(), "n1".to_string()));
        assert_ne!(fp.key(), ("c1".to_string(), "n2".to_string()));
        assert_ne!(fp.key(), ("c2".to_string(), "n1".to_string()));
    }

    #[test]
    fn claim_nonce_accepts_once_and_rejects_replay() {
        let mut store = InMemoryReplayStore::default();
        store.claim_nonce(fingerprint("c1", "n1"), 1_000).expect("first ok");
        let error = store.claim_nonce(fingerprint("c1", "n1"), 2_000).expect_err("replay");
        assert_eq!(error.existing.nonce, "n1");
    }

    #[test]
    fn claim_nonce_distinguishes_different_nonces() {
        let mut store = InMemoryReplayStore::default();
        store.claim_nonce(fingerprint("c1", "n1"), 1_000).expect("n1");
        store.claim_nonce(fingerprint("c1", "n2"), 1_000).expect("n2 is distinct");
        store.claim_nonce(fingerprint("c2", "n1"), 1_000).expect("c2 is distinct");
    }

    #[test]
    fn claim_nonce_expires_after_ttl() {
        let mut store = InMemoryReplayStore::with_ttl(1_000);
        store.claim_nonce(fingerprint("c1", "n1"), 1_000).expect("first");
        // 999ms later -> still within TTL (999 < 1000).
        store.claim_nonce(fingerprint("c1", "n2"), 1_999).expect("within ttl");
        // 1000ms later -> retained entry with 999ms age is still < 1000.
        // But a NEW claim at 2_001 for the same key is 1001ms old -> purged.
        let mut expired_store = InMemoryReplayStore::with_ttl(1_000);
        expired_store.claim_nonce(fingerprint("c1", "n1"), 1_000).expect("first");
        expired_store.claim_nonce(fingerprint("c1", "n1"), 2_001).expect("purged and re-claimable");
    }

    #[test]
    fn claim_nonce_at_exactly_ttl_is_expired() {
        // The retention check is `now - created < ttl` (strict): an entry
        // aged exactly `ttl` ms is purged.
        let mut store = InMemoryReplayStore::with_ttl(1_000);
        store.claim_nonce(fingerprint("c1", "n1"), 1_000).expect("first");
        store.claim_nonce(fingerprint("c1", "n1"), 2_000).expect("exactly ttl is expired");
    }

    #[test]
    fn payment_cache_round_trips() {
        let mut store = InMemoryReplayStore::default();
        assert!(store.cached_payment("p1").is_none());
        let auth = dummy_authorization();
        store.cache_payment("p1".to_string(), auth.clone());
        assert_eq!(store.cached_payment("p1"), Some(auth));
        assert!(store.cached_payment("p2").is_none());
    }

    fn dummy_authorization() -> VerifiedAuthorization {
        let holder = SignerRef::new(SigningAlgorithm::Ed25519, vec![1; 32]);
        let warrant = ledgerflow_core::Warrant {
            version: 1,
            id: vec![0xAB; 16],
            holder: holder.clone(),
            issuer: holder.clone(),
            issued_at: 1,
            expires_at: 2,
            depth: 0,
            max_depth: 1,
            parent_hash: None,
            merchant: ledgerflow_core::MerchantConstraint::with_ids(vec!["merchant-a".to_string()]),
            resource: ledgerflow_core::ResourceConstraint::default(),
            payment: ledgerflow_core::PaymentConstraint::new(100),
            tool: None,
            approval_gates: std::collections::BTreeMap::new(),
            required_approvers: Vec::new(),
            min_approvals: 0,
            extensions: std::collections::BTreeMap::new(),
            signature: ledgerflow_core::SignatureEnvelope {
                alg: SigningAlgorithm::Ed25519,
                value: vec![0; 64],
            },
        };
        VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            holder,
            leaf_warrant: warrant.clone(),
            root_warrant: warrant,
            chain_len: 1,
            amount: 100,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: ledgerflow_core::PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            warrant_digest: "sha256:w".to_string(),
        }
    }
}
