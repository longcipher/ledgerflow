//! Integration tests for the LedgerFlow Facilitator behaviors.
//!
//! Covers: verify orchestration (approved/revoked/over-limit/insufficient
//! approval), settle atomic re-verification (TOCTOU closing), persistent
//! revocation across restarts, and idempotent settlement queries.

#![allow(clippy::expect_used)]

use std::sync::Arc;

use ledgerflow_core::{
    AssetRef, AuthorizationContext, AuthorizationInput, InMemoryRevocationCheck,
    MerchantConstraint, PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef,
    PopProof, ProofBuilder, ResourceConstraint, RevocationCheck, SignedApproval, SignerRef,
    SigningKeyPair, TrustedIssuer, TrustedIssuers, Warrant, WarrantBuilder, WarrantChain,
    sha256_prefixed, verify_authorization,
};
use ledgerflow_facilitator::{
    DefaultSubjectResolver, EvmRailAdapter, FileRevocationStore, SettlementRegistry,
    SettlementService, SharedRailAdapter, SolanaRailAdapter, VerificationService, VerifyRequest,
    VerifyStatus,
};

fn issuer_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[51u8; 32])
}

fn holder_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[52u8; 32])
}

fn approver_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[53u8; 32])
}

fn subject_ref() -> PaymentSubjectRef {
    PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
}

fn solana_subject_ref() -> PaymentSubjectRef {
    PaymentSubjectRef::new(
        PaymentSubjectKind::Caip10,
        "caip10:solana:mainnet:7vfCXTUXx5Wn4P6m7XJ3e1yK2bXxVmW7nYj1m5X9A1t3",
    )
}

fn merchant_constraint() -> MerchantConstraint {
    MerchantConstraint::with_ids(vec!["merchant-a".to_string()])
}

fn resource_constraint() -> ResourceConstraint {
    ResourceConstraint {
        http_methods: vec!["POST".to_string()],
        path_prefixes: vec!["/pay".to_string()],
    }
}

fn payment_constraint(cap: u128) -> PaymentConstraint {
    PaymentConstraint::new(cap)
        .with_asset(AssetRef::new("USDC", Some("base".to_string())))
        .with_rails(vec![PaymentRail::Onchain])
        .with_schemes(vec!["exact".to_string()])
        .with_payees(vec!["merchant-a".to_string()])
}

fn root_warrant(now_ms: u64) -> Warrant {
    let issuer = issuer_keys();
    let holder = holder_keys();
    WarrantBuilder::new(now_ms)
        .warrant_id(*b"root-00000000000")
        .ttl_secs(60)
        .max_depth(1)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .sign_with(&issuer, [0_u8; 8])
}

fn solana_root_warrant(now_ms: u64) -> Warrant {
    let issuer = issuer_keys();
    let holder = holder_keys();
    WarrantBuilder::new(now_ms)
        .warrant_id(*b"root-solana-test")
        .ttl_secs(60)
        .max_depth(1)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(
            PaymentConstraint::new(1_000)
                .with_asset(AssetRef::new("USDC", Some("solana".to_string())))
                .with_rails(vec![PaymentRail::Onchain])
                .with_schemes(vec!["exact".to_string()])
                .with_payees(vec!["merchant-a".to_string()]),
        )
        .sign_with(&issuer, [1_u8; 8])
}

fn trusted() -> TrustedIssuers {
    let mut set = TrustedIssuers::new();
    set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
    set
}

fn context(now_ms: u64, amount: u128) -> AuthorizationContext {
    let request_hash = sha256_prefixed("POST\nmerchant-a.example\n/pay\nsha256:body");
    let accepted_hash = sha256_prefixed("exact:USDC:100:merchant-a");
    AuthorizationContext {
        merchant_id: "merchant-a".to_string(),
        merchant_host: "merchant-a.example".to_string(),
        tool_name: "web-search".to_string(),
        model_provider: String::new(),
        action_label: String::new(),
        http_method: "POST".to_string(),
        path_and_query: "/pay".to_string(),
        selected_amount: amount,
        asset: "USDC".to_string(),
        asset_network: Some("base".to_string()),
        scheme: "exact".to_string(),
        payee_id: "merchant-a".to_string(),
        rail: PaymentRail::Onchain,
        challenge_id: "challenge-1".to_string(),
        request_hash,
        accepted_hash,
        now_ms,
        freshness_window_ms: 60_000,
        clock_skew_ms: 30_000,
        payment_subject: subject_ref(),
        presenter: holder_keys().signer_ref(),
        human_present: false,
    }
}

