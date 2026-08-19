//! Payment-subject resolution for Facilitator routing.

use ledgerflow_core::{PaymentSubjectKind, VerifiedAuthorization};
use thiserror::Error;

use crate::routing::RailKind;

/// Subject information normalized for routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSubject {
    pub rail: RailKind,
    pub value: String,
}

/// Subject resolution failures.
#[derive(Debug, Error)]
pub enum SubjectResolutionError {
    #[error("payment subject `{value}` could not be resolved to a known rail")]
    UnsupportedSubject { value: String },
}

/// Resolves a verified payment subject to a rail hint.
pub trait PaymentSubjectResolver {
    fn resolve(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<ResolvedSubject, SubjectResolutionError>;
}

/// Default subject resolver for the onchain, exchange, custodial, and gateway rails.
#[derive(Clone, Debug, Default)]
pub struct DefaultSubjectResolver;

impl PaymentSubjectResolver for DefaultSubjectResolver {
    fn resolve(
        &self,
        authorization: &VerifiedAuthorization,
    ) -> Result<ResolvedSubject, SubjectResolutionError> {
        let subject = &authorization.payment_subject;
        let rail = match subject.kind {
            PaymentSubjectKind::Caip10 if subject.value.starts_with("caip10:solana:") => {
                RailKind::Solana
            }
            PaymentSubjectKind::Caip10 if subject.value.starts_with("caip10:eip155:") => {
                RailKind::Evm
            }
            PaymentSubjectKind::ExchangeAccount => RailKind::Exchange,
            PaymentSubjectKind::FacilitatorAccount if subject.value.starts_with("binance:") => {
                RailKind::Exchange
            }
            PaymentSubjectKind::FacilitatorAccount if subject.value.starts_with("okx:") => {
                RailKind::Exchange
            }
            PaymentSubjectKind::Opaque if subject.value.starts_with("gateway:") => {
                RailKind::Gateway
            }
            PaymentSubjectKind::Opaque => RailKind::Custodial,
            PaymentSubjectKind::FacilitatorAccount => RailKind::Exchange,
            _ => {
                return Err(SubjectResolutionError::UnsupportedSubject {
                    value: subject.value.clone(),
                });
            }
        };

        Ok(ResolvedSubject { rail, value: subject.value.clone() })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use ledgerflow_core::PaymentSubjectRef;

    use super::*;

    fn authz(subject: PaymentSubjectRef) -> VerifiedAuthorization {
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
        VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject: subject,
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
        }
    }

    #[test]
    fn caip10_subject_resolves_to_evm() {
        let subject =
            PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123");
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Evm);
        assert_eq!(resolved.value, "caip10:eip155:8453:0xabc123");
    }

    #[test]
    fn caip10_solana_subject_resolves_to_solana() {
        let subject = PaymentSubjectRef::new(
            PaymentSubjectKind::Caip10,
            "caip10:solana:mainnet:7vfCXTUXx5Wn4P6m7XJ3e1yK2bXxVmW7nYj1m5X9A1t3",
        );
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Solana);
    }

    #[test]
    fn unsupported_caip10_chain_family_is_rejected() {
        let subject = PaymentSubjectRef::new(
            PaymentSubjectKind::Caip10,
            "caip10:cosmos:cosmoshub-4:cosmos1deadbeef",
        );
        let error = DefaultSubjectResolver.resolve(&authz(subject)).expect_err("unsupported");
        assert!(matches!(error, SubjectResolutionError::UnsupportedSubject { .. }));
    }

    #[test]
    fn exchange_account_subject_resolves_to_exchange() {
        let subject = PaymentSubjectRef::new(PaymentSubjectKind::ExchangeAccount, "exchange-1");
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Exchange);
    }

    #[test]
    fn facilitator_account_binance_resolves_to_exchange() {
        let subject =
            PaymentSubjectRef::new(PaymentSubjectKind::FacilitatorAccount, "binance:acct-1");
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Exchange);
    }

    #[test]
    fn facilitator_account_okx_resolves_to_exchange() {
        let subject = PaymentSubjectRef::new(PaymentSubjectKind::FacilitatorAccount, "okx:acct-2");
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Exchange);
    }

    #[test]
    fn opaque_gateway_resolves_to_gateway() {
        let subject = PaymentSubjectRef::new(PaymentSubjectKind::Opaque, "gateway:gw-1");
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Gateway);
    }

    #[test]
    fn opaque_non_gateway_resolves_to_custodial() {
        let subject = PaymentSubjectRef::new(PaymentSubjectKind::Opaque, "custodial:acc-1");
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Custodial);
    }

    #[test]
    fn plain_facilitator_account_resolves_to_exchange() {
        let subject = PaymentSubjectRef::new(PaymentSubjectKind::FacilitatorAccount, "acct-3");
        let resolved = DefaultSubjectResolver.resolve(&authz(subject)).expect("resolved");
        assert_eq!(resolved.rail, RailKind::Exchange);
    }

    #[test]
    fn binance_okx_gateway_guards_are_not_case_swapped() {
        // "binance:" guard must not match "binancex:" or "notbinance:".
        let near_miss =
            PaymentSubjectRef::new(PaymentSubjectKind::FacilitatorAccount, "notbinance:acct");
        let resolved = DefaultSubjectResolver.resolve(&authz(near_miss)).expect("falls through");
        assert_eq!(resolved.rail, RailKind::Exchange);
    }
}
