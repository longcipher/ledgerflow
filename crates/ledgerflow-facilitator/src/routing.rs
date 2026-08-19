//! Routing logic for verified LedgerFlow authorizations.

use ledgerflow_core::VerifiedAuthorization;
use thiserror::Error;

use crate::{
    rails::{
        RailAdapter, RailError, RailQuote, custodial::CustodialRailAdapter, evm::EvmRailAdapter,
        exchange::ExchangeRailAdapter, gateway::GatewayRailAdapter, solana::SolanaRailAdapter,
    },
    subject::{
        DefaultSubjectResolver, PaymentSubjectResolver, ResolvedSubject, SubjectResolutionError,
    },
};

/// Supported settlement rails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailKind {
    Evm,
    Solana,
    Exchange,
    Custodial,
    Gateway,
}

/// Final routing decision returned by the Facilitator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteDecision {
    pub rail: RailKind,
    pub subject_value: String,
    pub merchant_flow_preserved: bool,
    pub quote: Option<RailQuote>,
}

/// Routing failures surfaced by the Facilitator.
#[derive(Debug, Error)]
pub enum RoutingError {
    #[error(transparent)]
    Subject(#[from] SubjectResolutionError),
    #[error(transparent)]
    Rail(#[from] RailError),
    #[error("no rail adapter could service the resolved subject")]
    NoCompatibleRail,
}

/// Small Facilitator that keeps merchant flows rail-agnostic.
pub struct Facilitator<R = DefaultSubjectResolver> {
    resolver: R,
    adapters: Vec<Box<dyn RailAdapter>>,
}

impl Default for Facilitator<DefaultSubjectResolver> {
    fn default() -> Self {
        Self::new(
            DefaultSubjectResolver,
            vec![
                Box::new(EvmRailAdapter),
                Box::new(SolanaRailAdapter),
                Box::new(ExchangeRailAdapter),
                Box::new(CustodialRailAdapter),
                Box::new(GatewayRailAdapter),
            ],
        )
    }
}

impl<R> Facilitator<R> {
    #[must_use]
    pub fn new(resolver: R, adapters: Vec<Box<dyn RailAdapter>>) -> Self {
        Self { resolver, adapters }
    }
}

impl<R> Facilitator<R>
where
    R: PaymentSubjectResolver,
{
    pub fn route(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<RouteDecision, RoutingError> {
        let resolved = self.resolver.resolve(authorization)?;
        let adapter = self
            .adapters
            .iter()
            .find(|adapter| adapter.supports(&resolved))
            .ok_or(RoutingError::NoCompatibleRail)?;

        let quote = adapter.quote(authorization)?;

        Ok(RouteDecision {
            rail: adapter.kind(),
            subject_value: resolved.value,
            merchant_flow_preserved: true,
            quote: Some(quote),
        })
    }

    /// Settles through the adapter that supports the resolved subject.
    pub fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<crate::rails::SettlementReceipt, RoutingError> {
        let resolved = self.resolver.resolve(authorization)?;
        let adapter = self
            .adapters
            .iter()
            .find(|adapter| adapter.supports(&resolved))
            .ok_or(RoutingError::NoCompatibleRail)?;
        Ok(adapter.settle(authorization)?)
    }

    /// Resolves the subject without routing (used by tests and tooling).
    pub fn resolve_subject(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<ResolvedSubject, SubjectResolutionError> {
        self.resolver.resolve(authorization)
    }
}
