//! Solana onchain settlement adapter (skeleton).
//!
//! This is a **skeleton** matching the design's Solana rail (design §8.2):
//! it returns deterministic receipts so the orchestration and TOCTOU-closing
//! logic can be exercised end-to-end. Real SPL Token / Token-2022 settlement
//! replaces the internals without changing the trait.

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailError, RailQuote, SettlementReceipt, VerificationResult},
    routing::RailKind,
    subject::ResolvedSubject,
};

/// Solana onchain settlement adapter (skeleton).
#[derive(Clone, Copy, Debug, Default)]
pub struct SolanaRailAdapter;

impl RailAdapter for SolanaRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Solana
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        matches!(subject.rail, RailKind::Solana)
    }

    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError> {
        Ok(RailQuote {
            rail: RailKind::Solana,
            estimated_fee: 0,
            estimated_time_ms: 5_000,
            asset: authorization.asset.clone(),
        })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RailError> {
        Ok(SettlementReceipt {
            rail: RailKind::Solana,
            transaction_id: format!("solana-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
            asset: authorization.asset.clone(),
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RailError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
