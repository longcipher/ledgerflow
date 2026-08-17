//! Human-in-the-loop approval gates (m-of-n multi-signature).
//!
//! A warrant may declare that certain tool calls (matching optional argument
//! constraints) require approval from `required_approvers`, with a threshold
//! of `min_approvals`. Approvals are single-layer signatures: they cannot be
//! delegated, and only keys listed in `required_approvers` are accepted.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    error::{AuthorizationError, Result},
    pop::PopTuple,
    warrant::{SignatureEnvelope, SignerRef, SigningKeyPair, sha256_prefixed},
};

/// Domain-separation prefix for approval signatures.
pub const APPROVAL_SIGN_DOMAIN: &[u8] = b"ledgerflow-approval-v1";

/// Default approval TTL (300 seconds).
pub const DEFAULT_APPROVAL_TTL_SECS: u64 = 300;

/// A signed human approval for a specific payment request.
///
/// The signature covers `APPROVAL_SIGN_DOMAIN || request_hash || approver
/// public key || expires_at`, so an approval is bound to exactly one request
/// and one approver.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignedApproval {
    /// Digest of the request being approved (the PoP tuple's request binding).
    pub request_hash: String,
    /// The approver's public key.
    pub approver: SignerRef,
    /// Unix seconds when the approval expires.
    pub expires_at: u64,
    /// The approval signature.
    pub signature: SignatureEnvelope,
}

impl SignedApproval {
    /// Signs an approval for a request using the approver's key pair.
    #[must_use]
    pub fn sign(
        request_hash: impl Into<String>,
        approver: &SignerRef,
        expires_at: u64,
        approver_keys: &SigningKeyPair,
    ) -> Self {
        let request_hash = request_hash.into();
        let preimage = approval_preimage(&request_hash, approver, expires_at);
        let signature = approver_keys.sign(&preimage);
        Self { request_hash, approver: approver.clone(), expires_at, signature }
    }

    /// Computes the domain-separated approval signing preimage.
    #[must_use]
    pub fn preimage(&self) -> Vec<u8> {
        approval_preimage(&self.request_hash, &self.approver, self.expires_at)
    }

    /// Verifies this approval signature using **strict** Ed25519 verification.
    pub fn verify_signature(&self) -> bool {
        self.signature.verify_strict(&self.approver, &self.preimage())
    }

    /// CBOR-encodes this approval (used for `approvals_digest` and transport).
    pub fn encode_cbor(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        #[allow(clippy::expect_used)]
        ciborium::ser::into_writer(self, &mut bytes)
            .expect("approval serialization is infallible");
        bytes
    }
}

fn approval_preimage(request_hash: &str, approver: &SignerRef, expires_at: u64) -> Vec<u8> {
    let mut preimage = Vec::with_capacity(APPROVAL_SIGN_DOMAIN.len() + 128);
    preimage.extend_from_slice(APPROVAL_SIGN_DOMAIN);
    preimage.extend_from_slice(request_hash.as_bytes());
    preimage.extend_from_slice(&approver.public_key);
    preimage.extend_from_slice(&expires_at.to_le_bytes());
    preimage
}

/// A gate configuration attached to a tool.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalGate {
    /// Optional argument constraints that trigger the gate.
    ///
    /// An empty map means "every invocation of this tool requires approval".
    pub argument_constraints: BTreeMap<String, String>,
}

impl ApprovalGate {
    /// Creates an unconditional gate for the tool.
    #[must_use]
    pub const fn unconditional() -> Self {
        Self { argument_constraints: BTreeMap::new() }
    }

    /// Returns `true` when this gate fires for the given tool arguments.
    ///
    /// The simplified v1 semantics: the gate fires when the constraint map is
    /// empty (approve every call) or when any configured argument constraint
    /// matches the supplied arguments. Constraint syntax is a small
    /// `key=value` allowlist.
    #[must_use]
    pub fn fires(&self, arguments: &BTreeMap<String, String>) -> bool {
        if self.argument_constraints.is_empty() {
            return true;
        }
        self.argument_constraints
            .iter()
            .all(|(key, expected)| arguments.get(key).is_some_and(|actual| actual == expected))
    }
}

/// Outcome of approval verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalVerification {
    /// Number of distinct valid approvers.
    pub valid_count: u32,
    /// The threshold that must be met.
    pub threshold: u32,
}

