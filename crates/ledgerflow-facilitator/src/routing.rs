//! Routing logic for verified LedgerFlow authorizations.

use ledgerflow_core::VerifiedAuthorization;
use thiserror::Error;

use crate::{
    rails::{RailAdapter, evm::EvmRailAdapter, exchange::ExchangeRailAdapter},
    subject::{DefaultSubjectResolver, PaymentSubjectResolver, SubjectResolutionError},
};

/// Supported settlement rails in the MVP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailKind {
    Evm,
    Exchange,
}

/// Final routing decision returned by the Facilitator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteDecision {
    pub rail: RailKind,
    pub subject_value: String,
    pub merchant_flow_preserved: bool,
}

/// Routing failures surfaced by the Facilitator.
#[derive(Debug, Error)]
pub enum RoutingError {
    #[error(transparent)]
    Subject(#[from] SubjectResolutionError),
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
            vec![Box::new(EvmRailAdapter), Box::new(ExchangeRailAdapter)],
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

        Ok(RouteDecision {
            rail: adapter.kind(),
            subject_value: resolved.value,
            merchant_flow_preserved: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use ledgerflow_core::{
        PaymentRail, PaymentSubjectKind, PaymentSubjectRef, SignerRef, SigningAlgorithm,
        VerifiedAuthorization,
    };

    use super::{Facilitator, RailKind};

    fn verified_authorization(payment_subject: PaymentSubjectRef) -> VerifiedAuthorization {
        VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject,
            payer: SignerRef::new(SigningAlgorithm::Ed25519, "agent-key"),
            warrant_digest: "sha256:warrant".to_string(),
            accepted_hash: "sha256:accepted".to_string(),
            request_hash: "sha256:request".to_string(),
            amount: 200,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
        }
    }

    #[test]
    fn routes_onchain_subjects_to_the_evm_adapter() {
        let facilitator = Facilitator::default();
        let authorization = verified_authorization(PaymentSubjectRef::new(
            PaymentSubjectKind::Caip10,
            "caip10:eip155:8453:0xabc123",
        ));

        let route = facilitator.route(&authorization).expect("route");

        assert_eq!(route.rail, RailKind::Evm);
        assert!(route.merchant_flow_preserved);
    }

    #[test]
    fn routes_exchange_subjects_to_the_exchange_adapter() {
        let facilitator = Facilitator::default();
        let authorization = verified_authorization(PaymentSubjectRef::new(
            PaymentSubjectKind::ExchangeAccount,
            "binance:uid:12345678",
        ));

        let route = facilitator.route(&authorization).expect("route");

        assert_eq!(route.rail, RailKind::Exchange);
        assert!(route.merchant_flow_preserved);
    }
}
