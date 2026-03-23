//! EVM onchain settlement adapter.

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    rails::{RailAdapter, RailQuote, SettlementReceipt, VerificationResult},
    routing::{RailKind, RoutingError},
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

    fn quote(&self, _authorization: &VerifiedAuthorization) -> Result<RailQuote, RoutingError> {
        Ok(RailQuote { rail: RailKind::Evm, estimated_fee: 0, estimated_time_ms: 15_000 })
    }

    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RoutingError> {
        Ok(SettlementReceipt {
            rail: RailKind::Evm,
            transaction_id: format!("evm-tx-{}", authorization.warrant_digest),
            settled_amount: authorization.amount,
        })
    }

    fn verify(&self, _receipt: &SettlementReceipt) -> Result<VerificationResult, RoutingError> {
        Ok(VerificationResult { verified: true, confirmations: 1 })
    }
}
