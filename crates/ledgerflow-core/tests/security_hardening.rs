//! Integration tests for the security-hardening improvements:
//! - delegation-chain cycle detection
//! - extensions unknown-key rejection
//! - 128-bit warrant id entropy
//! - PoP tool-args binding
//! - issuance-time static attenuation + issue bounds
//! - SRL (Signed Revocation List) semantics

#![allow(clippy::expect_used)]

use ledgerflow_core::{
    IssueBounds, MerchantConstraint, PaymentConstraint, PaymentRail, PopTuple,
    ResourceConstraint, SignerRef, SigningKeyPair, SignedRevocationList, SrlEntry, SrlState,
    ToolArguments, TrustedIssuer, TrustedIssuers, Warrant, WarrantBuilder, WarrantChain,
    generate_warrant_id_128, hex_encode_bytes, validate_attenuation, verify_chain,
};
use ledgerflow_core::constraint::{AuthorizationContext, Constraint};

fn issuer_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[0x11; 32])
}

fn holder_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[0x22; 32])
}

fn delegate_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[0x33; 32])
}

fn fixed_id(tag: &str) -> [u8; 16] {
    let mut id = [0_u8; 16];
    for (i, b) in tag.bytes().take(16).enumerate() {
        id[i] = b;
    }
    id
}

fn merchant() -> MerchantConstraint {
    MerchantConstraint::with_ids(vec!["merchant-a".to_string()])
}

fn resource() -> ResourceConstraint {
    ResourceConstraint {
        http_methods: vec!["POST".to_string()],
        path_prefixes: vec!["/pay".to_string()],
    }
}

const fn payment(cap: u128) -> PaymentConstraint {
    PaymentConstraint::new(cap)
}

fn root_warrant(now_ms: u64, ttl_secs: u64, max_depth: u8) -> Warrant {
    WarrantBuilder::new(now_ms)
        .warrant_id(fixed_id("root-00000000000"))
        .ttl_secs(ttl_secs)
        .max_depth(max_depth)
        .issuer(issuer_keys().signer_ref())
        .holder(holder_keys().signer_ref())
        .merchant(merchant())
        .resource(resource())
        .payment(payment(1_000))
        .sign_with(&issuer_keys(), [0_u8; 8])
}

fn trusted() -> TrustedIssuers {
    let mut set = TrustedIssuers::new();
    set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
    set
}

// ---------------------------------------------------------------------------
// 1. Delegation-chain cycle detection
// ---------------------------------------------------------------------------

#[test]
fn duplicate_warrant_id_in_chain_is_rejected() {
    let root = root_warrant(2_000, 86_400, 3);
    // Same root appears twice: cycle.
    let chain = WarrantChain { warrants: vec![root.clone(), root.clone()] };
    let ctx = context(2_000, &holder_keys().signer_ref());
    let proof = proof_for(&root, &ctx, &holder_keys());
    let error = verify_chain(&chain, &trusted(), &proof, &ctx).expect_err("cycle");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::DuplicateWarrantInChain { .. }
    ));
}

#[test]
fn distinct_ids_same_node_are_allowed() {
    // A -> B -> A (different ids) is allowed by tenuo's rule: the ids differ,
    // even though the holder patterns may repeat.
    let root = root_warrant(2_000, 86_400, 3);
    let first = ledgerflow_core::typestate::DelegatedWarrantBuilder::from(root.clone())
        .issue_to(delegate_keys().signer_ref(), &holder_keys(), 2_000, [1_u8; 8]);
    // Issue a second child from the first (different id via different seed).
    let second = ledgerflow_core::typestate::DelegatedWarrantBuilder::from(first.clone())
        .issue_to(holder_keys().signer_ref(), &delegate_keys(), 2_000, [2_u8; 8]);
    let chain = WarrantChain { warrants: vec![root, first, second] };
    let leaf = chain.leaf().expect("leaf");
    let ctx = context(2_000, &holder_keys().signer_ref());
    let proof = proof_for(leaf, &ctx, &holder_keys());
    let verified = verify_chain(&chain, &trusted(), &proof, &ctx).expect("distinct ids ok");
    assert_eq!(verified.chain_len, 3);
}

