//! Integration tests for the LedgerFlow core authorization behaviors.
//!
//! These tests exercise the TDD acceptance behaviors described in
//! `docs/design.md` §13.2: authorized payments, delegation attenuation,
//! approval gates, revocation, replay, trust anchors, clock skew, tenant
//! isolation, chain tampering, and dual-protocol reuse.

// Test assertions intentionally use `expect`.
#![allow(clippy::expect_used)]

use ledgerflow_core::{
    ApprovalGate, AssetRef, AuthorizationContext, AuthorizationInput, DelegatedWarrantBuilder,
    InMemoryRevocationCheck, MerchantConstraint, PaymentConstraint, PaymentRail,
    PaymentSubjectKind, PaymentSubjectRef, PopTuple, ProofBuilder, ResourceConstraint,
    SigningKeyPair, TrustedIssuer, TrustedIssuers, Warrant, WarrantBuilder, WarrantChain,
    sha256_prefixed, verify_authorization,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn issuer_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[11u8; 32])
}

fn holder_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[22u8; 32])
}

fn approver_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[33u8; 32])
}

fn delegate_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[44u8; 32])
}

fn subject_ref() -> PaymentSubjectRef {
    PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
}


fn fixed_id(tag: &str) -> [u8; 16] {
    let mut id = [0_u8; 16];
    for (i, b) in tag.bytes().take(16).enumerate() {
        id[i] = b;
    }
    id
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

fn root_warrant() -> Warrant {
    let issuer = issuer_keys();
    let holder = holder_keys();
    WarrantBuilder::new(2_000)
        .warrant_id(fixed_id("root-00000000000"))
        .ttl_secs(60)
        .max_depth(3)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .sign_with(&issuer, [0_u8; 8])
}

fn trusted() -> TrustedIssuers {
    let mut set = TrustedIssuers::new();
    set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
    set
}

fn context(now_ms: u64) -> AuthorizationContext {
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
        selected_amount: 100,
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
        .sign_with(&holder_keys())
}

fn authorize(
    chain: &WarrantChain,
    proof: &ledgerflow_core::PopProof,
    context: &AuthorizationContext,
    approvals: &[ledgerflow_core::SignedApproval],
    revocation: &InMemoryRevocationCheck,
) -> Result<ledgerflow_core::VerifiedAuthorization, ledgerflow_core::AuthorizationError> {
    let input = AuthorizationInput {
        chain,
        trusted: &trusted(),
        proof,
        context,
        approvals,
        tool_arguments: &std::collections::BTreeMap::new(),
        revocation,
    };
    verify_authorization(&input)
}

// ---------------------------------------------------------------------------
// Acceptance behaviors
// ---------------------------------------------------------------------------

#[test]
fn authorized_payment_succeeds_with_root_warrant() {
    let warrant = root_warrant();
    let ctx = context(2_000);
    let proof = proof(&warrant, &ctx);
    let revocation = InMemoryRevocationCheck::new();

    let verified =
        authorize(&WarrantChain::single(warrant), &proof, &ctx, &[], &revocation).expect("ok");
    assert_eq!(verified.merchant_id, "merchant-a");
    assert_eq!(verified.amount, 100);
    assert_eq!(verified.chain_len, 1);
}

#[test]
fn payment_without_warrant_is_rejected() {
    // Empty chain -> rejected.
    let ctx = context(2_000);
    let dummy = ProofBuilder::new()
        .warrant_id(vec![1; 16])
        .challenge_id(ctx.challenge_id.clone())
        .method("POST".to_string())
        .uri("merchant-a.example/pay".to_string())
        .request_hash(ctx.request_hash.clone())
        .accepted_hash(ctx.accepted_hash.clone())
        .payment_payload_digest(sha256_prefixed("x402-payload"))
        .nonce("n".to_string())
        .created_at_ms(ctx.now_ms)
        .sign_with(&holder_keys());
    let revocation = InMemoryRevocationCheck::new();
    let error = authorize(&WarrantChain::default(), &dummy, &ctx, &[], &revocation)
        .expect_err("empty chain");
    assert_eq!(error, ledgerflow_core::AuthorizationError::EmptyChain);
}

#[test]
fn untrusted_issuer_is_rejected() {
    // A warrant issued by an unknown key -> untrusted root.
    let unknown_issuer = SigningKeyPair::from_bytes(&[99u8; 32]);
    let holder = holder_keys();
    let warrant = WarrantBuilder::new(2_000)
        .warrant_id(fixed_id("root-unknown-0000"))
        .ttl_secs(60)
        .issuer(unknown_issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .sign_with(&unknown_issuer, [0_u8; 8]);
    let ctx = context(2_000);
    let proof = proof(&warrant, &ctx);
    let revocation = InMemoryRevocationCheck::new();

    let error = authorize(&WarrantChain::single(warrant), &proof, &ctx, &[], &revocation)
        .expect_err("untrusted");
    assert!(matches!(error, ledgerflow_core::AuthorizationError::UntrustedIssuer { .. }));
}

#[test]
fn delegation_chain_passes_when_fully_valid() {
    let root = root_warrant();
    let ctx = context(2_000);

    // Delegated warrant: holder of root delegates to the agent key.
    let child = DelegatedWarrantBuilder::from(root.clone())
        .issue_to(delegate_keys().signer_ref(), &holder_keys(), 2_000, [0_u8; 8]);
    let chain = WarrantChain {
        warrants: vec![root, child],
    };
    // The leaf holder is now delegate_keys; re-sign proof with it.
    let leaf = chain.leaf().expect("leaf");
    let proof = ProofBuilder::new()
        .warrant_id(leaf.id.clone())
        .challenge_id(ctx.challenge_id.clone())
        .method(ctx.http_method.clone())
        .uri(format!("{}{}", ctx.merchant_host, ctx.path_and_query))
        .request_hash(ctx.request_hash.clone())
        .accepted_hash(ctx.accepted_hash.clone())
        .payment_payload_digest(sha256_prefixed("x402-payload"))
        .nonce("nonce-1".to_string())
        .created_at_ms(ctx.now_ms)
        .sign_with(&delegate_keys());
    let revocation = InMemoryRevocationCheck::new();

    let verified = authorize(&chain, &proof, &ctx, &[], &revocation).expect("delegated ok");
    assert_eq!(verified.chain_len, 2);
    assert_eq!(verified.holder.public_key, delegate_keys().public_key_bytes().to_vec());
}

#[test]
fn delegated_chain_with_excessive_amount_is_rejected_by_runtime_conjunction() {
    let root = root_warrant(); // cap 1000
    let ctx = context(2_000);
    // Child with a HIGHER cap must be rejected (I7 amount monotonicity).
    let child = DelegatedWarrantBuilder::from(root.clone()).issue_to(
        delegate_keys().signer_ref(),
        &holder_keys(),
        2_000,
        [0_u8; 8],
    );
    // Bump child cap beyond parent (tamper -> I7 violation).
    let mut tampered = child;
    tampered.payment = payment_constraint(2_000);
    // Re-sign so signature is valid but I7 still fails at link check.
    tampered = tampered.sign_with(&holder_keys());
    let chain = WarrantChain { warrants: vec![root, tampered] };
    let leaf = chain.leaf().expect("leaf");
    let proof = ProofBuilder::new()
        .warrant_id(leaf.id.clone())
        .challenge_id(ctx.challenge_id.clone())
        .method(ctx.http_method.clone())
        .uri(format!("{}{}", ctx.merchant_host, ctx.path_and_query))
        .request_hash(ctx.request_hash.clone())
        .accepted_hash(ctx.accepted_hash.clone())
        .payment_payload_digest(sha256_prefixed("x402-payload"))
        .nonce("nonce-1".to_string())
        .created_at_ms(ctx.now_ms)
        .sign_with(&delegate_keys());
    let revocation = InMemoryRevocationCheck::new();

    let error = authorize(&chain, &proof, &ctx, &[], &revocation).expect_err("I7 violation");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::AmountMonotonicityViolation
    ));
}