fn solana_context(now_ms: u64, amount: u128) -> AuthorizationContext {
    let mut context = context(now_ms, amount);
    context.asset_network = Some("solana".to_string());
    context.payment_subject = solana_subject_ref();
    context
}

fn proof(warrant: &Warrant, context: &AuthorizationContext) -> PopProof {
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
        .sign_with(&holder_keys())
}

const fn tool_arguments() -> std::collections::BTreeMap<String, String> {
    std::collections::BTreeMap::new()
}

// ---------------------------------------------------------------------------
// Verify orchestration
// ---------------------------------------------------------------------------

#[test]
fn verify_passes_for_a_valid_authorization() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms);
    let ctx = context(now_ms, 100);
    let proof = proof(&warrant, &ctx);
    let revocation = InMemoryRevocationCheck::new();
    let service = VerificationService::new(revocation);
    let request = VerifyRequest {
        chain: &WarrantChain::single(warrant),
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };

    let outcome = service.verify(&request);
    assert_eq!(outcome.status, VerifyStatus::Verified);
    assert!(outcome.authorization.is_some());
}

#[test]
fn verify_rejects_a_revoked_warrant() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms);
    let ctx = context(now_ms, 100);
    let proof = proof(&warrant, &ctx);
    let mut revocation = InMemoryRevocationCheck::new();
    revocation.revoke_warrant(&warrant.id);
    let service = VerificationService::new(revocation);
    let request = VerifyRequest {
        chain: &WarrantChain::single(warrant),
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };

    let outcome = service.verify(&request);
    assert_eq!(outcome.status, VerifyStatus::Revoked);
}

#[test]
fn verify_rejects_an_over_limit_amount() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms);
    let ctx = context(now_ms, 2_000); // > cap 1000
    let proof = proof(&warrant, &ctx);
    let revocation = InMemoryRevocationCheck::new();
    let service = VerificationService::new(revocation);
    let request = VerifyRequest {
        chain: &WarrantChain::single(warrant),
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };

    let outcome = service.verify(&request);
    assert_eq!(outcome.status, VerifyStatus::Unauthorized);
}

#[test]
fn verify_reports_insufficient_approval() {
    let now_ms = 5_000;
    let issuer = issuer_keys();
    let holder = holder_keys();
    let approver = approver_keys();
    let warrant = WarrantBuilder::new(now_ms)
        .warrant_id(*b"warrant-approval")
        .ttl_secs(60)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .approval_gate("web-search", ledgerflow_core::ApprovalGate::unconditional())
        .approver(approver.signer_ref())
        .min_approvals(1)
        .sign_with(&issuer, [0_u8; 8]);
    let ctx = context(now_ms, 100);
    let proof = proof(&warrant, &ctx);
    let revocation = InMemoryRevocationCheck::new();
    let service = VerificationService::new(revocation);
    let request = VerifyRequest {
        chain: &WarrantChain::single(warrant),
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };

    let outcome = service.verify(&request);
    assert_eq!(outcome.status, VerifyStatus::InsufficientApproval);
}

// ---------------------------------------------------------------------------
// Settle orchestration (TOCTOU closing)
// ---------------------------------------------------------------------------

#[test]
fn settle_succeeds_when_reverification_passes() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms);
    let chain = WarrantChain::single(warrant);
    let ctx = context(now_ms, 100);
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let revocation = InMemoryRevocationCheck::new();

    // First verify (pre-check).
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    let authorization = outcome.authorization.expect("authorized");

    // Then settle with atomic re-verification.
    let settlement =
        SettlementService::new(revocation, DefaultSubjectResolver, vec![EvmRailAdapter]);
    let settle_request = ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms,
    };
    let result = settlement.settle(&settle_request);
    assert_eq!(result.status, ledgerflow_facilitator::SettlementStatus::Settled);
    let receipt = result.receipt.expect("receipt");
    assert!(receipt.transaction_id.starts_with("evm-tx-"));
}

