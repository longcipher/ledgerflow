//! Traditional payment gateway settlement adapter.

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailQuote, SettlementReceipt, VerificationResult},
    routing::{RailKind, RoutingError},
    subject::ResolvedSubject,
};

/// Traditional payment gateway settlement adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct GatewayRailAdapter;

impl RailAdapter for GatewayRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Gateway
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        matches!(subject.rail, RailKind::Gateway)
    }

    fn quote(&self, _authorization: &VerifiedAuthorization) -> Result<RailQuote, RoutingError> {
        Ok(RailQuote { rail: RailKind::Gateway, estimated_fee: 10, estimated_time_ms: 5_000 })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RoutingError> {
        Ok(SettlementReceipt {
            rail: RailKind::Gateway,
            transaction_id: format!("gw-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RoutingError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
