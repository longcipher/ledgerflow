//! Integration tests for the LedgerFlow wallet capability layer.

#![allow(clippy::expect_used)]

use ledgerflow_core::{SignerRef, SigningAlgorithm, SigningKeyPair};
use ledgerflow_wallet::{
    EmbeddedSigner, LocalRpcSigner, MockJsonRpcTransport, SignDomain, SignPaymentRequest,
    SignRequest, WalletSigner, request_approval,
};

fn agent_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[71u8; 32])
}

fn approver_keys() -> SigningKeyPair {
    SigningKeyPair::from_bytes(&[72u8; 32])
}

#[test]
fn embedded_signer_signs_and_verifies() {
    let signer = EmbeddedSigner::from_bytes(&[71u8; 32]);
    let request = SignRequest {
        domain: SignDomain::Proof,
        message: b"ledgerflow proof message".to_vec(),
        key: None,
    };
    let result = signer.sign(&request).expect("sign");
    let signer_ref = result.signer;
    assert!(result.signature.verify_strict(&signer_ref, &request.message));
}

#[test]
fn embedded_signer_rejects_mismatched_key_request() {
    let signer = EmbeddedSigner::from_bytes(&[71u8; 32]);
    let wrong_key = SignerRef::new(SigningAlgorithm::Ed25519, vec![9; 32]);
    let request =
        SignRequest { domain: SignDomain::Warrant, message: b"msg".to_vec(), key: Some(wrong_key) };
    let error = signer.sign(&request).expect_err("mismatch");
    assert!(matches!(error, ledgerflow_wallet::WalletError::NoMatchingKey));
}

#[test]
fn embedded_signer_lists_keys() {
    let signer = EmbeddedSigner::from_bytes(&[71u8; 32]);
    let keys = signer.keys().expect("keys");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].public_key, agent_keys().public_key_bytes().to_vec());
}

#[test]
fn embedded_signer_signs_payment_deterministically() {
    let signer = EmbeddedSigner::from_bytes(&[71u8; 32]);
    let request = SignPaymentRequest {
        chain_id: "eip155:8453".to_string(),
        asset: "eip155:8453/slip44:60".to_string(),
        amount: 100,
        payee: "0xpayee".to_string(),
        nonce: Some("1".to_string()),
    };
    let first = signer.sign_payment(&request).expect("first");
    let second = signer.sign_payment(&request).expect("second");
    assert_eq!(first.raw_transaction, second.raw_transaction);
    assert!(first.raw_transaction.starts_with("signed:eip155:8453"));
}

#[test]
fn sign_domains_are_distinct() {
    assert_ne!(SignDomain::Warrant.as_domain_bytes(), SignDomain::Proof.as_domain_bytes());
    assert_ne!(SignDomain::Approval.as_domain_bytes(), SignDomain::Payment.as_domain_bytes());
    assert_eq!(SignDomain::Warrant.as_domain_bytes(), b"ledgerflow-wallet-warrant");
}

#[test]
fn local_rpc_signer_proxies_calls_through_transport() {
    // Mock transport that echoes a fixed signature.
    let expected_sig = vec![7_u8; 64];
    let transport = MockJsonRpcTransport::new(move |method, _params| {
        assert_eq!(method, "ledgerflow_sign");
        Ok(serde_json::json!({
            "signer": {
                "alg": "ed25519",
                "public_key": base64_std(&[3_u8; 32]),
            },
            "signature": { "value": base64_std(&expected_sig) },
        }))
    });
    let signer = LocalRpcSigner::new(transport);
    let request =
        SignRequest { domain: SignDomain::Approval, message: b"approve me".to_vec(), key: None };
    let result = signer.sign(&request).expect("sign");
    assert_eq!(result.signer.public_key, vec![3_u8; 32]);
    assert_eq!(result.signature.value, vec![7_u8; 64]);
}

#[test]
fn local_rpc_signer_lists_keys_from_transport() {
    let transport = MockJsonRpcTransport::new(|method, _params| {
        assert_eq!(method, "ledgerflow_keys");
        Ok(serde_json::json!([
            { "alg": "ed25519", "public_key": base64_std(&[5_u8; 32]), "key_id": "k1" }
        ]))
    });
    let signer = LocalRpcSigner::new(transport);
    let keys = signer.keys().expect("keys");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].key_id.as_deref(), Some("k1"));
}

#[test]
fn request_approval_produces_verifiable_approval() {
    // The EmbeddedSigner holds the approver key, so the approval signature
    // can be verified with core's SignedApproval::verify_signature.
    let wallet = EmbeddedSigner::new(approver_keys());
    let approver = approver_keys().signer_ref();
    let approval = request_approval(&wallet, approver, "sha256:request", 2_000).expect("approval");
    assert!(approval.verify_signature());
    assert_eq!(approval.request_hash, "sha256:request");
}

#[test]
fn wallet_descriptor_is_available() {
    let signer = EmbeddedSigner::from_bytes(&[71u8; 32]);
    let descriptor = signer.descriptor();
    assert_eq!(descriptor.name, "embedded");
    assert!(descriptor.algorithms.contains(&SigningAlgorithm::Ed25519));
}

fn base64_std(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
