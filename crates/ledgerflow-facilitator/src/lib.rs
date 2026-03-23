//! Settlement-rail routing for verified LedgerFlow authorizations.

#![allow(missing_docs)]
#![allow(missing_debug_implementations)]

pub mod rails;
pub mod routing;
pub mod subject;

pub use crate::{
    rails::{
        RailAdapter, RailQuote, SettlementReceipt, VerificationResult,
        custodial::CustodialRailAdapter, evm::EvmRailAdapter, exchange::ExchangeRailAdapter,
        gateway::GatewayRailAdapter,
    },
    routing::{Facilitator, RailKind, RouteDecision, RoutingError},
    subject::{
        DefaultSubjectResolver, PaymentSubjectResolver, ResolvedSubject, SubjectResolutionError,
    },
};