fn context(now_ms: u64, holder: &SignerRef) -> AuthorizationContext {
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
        request_hash: ledgerflow_core::sha256_prefixed("req"),
        accepted_hash: ledgerflow_core::sha256_prefixed("acc"),
        now_ms,
        freshness_window_ms: 60_000,
        clock_skew_ms: 30_000,
        payment_subject: ledgerflow_core::PaymentSubjectRef::new(
            ledgerflow_core::PaymentSubjectKind::Caip10,
            "caip10:eip155:8453:0xabc123",
        ),
        presenter: holder.clone(),
    }
}

fn proof_for(
    warrant: &Warrant,
    ctx: &AuthorizationContext,
    signer: &SigningKeyPair,
) -> ledgerflow_core::PopProof {
    ledgerflow_core::ProofBuilder::new()
        .warrant_id(warrant.id.clone())
        .challenge_id(ctx.challenge_id.clone())
        .method(ctx.http_method.clone())
        .uri(format!("{}{}", ctx.merchant_host, ctx.path_and_query))
        .request_hash(ctx.request_hash.clone())
        .accepted_hash(ctx.accepted_hash.clone())
        .payment_payload_digest(ledgerflow_core::sha256_prefixed("payload"))
        .nonce("nonce-1".to_string())
        .created_at_ms(ctx.now_ms)
        .sign_with(signer)
}

// ---------------------------------------------------------------------------
// 2. Extensions unknown-key rejection
// ---------------------------------------------------------------------------

#[test]
fn unknown_extension_key_is_rejected_on_decode() {
    let mut warrant = root_warrant(2_000, 86_400, 3);
    warrant.extensions.insert("evil.unknown".to_string(), vec![1, 2, 3]);
    // Signing succeeds (the extension is application data), but decoding
    // rejects the unknown key.
    let bytes = warrant.encode_cbor().expect("encode");
    let error = Warrant::decode_cbor(&bytes).expect_err("unknown extension");
    assert!(matches!(error, ledgerflow_core::WireError::UnknownExtension { .. }));
}

#[test]
fn reserved_extension_keys_are_accepted() {
    let mut warrant = root_warrant(2_000, 86_400, 3);
    warrant.extensions.insert("ledgerflow.session_id".to_string(), b"sess-1".to_vec());
    let bytes = warrant.encode_cbor().expect("encode");
    let decoded = Warrant::decode_cbor(&bytes).expect("reserved key ok");
    assert_eq!(
        decoded.extensions.get("ledgerflow.session_id").map(|v| v.as_slice()),
        Some(&b"sess-1"[..])
    );
}

// ---------------------------------------------------------------------------
// 3. 128-bit warrant id entropy
// ---------------------------------------------------------------------------

#[test]
fn warrant_id_128_uses_all_16_bytes() {
    let a = generate_warrant_id_128(1_700_000_000_000, [0xAB; 16]);
    let b = generate_warrant_id_128(1_700_000_000_000, [0xCD; 16]);
    // Version 7 + variant bits.
    assert_eq!(a[6] >> 4, 0x7);
    assert_eq!(b[6] >> 4, 0x7);
    assert_eq!(a[8] >> 6, 0b10);
    // Random tail differs.
    assert_ne!(a, b);
    assert_ne!(&a[14..], &[0_u8; 2]); // bytes 14-15 are now filled
}

// ---------------------------------------------------------------------------
// 4. PoP tool-args binding
// ---------------------------------------------------------------------------

