//! Custodial ledger settlement adapter (demo implementation).

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailError, RailQuote, SettlementReceipt, VerificationResult},
    routing::RailKind,
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

    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError> {
        Ok(RailQuote {
            rail: RailKind::Custodial,
            estimated_fee: 0,
            estimated_time_ms: 1_000,
            asset: authorization.asset.clone(),
        })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RailError> {
        Ok(SettlementReceipt {
            rail: RailKind::Custodial,
            transaction_id: format!("custodial-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
            asset: authorization.asset.clone(),
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RailError> {
        Ok(VerificationResult { verified: true, confirmations: 0 })
    }
}