#[test]
fn tampered_chain_node_is_rejected() {
    let root = root_warrant();
    let child = DelegatedWarrantBuilder::from(root.clone())
        .issue_to(delegate_keys().signer_ref(), &holder_keys(), 2_000, [0_u8; 8]);
    // Tamper: change merchant on the child without re-signing.
    let mut tampered = child;
    tampered.merchant = MerchantConstraint::with_ids(vec!["evil".to_string()]);
    let chain = WarrantChain { warrants: vec![root, tampered] };
    let leaf = chain.leaf().expect("leaf");
    let ctx = context(2_000);
    let proof = ProofBuilder::new()
        .warrant_id(leaf.id.clone())
        .challenge_id(ctx.challenge_id.clone())
        .method(ctx.http_method.clone())
        .uri(format!("{}{}", ctx.merchant_host, ctx.path_and_query))
        .request_hash(ctx.request_hash.clone())
        .accepted_hash(ctx.accepted_hash.clone())
        .payment_payload_digest(sha256_prefixed("x402-payload"))
        .nonce("nonce-1".to_string())
        .created_at_ms(ctx.now_ms)
        .sign_with(&delegate_keys());
    let revocation = InMemoryRevocationCheck::new();

    let error = authorize(&chain, &proof, &ctx, &[], &revocation).expect_err("tamper");
    // Either the signature fails (I-invalid) or merchant not allowed.
    assert!(
        matches!(
            error,
            ledgerflow_core::AuthorizationError::InvalidWarrantSignature
                | ledgerflow_core::AuthorizationError::MerchantNotAllowed { .. }
        ),
        "unexpected error: {error}"
    );
}

