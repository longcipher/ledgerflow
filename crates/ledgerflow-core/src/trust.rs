//! Trusted-issuer anchors (trust model).
//!
//! Every verifier (merchant or Facilitator) configures a set of trusted
//! issuer public keys. The **root** warrant of any presented chain must have
//! been issued by one of these trusted issuers, otherwise the chain is
//! rejected (fail-closed). Key rotation is supported via `key_id`: a rotated
//! issuer simply replaces its entry in the set.

use serde::{Deserialize, Serialize};

use crate::{
    agent_identity::{AgentIdRef, IdentityResolver},
    error::{AuthorizationError, Result},
    warrant::{SignerRef, Warrant},
};

/// A single trusted issuer entry.
///
/// An entry may optionally be **anchored** to an EIP-8004 agent identity:
/// instead of (or in addition to) pinning one static key, the verifier
/// resolves the anchored identity's currently valid keys at verification
/// time via an [`IdentityResolver`]. This turns pairwise out-of-band trust
/// into discoverable trust and simplifies key rotation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TrustedIssuer {
    /// Machine-readable key id (used for rotation and audit).
    pub key_id: String,
    /// The issuer signer reference.
    pub issuer: SignerRef,
    /// Optional EIP-8004 agent identity anchoring this issuer.
    #[serde(default)]
    pub anchor: Option<AgentIdRef>,
}

impl TrustedIssuer {
    /// Creates a statically keyed trusted issuer (no anchor).
    #[must_use]
    pub const fn new(key_id: String, issuer: SignerRef) -> Self {
        Self { key_id, issuer, anchor: None }
    }

    /// Creates a trusted issuer anchored to an EIP-8004 agent identity.
    ///
    /// The `issuer` field is still required as the *bootstrap* key: it is
    /// accepted directly even before any resolution succeeds.
    #[must_use]
    pub const fn anchored(key_id: String, issuer: SignerRef, anchor: AgentIdRef) -> Self {
        Self { key_id, issuer, anchor: Some(anchor) }
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
        self.verify_root_with_resolver(root, None)
    }

    /// Verifies the root issuer against the trust set, optionally resolving
    /// EIP-8004 anchored identities through `resolver`.
    ///
    /// Acceptance order:
    ///
    /// 1. Direct static key match (bootstrap keys always work).
    /// 2. For each anchored entry: resolve the anchor's current keys and accept when the root
    ///    issuer matches any of them.
    ///
    /// Resolution failures are **not** silently skipped: an unreachable or
    /// unknown anchor fails closed with
    /// [`AuthorizationError::IdentityResolutionFailed`]. When no entry
    /// accepts the key the error is
    /// [`AuthorizationError::IssuerNotBoundToIdentity`] for anchored entries
    /// or [`AuthorizationError::UntrustedIssuer`] otherwise.
    pub fn verify_root_with_resolver(
        &self,
        root: &Warrant,
        resolver: Option<&dyn IdentityResolver>,
    ) -> Result<()> {
        if self.contains(&root.issuer) {
            return Ok(());
        }
        let mut saw_anchor = false;
        let mut last_anchor: Option<&AgentIdRef> = None;
        for entry in &self.issuers {
            let Some(anchor) = &entry.anchor else {
                continue;
            };
            saw_anchor = true;
            last_anchor = Some(anchor);
            let Some(resolver) = resolver else {
                continue;
            };
            let resolved = resolver.resolve_keys(anchor).map_err(|error| {
                AuthorizationError::IdentityResolutionFailed {
                    reference: anchor.to_string(),
                    detail: error.to_string(),
                }
            })?;
            if resolved
                .iter()
                .any(|key| key.alg == root.issuer.alg && key.public_key == root.issuer.public_key)
            {
                return Ok(());
            }
        }
        if saw_anchor {
            Err(AuthorizationError::IssuerNotBoundToIdentity {
                reference: last_anchor.map_or_else(String::new, std::string::ToString::to_string),
            })
        } else {
            Err(AuthorizationError::UntrustedIssuer {
                key_id: root.issuer.key_id.clone().unwrap_or_default(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::{agent_identity::AgentIdRef, warrant::SigningKeyPair};

    const ANCHOR: &str = "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/22";

    fn issuer_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x50; 32])
    }

    fn rotated_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[0x51; 32])
    }

    fn anchor_ref() -> AgentIdRef {
        AgentIdRef::parse(ANCHOR).expect("valid")
    }

    /// Resolver returning configured keys; `fail` forces resolution errors.
    struct MapResolver {
        keys: Vec<SignerRef>,
        fail: bool,
        calls: AtomicU32,
    }

