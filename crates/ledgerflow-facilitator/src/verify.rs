//! `/verify` orchestration: stateless authz verification plus revocation
//! pre-check.
//!
//! The verify step is a **pre-check**: it validates the full authorization
//! (chain invariants, PoP, approvals, trust anchor) and performs a revocation
//! check. Settlement MUST re-verify atomically (see [`crate::settle`]) to
//! close the verify→settle TOCTOU window.

use ledgerflow_core::{
    AuthorizationContext, AuthorizationError, PopProof, SignedApproval, ToolArguments,
    TrustedIssuers, WarrantChain, verify_authorization,
};
use ledgerflow_core::revocation::RevocationCheck;

use crate::outcome::{VerifyOutcome, VerifyStatus};

/// Inputs to the verify orchestration.
#[derive(Clone, Debug)]
pub struct VerifyRequest<'a> {
    pub chain: &'a WarrantChain,
    pub trusted: &'a TrustedIssuers,
    pub proof: &'a PopProof,
    pub context: &'a AuthorizationContext,
    pub approvals: &'a [SignedApproval],
    pub tool_arguments: &'a ToolArguments,
}

/// Stateless verification service.
#[derive(Clone, Debug)]
pub struct VerificationService<R> {
    pub revocation: R,
}

impl<R> VerificationService<R>
where
    R: RevocationCheck,
{
    /// Creates a new verification service over the given revocation store.
    #[must_use]
    pub const fn new(revocation: R) -> Self {
        Self { revocation }
    }

    /// Runs the verify orchestration.
    ///
    /// The revocation check is performed here as a pre-check; settlement
    /// re-verifies. All authorization failures are mapped to a
    /// [`VerifyStatus`].
    pub fn verify(&self, request: &VerifyRequest<'_>) -> VerifyOutcome {
        let input = ledgerflow_core::AuthorizationInput {
            chain: request.chain,
            trusted: request.trusted,
            proof: request.proof,
            context: request.context,
            approvals: request.approvals,
            tool_arguments: request.tool_arguments,
            revocation: &self.revocation,
        };
        match verify_authorization(&input) {
            Ok(authorization) => VerifyOutcome::ok(authorization),
            Err(error) => VerifyOutcome::error(map_error(&error), error.to_string()),
        }
    }
}

/// Maps an authorization error to a [`VerifyStatus`].
pub const fn map_error(error: &AuthorizationError) -> VerifyStatus {
    match error {
        AuthorizationError::InsufficientApprovals { .. }
        | AuthorizationError::ApprovalRequired
        | AuthorizationError::ApprovalExpired
        | AuthorizationError::ApproverNotAllowed
        | AuthorizationError::InvalidApprovalSignature
        | AuthorizationError::ApprovalsDigestMismatch
        | AuthorizationError::ApprovalRequestMismatch => VerifyStatus::InsufficientApproval,
        AuthorizationError::WarrantRevoked | AuthorizationError::HolderRevoked => {
            VerifyStatus::Revoked
        }
        AuthorizationError::WarrantExpired { .. }
        | AuthorizationError::WarrantNotYetValid { .. }
        | AuthorizationError::ProofOutsideFreshnessWindow { .. } => VerifyStatus::Expired,
        AuthorizationError::ChallengeMismatch => VerifyStatus::Replayed,
        _ => VerifyStatus::Unauthorized,
    }
}


#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn map_error_maps_approval_failures() {
        assert_eq!(map_error(&AuthorizationError::InsufficientApprovals { got: 0, need: 1 }), VerifyStatus::InsufficientApproval);
        assert_eq!(map_error(&AuthorizationError::ApprovalRequired), VerifyStatus::InsufficientApproval);
        assert_eq!(map_error(&AuthorizationError::ApprovalExpired), VerifyStatus::InsufficientApproval);
        assert_eq!(map_error(&AuthorizationError::ApproverNotAllowed), VerifyStatus::InsufficientApproval);
        assert_eq!(map_error(&AuthorizationError::InvalidApprovalSignature), VerifyStatus::InsufficientApproval);
        assert_eq!(map_error(&AuthorizationError::ApprovalsDigestMismatch), VerifyStatus::InsufficientApproval);
        assert_eq!(map_error(&AuthorizationError::ApprovalRequestMismatch), VerifyStatus::InsufficientApproval);
    }

    #[test]
    fn map_error_maps_revocation_failures() {
        assert_eq!(map_error(&AuthorizationError::WarrantRevoked), VerifyStatus::Revoked);
        assert_eq!(map_error(&AuthorizationError::HolderRevoked), VerifyStatus::Revoked);
    }

    #[test]
    fn map_error_maps_expiry_failures() {
        assert_eq!(map_error(&AuthorizationError::WarrantExpired { expires_at: 1 }), VerifyStatus::Expired);
        assert_eq!(map_error(&AuthorizationError::WarrantNotYetValid { issued_at: 1 }), VerifyStatus::Expired);
        assert_eq!(map_error(&AuthorizationError::ProofOutsideFreshnessWindow { created_at_ms: 1, now_ms: 2 }), VerifyStatus::Expired);
    }

    #[test]
    fn map_error_maps_challenge_mismatch_to_replayed() {
        assert_eq!(map_error(&AuthorizationError::ChallengeMismatch), VerifyStatus::Replayed);
    }

    #[test]
    fn map_error_defaults_to_unauthorized() {
        assert_eq!(map_error(&AuthorizationError::EmptyChain), VerifyStatus::Unauthorized);
        assert_eq!(map_error(&AuthorizationError::InvalidWarrantSignature), VerifyStatus::Unauthorized);
        assert_eq!(map_error(&AuthorizationError::MerchantNotAllowed { merchant_id: "x".to_string() }), VerifyStatus::Unauthorized);
        assert_eq!(map_error(&AuthorizationError::ParentHashMismatch), VerifyStatus::Unauthorized);
    }
}