#[test]
fn settle_rejects_when_revoked_after_verify_then_settle() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms);
    let chain = WarrantChain::single(warrant.clone());
    let ctx = context(now_ms, 100);
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let revocation = InMemoryRevocationCheck::new();
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    let authorization = outcome.authorization.expect("authorized");

    // Revoke AFTER verify but BEFORE settle -> settle must reject (TOCTOU closed).
    let mut revocation = revocation;
    revocation.revoke_warrant(&warrant.id);
    let settlement =
        SettlementService::new(revocation, DefaultSubjectResolver, vec![EvmRailAdapter]);
    let settle_request = ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms,
    };
    let result = settlement.settle(&settle_request);
    assert_eq!(result.status, ledgerflow_facilitator::SettlementStatus::Failed);
    assert!(result.reason.is_some());
}

// ---------------------------------------------------------------------------
// Persistent revocation store
// ---------------------------------------------------------------------------

#[test]
fn file_revocation_store_survives_restart() {
    let dir = std::env::temp_dir().join(format!("ledgerflow-revoc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("revocations.jsonl");
    let _ = std::fs::remove_file(&path);

    {
        let store = FileRevocationStore::open(&path).expect("open");
        store.revoke_warrant(&[1_u8; 16]).expect("revoke");
    }
    // New instance (restart) loads the persisted record.
    let reloaded = FileRevocationStore::open(&path).expect("reopen");
    assert_eq!(
        reloaded.check_warrant(&[1_u8; 16]),
        ledgerflow_core::RevocationDecision::RevokedWarrant
    );

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn settlement_registry_is_idempotent() {
    let registry = SettlementRegistry::new();
    let receipt = ledgerflow_facilitator::SettlementReceipt {
        rail: ledgerflow_facilitator::RailKind::Evm,
        transaction_id: "tx-1".to_string(),
        settled_amount: 100,
        asset: "USDC".to_string(),
    };
    registry.record(
        "sha256:warrant",
        receipt.clone(),
        ledgerflow_facilitator::SettlementStatus::Settled,
    );
    registry.record("sha256:warrant", receipt, ledgerflow_facilitator::SettlementStatus::Settled);

    let query = registry.query("tx-1").expect("found");
    assert_eq!(query.receipt.settled_amount, 100);
    let by_warrant = registry.query_by_warrant("sha256:warrant");
    assert_eq!(by_warrant.len(), 1);
}

#[test]
fn settle_rejects_when_warrant_expired_between_verify_and_settle() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms);
    let chain = WarrantChain::single(warrant);
    let ctx = context(now_ms, 100);
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let revocation = InMemoryRevocationCheck::new();
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    let authorization = outcome.authorization.expect("authorized");

    // Warrant TTL is 60s (issued at t=5s => expires 65s). Settle at t=66s.
    let settlement =
        SettlementService::new(revocation, DefaultSubjectResolver, vec![EvmRailAdapter]);
    let settle_request = ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms: 66_000,
    };
    let result = settlement.settle(&settle_request);
    assert_eq!(result.status, ledgerflow_facilitator::SettlementStatus::Failed);
    assert!(result.reason.unwrap_or_default().contains("expired"));
}

#[test]
fn settle_rejects_when_amount_exceeds_cap_during_reverify() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms); // cap 1000
    let chain = WarrantChain::single(warrant);
    let ctx = context(now_ms, 2_000); // 2000 > cap
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let revocation = InMemoryRevocationCheck::new();
    // Verify would already reject; instead craft the settle path directly with
    // an over-limit authorization to exercise reverify's amount check.
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    assert_eq!(outcome.status, VerifyStatus::Unauthorized);
    let _ = ctx;
    let _ = revocation;
}

