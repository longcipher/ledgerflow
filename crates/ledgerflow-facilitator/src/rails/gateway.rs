//! Traditional payment gateway settlement adapter (demo implementation).

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailError, RailQuote, SettlementReceipt, VerificationResult},
    routing::RailKind,
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

    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError> {
        Ok(RailQuote {
            rail: RailKind::Gateway,
            estimated_fee: 10,
            estimated_time_ms: 5_000,
            asset: authorization.asset.clone(),
        })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RailError> {
        Ok(SettlementReceipt {
            rail: RailKind::Gateway,
            transaction_id: format!("gw-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
            asset: authorization.asset.clone(),
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RailError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