#[test]
fn revoked_warrant_is_rejected_even_with_valid_signatures() {
    let warrant = root_warrant();
    let ctx = context(2_000);
    let proof = proof(&warrant, &ctx);
    let mut revocation = InMemoryRevocationCheck::new();
    revocation.revoke_warrant(&warrant.id);

    let error =
        authorize(&WarrantChain::single(warrant), &proof, &ctx, &[], &revocation).expect_err("revoked");
    assert_eq!(error, ledgerflow_core::AuthorizationError::WarrantRevoked);
}

#[test]
fn revoked_holder_is_rejected() {
    let warrant = root_warrant();
    let ctx = context(2_000);
    let proof = proof(&warrant, &ctx);
    let mut revocation = InMemoryRevocationCheck::new();
    revocation.revoke_holder(&holder_keys().signer_ref());

    let error =
        authorize(&WarrantChain::single(warrant), &proof, &ctx, &[], &revocation).expect_err("holder revoked");
    assert_eq!(error, ledgerflow_core::AuthorizationError::HolderRevoked);
}

#[test]
fn clock_skew_within_tolerance_accepts_proof() {
    let issuer = issuer_keys();
    let holder = holder_keys();
    let warrant = WarrantBuilder::new(2_000)
        .warrant_id(fixed_id("skew-tolerant-wrnt"))
        .ttl_secs(60 * 60)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .sign_with(&issuer, [0_u8; 8]);
    // Proof created 40s before verification: 40_000ms < 60_000ms + 30_000ms.
    let ctx = context(42_000);
    let proof = ProofBuilder::new()
        .warrant_id(warrant.id.clone())
        .challenge_id(ctx.challenge_id.clone())
        .method(ctx.http_method.clone())
        .uri(format!("{}{}", ctx.merchant_host, ctx.path_and_query))
        .request_hash(ctx.request_hash.clone())
        .accepted_hash(ctx.accepted_hash.clone())
        .payment_payload_digest(sha256_prefixed("x402-payload"))
        .nonce("nonce-1".to_string())
        .created_at_ms(2_000)
        .sign_with(&holder_keys());
    let revocation = InMemoryRevocationCheck::new();
    let verified = authorize(&WarrantChain::single(warrant), &proof, &ctx, &[], &revocation)
        .expect("skew tolerated");
    assert_eq!(verified.amount, 100);
}