    impl IdentityResolver for MapResolver {
        fn resolve_keys(&self, _agent: &AgentIdRef) -> crate::error::Result<Vec<SignerRef>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(AuthorizationError::IdentityResolutionFailed {
                    reference: ANCHOR.to_string(),
                    detail: "rpc unreachable".to_string(),
                });
            }
            Ok(self.keys.clone())
        }
    }

    fn anchored_set() -> TrustedIssuers {
        let mut set = TrustedIssuers::new();
        set.add(TrustedIssuer::anchored(
            "issuer-1".to_string(),
            issuer_keys().signer_ref(),
            anchor_ref(),
        ));
        set
    }

    #[test]
    fn bootstrap_key_is_accepted_without_resolution() {
        let set = anchored_set();
        let warrant = sample_root(issuer_keys());
        let resolver = MapResolver { keys: Vec::new(), fail: false, calls: AtomicU32::new(0) };
        set.verify_root_with_resolver(&warrant, Some(&resolver)).expect("bootstrap key");
        assert_eq!(resolver.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn rotated_key_accepted_via_anchor_resolution() {
        let set = anchored_set();
        let warrant = sample_root(rotated_keys());
        let resolver = MapResolver {
            keys: vec![rotated_keys().signer_ref()],
            fail: false,
            calls: AtomicU32::new(0),
        };
        set.verify_root_with_resolver(&warrant, Some(&resolver)).expect("resolved key");
        assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn unbound_key_fails_with_identity_error() {
        let set = anchored_set();
        let stranger = SigningKeyPair::from_bytes(&[0x52; 32]);
        let warrant = sample_root(stranger);
        let resolver = MapResolver {
            keys: vec![rotated_keys().signer_ref()],
            fail: false,
            calls: AtomicU32::new(0),
        };
        let error =
            set.verify_root_with_resolver(&warrant, Some(&resolver)).expect_err("not bound");
        assert!(matches!(error, AuthorizationError::IssuerNotBoundToIdentity { .. }));
    }

    #[test]
    fn resolution_failure_fails_closed() {
        let set = anchored_set();
        let warrant = sample_root(rotated_keys());
        let resolver = MapResolver { keys: Vec::new(), fail: true, calls: AtomicU32::new(0) };
        let error =
            set.verify_root_with_resolver(&warrant, Some(&resolver)).expect_err("resolver down");
        assert!(matches!(error, AuthorizationError::IdentityResolutionFailed { .. }));
    }

    #[test]
    fn anchored_entry_without_resolver_is_rejected_for_foreign_keys() {
        let set = anchored_set();
        let warrant = sample_root(rotated_keys());
        // No resolver available: anchors cannot be consulted.
        let error = set.verify_root_with_resolver(&warrant, None).expect_err("no resolver");
        assert!(matches!(error, AuthorizationError::IssuerNotBoundToIdentity { .. }));
    }

    #[test]
    fn static_only_set_preserves_untrusted_error() {
        let mut set = TrustedIssuers::new();
        set.add(TrustedIssuer::new("a".to_string(), issuer_keys().signer_ref()));
        let warrant = sample_root(rotated_keys());
        let error = set.verify_root(&warrant).expect_err("untrusted");
        assert!(matches!(error, AuthorizationError::UntrustedIssuer { .. }));
    }

    #[test]
    fn empty_set_still_fail_closed() {
        let set = TrustedIssuers::new();
        let warrant = sample_root(issuer_keys());
        assert!(set.verify_root(&warrant).is_err());
        // is_empty reflects the exact emptiness state.
        assert!(set.is_empty());
        let mut populated = TrustedIssuers::new();
        populated.add(TrustedIssuer::new("a".to_string(), issuer_keys().signer_ref()));
        assert!(!populated.is_empty());
    }

    #[test]
    fn serde_round_trips_anchor() {
        let entry =
            TrustedIssuer::anchored("k".to_string(), issuer_keys().signer_ref(), anchor_ref());
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("eip155:1:0x8004a169"));
        let back: TrustedIssuer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, entry);
        // Legacy entries without the anchor field deserialize to None.
        let legacy = r#"{"key_id":"k","issuer":{"alg":"Ed25519","public_key":[1,2,3]}}"#;
        let parsed: TrustedIssuer = serde_json::from_str(legacy).expect("legacy");
        assert!(parsed.anchor.is_none());
    }

    fn sample_root(keys: SigningKeyPair) -> Warrant {
        use crate::constraint::{MerchantConstraint, PaymentConstraint, ResourceConstraint};
        crate::typestate::WarrantBuilder::new(1_000)
            .issuer(keys.signer_ref())
            .holder(SigningKeyPair::from_bytes(&[0x60; 32]).signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["m".to_string()]))
            .resource(ResourceConstraint::default())
            .payment(PaymentConstraint::new(100))
            .sign_with(&keys, [0_u8; 8])
    }
}
