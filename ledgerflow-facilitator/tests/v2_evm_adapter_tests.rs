use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{Json, Router, extract::State, routing::post};
use axum_test::TestServer;
use ledgerflow_facilitator::{
    AppConfig,
    adapters::{AdapterDescriptor, AdapterRegistry, EvmAdapter, EvmAdapterConfig},
    build_app,
    service::FacilitatorService,
};
use serde_json::json;

fn evm_service(networks: &[&str], rpc_url: &str, chain_id: u64) -> FacilitatorService {
    let descriptor = AdapterDescriptor {
        id: "evm-test".to_string(),
        x402_version: 2,
        scheme: "exact".to_string(),
        networks: networks
            .iter()
            .map(|network| network.parse().expect("valid chain pattern"))
            .collect(),
    };

    let adapter = EvmAdapter::try_new(EvmAdapterConfig {
        descriptor,
        rpc_url: rpc_url.to_string(),
        chain_id,
        vault_address: Some("0x00000000000000000000000000000000000000F1".to_string()),
        signer_key: None,
        signers: vec![],
    })
    .expect("build evm adapter");

    let registry = AdapterRegistry::new(vec![Arc::new(adapter)]);
    FacilitatorService::new(registry)
}

fn evm_verify_request(network: &str) -> serde_json::Value {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_secs();

    let requirements = json!({
        "scheme": "exact",
        "network": network,
        "amount": "1000000",
        "payTo": "0x0000000000000000000000000000000000000001",
        "maxTimeoutSeconds": 300,
        "asset": "0x0000000000000000000000000000000000000010",
        "extra": null
    });

    json!({
        "x402Version": 2,
        "paymentPayload": {
            "x402Version": 2,
            "accepted": requirements.clone(),
            "payload": {
                "signature": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "authorization": {
                    "from": "0x00000000000000000000000000000000000000aa",
                    "to": "0x0000000000000000000000000000000000000001",
                    "value": "1000000",
                    "validAfter": format!("{}", now.saturating_sub(5)),
                    "validBefore": format!("{}", now + 300),
                    "nonce": "0x1111111111111111111111111111111111111111111111111111111111111111"
                }
            },
            "resource": {
                "url": "https://example.com/data",
                "description": "evm test payload",
                "mimeType": "application/json"
            }
        },
        "paymentRequirements": requirements
    })
}

fn evm_verify_request_with_value(network: &str, value: &str) -> serde_json::Value {
    let mut request = evm_verify_request(network);
    request["paymentPayload"]["payload"]["authorization"]["value"] = json!(value);
    request
}

fn evm_verify_request_with_nonce(network: &str, nonce: &str) -> serde_json::Value {
    let mut request = evm_verify_request(network);
    request["paymentPayload"]["payload"]["authorization"]["nonce"] = json!(nonce);
    request
}

#[derive(Clone)]
struct MockRpcState {
    chain_id_hex: String,
    balance_hex: String,
    nonce_used: bool,
    eth_call_index: Arc<AtomicUsize>,
}

fn padded_u256_hex(value: u128) -> String {
    format!("0x{value:064x}")
}

async fn mock_rpc_handler(
    State(state): State<MockRpcState>,
    Json(request): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let id = request.get("id").cloned().unwrap_or_else(|| json!(1));
    let method = request
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    let result = match method {
        "eth_chainId" => serde_json::Value::String(state.chain_id_hex),
        "eth_call" => match state.eth_call_index.fetch_add(1, Ordering::Relaxed) {
            0 => serde_json::Value::String(state.balance_hex),
            1 => serde_json::Value::String(if state.nonce_used {
                padded_u256_hex(1)
            } else {
                padded_u256_hex(0)
            }),
            _ => serde_json::Value::String("0x".to_string()),
        },
        _ => serde_json::Value::String("0x".to_string()),
    };

    Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}

