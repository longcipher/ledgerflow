//! Embedded wallet local JSON-RPC server.
//!
//! Implements the LedgerFlow wallet wire protocol (`ledgerflow_sign`,
//! `ledgerflow_keys`, `ledgerflow_sign_payment`) on top of a
//! [`WalletSigner`]. The dispatch logic is a pure function
//! ([`handle_jsonrpc`]) that can be embedded anywhere; a thin
//! [`EmbeddedWalletServer`] wraps a signer and produces full JSON-RPC 2.0
//! responses; and a minimal loopback HTTP/1.1 listener (feature `http`)
//! serves the same protocol over the network for end-to-end use with
//! [`crate::local_rpc::HttpJsonRpcTransport`].
//!
//! Wire protocol (must stay in lockstep with
//! [`crate::local_rpc::LocalRpcSigner`]):
//!
//! - `ledgerflow_sign` params: `{"domain", "message" (base64), "key" (alg/`public_key`
//!   base64/`key_id` | null)}`, result `{"signer": {alg lowercase, public_key base64}, "signature":
//!   {value base64}}`.
//! - `ledgerflow_keys` params `null`, result a JSON array of `{alg lowercase, public_key base64,
//!   key_id}`.
//! - `ledgerflow_sign_payment` params `{"chain_id", "asset", "amount" (decimal string), "payee",
//!   "nonce"}`, result `{"raw_transaction", "tx_hash"}`.

use std::sync::Arc;

use ledgerflow_core::{SignerRef, SigningAlgorithm};

use crate::{
    error::WalletError,
    local_rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, base64_decode, base64_encode},
    signer::{SignDomain, SignPaymentRequest, SignRequest, WalletSigner},
};

/// JSON-RPC method names handled by this server.
pub const SUPPORTED_METHODS: [&str; 3] =
    ["ledgerflow_sign", "ledgerflow_keys", "ledgerflow_sign_payment"];

/// Dispatches a wallet JSON-RPC method to a [`WalletSigner`], returning the
/// raw `result` value (or a `WalletError` on failure). This is a pure
/// function — no network or runtime is involved.
///
/// Only [`SUPPORTED_METHODS`] are recognised; anything else yields
/// [`WalletError::InvalidPayload`].
pub async fn handle_jsonrpc(
    wallet: &dyn WalletSigner,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, WalletError> {
    match method {
        "ledgerflow_sign" => handle_sign(wallet, params).await,
        "ledgerflow_keys" => handle_keys(wallet).await,
        "ledgerflow_sign_payment" => handle_sign_payment(wallet, params).await,
        other => Err(WalletError::InvalidPayload(format!("unknown wallet RPC method: {other}"))),
    }
}

async fn handle_sign(
    wallet: &dyn WalletSigner,
    params: &serde_json::Value,
) -> Result<serde_json::Value, WalletError> {
    let obj = params.as_object().ok_or_else(|| {
        WalletError::InvalidPayload("ledgerflow_sign params must be an object".into())
    })?;

    let domain = match obj.get("domain").and_then(serde_json::Value::as_str) {
        Some("warrant") => SignDomain::Warrant,
        Some("proof") => SignDomain::Proof,
        Some("approval") => SignDomain::Approval,
        Some("payment") => SignDomain::Payment,
        _ => {
            return Err(WalletError::InvalidPayload(
                "ledgerflow_sign: invalid or missing `domain`".into(),
            ));
        }
    };

    let message_b64 = obj
        .get("message")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| WalletError::InvalidPayload("ledgerflow_sign: missing `message`".into()))?;
    let message = base64_decode(message_b64)?;

    let key = match obj.get("key") {
        Some(serde_json::Value::Null) | None => None,
        Some(value) => Some(parse_signer_ref(value, "ledgerflow_sign: key")?),
    };

    let result = wallet.sign(&SignRequest { domain, message, key }).await?;
    Ok(serde_json::json!({
        "signer": {
            "alg": result.signer.alg.to_string(),
            "public_key": base64_encode(&result.signer.public_key),
            "key_id": result.signer.key_id,
        },
        "signature": {
            "value": base64_encode(&result.signature.value),
        },
    }))
}

async fn handle_keys(wallet: &dyn WalletSigner) -> Result<serde_json::Value, WalletError> {
    let keys = wallet.keys().await?;
    let array: Vec<serde_json::Value> = keys
        .iter()
        .map(|key| {
            serde_json::json!({
                "alg": key.alg.to_string(),
                "public_key": base64_encode(&key.public_key),
                "key_id": key.key_id,
            })
        })
        .collect();
    Ok(serde_json::Value::Array(array))
}

