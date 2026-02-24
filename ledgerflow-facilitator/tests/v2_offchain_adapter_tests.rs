use std::sync::Arc;

use axum_test::TestServer;
use ledgerflow_facilitator::{
    AppConfig,
    adapters::{
        AdapterDescriptor, AdapterRegistry, OffchainAdapter, OffchainAdapterConfig,
        OffchainBackendConfig,
    },
    build_app,
    service::FacilitatorService,
};
use serde_json::json;

fn test_service() -> FacilitatorService {
    let descriptor = AdapterDescriptor {
        id: "mock-cex".to_string(),
        x402_version: 2,
        scheme: "exact".to_string(),
        networks: vec!["offchain:*".parse().expect("valid pattern")],
    };

    let adapter = OffchainAdapter::try_new(OffchainAdapterConfig {
        descriptor,
        backend: OffchainBackendConfig::Mock {
            payer: "cex:user:alice".to_string(),
            transaction_prefix: "cex-tx".to_string(),
        },
        signers: vec!["cex-facilitator".to_string()],
    })
    .expect("build mock offchain adapter");

    let registry = AdapterRegistry::new(vec![Arc::new(adapter)]);
    FacilitatorService::new(registry)
}

#[tokio::test]
async fn verify_v2_offchain_payment() {
    let app = build_app(test_service(), AppConfig::default());
    let server = TestServer::new(app).expect("start server");

    let request = json!({
        "x402Version": 2,
        "paymentPayload": {
            "x402Version": 2,
            "accepted": {
                "scheme": "exact",
                "network": "offchain:binance",
                "amount": "1000000",
                "payTo": "merchant-001",
                "maxTimeoutSeconds": 300,
                "asset": "USDT",
                "extra": {"provider":"binance"}
            },
            "payload": {
                "paymentIntentId": "pi_123",
                "signature": "test-signature"
            },
            "resource": {
                "description": "premium data",
                "mimeType": "application/json",
                "url": "https://example.com/data"
            }
        },
        "paymentRequirements": {
            "scheme": "exact",
            "network": "offchain:binance",
            "amount": "1000000",
            "payTo": "merchant-001",
            "maxTimeoutSeconds": 300,
            "asset": "USDT",
            "extra": {"provider":"binance"}
        }
    });

    let response = server.post("/verify").json(&request).await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["isValid"], true);
    assert_eq!(body["payer"], "cex:user:alice");
}

#[tokio::test]
async fn settle_v2_offchain_payment() {
    let app = build_app(test_service(), AppConfig::default());
    let server = TestServer::new(app).expect("start server");

    let request = json!({
        "x402Version": 2,
        "paymentPayload": {
            "x402Version": 2,
            "accepted": {
                "scheme": "exact",
                "network": "offchain:okx",
                "amount": "2500000",
                "payTo": "merchant-888",
                "maxTimeoutSeconds": 300,
                "asset": "USDC",
                "extra": {"provider":"okx"}
            },
            "payload": {
                "paymentIntentId": "pi_888",
                "signature": "test-signature"
            },
            "resource": {
                "description": "report",
                "mimeType": "application/json",
                "url": "https://example.com/report"
            }
        },
        "paymentRequirements": {
            "scheme": "exact",
            "network": "offchain:okx",
            "amount": "2500000",
            "payTo": "merchant-888",
            "maxTimeoutSeconds": 300,
            "asset": "USDC",
            "extra": {"provider":"okx"}
        }
    });

    let response = server.post("/settle").json(&request).await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["success"], true);
    assert_eq!(body["payer"], "cex:user:alice");
    assert!(
        body["transaction"]
            .as_str()
            .unwrap_or_default()
            .starts_with("cex-tx-")
    );
    assert_eq!(body["network"], "offchain:okx");
}

#[tokio::test]
async fn supported_includes_offchain_networks() {
    let app = build_app(test_service(), AppConfig::default());
    let server = TestServer::new(app).expect("start server");
    let response = server.get("/supported").await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["kinds"][0]["x402Version"], 2);
    assert_eq!(body["kinds"][0]["scheme"], "exact");
    assert_eq!(body["kinds"][0]["network"], "offchain:*");
}