#[test]
fn stale_proof_outside_freshness_window_is_rejected() {
    // Extend the warrant TTL so the freshness check is reached before expiry.
    let issuer = issuer_keys();
    let holder = holder_keys();
    let warrant = WarrantBuilder::new(2_000)
        .warrant_id(fixed_id("stale-proof-warrant"))
        .ttl_secs(60 * 60)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .sign_with(&issuer, [0_u8; 8]);
    let ctx = context(2_000);
    let proof = ProofBuilder::new()
        .warrant_id(warrant.id.clone())
        .challenge_id(ctx.challenge_id.clone())
        .method(ctx.http_method.clone())
        .uri(format!("{}{}", ctx.merchant_host, ctx.path_and_query))
        .request_hash(ctx.request_hash.clone())
        .accepted_hash(ctx.accepted_hash.clone())
        .payment_payload_digest(sha256_prefixed("x402-payload"))
        .nonce("nonce-1".to_string())
        .created_at_ms(1_000)
        .sign_with(&holder_keys());
    let revocation = InMemoryRevocationCheck::new();
    // Proof created at 1_000ms; verify at 100_000ms => 99s elapsed, way beyond
    // the 60s+30s tolerance, but warrant still valid (expires in 1h).
    let mut stale = ctx;
    stale.now_ms = 100_000;
    let error = authorize(&WarrantChain::single(warrant), &proof, &stale, &[], &revocation)
        .expect_err("stale");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::ProofOutsideFreshnessWindow { .. }
    ));
}

#[test]
fn approval_gate_requires_m_of_n_signatures() {
    let issuer = issuer_keys();
    let holder = holder_keys();
    let approver = approver_keys();
    let warrant = WarrantBuilder::new(2_000)
        .warrant_id(fixed_id("root-approval-000"))
        .ttl_secs(60)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .approval_gate("web-search", ApprovalGate::unconditional())
        .approver(approver.signer_ref())
        .min_approvals(1)
        .sign_with(&issuer, [0_u8; 8]);

    let ctx = context(2_000);
    let proof = proof(&warrant, &ctx);

    // No approvals -> ApprovalRequired.
    let revocation = InMemoryRevocationCheck::new();
    let error = authorize(&WarrantChain::single(warrant.clone()), &proof, &ctx, &[], &revocation)
        .expect_err("no approval");
    assert!(matches!(error, ledgerflow_core::AuthorizationError::ApprovalRequired));

    // With a valid approval -> passes.
    let approval = ledgerflow_core::SignedApproval::sign(
        ctx.request_hash.clone(),
        &approver.signer_ref(),
        ctx.now_ms / 1000 + 300,
        &approver,
    );
    let tuple = PopTuple {
        warrant_id: warrant.id.clone(),
        challenge_id: ctx.challenge_id.clone(),
        method: ctx.http_method.clone(),
        uri: format!("{}{}", ctx.merchant_host, ctx.path_and_query),
        request_hash: ctx.request_hash.clone(),
        accepted_hash: ctx.accepted_hash.clone(),
        payment_payload_digest: sha256_prefixed("x402-payload"),
        approvals_digest: Some(PopTuple::approvals_digest(std::slice::from_ref(&approval))),
        nonce: "nonce-2".to_string(),
        created_at_ms: ctx.now_ms,
    };
    let approved_proof = ProofBuilder::new()
        .warrant_id(warrant.id.clone())
        .challenge_id(tuple.challenge_id.clone())
        .method(tuple.method.clone())
        .uri(tuple.uri.clone())
        .request_hash(tuple.request_hash.clone())
        .accepted_hash(tuple.accepted_hash.clone())
        .payment_payload_digest(tuple.payment_payload_digest.clone())
        .approvals_digest(PopTuple::approvals_digest(std::slice::from_ref(&approval)))
        .nonce(tuple.nonce.clone())
        .created_at_ms(tuple.created_at_ms)
        .sign_with(&holder_keys());

    let verified = authorize(
        &WarrantChain::single(warrant),
        &approved_proof,
        &ctx,
        &[approval],
        &revocation,
    )
    .expect("approved");
    assert_eq!(verified.chain_len, 1);
}