/// Verifies that a set of approvals satisfies the warrant's m-of-n threshold
/// for the given request and tool invocation.
///
/// - Every approval must be signed by a key in `required_approvers`.
/// - Every approval must be unexpired and must bind the request hash.
/// - Duplicate approvers count once.
/// - At least `min_approvals` distinct approvers are required.
/// - The PoP tuple's `approvals_digest` must match the supplied approvals,
///   closing the PoP/approvals ambiguity window.
pub fn verify_approvals(
    approvals: &[SignedApproval],
    required_approvers: &[SignerRef],
    min_approvals: u32,
    request_hash: &str,
    now_ms: u64,
    pop_tuple: &PopTuple,
) -> Result<ApprovalVerification> {
    if approvals.is_empty() {
        return Err(AuthorizationError::ApprovalRequired);
    }

    // The PoP must have been bound to these exact approvals.
    let expected_digest = PopTuple::approvals_digest(approvals);
    if pop_tuple.approvals_digest.as_deref() != Some(expected_digest.as_str()) {
        return Err(AuthorizationError::ApprovalsDigestMismatch);
    }

    let now_secs = now_ms / 1000;
    let mut valid: Vec<Vec<u8>> = Vec::new();
    for approval in approvals {
        if approval.request_hash != request_hash {
            return Err(AuthorizationError::ApprovalRequestMismatch);
        }
        if approval.expires_at < now_secs {
            return Err(AuthorizationError::ApprovalExpired);
        }
        if !required_approvers.contains(&approval.approver) {
            return Err(AuthorizationError::ApproverNotAllowed);
        }
        if !approval.verify_signature() {
            return Err(AuthorizationError::InvalidApprovalSignature);
        }
        if !valid.iter().any(|key| key == &approval.approver.public_key) {
            valid.push(approval.approver.public_key.clone());
        }
    }

    let valid_count = valid.len() as u32;
    let threshold = if min_approvals == 0 {
        required_approvers.len() as u32
    } else {
        min_approvals
    };
    if valid_count < threshold {
        return Err(AuthorizationError::InsufficientApprovals {
            got: valid_count,
            need: threshold,
        });
    }

    Ok(ApprovalVerification { valid_count, threshold })
}

/// Verifies the m-of-n threshold only (used when gates did not fire).
pub fn verify_approval_threshold(
    approvals: &[SignedApproval],
    required_approvers: &[SignerRef],
    min_approvals: u32,
    request_hash: &str,
    now_ms: u64,
) -> Result<ApprovalVerification> {
    // Digest check is skipped here; callers that also possess the PoP tuple
    // should use [`verify_approvals`].
    let now_secs = now_ms / 1000;
    let mut valid: Vec<Vec<u8>> = Vec::new();
    for approval in approvals {
        if approval.request_hash != request_hash {
            return Err(AuthorizationError::ApprovalRequestMismatch);
        }
        if approval.expires_at < now_secs {
            return Err(AuthorizationError::ApprovalExpired);
        }
        if !required_approvers.contains(&approval.approver) {
            return Err(AuthorizationError::ApproverNotAllowed);
        }
        if !approval.verify_signature() {
            return Err(AuthorizationError::InvalidApprovalSignature);
        }
        if !valid.iter().any(|key| key == &approval.approver.public_key) {
            valid.push(approval.approver.public_key.clone());
        }
    }

    let valid_count = valid.len() as u32;
    let threshold = if min_approvals == 0 {
        required_approvers.len() as u32
    } else {
        min_approvals
    };
    if valid_count < threshold {
        return Err(AuthorizationError::InsufficientApprovals {
            got: valid_count,
            need: threshold,
        });
    }

    Ok(ApprovalVerification { valid_count, threshold })
}

