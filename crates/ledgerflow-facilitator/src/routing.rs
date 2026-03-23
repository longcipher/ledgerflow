//! Routing logic for verified LedgerFlow authorizations.

use ledgerflow_core::VerifiedAuthorization;
use thiserror::Error;

use crate::{
    rails::{
        RailAdapter, RailQuote, custodial::CustodialRailAdapter, evm::EvmRailAdapter,
        exchange::ExchangeRailAdapter, gateway::GatewayRailAdapter,
    },
    subject::{DefaultSubjectResolver, PaymentSubjectResolver, SubjectResolutionError},
};

/// Supported settlement rails in the MVP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RailKind {
    Evm,
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
        assert!(route.quote.is_some());
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
        assert!(route.quote.is_some());
    }

    #[test]
    fn routes_custodial_subjects_to_the_custodial_adapter() {
        let facilitator = Facilitator::default();
        let authorization = verified_authorization(PaymentSubjectRef::new(
            PaymentSubjectKind::Opaque,
            "custodial:internal-id-abc",
        ));

        let route = facilitator.route(&authorization).expect("route");

        assert_eq!(route.rail, RailKind::Custodial);
        assert!(route.merchant_flow_preserved);
        let quote = route.quote.expect("quote");
        assert_eq!(quote.rail, RailKind::Custodial);
        assert_eq!(quote.estimated_fee, 0);
    }

    #[test]
    fn routes_gateway_subjects_to_the_gateway_adapter() {
        let facilitator = Facilitator::default();
        let authorization = verified_authorization(PaymentSubjectRef::new(
            PaymentSubjectKind::Opaque,
            "gateway:stripe:acct_abc123",
        ));

        let route = facilitator.route(&authorization).expect("route");

        assert_eq!(route.rail, RailKind::Gateway);
        assert!(route.merchant_flow_preserved);
        let quote = route.quote.expect("quote");
        assert_eq!(quote.rail, RailKind::Gateway);
        assert_eq!(quote.estimated_fee, 10);
    }
}
