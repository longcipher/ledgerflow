//! End-to-end tests: real `HttpJsonRpcTransport` (hpx) against the loopback
//! HTTP JSON-RPC server.
//!
//! Only compiled/run when the `http` feature is enabled:
//! `cargo test -p ledgerflow-wallet --features http`.

#![cfg(feature = "http")]
#![allow(clippy::expect_used)]

use std::sync::Arc;

use ledgerflow_wallet::{
    EmbeddedSigner, LocalRpcConfig, LocalRpcSigner, LoopbackJsonRpcServer, SignDomain,
    SignPaymentRequest, SignRequest, WalletSigner,
};

fn wallet() -> Arc<dyn WalletSigner> {
    Arc::new(EmbeddedSigner::from_bytes(&[0x42; 32]))
}

#[test]
fn http_transport_signs_through_loopback_server() {
    let wallet = wallet();
    let server = LoopbackJsonRpcServer::start(Arc::clone(&wallet)).expect("start server");
    let signer = LocalRpcSigner::new_http(LocalRpcConfig { url: server.url(), timeout_ms: 5_000 });

    let request = SignRequest {
        domain: SignDomain::Proof,
        message: b"ledgerflow over HTTP".to_vec(),
        key: None,
    };
    let result = signer.sign(&request).expect("sign");
    assert_eq!(result.signature.value.len(), 64);
    let expected_pk = wallet.keys().expect("keys")[0].public_key.clone();
    assert_eq!(result.signer.public_key, expected_pk);
}

#[test]
fn http_transport_lists_keys_through_loopback_server() {
    let wallet = wallet();
    let server = LoopbackJsonRpcServer::start(Arc::clone(&wallet)).expect("start server");
    let signer = LocalRpcSigner::new_http(LocalRpcConfig { url: server.url(), timeout_ms: 5_000 });

    let keys = signer.keys().expect("keys");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].public_key, wallet.keys().expect("keys")[0].public_key);
}

#[test]
fn http_transport_signs_payment_through_loopback_server() {
    let wallet = wallet();
    let server = LoopbackJsonRpcServer::start(Arc::clone(&wallet)).expect("start server");
    let signer = LocalRpcSigner::new_http(LocalRpcConfig { url: server.url(), timeout_ms: 5_000 });

    let request = SignPaymentRequest {
        chain_id: "eip155:8453".to_string(),
        asset: "eip155:8453/slip44:60".to_string(),
        amount: 1_000_000,
        payee: "0xpayee".to_string(),
        nonce: Some("7".to_string()),
    };
    let payment = signer.sign_payment(&request).expect("sign_payment");
    assert!(payment.raw_transaction.starts_with("signed:eip155:8453"));
    assert!(payment.raw_transaction.contains("1000000"));
}

#[test]
fn http_transport_rejects_unknown_wallet() {
    // Point the transport at a listener that never speaks JSON-RPC (a dead
    // port) and assert the error is surfaced as Unreachable.
    let signer = LocalRpcSigner::new_http(LocalRpcConfig {
        url: "http://127.0.0.1:1/".to_string(),
        timeout_ms: 500,
    });
    let error = signer.keys().expect_err("should be unreachable");
    assert!(matches!(
        error,
        ledgerflow_wallet::WalletError::Unreachable(_) |
            ledgerflow_wallet::WalletError::Transport(_)
    ));
}
