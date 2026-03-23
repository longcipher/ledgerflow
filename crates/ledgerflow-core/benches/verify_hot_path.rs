//! Criterion benchmark for the LedgerFlow verification hot path.

use criterion::{Criterion, criterion_group, criterion_main};
use ledgerflow_core::{
    AmountLimit, AssetRef, AudienceScope, AuthorizationContext, Constraint, DelegationPolicy,
    MerchantConstraint, PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef,
    Proof, ResourceConstraint, SignerRef, SigningAlgorithm, SponsorshipConstraint, ToolConstraint,
    WARRANT_VERSION_V1, Warrant, WarrantMetadata, sha256_prefixed, verify_authorization,
};

fn issuer() -> SignerRef {
    SignerRef::new(SigningAlgorithm::Ed25519, "issuer-key")
}

fn subject_signer() -> SignerRef {
    SignerRef::new(SigningAlgorithm::Ed25519, "agent-key")
}

fn subject_ref() -> PaymentSubjectRef {
    PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
}

fn warrant() -> Warrant {
    Warrant {
        version: WARRANT_VERSION_V1,
        warrant_id: "warrant-1".to_string(),
        issuer: issuer(),
        subject_signer: subject_signer(),
        payment_subjects: vec![subject_ref()],
        audience: AudienceScope::MerchantIds(vec!["merchant-a".to_string()]),
        not_before_ms: 1_000,
        expires_at_ms: 5_000,
        delegation: DelegationPolicy { can_delegate: true, max_depth: 1 },
        constraints: vec![
            Constraint::Merchant(MerchantConstraint {
                merchant_ids: vec!["merchant-a".to_string()],
                host_suffixes: vec![],
            }),
            Constraint::Resource(ResourceConstraint {
                http_methods: vec!["GET".to_string()],
                path_prefixes: vec!["/search".to_string()],
            }),
            Constraint::Tool(ToolConstraint {
                tool_names: vec!["web-search".to_string()],
                model_providers: vec![],
                action_labels: vec![],
            }),
            Constraint::Payment(PaymentConstraint {
                max_per_request: AmountLimit { amount: 200 },
                period_limit: None,
                allowed_assets: vec![AssetRef::new("USDC", Some("base".to_string()))],
                allowed_rails: vec![PaymentRail::Onchain],
                allowed_schemes: vec!["exact".to_string()],
                payee_ids: vec!["merchant-a".to_string()],
            }),
            Constraint::Sponsorship(SponsorshipConstraint {
                allow_sponsored_execution: false,
                sponsor_ids: vec![],
            }),
        ],
        metadata: WarrantMetadata::default(),
        signature: issuer().sign_message("placeholder"),
    }
    .sign()
}

fn context() -> AuthorizationContext {
    AuthorizationContext {
        merchant_id: "merchant-a".to_string(),
        merchant_host: "merchant-a.example".to_string(),
        tool_name: "web-search".to_string(),
        http_method: "GET".to_string(),
        path_and_query: "/search?q=ledgerflow".to_string(),
        selected_quote_amount: 200,
        asset: "USDC".to_string(),
        scheme: "exact".to_string(),
        payee_id: "merchant-a".to_string(),
        rail: PaymentRail::Onchain,
        challenge_id: "challenge-1".to_string(),
        request_hash: sha256_prefixed("GET\nmerchant-a.example\n/search?q=ledgerflow\nsha256:body"),
        accepted_hash: sha256_prefixed("exact:USDC:200:merchant-a"),
        now_ms: 2_000,
        freshness_window_ms: 60_000,
        presented_delegation_depth: 1,
        payment_subject: subject_ref(),
    }
}

fn proof(warrant: &Warrant, context: &AuthorizationContext) -> Proof {
    Proof::new_signed(
        context.challenge_id.clone(),
        warrant.digest(),
        context.accepted_hash.clone(),
        context.request_hash.clone(),
        context.now_ms,
        "nonce-1",
        &subject_signer(),
    )
}

fn bench_verify_authorization_hot_path(criterion: &mut Criterion) {
    let warrant = warrant();
    let context = context();
    let proof = proof(&warrant, &context);

    criterion.bench_function("verify_authorization_hot_path", |bench| {
        bench.iter(|| {
            let result = verify_authorization(&warrant, &proof, &context);
            assert!(result.is_ok());
        });
    });
}

criterion_group!(benches, bench_verify_authorization_hot_path);
criterion_main!(benches);
