//! Stripe (card acquiring / SPT) settlement adapter (demo implementation).
//!
//! Stripe adapter demonstrates fiat off-ramp integration. Returns
//! deterministic receipts for orchestration testing. Real Stripe
//! integration replaces the internals without changing the `RailAdapter`
//! trait.

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailError, RailQuote, SettlementReceipt, VerificationResult},
    routing::RailKind,
    subject::ResolvedSubject,
};

/// Stripe card acquiring settlement adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct StripeRailAdapter;

impl RailAdapter for StripeRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Stripe
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        matches!(subject.rail, RailKind::Stripe)
    }

    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError> {
        Ok(RailQuote {
            rail: RailKind::Stripe,
            estimated_fee: authorization.amount / 100, // 1% demo fee
            estimated_time_ms: 3_000,
            asset: authorization.asset.clone(),
        })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RailError> {
        Ok(SettlementReceipt {
            rail: RailKind::Stripe,
            transaction_id: format!("stripe-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
            asset: authorization.asset.clone(),
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RailError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
