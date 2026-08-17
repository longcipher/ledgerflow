//! EVM onchain settlement adapter (demo implementation).

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailError, RailQuote, SettlementReceipt, VerificationResult},
    routing::RailKind,
    subject::ResolvedSubject,
};

/// EVM onchain settlement adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvmRailAdapter;

impl RailAdapter for EvmRailAdapter {
    fn kind(&self) -> RailKind {
        RailKind::Evm
    }

    fn supports(&self, subject: &ResolvedSubject) -> bool {
        matches!(subject.rail, RailKind::Evm)
    }

    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError> {
        Ok(RailQuote {
            rail: RailKind::Evm,
            estimated_fee: 0,
            estimated_time_ms: 15_000,
            asset: authorization.asset.clone(),
        })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RailError> {
        Ok(SettlementReceipt {
            rail: RailKind::Evm,
            transaction_id: format!("evm-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
            asset: authorization.asset.clone(),
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RailError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
