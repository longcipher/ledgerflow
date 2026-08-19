//! Integration tests for the LedgerFlow server.

#![allow(clippy::expect_used)]

use std::sync::Mutex;

use ledgerflow_server::{
    config::{SaasMode, ServerConfig},
    saas::{SaaSContext, SaasAuthExtractor},
};

/// Serializes tests that mutate process-global environment variables. Rust
/// runs tests in parallel threads within one process, so env-var access must
/// be exclusive to avoid cross-test interference.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn saas_extractor(mode: SaasMode, token: Option<&str>) -> SaasAuthExtractor {
    SaasAuthExtractor {
        mode,
        service_token: token.map(str::to_string),
        standalone_tenant: "default".to_string(),
    }
}

fn headers(pairs: &[(&str, &str)]) -> axum::http::HeaderMap {
    let mut map = axum::http::HeaderMap::new();
    for (key, value) in pairs {
        map.insert(
            axum::http::header::HeaderName::from_bytes(key.as_bytes()).expect("header name"),
            axum::http::header::HeaderValue::from_str(value).expect("header value"),
        );
    }
    map
}

#[test]
fn standalone_mode_injects_fixed_tenant() {
    let extractor = saas_extractor(SaasMode::Standalone, None);
    let ctx = extractor.extract_from_headers(&headers(&[])).expect("standalone always succeeds");
    assert_eq!(ctx.tenant_id, "default");
    assert_eq!(ctx, SaaSContext::standalone("default"));
}

#[test]
fn saas_mode_rejects_missing_token() {
    let extractor = saas_extractor(SaasMode::Saas, Some("secret"));
    let error = extractor.extract_from_headers(&headers(&[])).expect_err("missing token");
    assert!(matches!(error, ledgerflow_server::SaasAuthError::InvalidToken));
}

#[test]
fn saas_mode_rejects_wrong_token() {
    let extractor = saas_extractor(SaasMode::Saas, Some("secret"));
    let error = extractor
        .extract_from_headers(&headers(&[("authorization", "Bearer wrong")]))
        .expect_err("wrong token");
    assert!(matches!(error, ledgerflow_server::SaasAuthError::InvalidToken));
}

#[test]
fn saas_mode_accepts_valid_token_and_internal_headers() {
    let extractor = saas_extractor(SaasMode::Saas, Some("secret"));
    let ctx = extractor
        .extract_from_headers(&headers(&[
            ("authorization", "Bearer secret"),
            ("x-internal-tenant-id", "org:acme"),
            ("x-internal-user-id", "user-1"),
            ("x-internal-roles", "admin,user"),
            ("x-internal-principal", "user-1@issuer"),
        ]))
        .expect("valid");
    assert_eq!(ctx.tenant_id, "org:acme");
    assert_eq!(ctx.user_id.as_deref(), Some("user-1"));
    assert_eq!(ctx.roles, vec!["admin", "user"]);
}

#[test]
fn saas_mode_requires_tenant_header() {
    let extractor = saas_extractor(SaasMode::Saas, Some("secret"));
    let error = extractor
        .extract_from_headers(&headers(&[("authorization", "Bearer secret")]))
        .expect_err("missing tenant");
    assert!(matches!(error, ledgerflow_server::SaasAuthError::MissingTenant));
}

#[test]
fn config_fail_fast_on_invalid_mode() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("LEDGERFLOW_SAAS_MODE", "bogus");
    }
    let error = ServerConfig::from_env().expect_err("invalid mode is fatal");
    assert!(error.to_string().contains("invalid LEDGERFLOW_SAAS_MODE"));
    unsafe {
        std::env::remove_var("LEDGERFLOW_SAAS_MODE");
    }
}

#[test]
fn config_requires_service_token_in_saas_mode() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("LEDGERFLOW_SAAS_MODE", "saas");
    }
    unsafe {
        std::env::remove_var("LEDGERFLOW_SERVICE_TOKEN");
    }
    let error = ServerConfig::from_env().expect_err("missing token is fatal");
    assert!(error.to_string().contains("LEDGERFLOW_SERVICE_TOKEN is required"));
    unsafe {
        std::env::remove_var("LEDGERFLOW_SAAS_MODE");
    }
}

#[test]
fn config_defaults_to_standalone_without_saas_env() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::remove_var("LEDGERFLOW_SAAS_MODE");
    }
    unsafe {
        std::env::remove_var("LEDGERFLOW_SERVICE_TOKEN");
    }
    // The issuer key is mandatory (fail-fast); supply a valid 32-byte hex key.
    unsafe {
        std::env::set_var(
            "LEDGERFLOW_ISSUER_KEY",
            "0101010101010101010101010101010101010101010101010101010101010101",
        );
    }
    let config = ServerConfig::from_env().expect("standalone default");
    assert_eq!(config.saas.mode, SaasMode::Standalone);
    assert_eq!(config.saas.tenant_id, "default");
    unsafe {
        std::env::remove_var("LEDGERFLOW_ISSUER_KEY");
    }
}

#[test]
fn api_health_endpoint_responds() {
    let state = ledgerflow_server::NewAppState::demo().expect("demo state");
    let app = ledgerflow_server::api::router().with_state(state);
    let response = tokio::runtime::Runtime::new().expect("runtime").block_on(async {
        use tower::ServiceExt as _;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/healthz")
                    .body(axum::body::Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
        (status, String::from_utf8_lossy(&body).to_string())
    });
    assert_eq!(response.0, axum::http::StatusCode::OK);
    assert!(response.1.contains("\"ok\":true"));
}

#[test]
fn api_revocation_endpoint_works_end_to_end() {
    let state = ledgerflow_server::NewAppState::demo().expect("demo state");
    let app = ledgerflow_server::api::router().with_state(state);
    let result = tokio::runtime::Runtime::new().expect("runtime").block_on(async {
        use tower::ServiceExt as _;
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/v1/revocations")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({ "warrant_id": "0102030405060708090a0b0c0d0e0f10" }).to_string(),
            ))
            .expect("request");
        let response = app.oneshot(request).await.expect("response");
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.expect("body");
        (status, String::from_utf8_lossy(&body).to_string())
    });
    assert_eq!(result.0, axum::http::StatusCode::OK);
    assert!(result.1.contains("\"ok\":true"));
}
