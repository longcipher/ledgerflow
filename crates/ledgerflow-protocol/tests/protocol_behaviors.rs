//! Integration tests for the LedgerFlow protocol binding layer.

#![allow(clippy::expect_used)]

use ledgerflow_core::{
    AssetRef, InMemoryRevocationCheck, MerchantConstraint, PaymentConstraint, PaymentRail,
    PaymentSubjectKind, PaymentSubjectRef, SigningKeyPair, TrustedIssuer, TrustedIssuers, Warrant,
    WarrantBuilder, WarrantChain,
};
use ledgerflow_protocol::{
    AcceptedQuote, HttpRequest, InMemoryReplayStore, InMemoryWarrantRepository,
    LedgerFlowAuthorizationExtension, LedgerFlowCarrier, LedgerFlowChallenge, MerchantVerifier,
    PaymentPayloadSeed, SlimAuthorization, build_payment_payload, canonical_accepted_hash,
    canonical_request_hash, decode_authorization_param, decode_challenge_param,
    encode_authorization_param, encode_challenge_param, merchant_payment_required,
};

fn issuer_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[61u8; 32])
}

fn holder_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[62u8; 32])
}

fn subject_ref() -> PaymentSubjectRef {
    PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
}

fn root_warrant() -> Warrant {
    let issuer = issuer_keys();
    let holder = holder_keys();
    WarrantBuilder::new(2_000)
        .warrant_id(*b"root-00000000000")
        .ttl_secs(60)
        .max_depth(1)
        .issuer(issuer.signer_ref())
        .holder(holder.signer_ref())
        .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
        .resource(ResourceConstraintFixture::make())
        .payment(
            PaymentConstraint::new(1_000)
                .with_asset(AssetRef::new("USDC", Some("base".to_string())))
                .with_rails(vec![PaymentRail::Onchain])
                .with_schemes(vec!["exact".to_string()]),
        )
        .sign_with(&issuer, [0_u8; 8])
}

struct ResourceConstraintFixture;

impl ResourceConstraintFixture {
    fn make() -> ledgerflow_core::ResourceConstraint {
        ledgerflow_core::ResourceConstraint {
            http_methods: vec!["POST".to_string()],
            path_prefixes: vec!["/pay".to_string()],
        }
    }
}

fn trusted() -> TrustedIssuers {
    let mut set = TrustedIssuers::new();
    set.add(TrustedIssuer::new("issuer-1".to_string(), issuer_keys().signer_ref()));
    set
}

fn challenge() -> LedgerFlowChallenge {
    merchant_payment_required(
        "challenge-1",
        "merchant-a",
        "/pay",
        vec![AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()))],
        60_000,
    )
    .ledgerflow
    .expect("challenge")
}

fn request() -> HttpRequest {
    HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec())
}

const fn tool_arguments() -> std::collections::BTreeMap<String, String> {
    std::collections::BTreeMap::new()
}

fn seed() -> PaymentPayloadSeed {
    PaymentPayloadSeed {
        payment_subject: subject_ref(),
        signer: holder_keys(),
        created_at_ms: 2_000,
        nonce: "nonce-1".to_string(),
        payment_identifier: Some("payment-1".to_string()),
        tool_args: tool_arguments(),
        approvals: Vec::new(),
    }
}

#[test]
fn challenge_cbor_round_trip_preserves_fields() {
    let challenge = challenge();
    let encoded = challenge.encode_cbor().expect("encode");
    let decoded = LedgerFlowChallenge::decode_cbor(&encoded).expect("decode");
    assert_eq!(decoded, challenge);
    assert_eq!(decoded.challenge_id, "challenge-1");
}

#[test]
fn authorization_extension_cbor_round_trip_preserves_fields() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let payload = build_payment_payload(
        &challenge(),
        &request(),
        accepted,
        WarrantChain::single(root_warrant()),
        seed(),
    )
    .expect("build");
    let extension = payload.ledgerflow.expect("extension");
    let encoded = extension.encode_cbor().expect("encode");
    let decoded = LedgerFlowAuthorizationExtension::decode_cbor(&encoded).expect("decode");
    assert_eq!(decoded, extension);
    assert_eq!(decoded.warrant_chain.len(), 1);
}

