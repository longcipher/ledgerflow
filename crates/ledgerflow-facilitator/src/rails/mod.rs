//! Settlement rail adapters for the LedgerFlow Facilitator.
//!
//! Each rail adapter implements [`RailAdapter`] with a small trait surface:
//! quoting, settlement, and receipt verification. Adapters are **demo-grade**
//! in v1: they return deterministic receipts so the orchestration and
//! TOCTOU-closing logic can be exercised end-to-end. Real chain integrations
//! (EVM RPC, Solana, Tempo, Stripe) replace the internals without changing
//! the trait.

pub mod custodial;
pub mod evm;
pub mod exchange;
pub mod gateway;

use ledgerflow_core::VerifiedAuthorization;
use thiserror::Error;

use crate::{
    routing::RailKind,
    subject::ResolvedSubject,
};

/// Settlement quote returned by a rail adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RailQuote {
    pub rail: RailKind,
    /// Estimated fee in the asset's base units.
    pub estimated_fee: u128,
    pub estimated_time_ms: u64,
    pub asset: String,
}

/// Receipt returned after settlement execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementReceipt {
    pub rail: RailKind,
    pub transaction_id: String,
    pub settled_amount: u128,
    pub asset: String,
}

/// Result of receipt verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationResult {
    pub verified: bool,
    pub confirmations: u32,
}

/// Rail settlement failures.
#[derive(Debug, Error)]
pub enum RailError {
    #[error("the rail does not support this payment subject")]
    Unsupported,
    #[error("rail settlement failed: {0}")]
    SettlementFailed(String),
    #[error("rail verification failed: {0}")]
    VerificationFailed(String),
}

/// Trait implemented by each settlement rail adapter.
pub trait RailAdapter: Send + Sync {
    fn kind(&self) -> RailKind;
    fn supports(&self, subject: &ResolvedSubject) -> bool;
    fn quote(&self, authorization: &VerifiedAuthorization) -> Result<RailQuote, RailError>;
    fn settle(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<SettlementReceipt, RailError>;
    fn verify(&self, receipt: &SettlementReceipt) -> Result<VerificationResult, RailError>;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::{rails::{custodial::CustodialRailAdapter, evm::EvmRailAdapter, exchange::ExchangeRailAdapter, gateway::GatewayRailAdapter}, routing::RailKind};

    fn subject(rail: RailKind) -> ResolvedSubject {
        ResolvedSubject { rail, value: "sub-1".to_string() }
    }

    #[test]
    fn evm_adapter_only_supports_evm() {
        let adapter = EvmRailAdapter;
        assert!(adapter.supports(&subject(RailKind::Evm)));
        assert!(!adapter.supports(&subject(RailKind::Exchange)));
        assert!(!adapter.supports(&subject(RailKind::Gateway)));
        assert!(!adapter.supports(&subject(RailKind::Custodial)));
        assert_eq!(adapter.kind(), RailKind::Evm);
    }

    #[test]
    fn exchange_adapter_only_supports_exchange() {
        let adapter = ExchangeRailAdapter;
        assert!(adapter.supports(&subject(RailKind::Exchange)));
        assert!(!adapter.supports(&subject(RailKind::Evm)));
        assert!(!adapter.supports(&subject(RailKind::Gateway)));
        assert_eq!(adapter.kind(), RailKind::Exchange);
    }

    #[test]
    fn gateway_adapter_only_supports_gateway() {
        let adapter = GatewayRailAdapter;
        assert!(adapter.supports(&subject(RailKind::Gateway)));
        assert!(!adapter.supports(&subject(RailKind::Evm)));
        assert!(!adapter.supports(&subject(RailKind::Exchange)));
        assert_eq!(adapter.kind(), RailKind::Gateway);
    }

    #[test]
    fn custodial_adapter_only_supports_custodial() {
        let adapter = CustodialRailAdapter;
        assert!(adapter.supports(&subject(RailKind::Custodial)));
        assert!(!adapter.supports(&subject(RailKind::Evm)));
        assert_eq!(adapter.kind(), RailKind::Custodial);
    }

    #[test]
    fn rail_quote_settle_verify_round_trip() {
        // Construct a minimal verified authorization.
        let holder = ledgerflow_core::SignerRef::new(
            ledgerflow_core::SigningAlgorithm::Ed25519,
            vec![1; 32],
        );
        let warrant = ledgerflow_core::Warrant {
            version: 1,
            id: vec![0xAB; 16],
            holder: holder.clone(),
            issuer: holder.clone(),
            issued_at: 1,
            expires_at: 2,
            depth: 0,
            max_depth: 1,
            parent_hash: None,
            merchant: ledgerflow_core::MerchantConstraint::with_ids(vec!["merchant-a".to_string()]),
            resource: ledgerflow_core::ResourceConstraint::default(),
            payment: ledgerflow_core::PaymentConstraint::new(100),
            tool: None,
            approval_gates: std::collections::BTreeMap::new(),
            required_approvers: Vec::new(),
            min_approvals: 0,
            extensions: std::collections::BTreeMap::new(),
            signature: ledgerflow_core::SignatureEnvelope {
                alg: ledgerflow_core::SigningAlgorithm::Ed25519,
                value: vec![0; 64],
            },
        };
        let authorization = ledgerflow_core::VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject: ledgerflow_core::PaymentSubjectRef::new(
                ledgerflow_core::PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            holder,
            leaf_warrant: warrant.clone(),
            root_warrant: warrant,
            chain_len: 1,
            amount: 100,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: ledgerflow_core::PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            warrant_digest: "sha256:w".to_string(),
        };

        let adapter = EvmRailAdapter;
        let quote = adapter.quote(&authorization).expect("quote");
        assert_eq!(quote.rail, RailKind::Evm);
        assert_eq!(quote.asset, "USDC");
        assert!(quote.estimated_time_ms > 0);

        let receipt = adapter.settle(&authorization).expect("settle");
        assert!(receipt.transaction_id.starts_with("evm-tx-"));
        assert_eq!(receipt.settled_amount, 100);
        assert_eq!(receipt.asset, "USDC");

        let verification = adapter.verify(&receipt).expect("verify");
        assert!(verification.verified);
        assert_eq!(verification.confirmations, 1);
    }

    #[test]
    fn rail_error_messages_are_meaningful() {
        assert!(RailError::Unsupported.to_string().contains("does not support"));
        assert!(RailError::SettlementFailed("x".to_string()).to_string().contains("settlement failed"));
        assert!(RailError::VerificationFailed("x".to_string()).to_string().contains("verification failed"));
    }
}