#[test]
fn tool_args_digest_is_order_independent() {
    let mut args1: ToolArguments = std::collections::BTreeMap::new();
    args1.insert("model".to_string(), "gpt-4o".to_string());
    args1.insert("max_tokens".to_string(), "100".to_string());
    let mut args2: ToolArguments = std::collections::BTreeMap::new();
    args2.insert("max_tokens".to_string(), "100".to_string());
    args2.insert("model".to_string(), "gpt-4o".to_string());
    assert_eq!(PopTuple::tool_args_digest(&args1), PopTuple::tool_args_digest(&args2));
    assert!(PopTuple::tool_args_digest(&args1).is_some());
}

#[test]
fn tool_args_digest_is_none_for_empty() {
    assert_eq!(PopTuple::tool_args_digest(&ToolArguments::new()), None);
}

#[test]
fn tool_args_digest_differs_for_different_values() {
    let mut args1: ToolArguments = std::collections::BTreeMap::new();
    args1.insert("model".to_string(), "gpt-4o".to_string());
    let mut args2: ToolArguments = std::collections::BTreeMap::new();
    args2.insert("model".to_string(), "gpt-4o-mini".to_string());
    assert_ne!(
        PopTuple::tool_args_digest(&args1),
        PopTuple::tool_args_digest(&args2)
    );
}

// ---------------------------------------------------------------------------
// 5. Issuance-time static attenuation
// ---------------------------------------------------------------------------

#[test]
fn validate_attenuation_rejects_wider_merchant() {
    let parent = Constraint::Merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]));
    let child = Constraint::Merchant(MerchantConstraint::with_ids(vec![
        "merchant-a".to_string(),
        "merchant-b".to_string(),
    ]));
    let error = validate_attenuation(&parent, &child).expect_err("widening");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::AttenuationViolation { .. }
    ));
}

#[test]
fn validate_attenuation_allows_narrowing() {
    let parent = Constraint::Merchant(MerchantConstraint::with_ids(vec![
        "merchant-a".to_string(),
        "merchant-b".to_string(),
    ]));
    let child = Constraint::Merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]));
    validate_attenuation(&parent, &child).expect("narrowing ok");
}

#[test]
fn validate_attenuation_rejects_wider_amount_cap() {
    let parent = Constraint::Payment(payment(100));
    let child = Constraint::Payment(payment(200));
    let error = validate_attenuation(&parent, &child).expect_err("cap widening");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::AttenuationViolation { .. }
    ));
}

#[test]
fn delegated_builder_narrowing_is_checked_at_issuance() {
    let root = root_warrant(2_000, 86_400, 3);
    // Narrowing the amount cap is allowed.
    let child = ledgerflow_core::typestate::DelegatedWarrantBuilder::from(root)
        .with_payment(payment(50))
        .issue_to(delegate_keys().signer_ref(), &holder_keys(), 2_000, [3_u8; 8]);
    assert_eq!(child.payment.max_per_charge, 50);
}

// ---------------------------------------------------------------------------
// 6. Issue bounds
// ---------------------------------------------------------------------------

#[test]
fn issue_bounds_restrict_delegated_merchants() {
    let bounds = IssueBounds {
        merchant_ids: vec!["merchant-a".to_string()],
        ..IssueBounds::unrestricted()
    };
    let encoded = bounds.encode_cbor().expect("encode");
    let decoded = IssueBounds::decode_cbor(&encoded).expect("decode");
    assert_eq!(decoded, bounds);
    assert_eq!(decoded.merchant_ids, vec!["merchant-a".to_string()]);
}

#[test]
fn warrant_carries_issue_bounds_extension() {
    let bounds = IssueBounds {
        max_per_charge: Some(500),
        ..IssueBounds::unrestricted()
    };
    let bytes = bounds.encode_cbor().expect("encode");
    let warrant = WarrantBuilder::new(2_000)
        .warrant_id(fixed_id("bounds-root-0000"))
        .ttl_secs(86_400)
        .max_depth(3)
        .issuer(issuer_keys().signer_ref())
        .holder(holder_keys().signer_ref())
        .merchant(merchant())
        .resource(resource())
        .payment(payment(1_000))
        .extension(ledgerflow_core::ISSUE_BOUNDS_EXTENSION, bytes)
        .sign_with(&issuer_keys(), [4_u8; 8]);
    let carried = warrant.issue_bounds().expect("bounds carried");
    assert_eq!(carried.max_per_charge, Some(500));
}

