//! Structured verification and settlement outcomes with error codes.
//!
//! The Facilitator maps low-level [`AuthorizationError`]s and rail failures
//! onto a small, protocol-friendly status vocabulary so merchants and agents
//! can branch on the signal rather than the message (aligned with x402
//! `ErrorReason` semantics).

use ledgerflow_core::VerifiedAuthorization;

use crate::rails::SettlementReceipt;

/// Verification result status (aligned with x402 ErrorReason semantics).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifyStatus {
    /// Verification passed (pre-check). Settlement must still re-verify.
    Verified,
    /// Authorization failed (constraints, signatures, chain invariants).
    Unauthorized,
    /// The payment requires m-of-n approval and the threshold was not met.
    InsufficientApproval,
    /// The PoP was replayed.
    Replayed,
    /// The warrant or proof expired.
    Expired,
    /// The warrant or holder was revoked.
    Revoked,
    /// The payment payload itself is invalid.
    InvalidPayment,
}

impl VerifyStatus {
    /// Returns `true` when verification passed.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }
}

/// Output of a `/verify` orchestration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifyOutcome {
    pub status: VerifyStatus,
    pub authorization: Option<VerifiedAuthorization>,
    pub reason: Option<String>,
}

impl VerifyOutcome {
    #[must_use]
    pub const fn ok(authorization: VerifiedAuthorization) -> Self {
        Self { status: VerifyStatus::Verified, authorization: Some(authorization), reason: None }
    }

    #[must_use]
    pub const fn error(status: VerifyStatus, reason: String) -> Self {
        Self { status, authorization: None, reason: Some(reason) }
    }
}

/// Settlement status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementStatus {
    Pending,
    Settled,
    Failed,
}

/// Output of a `/settle` orchestration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementOutcome {
    pub status: SettlementStatus,
    pub receipt: Option<SettlementReceipt>,
    pub reason: Option<String>,
}

impl SettlementOutcome {
    #[must_use]
    pub const fn settled(receipt: SettlementReceipt) -> Self {
        Self { status: SettlementStatus::Settled, receipt: Some(receipt), reason: None }
    }

    #[must_use]
    pub const fn failed(reason: String) -> Self {
        Self { status: SettlementStatus::Failed, receipt: None, reason: Some(reason) }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn verify_status_is_verified_only_for_verified() {
        assert!(VerifyStatus::Verified.is_verified());
        assert!(!VerifyStatus::Unauthorized.is_verified());
        assert!(!VerifyStatus::InsufficientApproval.is_verified());
        assert!(!VerifyStatus::Replayed.is_verified());
        assert!(!VerifyStatus::Expired.is_verified());
        assert!(!VerifyStatus::Revoked.is_verified());
        assert!(!VerifyStatus::InvalidPayment.is_verified());
    }

    #[test]
    fn verify_outcome_ok_and_error_shapes() {
        let holder = ledgerflow_core::SignerRef::new(
            ledgerflow_core::SigningAlgorithm::Ed25519,
            vec![1; 32],
        );
        let subject = ledgerflow_core::PaymentSubjectRef::new(
            ledgerflow_core::PaymentSubjectKind::Caip10,
            "caip10:eip155:8453:0xabc123",
        );
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
                alg: ledgerflow_core::SigningAlgorithm::Ed25519,
                value: vec![0; 64],
            },
        };
        let authorization = ledgerflow_core::VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject: subject,
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
        };
        let ok = VerifyOutcome::ok(authorization);
        assert!(ok.status.is_verified());
        assert!(ok.authorization.is_some());
        assert!(ok.reason.is_none());

        let err = VerifyOutcome::error(VerifyStatus::Revoked, "revoked".to_string());
        assert!(!err.status.is_verified());
        assert!(err.authorization.is_none());
        assert_eq!(err.reason.as_deref(), Some("revoked"));
    }

    #[test]
    fn settlement_outcome_shapes() {
        let receipt = SettlementReceipt {
            rail: crate::RailKind::Evm,
            transaction_id: "tx-1".to_string(),
            settled_amount: 100,
            asset: "USDC".to_string(),
        };
        let settled = SettlementOutcome::settled(receipt);
        assert_eq!(settled.status, SettlementStatus::Settled);
        assert!(settled.receipt.is_some());
        assert!(settled.reason.is_none());

        let failed = SettlementOutcome::failed("boom".to_string());
        assert_eq!(failed.status, SettlementStatus::Failed);
        assert!(failed.receipt.is_none());
        assert_eq!(failed.reason.as_deref(), Some("boom"));
    }

    #[test]
    fn settlement_statuses_are_distinct() {
        assert_ne!(SettlementStatus::Pending, SettlementStatus::Settled);
        assert_ne!(SettlementStatus::Pending, SettlementStatus::Failed);
        assert_ne!(SettlementStatus::Settled, SettlementStatus::Failed);
    }
}