async fn handle_sign_payment(
    wallet: &dyn WalletSigner,
    params: &serde_json::Value,
) -> Result<serde_json::Value, WalletError> {
    let obj = params.as_object().ok_or_else(|| {
        WalletError::InvalidPayload("ledgerflow_sign_payment params must be an object".into())
    })?;

    let chain_id = required_str(obj, "ledgerflow_sign_payment", "chain_id")?;
    let asset = required_str(obj, "ledgerflow_sign_payment", "asset")?;
    let amount_str = required_str(obj, "ledgerflow_sign_payment", "amount")?;
    let amount: u128 = amount_str.parse().map_err(|_| {
        WalletError::InvalidPayload(format!(
            "ledgerflow_sign_payment: invalid `amount` {amount_str}"
        ))
    })?;
    let payee = required_str(obj, "ledgerflow_sign_payment", "payee")?;
    let nonce = obj.get("nonce").and_then(serde_json::Value::as_str).map(str::to_string);

    let result =
        wallet.sign_payment(&SignPaymentRequest { chain_id, asset, amount, payee, nonce }).await?;
    Ok(serde_json::json!({
        "raw_transaction": result.raw_transaction,
        "tx_hash": result.tx_hash,
    }))
}

/// Reads a required string field from a JSON-RPC params object.
fn required_str(
    obj: &serde_json::Map<String, serde_json::Value>,
    method: &str,
    field: &str,
) -> Result<String, WalletError> {
    obj.get(field).and_then(serde_json::Value::as_str).map(str::to_string).ok_or_else(|| {
        WalletError::InvalidPayload(format!("{method}: missing or invalid `{field}`"))
    })
}

/// Parses a `SignerRef` from its wire representation
/// `{alg (Debug name), public_key (base64), key_id}`.
fn parse_signer_ref(value: &serde_json::Value, context: &str) -> Result<SignerRef, WalletError> {
    let obj = value
        .as_object()
        .ok_or_else(|| WalletError::InvalidPayload(format!("{context} must be an object")))?;
    let alg = match obj.get("alg").and_then(serde_json::Value::as_str) {
        Some("Ed25519" | "ed25519") => SigningAlgorithm::Ed25519,
        Some("Secp256k1" | "secp256k1") => SigningAlgorithm::Secp256k1,
        _ => {
            return Err(WalletError::InvalidPayload(format!("{context}: unsupported `alg`")));
        }
    };
    let public_key_b64 = required_str(obj, context, "public_key")?;
    let public_key = base64_decode(&public_key_b64)?;
    let key_id = obj.get("key_id").and_then(serde_json::Value::as_str).map(str::to_string);
    Ok(SignerRef { alg, public_key, key_id })
}

/// Converts a [`WalletError`] into a JSON-RPC error code.
fn to_jsonrpc_error(error: &WalletError) -> (i64, String) {
    match error {
        WalletError::InvalidPayload(_) => (-32_602, error.to_string()),
        WalletError::UnsupportedDomain(_) |
        WalletError::NoMatchingKey |
        WalletError::Rejected(_) |
        WalletError::Unreachable(_) |
        WalletError::Transport(_) => (-32_000, error.to_string()),
    }
}

/// An in-memory JSON-RPC wallet server wrapping a [`WalletSigner`].
///
/// Not `Clone`; share via [`Arc`] if multiple consumers need it.
pub struct EmbeddedWalletServer {
    inner: Arc<dyn WalletSigner>,
}

impl std::fmt::Debug for EmbeddedWalletServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedWalletServer").finish_non_exhaustive()
    }
}

impl EmbeddedWalletServer {
    /// Wraps a wallet signer behind the JSON-RPC wire protocol.
    #[must_use]
    pub fn new(inner: Arc<dyn WalletSigner>) -> Self {
        Self { inner }
    }

