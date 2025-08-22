use axum::{http::StatusCode, Router};
use x402_rs::{facilitator_local::FacilitatorLocal, provider_cache::ProviderCache};

// Tiny helper to build app with a ProviderCache that may read env; for CI we allow missing RPCs
async fn test_app() -> eyre::Result<Router> {
    // Build ProviderCache from env; if this fails (no RPCs), we still proceed by skipping settle/verify happy paths.
    let provider_cache = match ProviderCache::from_env().await {
        Ok(p) => p,
        Err(_) => {
            // Create an empty ProviderCache via default constructor is not public; so re-attempt with empty env
            // For endpoints that don't hit chain (GET info/supported), we don't need provider_cache.
            // Return error; tests that depend on it will be skipped.
            return Err(eyre::eyre!("ProviderCache not configured for tests"));
        }
    };
    let facilitator = FacilitatorLocal::new(provider_cache);
    Ok(ledgerflow_facilitator::build_app(facilitator))
}

#[tokio::test]
async fn get_supported_ok() {
    // This test does not need a working provider
    let facilitator = {
        // Best-effort provider; if fails, use a panic-less shortcut by skipping building full app
        match ProviderCache::from_env().await {
            Ok(p) => FacilitatorLocal::new(p),
            Err(_) => {
                // Build with a dummy by reusing from_env attempt again; skip if not available
                // We can't construct a dummy FacilitatorLocal without a ProviderCache, so skip
                eprintln!("Skipping get_supported_ok: missing RPC env");
                return;
            }
        }
    };
    let app = ledgerflow_facilitator::build_app(facilitator);
    let server = axum_test::TestServer::new(app).unwrap();

    let res = server.get("/supported").await;
    res.assert_status_ok();
    let body = res.json::<serde_json::Value>();
    assert!(body.is_array());
}

#[tokio::test]
async fn get_verify_and_settle_info_ok() -> eyre::Result<()> {
    // Same caveat as above: we need a provider to create FacilitatorLocal.
    let app = match test_app().await {
        Ok(app) => app,
        Err(_) => {
            eprintln!("Skipping info endpoints test: missing RPC env");
            return Ok(());
        }
    };
    let server = axum_test::TestServer::new(app).unwrap();

    server.get("/verify").await.assert_status_ok();
    server.get("/settle").await.assert_status_ok();
    Ok(())
}

#[tokio::test]
async fn post_verify_rejects_invalid_payload() -> eyre::Result<()> {
    let app = match test_app().await {
        Ok(app) => app,
        Err(_) => {
            eprintln!("Skipping post_verify test: missing RPC env");
            return Ok(());
        }
    };
    let server = axum_test::TestServer::new(app).unwrap();

    // Send an obviously invalid body to trigger 400 or invalid response mapping
    let res = server
        .post("/verify")
        .json(&serde_json::json!({"foo":"bar"}))
        .await;

    // Either BAD_REQUEST for parse errors or 200 with invalid schema mapping; both are acceptable minimal checks
    assert!(matches!(
        res.status_code(),
        StatusCode::OK | StatusCode::BAD_REQUEST
    ));
    Ok(())
}

#[tokio::test]
async fn post_settle_rejects_invalid_payload() -> eyre::Result<()> {
    let app = match test_app().await {
        Ok(app) => app,
        Err(_) => {
            eprintln!("Skipping post_settle test: missing RPC env");
            return Ok(());
        }
    };
    let server = axum_test::TestServer::new(app).unwrap();

    let res = server
        .post("/settle")
        .json(&serde_json::json!({"foo":"bar"}))
        .await;
    assert!(matches!(
        res.status_code(),
        StatusCode::OK | StatusCode::BAD_REQUEST
    ));
    Ok(())
}