#[test]
fn merchant_verifier_accepts_a_valid_inline_warrant() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let payload = build_payment_payload(
        &challenge(),
        &request(),
        accepted,
        WarrantChain::single(root_warrant()),
        seed(),
    )
    .expect("build");
    let mut verifier = MerchantVerifier::new(
        InMemoryReplayStore::default(),
        InMemoryWarrantRepository::default(),
        InMemoryRevocationCheck::new(),
    );

    let outcome = verifier
        .verify_payment(
            &challenge(),
            &request(),
            &payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_000,
        )
        .expect("verified");

    assert_eq!(outcome.authorization.merchant_id, "merchant-a");
    assert!(!outcome.settlement_reused);
}

#[test]
fn merchant_verifier_rejects_replay() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let challenge = challenge();
    let mut verifier = MerchantVerifier::new(
        InMemoryReplayStore::default(),
        InMemoryWarrantRepository::default(),
        InMemoryRevocationCheck::new(),
    );

    let first_request = HttpRequest::new("POST", "merchant-a.example", "/pay", b"one".to_vec());
    let first_payload = build_payment_payload(
        &challenge,
        &first_request,
        accepted.clone(),
        WarrantChain::single(root_warrant()),
        PaymentPayloadSeed {
            payment_subject: subject_ref(),
            signer: holder_keys(),
            created_at_ms: 2_000,
            nonce: "nonce-1".to_string(),
            payment_identifier: Some("payment-1".to_string()),
            tool_args: tool_arguments(),
            approvals: Vec::new(),
        },
    )
    .expect("build");
    verifier
        .verify_payment(
            &challenge,
            &first_request,
            &first_payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_000,
        )
        .expect("first");

    let second_request = HttpRequest::new("POST", "merchant-a.example", "/pay", b"two".to_vec());
    let second_payload = build_payment_payload(
        &challenge,
        &second_request,
        accepted,
        WarrantChain::single(root_warrant()),
        PaymentPayloadSeed {
            payment_subject: subject_ref(),
            signer: holder_keys(),
            created_at_ms: 2_100,
            nonce: "nonce-1".to_string(),
            payment_identifier: Some("payment-2".to_string()),
            tool_args: tool_arguments(),
            approvals: Vec::new(),
        },
    )
    .expect("build");

    let error = verifier
        .verify_payment(
            &challenge,
            &second_request,
            &second_payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_100,
        )
        .expect_err("replay");

    assert!(matches!(error, ledgerflow_protocol::MerchantVerificationError::ReplayDetected));
}

#[test]
fn merchant_verifier_reuses_cached_payment_identifier() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let challenge = challenge();
    let payload = build_payment_payload(
        &challenge,
        &request(),
        accepted,
        WarrantChain::single(root_warrant()),
        seed(),
    )
    .expect("build");
    let mut verifier = MerchantVerifier::new(
        InMemoryReplayStore::default(),
        InMemoryWarrantRepository::default(),
        InMemoryRevocationCheck::new(),
    );

    verifier
        .verify_payment(
            &challenge,
            &request(),
            &payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_000,
        )
        .expect("first");
    let second = verifier
        .verify_payment(
            &challenge,
            &request(),
            &payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_500,
        )
        .expect("cached");

    assert!(second.settlement_reused);
}

#[test]
fn merchant_verifier_rejects_cached_payment_identifier_with_mismatched_request_binding() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let challenge = challenge();
    let payload = build_payment_payload(
        &challenge,
        &request(),
        accepted,
        WarrantChain::single(root_warrant()),
        seed(),
    )
    .expect("build");
    let mut verifier = MerchantVerifier::new(
        InMemoryReplayStore::default(),
        InMemoryWarrantRepository::default(),
        InMemoryRevocationCheck::new(),
    );

    verifier
        .verify_payment(
            &challenge,
            &request(),
            &payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_000,
        )
        .expect("first");

    let mismatched_request =
        HttpRequest::new("POST", "merchant-a.example", "/pay?other=1", br#"{"ok":true}"#.to_vec());
    let error = verifier
        .verify_payment(
            &challenge,
            &mismatched_request,
            &payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_500,
        )
        .expect_err("mismatched request binding must not reuse cached settlement");

    assert!(matches!(error, ledgerflow_protocol::MerchantVerificationError::ReplayDetected));
}

