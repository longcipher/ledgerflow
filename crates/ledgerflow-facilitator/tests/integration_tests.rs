//! End-to-end integration tests for the LedgerFlow facilitator.
//!
//! These tests exercise the complete payment flow:
//! 1. Warrant issuance
//! 2. Payment verification (x402 + MPP)
//! 3. Settlement routing
//! 4. Revocation
//! 5. Budget enforcement
//! 6. Session management

#![allow(clippy::expect_used)]

use std::{collections::BTreeMap, sync::Arc};

use ledgerflow_core::{
    AssetRef, AuthorizationContext, InMemoryRevocationCheck, MerchantConstraint, PaymentConstraint,
    PaymentRail, PaymentSubjectKind, PaymentSubjectRef, PopProof, ProofBuilder, ResourceConstraint,
    RevocationCheck, RevocationDecision, SigningKeyPair, TrustedIssuer, TrustedIssuers,
    WarrantBuilder, WarrantChain, sha256_prefixed,
};
use ledgerflow_facilitator::{
    DefaultSubjectResolver, EvmRailAdapter, FileRevocationStore, RailKind, SettlementService,
    VerificationService,
    budget::{BudgetLimit, BudgetTracker},
    session::{SessionConfig, SessionEvent, SessionId, SessionManager, SessionState},
};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

fn issuer_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[0x11; 32])
}

fn holder_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[0x22; 32])
}

fn approver_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[0x33; 32])
}

fn trusted() -> TrustedIssuers {
    let mut set = TrustedIssuers::new();
    set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
    set
}

fn root_warrant(now_ms: u64) -> ledgerflow_core::Warrant {
    WarrantBuilder::new(now_ms)
        .ttl_secs(3_600)
        .max_depth(2)
        .issuer(issuer_keys().signer_ref())
        .holder(holder_keys().signer_ref())
        .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
        .resource(ResourceConstraint {
            http_methods: vec!["POST".to_string()],
            path_prefixes: vec!["/pay".to_string()],
        })
        .payment(
            PaymentConstraint::new(1_000_000)
                .with_asset(AssetRef::new("USDC", Some("base".to_string())))
                .with_rails(vec![PaymentRail::Onchain])
                .with_schemes(vec!["exact".to_string()])
                .with_payees(vec!["merchant-a".to_string()]),
        )
        .sign_with(&issuer_keys(), [0_u8; 8])
}

fn context(now_ms: u64) -> AuthorizationContext {
    AuthorizationContext {
        merchant_id: "merchant-a".to_string(),
        merchant_host: "merchant-a.example".to_string(),
        tool_name: "web-search".to_string(),
        model_provider: String::new(),
        action_label: String::new(),
        http_method: "POST".to_string(),
        path_and_query: "/pay".to_string(),
        selected_amount: 100_000,
        asset: "USDC".to_string(),
        asset_network: Some("base".to_string()),
        scheme: "exact".to_string(),
        payee_id: "merchant-a".to_string(),
        rail: PaymentRail::Onchain,
        challenge_id: "challenge-1".to_string(),
        request_hash: sha256_prefixed("POST\nmerchant-a.example\n/pay\nsha256:body"),
        accepted_hash: sha256_prefixed("exact:USDC:100000:merchant-a"),
        now_ms,
        freshness_window_ms: 60_000,
        clock_skew_ms: 30_000,
        payment_subject: PaymentSubjectRef::new(
            PaymentSubjectKind::Caip10,
            "caip10:eip155:8453:0xabc123",
        ),
        presenter: holder_keys().signer_ref(),
        human_present: false,
    }
}

fn proof_for(warrant: &ledgerflow_core::Warrant, context: &AuthorizationContext) -> PopProof {
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

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

#[test]
fn full_payment_flow_verifies_and_settles() {
    let now_ms = 1_000_000;
    let warrant = root_warrant(now_ms);
    let chain = WarrantChain::single(warrant.clone());
    let ctx = context(now_ms);
    let proof = proof_for(&warrant, &ctx);
    let tool_args = BTreeMap::new();

    // Verify
    let verification = VerificationService::new(InMemoryRevocationCheck::new());
    let outcome = verification.verify(&ledgerflow_facilitator::VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_args,
    });

    assert!(outcome.status.is_verified());
    let authorization = outcome.authorization.expect("authorized");

    // Settle
    let settlement = SettlementService::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![Arc::new(EvmRailAdapter)],
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
    assert_eq!(receipt.rail, RailKind::Evm);
    assert_eq!(receipt.settled_amount, 100_000);
}