/// Computes a canonical request hash helper for approval flows.
///
/// This mirrors how the x402 layer computes request hashes so approvals bind
/// to the same value.
#[must_use]
pub fn approval_request_hash(preimage: &[u8]) -> String {
    sha256_prefixed(preimage)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn approver_keys(tag: u8) -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[tag; 32])
    }

    fn approval(request_hash: &str, approver: &SigningKeyPair, expires_at: u64) -> SignedApproval {
        SignedApproval::sign(request_hash, &approver.signer_ref(), expires_at, approver)
    }

    fn approvers(keys: &[&SigningKeyPair]) -> Vec<SignerRef> {
        keys.iter().map(|key| key.signer_ref()).collect()
    }

    #[test]
    fn threshold_passes_with_enough_distinct_approvers() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let required = approvers(&[&a, &b]);
        let now_ms = 10_000;
        let approvals = vec![
            approval("sha256:req", &a, now_ms / 1000 + 300),
            approval("sha256:req", &b, now_ms / 1000 + 300),
        ];
        let result = verify_approval_threshold(&approvals, &required, 2, "sha256:req", now_ms)
            .expect("2-of-2");
        assert_eq!(result.valid_count, 2);
        assert_eq!(result.threshold, 2);
    }

    #[test]
    fn threshold_uses_all_approvers_when_min_is_zero() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let c = approver_keys(3);
        let required = approvers(&[&a, &b, &c]);
        let now_ms = 10_000;
        // min_approvals = 0 => threshold = all required approvers (3).
        let approvals = vec![
            approval("sha256:req", &a, now_ms / 1000 + 300),
            approval("sha256:req", &b, now_ms / 1000 + 300),
        ];
        let error = verify_approval_threshold(&approvals, &required, 0, "sha256:req", now_ms)
            .expect_err("2 of 3 insufficient");
        assert!(matches!(error, AuthorizationError::InsufficientApprovals { .. }));
    }

    #[test]
    fn threshold_rejects_request_mismatch() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:other", &a, 10_300)];
        let error = verify_approval_threshold(&approvals, &required, 1, "sha256:req", 10_000)
            .expect_err("request mismatch");
        assert_eq!(error, AuthorizationError::ApprovalRequestMismatch);
    }

    #[test]
    fn threshold_rejects_expired_approval() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        // expires_at (10_000/1000 = 10) < now 10_000 => expired.
        let approvals = vec![approval("sha256:req", &a, 9)];
        let error = verify_approval_threshold(&approvals, &required, 1, "sha256:req", 10_000)
            .expect_err("expired");
        assert_eq!(error, AuthorizationError::ApprovalExpired);
    }

    #[test]
    fn threshold_rejects_unknown_approver() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &b, 10_300)];
        let error = verify_approval_threshold(&approvals, &required, 1, "sha256:req", 10_000)
            .expect_err("not allowed");
        assert_eq!(error, AuthorizationError::ApproverNotAllowed);
    }

    #[test]
    fn threshold_rejects_invalid_signature() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let mut forged = approval("sha256:req", &a, 10_300);
        forged.signature.value = vec![0xFF; 64];
        let error = verify_approval_threshold(&[forged], &required, 1, "sha256:req", 10_000)
            .expect_err("invalid signature");
        assert_eq!(error, AuthorizationError::InvalidApprovalSignature);
    }

    #[test]
    fn threshold_counts_duplicate_approver_once() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let required = approvers(&[&a, &b]);
        // Two approvals from the same approver => only 1 distinct.
        let approvals = vec![
            approval("sha256:req", &a, 10_300),
            approval("sha256:req", &a, 10_300),
        ];
        let error = verify_approval_threshold(&approvals, &required, 2, "sha256:req", 10_000)
            .expect_err("duplicate counts once");
        assert!(matches!(error, AuthorizationError::InsufficientApprovals { got: 1, .. }));
    }

    #[test]
    fn threshold_accepts_mixed_valid_approvals() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let c = approver_keys(3);
        let required = approvers(&[&a, &b, &c]);
        // 2-of-3, with a duplicate b entry; valid distinct = 2 (a, b).
        let approvals = vec![
            approval("sha256:req", &a, 10_300),
            approval("sha256:req", &b, 10_300),
            approval("sha256:req", &b, 10_300),
        ];
        let result = verify_approval_threshold(&approvals, &required, 2, "sha256:req", 10_000)
            .expect("2 distinct of 3");
        assert_eq!(result.valid_count, 2);
    }

    #[test]
    fn threshold_rejects_insufficient_valid_count() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &a, 10_300)];
        let error = verify_approval_threshold(&approvals, &required, 2, "sha256:req", 10_000)
            .expect_err("need 2 got 1");
        assert!(matches!(
            error,
            AuthorizationError::InsufficientApprovals { got: 1, need: 2 }
        ));
    }

    #[test]
    fn approval_request_hash_is_domain_prefixed_sha256() {
        let hash = approval_request_hash(b"preimage");
        assert!(hash.starts_with("sha256:"));
        assert_eq!(hash, sha256_prefixed(b"preimage"));
        assert_ne!(hash, sha256_prefixed(b"other"));
    }

    #[test]
    fn signed_approval_round_trips_cbor() {
        let a = approver_keys(1);
        let signed = approval("sha256:req", &a, 10_300);
        let bytes = signed.encode_cbor();
        let decoded: SignedApproval = ciborium::de::from_reader(bytes.as_slice()).expect("decode");
        assert_eq!(decoded, signed);
        assert!(decoded.verify_signature());
    }

    #[test]
    fn approval_gate_fires_for_empty_constraints() {
        let gate = ApprovalGate::unconditional();
        assert!(gate.fires(&BTreeMap::new()));
        assert!(gate.fires(&BTreeMap::from([("amount".to_string(), "50".to_string())])));
    }

    #[test]
    fn approval_gate_fires_only_when_all_constraints_match() {
        let gate = ApprovalGate {
            argument_constraints: BTreeMap::from([
                ("env".to_string(), "prod".to_string()),
                ("amount".to_string(), "100".to_string()),
            ]),
        };
        assert!(gate.fires(&BTreeMap::from([
            ("env".to_string(), "prod".to_string()),
            ("amount".to_string(), "100".to_string()),
        ])));
        assert!(!gate.fires(&BTreeMap::from([
            ("env".to_string(), "staging".to_string()),
            ("amount".to_string(), "100".to_string()),
        ])));
        assert!(!gate.fires(&BTreeMap::from([("env".to_string(), "prod".to_string())])));
    }

    #[test]
    fn approval_verification_round_trips() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &a, 10_300)];
        let result = verify_approval_threshold(&approvals, &required, 1, "sha256:req", 10_000)
            .expect("ok");
        assert_eq!(
            result,
            ApprovalVerification { valid_count: 1, threshold: 1 }
        );
    }

    // ---------------------------------------------------------------------
    // verify_approvals (with PoP digest binding)
    // ---------------------------------------------------------------------

    fn pop_tuple(request_hash: &str, approvals: &[SignedApproval]) -> PopTuple {
        PopTuple {
            warrant_id: vec![1_u8; 16],
            challenge_id: "challenge-1".to_string(),
            method: "POST".to_string(),
            uri: "merchant-a.example/pay".to_string(),
            request_hash: request_hash.to_string(),
            accepted_hash: "accepted-hash".to_string(),
            payment_payload_digest: "payment-digest".to_string(),
            approvals_digest: Some(PopTuple::approvals_digest(approvals)),
            nonce: "nonce-1".to_string(),
            created_at_ms: 10_000,
        }
    }

    #[test]
    fn verify_approvals_passes_with_valid_digest() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let required = approvers(&[&a, &b]);
        let approvals = vec![
            approval("sha256:req", &a, 10_300),
            approval("sha256:req", &b, 10_300),
        ];
        let tuple = pop_tuple("sha256:req", &approvals);
        let result = verify_approvals(&approvals, &required, 2, "sha256:req", 10_000, &tuple)
            .expect("2-of-2");
        assert_eq!(result.valid_count, 2);
    }

    #[test]
    fn verify_approvals_rejects_digest_mismatch() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &a, 10_300)];
        // Tuple binds a DIFFERENT approvals set (empty).
        let mut tuple = pop_tuple("sha256:req", &approvals);
        tuple.approvals_digest = None;
        let error = verify_approvals(&approvals, &required, 1, "sha256:req", 10_000, &tuple)
            .expect_err("digest mismatch");
        assert_eq!(error, AuthorizationError::ApprovalsDigestMismatch);
    }

    #[test]
    fn verify_approvals_rejects_empty() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let tuple = pop_tuple("sha256:req", &[]);
        let error =
            verify_approvals(&[], &required, 1, "sha256:req", 10_000, &tuple).expect_err("empty");
        assert_eq!(error, AuthorizationError::ApprovalRequired);
    }

    #[test]
    fn verify_approvals_rejects_request_mismatch() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:other", &a, 10_300)];
        let tuple = pop_tuple("sha256:req", &approvals);
        let error = verify_approvals(&approvals, &required, 1, "sha256:req", 10_000, &tuple)
            .expect_err("request mismatch");
        assert_eq!(error, AuthorizationError::ApprovalRequestMismatch);
    }

    #[test]
    fn verify_approvals_rejects_expired() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &a, 9)];
        let tuple = pop_tuple("sha256:req", &approvals);
        let error = verify_approvals(&approvals, &required, 1, "sha256:req", 10_000, &tuple)
            .expect_err("expired");
        assert_eq!(error, AuthorizationError::ApprovalExpired);
    }

    #[test]
    fn verify_approvals_rejects_unknown_approver() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &b, 10_300)];
        let tuple = pop_tuple("sha256:req", &approvals);
        let error = verify_approvals(&approvals, &required, 1, "sha256:req", 10_000, &tuple)
            .expect_err("not allowed");
        assert_eq!(error, AuthorizationError::ApproverNotAllowed);
    }

    #[test]
    fn verify_approvals_rejects_invalid_signature() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let mut forged = approval("sha256:req", &a, 10_300);
        forged.signature.value = vec![0xFF; 64];
        let approvals = vec![forged];
        let tuple = pop_tuple("sha256:req", &approvals);
        let error = verify_approvals(&approvals, &required, 1, "sha256:req", 10_000, &tuple)
            .expect_err("invalid signature");
        assert_eq!(error, AuthorizationError::InvalidApprovalSignature);
    }

    #[test]
    fn verify_approvals_uses_all_approvers_when_min_zero() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let c = approver_keys(3);
        let required = approvers(&[&a, &b, &c]);
        let approvals = vec![
            approval("sha256:req", &a, 10_300),
            approval("sha256:req", &b, 10_300),
        ];
        let tuple = pop_tuple("sha256:req", &approvals);
        let error = verify_approvals(&approvals, &required, 0, "sha256:req", 10_000, &tuple)
            .expect_err("2 of 3");
        assert!(matches!(error, AuthorizationError::InsufficientApprovals { .. }));
    }

    #[test]
    fn verify_approvals_rejects_duplicate_approver() {
        let a = approver_keys(1);
        let b = approver_keys(2);
        let required = approvers(&[&a, &b]);
        let approvals = vec![
            approval("sha256:req", &a, 10_300),
            approval("sha256:req", &a, 10_300),
        ];
        let tuple = pop_tuple("sha256:req", &approvals);
        let error = verify_approvals(&approvals, &required, 2, "sha256:req", 10_000, &tuple)
            .expect_err("duplicate counts once");
        assert!(matches!(
            error,
            AuthorizationError::InsufficientApprovals { got: 1, .. }
        ));
    }

    #[test]
    fn threshold_accepts_approval_expiring_at_now() {
        // The check is `expires_at < now_secs` (strict): an approval expiring
        // exactly at the current second is still valid.
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &a, 10)];
        let result = verify_approval_threshold(&approvals, &required, 1, "sha256:req", 10_000)
            .expect("expires_at == now is valid");
        assert_eq!(result.valid_count, 1);
    }

    #[test]
    fn verify_approvals_accepts_expiring_at_now() {
        let a = approver_keys(1);
        let required = approvers(&[&a]);
        let approvals = vec![approval("sha256:req", &a, 10)];
        let tuple = pop_tuple("sha256:req", &approvals);
        let result = verify_approvals(&approvals, &required, 1, "sha256:req", 10_000, &tuple)
            .expect("expires_at == now is valid");
        assert_eq!(result.valid_count, 1);
    }

    #[test]
    fn approval_preimage_is_domain_separated_and_deterministic() {
        let a = approver_keys(1);
        let signed = approval("sha256:req", &a, 10_300);
        let preimage = signed.preimage();
        // Must start with the domain prefix.
        assert!(preimage.starts_with(APPROVAL_SIGN_DOMAIN));
        // Must contain the request hash, approver key, and expiry.
        assert!(preimage
            .windows(b"sha256:req".len())
            .any(|window| window == b"sha256:req"));
        assert!(preimage
            .windows(a.public_key_bytes().len())
            .any(|window| window == a.public_key_bytes()));
        // Deterministic across two constructions.
        let again = approval("sha256:req", &a, 10_300);
        assert_eq!(preimage, again.preimage());
    }
}