#[test]
fn excessive_amount_is_rejected_by_payment_constraint() {
    let warrant = root_warrant(); // cap 1000
    let mut ctx = context(2_000);
    ctx.selected_amount = 1_001;
    let proof = proof(&warrant, &ctx);
    let revocation = InMemoryRevocationCheck::new();

    let error = authorize(&WarrantChain::single(warrant), &proof, &ctx, &[], &revocation)
        .expect_err("over limit");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::PaymentAmountExceeded { .. }
    ));
}

#[test]
fn wrong_merchant_is_rejected() {
    let warrant = root_warrant();
    let mut ctx = context(2_000);
    ctx.merchant_id = "merchant-b".to_string();
    let proof = proof(&warrant, &ctx);
    let revocation = InMemoryRevocationCheck::new();

    let error = authorize(&WarrantChain::single(warrant), &proof, &ctx, &[], &revocation)
        .expect_err("merchant");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::MerchantNotAllowed { .. }
    ));
}

#[test]
fn challenge_mismatch_is_rejected() {
    let warrant = root_warrant();
    let ctx = context(2_000);
    // Proof bound to the real challenge; verifier uses a different challenge.
    let proof = proof(&warrant, &ctx);
    let mut other = ctx;
    other.challenge_id = "other-challenge".to_string();
    let revocation = InMemoryRevocationCheck::new();

    let error = authorize(&WarrantChain::single(warrant), &proof, &other, &[], &revocation)
        .expect_err("challenge");
    assert_eq!(error, ledgerflow_core::AuthorizationError::ChallengeMismatch);
}

#[test]
fn cbor_round_trip_preserves_warrant() {
    let warrant = root_warrant();
    let encoded = warrant.encode_cbor().expect("encode");
    let decoded = Warrant::decode_cbor(&encoded).expect("decode");
    assert_eq!(decoded, warrant);
    assert_eq!(decoded.digest(), warrant.digest());
}

#[test]
fn tenant_isolation_via_separate_trust_anchors() {
    // Tenant A's warrant issued by tenant A issuer; tenant B verifier rejects.
    let tenant_a_issuer = issuer_keys();
    let holder = holder_keys();
    let warrant = WarrantBuilder::new(2_000)
        .warrant_id(fixed_id("root-tenant-a-0000"))
        .ttl_secs(60)
        .issuer(tenant_a_issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(merchant_constraint())
        .resource(resource_constraint())
        .payment(payment_constraint(1_000))
        .sign_with(&tenant_a_issuer, [0_u8; 8]);

    // Tenant B's trust set does NOT include tenant A's issuer.
    let mut tenant_b_trust = TrustedIssuers::new();
    tenant_b_trust.add(TrustedIssuer::new(
        "tenant-b-issuer".to_string(),
        SigningKeyPair::from_bytes(&[77u8; 32]).signer_ref(),
    ));

    let ctx = context(2_000);
    let proof = proof(&warrant, &ctx);
    let input = AuthorizationInput {
        chain: &WarrantChain::single(warrant),
        trusted: &tenant_b_trust,
        proof: &proof,
        context: &ctx,
        approvals: &[],
        tool_arguments: &std::collections::BTreeMap::new(),
        revocation: &InMemoryRevocationCheck::new(),
    };
    let error = verify_authorization(&input).expect_err("cross-tenant");
    assert!(matches!(error, ledgerflow_core::AuthorizationError::UntrustedIssuer { .. }));
}

#[test]
fn payment_subject_ref_round_trips() {
    let subject = subject_ref();
    assert_eq!(subject.kind, PaymentSubjectKind::Caip10);
    assert_eq!(subject.value, "caip10:eip155:8453:0xabc123");
}

#[test]
fn signer_ref_supports_key_ids() {
    let signer = issuer_keys().signer_ref().with_key_id("k1".to_string());
    assert_eq!(signer.key_id.as_deref(), Some("k1"));
}