    /// Handles a single method call, returning the `result` value or a
    /// JSON-RPC error.
    pub async fn handle(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, JsonRpcError> {
        handle_jsonrpc(self.inner.as_ref(), method, &params).await.map_err(|error| {
            let (code, message) = to_jsonrpc_error(&error);
            JsonRpcError { code, message }
        })
    }

    /// Processes a full JSON-RPC request into a JSON-RPC response, addressing
    /// unknown methods and parse errors in the JSON-RPC 2.0 way.
    pub async fn process_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        // Unknown methods get the standard method-not-found code rather than
        // leaking the raw payload error through `handle`.
        let (result, error) = if SUPPORTED_METHODS.contains(&request.method.as_str()) {
            match self.handle(&request.method, request.params.clone()).await {
                Ok(result) => (Some(result), None),
                Err(error) => (None, Some(error)),
            }
        } else {
            (None, Some(JsonRpcError { code: -32_601, message: "method not found".to_string() }))
        };
        JsonRpcResponse { jsonrpc: "2.0".to_string(), id: request.id, result, error }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::embedded::EmbeddedSigner;

    fn signer() -> EmbeddedSigner {
        EmbeddedSigner::from_bytes(&[0x42; 32])
    }

    fn server() -> EmbeddedWalletServer {
        EmbeddedWalletServer::new(Arc::new(signer()))
    }

    fn key() -> SignerRef {
        signer().keypair().signer_ref()
    }

    #[tokio::test]
    async fn handle_sign_returns_roundtrippable_signer_and_signature() {
        let server = server();
        let params = serde_json::json!({
            "domain": "proof",
            "message": crate::local_rpc::base64_encode(b"hello"),
            "key": null,
        });
        let result = server.handle("ledgerflow_sign", params).await.expect("sign");
        assert_eq!(result["signer"]["alg"], "ed25519");
        let public_key = crate::local_rpc::base64_decode(
            result["signer"]["public_key"].as_str().expect("public_key"),
        )
        .expect("decode");
        assert_eq!(public_key, key().public_key);
        let signature =
            crate::local_rpc::base64_decode(result["signature"]["value"].as_str().expect("value"))
                .expect("decode");
        assert_eq!(signature.len(), 64);
    }

    #[tokio::test]
    async fn handle_sign_accepts_explicit_key_and_lowercase_alg() {
        let server = server();
        let params = serde_json::json!({
            "domain": "warrant",
            "message": crate::local_rpc::base64_encode(b"msg"),
            "key": {
                "alg": "Ed25519",
                "public_key": crate::local_rpc::base64_encode(&key().public_key),
                "key_id": key().key_id,
            },
        });
        let result = server.handle("ledgerflow_sign", params).await.expect("sign");
        assert_eq!(
            result["signer"]["public_key"],
            serde_json::Value::String(crate::local_rpc::base64_encode(&key().public_key))
        );
    }

    #[tokio::test]
    async fn handle_sign_rejects_mismatched_key() {
        let server = server();
        let params = serde_json::json!({
            "domain": "approval",
            "message": crate::local_rpc::base64_encode(b"m"),
            "key": { "alg": "Ed25519", "public_key": crate::local_rpc::base64_encode(&[9_u8; 32]), "key_id": null },
        });
        let error = server.handle("ledgerflow_sign", params).await.expect_err("mismatch");
        assert_eq!(error.code, -32_000);
    }

    #[tokio::test]
    async fn handle_keys_returns_lowercase_alg_and_base64_public_key() {
        let server = server();
        let result = server.handle("ledgerflow_keys", serde_json::Value::Null).await.expect("keys");
        let array = result.as_array().expect("array");
        assert_eq!(array.len(), 1);
        assert_eq!(array[0]["alg"], "ed25519");
        let public_key =
            crate::local_rpc::base64_decode(array[0]["public_key"].as_str().expect("public_key"))
                .expect("decode");
        assert_eq!(public_key, key().public_key);
    }

    #[tokio::test]
    async fn handle_sign_payment_returns_raw_transaction() {
        let server = server();
        let params = serde_json::json!({
            "chain_id": "eip155:8453",
            "asset": "eip155:8453/slip44:60",
            "amount": "100",
            "payee": "0xpayee",
            "nonce": "1",
        });
        let result = server.handle("ledgerflow_sign_payment", params).await.expect("sign_payment");
        assert!(
            result["raw_transaction"]
                .as_str()
                .expect("raw_transaction")
                .starts_with("signed:eip155:8453")
        );
        assert!(result["tx_hash"].is_null());
    }

    #[tokio::test]
    async fn unknown_method_returns_method_not_found() {
        let server = server();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 7,
            method: "nope".to_string(),
            params: serde_json::Value::Null,
        };
        let response = server.process_request(&request).await;
        let error = response.error.expect("error");
        assert_eq!(error.code, -32_601);
        assert_eq!(response.id, 7);
        assert!(response.result.is_none());
    }