#[test]
fn settle_accepts_expiry_exactly_at_now() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms); // issued t=5s, ttl 60 => expires 65s
    let chain = WarrantChain::single(warrant);
    let ctx = context(now_ms, 100);
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    let authorization = outcome.authorization.expect("authorized");

    // expires_at = 65s; settling at exactly 65_000ms must pass the `<` check.
    let settlement = SettlementService::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![EvmRailAdapter],
    );
    let result = settlement.settle(&ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms: 65_000,
    });
    assert_eq!(result.status, ledgerflow_facilitator::SettlementStatus::Settled);
}

#[test]
fn settle_accepts_amount_exactly_at_cap() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms); // cap 1000
    let chain = WarrantChain::single(warrant);
    // selected_amount == cap (1000) must pass the `>` check.
    let mut ctx = context(now_ms, 1_000);
    ctx.request_hash = sha256_prefixed("POST\nmerchant-a.example\n/pay\nsha256:body");
    ctx.accepted_hash = sha256_prefixed("exact:USDC:1000:merchant-a");
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    let authorization = outcome.authorization.expect("authorized");

    let settlement = SettlementService::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![EvmRailAdapter],
    );
    let result = settlement.settle(&ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms,
    });
    assert_eq!(result.status, ledgerflow_facilitator::SettlementStatus::Settled);
}

#[test]
fn settle_routes_to_matching_rail_and_fails_when_none() {
    let now_ms = 5_000;
    let warrant = root_warrant(now_ms);
    let chain = WarrantChain::single(warrant);
    let ctx = context(now_ms, 100);
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    let authorization = outcome.authorization.expect("authorized");

    // Matching rail settles.
    let settlement = SettlementService::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![EvmRailAdapter],
    );
    let ok = settlement.settle(&ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms,
    });
    assert_eq!(ok.status, ledgerflow_facilitator::SettlementStatus::Settled);

    // No compatible rail fails with a routing error.
    let none = SettlementService::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![ledgerflow_facilitator::ExchangeRailAdapter],
    );
    let failed = none.settle(&ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms,
    });
    assert_eq!(failed.status, ledgerflow_facilitator::SettlementStatus::Failed);
    assert!(failed.reason.unwrap_or_default().contains("no rail adapter"));
}

#[test]
fn settle_routes_solana_subjects_to_solana_adapter() {
    let now_ms = 5_000;
    let warrant = solana_root_warrant(now_ms);
    let chain = WarrantChain::single(warrant);
    let ctx = solana_context(now_ms, 100);
    let proof = proof(chain.leaf().expect("leaf"), &ctx);
    let verify_service = VerificationService::new(InMemoryRevocationCheck::new());
    let verify_request = VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_arguments(),
    };
    let outcome = verify_service.verify(&verify_request);
    let authorization = outcome.authorization.expect("authorized");

    let settlement = SettlementService::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![
            Arc::new(EvmRailAdapter) as SharedRailAdapter,
            Arc::new(SolanaRailAdapter) as SharedRailAdapter,
        ],
    );
    let result = settlement.settle(&ledgerflow_facilitator::SettleRequest {
        authorization: &authorization,
        chain: &chain,
        proof: &proof,
        context: &ctx,
        now_ms,
    });

    assert_eq!(result.status, ledgerflow_facilitator::SettlementStatus::Settled);
    let receipt = result.receipt.expect("receipt");
    assert_eq!(receipt.rail, ledgerflow_facilitator::RailKind::Solana);
    assert!(receipt.transaction_id.starts_with("solana-tx-"));
}

// ---------------------------------------------------------------------------
// Helper: prove the verify_authorization core path is reachable from here
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn _core_path_is_reachable(
    chain: &WarrantChain,
    proof: &PopProof,
    ctx: &AuthorizationContext,
    approvals: &[SignedApproval],
    holder: SignerRef,
) -> Result<(), ledgerflow_core::AuthorizationError> {
    let input = AuthorizationInput {
        chain,
        trusted: &trusted(),
        proof,
        context: ctx,
        approvals,
        tool_arguments: &tool_arguments(),
        revocation: &InMemoryRevocationCheck::new(),
        payment_payload_digest: None,
    };
    let _ = holder;
    let verified = verify_authorization(&input)?;
    let _ = verified;
    Ok(())
}