#[test]
fn revoked_warrant_is_rejected() {
    let now_ms = 1_000_000;
    let warrant = root_warrant(now_ms);
    let chain = WarrantChain::single(warrant.clone());
    let ctx = context(now_ms);
    let proof = proof_for(&warrant, &ctx);
    let tool_args = BTreeMap::new();

    // Revoke the warrant
    let mut revocation = InMemoryRevocationCheck::new();
    revocation.revoke_warrant(&warrant.id);

    // Verify should fail
    let verification = VerificationService::new(revocation);
    let outcome = verification.verify(&ledgerflow_facilitator::VerifyRequest {
        chain: &chain,
        trusted: &trusted(),
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &tool_args,
    });

    assert_eq!(outcome.status, ledgerflow_facilitator::VerifyStatus::Revoked);
}

#[test]
fn budget_enforcement_blocks_overspending() {
    let mut tracker = BudgetTracker::new();
    let limit = BudgetLimit::daily(500_000);

    // First payment within budget
    tracker.record_payment("warrant-1", 300_000, &limit, 1_000_000).expect("payment 1");

    // Second payment exceeds daily budget
    let result = tracker.record_payment("warrant-1", 300_000, &limit, 1_000_001);
    assert!(result.is_err());
}

#[test]
fn session_lifecycle_works() {
    let mut manager = SessionManager::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![Arc::new(EvmRailAdapter)],
        SessionConfig::default(),
    );

    let session_id = SessionId::new("test-session");
    let now = 1_000_000;
    let holder =
        ledgerflow_core::SignerRef::new(ledgerflow_core::SigningAlgorithm::Ed25519, vec![0x42; 32]);
    let payment_subject = ledgerflow_core::PaymentSubjectRef::new(
        ledgerflow_core::PaymentSubjectKind::Caip10,
        "caip10:eip155:8453:0xabc123",
    );

    // Open session
    manager
        .open_session(session_id.clone(), "USDC", "warrant-digest-1", holder, payment_subject, now)
        .expect("open session");
    assert!(manager.is_active(&session_id));

    // Process payment
    manager
        .process_event(
            &session_id,
            SessionEvent::PaymentReceived { amount: 100_000, asset: "USDC".to_string() },
            now + 1_000,
        )
        .expect("payment");

    // Close session
    let outcome = manager.close_session(&session_id, now + 2_000);
    assert!(outcome.is_ok());
    assert!(!manager.is_active(&session_id));
}

#[test]
fn file_revocation_store_persists_across_restart() {
    let dir = std::env::temp_dir().join(format!("ledgerflow-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create dir");
    let path = dir.join("revocations.jsonl");
    let _ = std::fs::remove_file(&path);

    let now_ms = 1_000_000;
    let warrant = root_warrant(now_ms);

    // Revoke and drop
    {
        let store = FileRevocationStore::open(&path).expect("open");
        store.revoke_warrant(&warrant.id).expect("revoke");
    }

    // Reload and verify
    {
        let store = FileRevocationStore::open(&path).expect("reopen");
        assert_eq!(store.check_warrant(&warrant.id), RevocationDecision::RevokedWarrant);
    }

    // Cleanup
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn mpp_session_revocation_takes_effect_at_next_tick() {
    let mut manager = SessionManager::new(
        InMemoryRevocationCheck::new(),
        DefaultSubjectResolver,
        vec![Arc::new(EvmRailAdapter)],
        SessionConfig::default(),
    );

    let session_id = SessionId::new("mpp-session");
    let now = 1_000_000;
    let holder =
        ledgerflow_core::SignerRef::new(ledgerflow_core::SigningAlgorithm::Ed25519, vec![0x42; 32]);
    let payment_subject = ledgerflow_core::PaymentSubjectRef::new(
        ledgerflow_core::PaymentSubjectKind::Caip10,
        "caip10:eip155:8453:0xabc123",
    );

    manager
        .open_session(session_id.clone(), "USDC", "warrant-digest-1", holder, payment_subject, now)
        .expect("open session");

    // Simulate revocation
    let state =
        manager.process_event(&session_id, SessionEvent::Revoked, now + 1_000).expect("revoke");

    // Session should be closing (not yet closed)
    assert_eq!(state, SessionState::Closing);

    // Next tick closes the session
    let state = manager.process_event(&session_id, SessionEvent::Tick, now + 2_000).expect("tick");
    assert_eq!(state, SessionState::Closed);
}
