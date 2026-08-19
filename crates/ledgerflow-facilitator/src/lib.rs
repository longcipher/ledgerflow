//! Settlement-rail routing and verification orchestration for LedgerFlow.
//!
//! The Facilitator composes the stateless authorization checks from
//! `ledgerflow-core` with online revocation and settlement routing:
//!
//! - [`verify`]: stateless authz verification + revocation pre-check.
//! - [`settle`]: atomic re-verification (TOCTOU closing) + rail settlement.
//! - [`status`]: idempotent settlement queries.
//! - [`revocation_store`]: persistent, restart-safe revocation.
//! - [`routing`] / [`subject`] / [`rails`]: rail-agnostic routing.

#![allow(missing_docs)]
#![allow(missing_debug_implementations)]

pub mod outcome;
pub mod rails;
pub mod revocation_store;
pub mod routing;
pub mod settle;
pub mod srl_sync;
pub mod status;
pub mod subject;
pub mod verify;

pub use crate::{
    outcome::{SettlementOutcome, SettlementStatus, VerifyOutcome, VerifyStatus},
    rails::{
        RailAdapter, RailError, RailQuote, SettlementReceipt, SharedRailAdapter,
        VerificationResult, custodial::CustodialRailAdapter, evm::EvmRailAdapter,
        exchange::ExchangeRailAdapter, gateway::GatewayRailAdapter, solana::SolanaRailAdapter,
    },
    revocation_store::{FileRevocationStore, InsecureMemoryRevocationStore, RevocationStoreError},
    routing::{Facilitator, RailKind, RouteDecision, RoutingError},
    settle::{SettleRequest, SettlementService},
    srl_sync::{SrlSync, SrlSyncError},
    status::{RegistryEntry, SettlementRegistry},
    subject::{
        DefaultSubjectResolver, PaymentSubjectResolver, ResolvedSubject, SubjectResolutionError,
    },
    verify::{VerificationService, VerifyRequest},
};