    #[tokio::test]
    async fn process_request_wraps_result() {
        let server = server();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 3,
            method: "ledgerflow_keys".to_string(),
            params: serde_json::Value::Null,
        };
        let response = server.process_request(&request).await;
        assert!(response.error.is_none());
        assert_eq!(response.id, 3);
        assert!(response.result.is_some());
    }
}

// -------------------------------------------------------------------------
// Loopback HTTP/1.1 listener (feature `http`)
// -------------------------------------------------------------------------

/// A minimal HTTP/1.1 loopback JSON-RPC server backed by an
/// [`EmbeddedWalletServer`].
///
/// Bound to `127.0.0.1` on an ephemeral port. Each accepted connection is
/// handled in its own thread and closed after a single request
/// (`Connection: close`). Designed for tests and local development, not
/// production serving.
#[cfg(feature = "http")]
pub struct LoopbackJsonRpcServer {
    /// Bound loopback address (e.g. `127.0.0.1:PORT`).
    pub addr: std::net::SocketAddr,
    handle: Option<tokio::task::JoinHandle<()>>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(feature = "http")]
impl std::fmt::Debug for LoopbackJsonRpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopbackJsonRpcServer").field("addr", &self.addr).finish_non_exhaustive()
    }
}

#[cfg(feature = "http")]
impl LoopbackJsonRpcServer {
    /// Starts a loopback JSON-RPC server on an ephemeral 127.0.0.1 port.
    ///
    /// # Errors
    ///
    /// Returns a [`WalletError`] if the listener cannot be bound.
    pub async fn start(wallet: Arc<dyn WalletSigner>) -> Result<Self, WalletError> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|error| {
            WalletError::Transport(format!("failed to bind loopback JSON-RPC listener: {error}"))
        })?;
        let addr = listener.local_addr().map_err(|error| {
            WalletError::Transport(format!("failed to read listener address: {error}"))
        })?;

        let server = Arc::new(EmbeddedWalletServer::new(wallet));
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_flag = Arc::clone(&shutdown);
        let server_for_tasks = Arc::clone(&server);

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, _)) => {
                                let server = Arc::clone(&server_for_tasks);
                                tokio::spawn(async move {
                                    handle_connection(stream, server).await;
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    () = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                        if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self { addr, handle: Some(handle), shutdown })
    }

    /// The base URL to POST JSON-RPC requests to.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    /// Stops the loopback listener (aborting the background task).
    pub fn stop(&mut self) {
        self.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

#[cfg(feature = "http")]
impl Drop for LoopbackJsonRpcServer {
    fn drop(&mut self) {
        self.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

/// Handles a single HTTP/1.1 request over a connection and responds with a
/// JSON-RPC response body.
#[cfg(feature = "http")]
async fn handle_connection(mut stream: tokio::net::TcpStream, server: Arc<EmbeddedWalletServer>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = Vec::new();
    let mut temp_buf = [0u8; 4096];
    let content_length;

    // Read headers until blank line, capturing Content-Length.
    loop {
        match stream.read(&mut temp_buf).await {
            Ok(0) => return,
            Ok(n) => {
                buf.extend_from_slice(&temp_buf[..n]);
                if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                    content_length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break;
                }
                if buf.len() > 64 * 1024 {
                    return;
                }
            }
            Err(_) => return,
        }
    }

    // Read the request body.
    let body_start = find_subslice(&buf, b"\r\n\r\n").map_or(0, |p| p + 4);
    while buf.len() < body_start + content_length {
        match stream.read(&mut temp_buf).await {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&temp_buf[..n]),
            Err(_) => return,
        }
    }

    let body = &buf[body_start..(body_start + content_length).min(buf.len())];

    let response = match serde_json::from_slice::<JsonRpcRequest>(body) {
        Ok(request) => server.process_request(&request).await,
        Err(error) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 0,
            result: None,
            error: Some(JsonRpcError {
                code: -32_700,
                message: format!("invalid JSON-RPC request: {error}"),
            }),
        },
    };

    let body_json = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body_json.len()
    );
    let _ = stream.write_all(headers.as_bytes()).await;
    let _ = stream.write_all(body_json.as_bytes()).await;
    let _ = stream.flush().await;
}

#[cfg(feature = "http")]
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
