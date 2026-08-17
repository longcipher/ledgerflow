//! Exchange rail adapter for offchain account settlement (demo implementation).

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailError, RailQuote, SettlementReceipt, VerificationResult},
    routing::RailKind,
    subject::ResolvedSubject,
};

/// Adapter for exchange-style settlement.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExchangeRailAdapter;

impl RailAdapter for ExchangeRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Exchange
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        matches!(subject.rail, RailKind::Exchange)
    }

    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError> {
        Ok(RailQuote {
            rail: RailKind::Exchange,
            estimated_fee: 0,
            estimated_time_ms: 2_000,
            asset: authorization.asset.clone(),
        })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RailError> {
        Ok(SettlementReceipt {
            rail: RailKind::Exchange,
            transaction_id: format!("exchange-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
            asset: authorization.asset.clone(),
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RailError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
