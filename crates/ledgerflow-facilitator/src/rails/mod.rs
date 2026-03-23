//! Rail adapters supported by the MVP Facilitator.

pub mod custodial;
pub mod evm;
pub mod exchange;
pub mod gateway;

use ledgerflow_core::VerifiedAuthorization;

use crate::{
    routing::{RailKind, RoutingError},
    subject::ResolvedSubject,
};

/// Settlement quote returned by a rail adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RailQuote {
    pub rail: RailKind,
    pub estimated_fee: u64,
    pub estimated_time_ms: u64,
}

/// Receipt returned after settlement execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementReceipt {
    pub rail: RailKind,
    pub transaction_id: String,
    pub settled_amount: u64,
}

/// Result of receipt verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationResult {
    pub verified: bool,
    pub confirmations: u32,
}

/// Trait implemented by each settlement rail adapter.
pub trait RailAdapter {
    fn kind(&self) -> RailKind;
    fn supports(&self, subject: &ResolvedSubject) -> bool;
    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RoutingError>;
    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RoutingError>;
    fn verify(&self, receipt: &SettlementReceipt) -> Result<VerificationResult, RoutingError>;
}