// ---------------------------------------------------------------------------
// 7. SRL semantics
// ---------------------------------------------------------------------------

#[test]
fn srl_verify_signature_roundtrip() {
    let control = SigningKeyPair::from_bytes(&[0x55; 32]);
    let list = SignedRevocationList::sign(
        1,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xAA; 16]) }],
        &control,
    );
    assert!(list.verify_signature(&control.signer_ref()));
    // Wrong signer fails.
    let other = SigningKeyPair::from_bytes(&[0x56; 32]);
    assert!(!list.verify_signature(&other.signer_ref()));
}

#[test]
fn srl_state_applies_and_detects_revocations() {
    let control = SigningKeyPair::from_bytes(&[0x55; 32]);
    let holder = SigningKeyPair::from_bytes(&[0x57; 32]).signer_ref();
    let list = SignedRevocationList::sign(
        1,
        vec![
            SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xAA; 16]) },
            SrlEntry::Holder { key_hex: hex_encode_bytes(&holder.public_key) },
        ],
        &control,
    );
    let mut state = SrlState::new();
    state.apply(&list, &control.signer_ref()).expect("apply");
    assert_eq!(state.applied_version, 1);
    assert!(state.is_warrant_revoked(&[0xAA; 16]));
    assert!(state.is_holder_revoked(&holder));
    assert!(!state.is_warrant_revoked(&[0xBB; 16]));
}

#[test]
fn srl_anti_rollback_rejects_old_version() {
    let control = SigningKeyPair::from_bytes(&[0x55; 32]);
    let v2 = SignedRevocationList::sign(
        2,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xAA; 16]) }],
        &control,
    );
    let mut state = SrlState::new();
    state.apply(&v2, &control.signer_ref()).expect("apply v2");

    let v1 = SignedRevocationList::sign(
        1,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xBB; 16]) }],
        &control,
    );
    let error = state.apply(&v1, &control.signer_ref()).expect_err("rollback");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::SrlVersionRegression { .. }
    ));
}

#[test]
fn srl_rejects_invalid_signature() {
    let control = SigningKeyPair::from_bytes(&[0x55; 32]);
    let attacker = SigningKeyPair::from_bytes(&[0x58; 32]);
    let list = SignedRevocationList::sign(
        1,
        vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xAA; 16]) }],
        &attacker,
    );
    let mut state = SrlState::new();
    let error = state.apply(&list, &control.signer_ref()).expect_err("bad sig");
    assert!(matches!(
        error,
        ledgerflow_core::AuthorizationError::InvalidSrlSignature
    ));
}

#[test]
fn srl_is_additive_across_versions() {
    let control = SigningKeyPair::from_bytes(&[0x55; 32]);
    let mut state = SrlState::new();
    state
        .apply(
            &SignedRevocationList::sign(
                1,
                vec![SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xAA; 16]) }],
                &control,
            ),
            &control.signer_ref(),
        )
        .expect("v1");
    state
        .apply(
            &SignedRevocationList::sign(
                2,
                vec![
                    SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xAA; 16]) },
                    SrlEntry::Warrant { id_hex: hex_encode_bytes(&[0xBB; 16]) },
                ],
                &control,
            ),
            &control.signer_ref(),
        )
        .expect("v2");
    // Both revocations still hold after v2 (union semantics).
    assert!(state.is_warrant_revoked(&[0xAA; 16]));
    assert!(state.is_warrant_revoked(&[0xBB; 16]));
}
