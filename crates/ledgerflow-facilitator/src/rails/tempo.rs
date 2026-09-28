//! Tempo (MPP charge/session) settlement adapter (demo implementation).
//!
//! Tempo is a fee-sponsored L2 for MPP payments. This adapter demonstrates
//! the integration pattern: it returns deterministic receipts so the
//! orchestration and TOCTOU-closing logic can be exercised end-to-end.
//! Real Tempo integration replaces the internals without changing the
//! `RailAdapter` trait.

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailError, RailQuote, SettlementReceipt, VerificationResult},
    routing::RailKind,
    subject::ResolvedSubject,
};

/// Tempo MPP settlement adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct TempoRailAdapter;

impl RailAdapter for TempoRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Tempo
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        matches!(subject.rail, RailKind::Tempo)
    }

    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError> {
        Ok(RailQuote {
            rail: RailKind::Tempo,
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
            rail: RailKind::Tempo,
            transaction_id: format!("tempo-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
            asset: authorization.asset.clone(),
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RailError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