#[test]
fn merchant_verifier_rejects_challenge_mismatch() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let other_challenge = LedgerFlowChallenge {
        version: "lfx402/v1".to_string(),
        challenge_id: "other".to_string(),
        merchant_id: "merchant-a".to_string(),
        resource: "/pay".to_string(),
        proof_freshness_ms: 60_000,
        clock_skew_ms: 30_000,
        challenge_ttl_ms: 300_000,
        required_subject_kinds: Vec::new(),
        ledger: None,
        human_present: false,
    };
    let payload = build_payment_payload(
        &challenge(),
        &request(),
        accepted,
        WarrantChain::single(root_warrant()),
        seed(),
    )
    .expect("build");
    let mut verifier = MerchantVerifier::new(
        InMemoryReplayStore::default(),
        InMemoryWarrantRepository::default(),
        InMemoryRevocationCheck::new(),
    );

    let error = verifier
        .verify_payment(
            &other_challenge,
            &request(),
            &payload,
            &trusted(),
            "web-search",
            &tool_arguments(),
            2_000,
        )
        .expect_err("challenge");

    assert!(matches!(error, ledgerflow_protocol::MerchantVerificationError::ChallengeMismatch));
}

#[test]
fn mpp_challenge_param_round_trips() {
    let challenge = challenge();
    let encoded = encode_challenge_param(&challenge).expect("encode");
    let decoded = decode_challenge_param(&encoded).expect("decode");
    assert_eq!(decoded, challenge);
}

#[test]
fn mpp_authorization_param_round_trips_as_slim_payload() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let payload = build_payment_payload(
        &challenge(),
        &request(),
        accepted,
        WarrantChain::single(root_warrant()),
        seed(),
    )
    .expect("build");
    let extension = payload.ledgerflow.expect("extension");
    let encoded = encode_authorization_param(
        &extension.warrant_chain,
        &extension.proof,
        &extension.signer,
        &extension.payment_subject,
        &extension.approvals,
    )
    .expect("encode");
    let slim: SlimAuthorization = decode_authorization_param(&encoded).expect("decode");

    assert_eq!(slim.leaf.digest(), root_warrant().digest());
    assert_eq!(slim.payment_subject, subject_ref());
}

#[test]
fn carrier_size_policy_rejects_oversized_header_payloads() {
    let error = LedgerFlowCarrier::HttpHeader
        .validate(ledgerflow_protocol::MAX_HEADER_CBOR_BYTES + 1)
        .expect_err("too large");
    assert!(matches!(error, ledgerflow_protocol::ProtocolError::CarrierTooLarge { .. }));
    // Body carrier has no limit.
    LedgerFlowCarrier::HttpBody
        .validate(ledgerflow_protocol::MAX_HEADER_CBOR_BYTES + 1)
        .expect("body allows large payloads");
}

#[test]
fn canonical_hashes_are_stable() {
    let request = request();
    let left = canonical_request_hash(&request);
    let right = canonical_request_hash(&request);
    assert_eq!(left, right);
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    assert_eq!(canonical_accepted_hash(&accepted), canonical_accepted_hash(&accepted));
}

#[test]
fn proof_binds_leaf_warrant_id_and_hashes() {
    let accepted = AcceptedQuote::exact("USDC", 100, "merchant-a", Some("base".to_string()));
    let payload = build_payment_payload(
        &challenge(),
        &request(),
        accepted,
        WarrantChain::single(root_warrant()),
        seed(),
    )
    .expect("build");
    let extension = payload.ledgerflow.expect("extension");
    assert_eq!(extension.proof.tuple.warrant_id, root_warrant().id);
    assert_eq!(extension.proof.tuple.accepted_hash, canonical_accepted_hash(&payload.accepted));
    assert_eq!(extension.proof.tuple.request_hash, canonical_request_hash(&request()));
}
