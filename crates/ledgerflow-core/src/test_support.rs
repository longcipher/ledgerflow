//! Shared test fixtures for LedgerFlow core.

#![cfg(test)]
#![allow(dead_code)]

use crate::{
    approval::ApprovalGate,
    constraint::{MerchantConstraint, PaymentConstraint, ResourceConstraint, ToolConstraint},
    warrant::{AssetRef, PaymentRail, PaymentSubjectKind, PaymentSubjectRef, SigningKeyPair, Warrant},
};

/// Deterministic issuer keys used across tests.
pub(crate) fn issuer_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[1u8; 32])
}

/// Deterministic holder keys used across tests.
pub(crate) fn holder_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[2u8; 32])
}

/// A sample payment subject.
pub(crate) fn sample_subject() -> PaymentSubjectRef {
    PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
}

/// A fixed 16-byte warrant id for deterministic fixtures.
pub(crate) fn sample_warrant_id() -> [u8; 16] {
    *b"lfw-000000000001"
}

/// A sample root warrant with a merchant allowlist and a payment cap.
pub(crate) fn sample_warrant() -> Warrant {
    let issuer = issuer_keys();
    let holder = holder_keys();
    let merchant = MerchantConstraint::with_ids(vec!["merchant-a".to_string()]);
    let resource = ResourceConstraint {
        http_methods: vec!["POST".to_string()],
        path_prefixes: vec!["/pay".to_string()],
    };
    let tool = ToolConstraint {
        tool_names: vec!["web-search".to_string()],
        model_providers: Vec::new(),
        action_labels: Vec::new(),
    };
    let payment = PaymentConstraint::new(200)
        .with_asset(AssetRef::new("USDC", Some("base".to_string())))
        .with_rails(vec![PaymentRail::Onchain])
        .with_schemes(vec!["exact".to_string()])
        .with_payees(vec!["merchant-a".to_string()]);

    crate::typestate::WarrantBuilder::new(2_000)
        .warrant_id(sample_warrant_id())
        .ttl_secs(10)
        .max_depth(1)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant)
        .resource(resource)
        .tool(tool)
        .payment(payment)
        .approval_gate("web-search", ApprovalGate::unconditional())
        .sign_with(&issuer, [0_u8; 8])
}

/// Zero random bytes for deterministic UUIDv7 id generation in tests.
pub(crate) const ZERO_RANDOM: [u8; 8] = [0_u8; 8];