async fn start_mock_rpc(chain_id: u64, balance: u128, nonce_used: bool) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock rpc");
    let addr = listener.local_addr().expect("mock rpc local addr");

    let state = MockRpcState {
        chain_id_hex: format!("0x{chain_id:x}"),
        balance_hex: padded_u256_hex(balance),
        nonce_used,
        eth_call_index: Arc::new(AtomicUsize::new(0)),
    };
    let app = Router::new()
        .route("/", post(mock_rpc_handler))
        .with_state(state);

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("mock rpc server exited");
    });

    format!("http://{addr}")
}

#[tokio::test]
async fn verify_v2_evm_payment_with_mock_rpc() {
    let rpc_url = start_mock_rpc(84532, 5_000_000, false).await;
    let app = build_app(
        evm_service(&["eip155:84532"], &rpc_url, 84532),
        AppConfig::default(),
    );
    let server = TestServer::new(app).expect("start server");

    let response = server
        .post("/verify")
        .json(&evm_verify_request("eip155:84532"))
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["isValid"], true);
    assert_eq!(
        body["payer"]
            .as_str()
            .unwrap_or_default()
            .to_ascii_lowercase(),
        "0x00000000000000000000000000000000000000aa"
    );
}

#[tokio::test]
async fn verify_rejects_chain_id_mismatch_even_when_pattern_matches() {
    let app = build_app(
        evm_service(&["eip155:*"], "http://127.0.0.1:9", 84532),
        AppConfig::default(),
    );
    let server = TestServer::new(app).expect("start server");

    let response = server
        .post("/verify")
        .json(&evm_verify_request("eip155:1"))
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["isValid"], false);
    assert!(
        body["invalidReason"]
            .as_str()
            .unwrap_or_default()
            .contains("chain id")
    );
}

#[tokio::test]
async fn verify_rejects_non_exact_amount() {
    let app = build_app(
        evm_service(&["eip155:84532"], "http://127.0.0.1:9", 84532),
        AppConfig::default(),
    );
    let server = TestServer::new(app).expect("start server");

    let response = server
        .post("/verify")
        .json(&evm_verify_request_with_value("eip155:84532", "1000001"))
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["isValid"], false);
    assert!(
        body["invalidReason"]
            .as_str()
            .unwrap_or_default()
            .contains("amount mismatch")
    );
}

#[tokio::test]
async fn settle_v2_evm_without_signer_returns_x402_error() {
    let app = build_app(
        evm_service(&["eip155:84532"], "http://127.0.0.1:9", 84532),
        AppConfig::default(),
    );
    let server = TestServer::new(app).expect("start server");

    let response = server
        .post("/settle")
        .json(&evm_verify_request("eip155:84532"))
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    let reason = body["errorReason"]
        .as_str()
        .or_else(|| body["error_reason"].as_str())
        .or_else(|| body["reason"].as_str())
        .unwrap_or_default();
    assert!(
        reason.contains("invalid_request: settlement requires a signer key"),
        "unexpected settle error payload: {body}",
    );
}

#[tokio::test]
async fn verify_invalid_request_uses_x402_invalid_shape() {
    let app = build_app(
        evm_service(&["eip155:84532"], "http://127.0.0.1:9", 84532),
        AppConfig::default(),
    );
    let server = TestServer::new(app).expect("start server");

    let response = server
        .post("/verify")
        .json(&evm_verify_request_with_nonce(
            "eip155:84532",
            "0x1234", // invalid nonce length
        ))
        .await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    assert_eq!(body["isValid"], false);
    assert!(
        body["invalidReason"]
            .as_str()
            .unwrap_or_default()
            .contains("invalid_request:")
    );
}

#[tokio::test]
async fn supported_includes_exact_eip3009_extension_for_evm_adapter() {
    let app = build_app(
        evm_service(&["eip155:84532"], "http://127.0.0.1:9", 84532),
        AppConfig::default(),
    );
    let server = TestServer::new(app).expect("start server");

    let response = server.get("/supported").await;
    response.assert_status_ok();

    let body: serde_json::Value = response.json();
    let extensions = body["extensions"].as_array().cloned().unwrap_or_default();
    assert!(extensions.iter().any(|value| value == "exact-eip3009"));
}
