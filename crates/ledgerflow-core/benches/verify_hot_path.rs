//! Criterion benchmark for the LedgerFlow verification hot path.

// Benchmarks are development tooling; asserting fixture invariants with
// `expect` is intentional.
#![allow(clippy::expect_used)]

use criterion::{Criterion, criterion_group, criterion_main};
use ledgerflow_core::{
    AssetRef, AuthorizationContext, AuthorizationInput, InMemoryRevocationCheck,
    MerchantConstraint, PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef,
    ProofBuilder, ResourceConstraint, SigningKeyPair, ToolConstraint, TrustedIssuer,
    TrustedIssuers, Warrant, WarrantBuilder, WarrantChain, sha256_prefixed, verify_authorization,
};

fn issuer() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[1u8; 32])
}

fn holder() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[2u8; 32])
}

fn subject_ref() -> PaymentSubjectRef {
    PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
}

fn warrant() -> Warrant {
    let issuer_key = issuer();
    let holder_key = holder();
    WarrantBuilder::new(2_000)
        .warrant_id([1_u8; 16])
        .ttl_secs(60)
        .max_depth(1)
        .issuer(issuer_key.signer_ref())
        .holder(holder_key.signer_ref())
        .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
        .resource(ResourceConstraint {
            http_methods: vec!["POST".to_string()],
            path_prefixes: vec!["/pay".to_string()],
        })
        .tool(ToolConstraint {
            tool_names: vec!["web-search".to_string()],
            model_providers: Vec::new(),
            action_labels: Vec::new(),
        })
        .payment(
            PaymentConstraint::new(200)
                .with_asset(AssetRef::new("USDC", Some("base".to_string())))
                .with_rails(vec![PaymentRail::Onchain])
                .with_schemes(vec!["exact".to_string()]),
        )
        .sign_with(&issuer_key, [0_u8; 8])
}

fn context() -> AuthorizationContext {
    let request_hash = sha256_prefixed("POST\nmerchant-a.example\n/pay\nsha256:body");
    let accepted_hash = sha256_prefixed("exact:USDC:200:merchant-a");
    AuthorizationContext {
        merchant_id: "merchant-a".to_string(),
        merchant_host: "merchant-a.example".to_string(),
        tool_name: "web-search".to_string(),
        model_provider: String::new(),
        action_label: String::new(),
        http_method: "POST".to_string(),
        path_and_query: "/pay".to_string(),
        selected_amount: 200,
        asset: "USDC".to_string(),
        asset_network: Some("base".to_string()),
        scheme: "exact".to_string(),
        payee_id: "merchant-a".to_string(),
        rail: PaymentRail::Onchain,
        challenge_id: "challenge-1".to_string(),
        request_hash,
        accepted_hash,
        now_ms: 2_000,
        freshness_window_ms: 60_000,
        clock_skew_ms: 30_000,
        payment_subject: subject_ref(),
        presenter: holder().signer_ref(),
    }
}

fn proof(warrant: &Warrant, context: &AuthorizationContext) -> ledgerflow_core::PopProof {
    ProofBuilder::new()
        .warrant_id(warrant.id.clone())
        .challenge_id(context.challenge_id.clone())
        .method(context.http_method.clone())
        .uri(format!("{}{}", context.merchant_host, context.path_and_query))
        .request_hash(context.request_hash.clone())
        .accepted_hash(context.accepted_hash.clone())
        .payment_payload_digest(sha256_prefixed("x402-payload"))
        .nonce("nonce-1".to_string())
        .created_at_ms(context.now_ms)
        .sign_with(&holder())
}

fn trusted() -> TrustedIssuers {
    let mut set = TrustedIssuers::new();
    set.add(TrustedIssuer::new("issuer-1".to_string(), issuer().signer_ref()));
    set
}

fn bench_verify_authorization_hot_path(criterion: &mut Criterion) {
    let chain = WarrantChain::single(warrant());
    let context = context();
    let proof = proof(chain.leaf().expect("leaf"), &context);
    let trusted = trusted();
    let revocation = InMemoryRevocationCheck::new();
    let tool_arguments = std::collections::BTreeMap::new();

    criterion.bench_function("verify_authorization_hot_path", |bench| {
        bench.iter(|| {
            let input = AuthorizationInput {
                chain: &chain,
                trusted: &trusted,
                proof: &proof,
                context: &context,
                approvals: &[],
                tool_arguments: &tool_arguments,
                revocation: &revocation,
            };
            let result = verify_authorization(&input);
            assert!(result.is_ok());
        });
    });
}

criterion_group!(benches, bench_verify_authorization_hot_path);
criterion_main!(benches);
