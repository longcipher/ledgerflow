//! Custodial ledger settlement adapter.

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailQuote, SettlementReceipt, VerificationResult},
    routing::{RailKind, RoutingError},
    subject::ResolvedSubject,
};

/// Custodial ledger settlement adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct CustodialRailAdapter;

impl RailAdapter for CustodialRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Custodial
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        matches!(subject.rail, RailKind::Custodial)
    }

    fn quote(&self, _authorization: &VerifiedAuthorization) -> Result<RailQuote, RoutingError> {
        Ok(RailQuote { rail: RailKind::Custodial, estimated_fee: 0, estimated_time_ms: 1_000 })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RoutingError> {
        Ok(SettlementReceipt {
            rail: RailKind::Custodial,
            transaction_id: format!("custodial-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RoutingError> {
        Ok(VerificationResult { verified: true, confirmations: 0 })
    }
}
