//! Local JSON-RPC 2.0 wallet adapter.
//!
//! Talks to a local wallet daemon over loopback HTTP JSON-RPC 2.0. The method
//! names are LedgerFlow-standard (`ledgerflow_sign`, `ledgerflow_keys`,
//! `ledgerflow_sign_payment`) so any wallet daemon can implement them without
//! depending on LedgerFlow.

use ledgerflow_core::{SignatureEnvelope, SignerRef, SigningAlgorithm};

use crate::{
    error::WalletError,
    signer::{
        SignDomain, SignPaymentRequest, SignRequest, SignResult, SignedPayment, WalletDescriptor,
        WalletSigner,
    },
};

/// JSON-RPC 2.0 request.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

/// JSON-RPC 2.0 error object.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

/// JSON-RPC 2.0 response.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

/// Transport seam for JSON-RPC calls (enables mock testing without HTTP).
pub trait RpcTransport: Send + Sync {
    fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, WalletError>;
}

/// A mock transport for tests (in-memory handler).
pub struct MockJsonRpcTransport {
    handler: RpcHandler,
}

/// Handler closure signature for [`MockJsonRpcTransport`].
type RpcHandler =
    Box<dyn Fn(&str, serde_json::Value) -> Result<serde_json::Value, WalletError> + Send + Sync>;

impl std::fmt::Debug for MockJsonRpcTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockJsonRpcTransport").finish_non_exhaustive()
    }
}

impl MockJsonRpcTransport {
    /// Creates a mock transport with a handler closure.
    #[must_use]
    pub fn new(
        handler: impl Fn(&str, serde_json::Value) -> Result<serde_json::Value, WalletError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self { handler: Box::new(handler) }
    }
}

impl RpcTransport for MockJsonRpcTransport {
    fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, WalletError> {
        (self.handler)(method, params)
    }
}

/// Configuration for the local RPC signer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalRpcConfig {
    pub url: String,
    pub timeout_ms: u64,
}

impl Default for LocalRpcConfig {
    fn default() -> Self {
        Self { url: "http://127.0.0.1:18080".to_string(), timeout_ms: 5_000 }
    }
}

/// JSON-RPC signer over a [`RpcTransport`].
pub struct LocalRpcSigner<T> {
    transport: T,
    descriptor: WalletDescriptor,
}

impl<T> std::fmt::Debug for LocalRpcSigner<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalRpcSigner")
            .field("descriptor", &self.descriptor)
            .finish_non_exhaustive()
    }
}

impl<T> LocalRpcSigner<T>
where
    T: RpcTransport,
{
    /// Creates a local RPC signer over the given transport.
    #[must_use]
    pub fn new(transport: T) -> Self {
        let descriptor = WalletDescriptor {
            name: "local-rpc".to_string(),
            algorithms: vec![SigningAlgorithm::Ed25519, SigningAlgorithm::Secp256k1],
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        Self { transport, descriptor }
    }

    fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, WalletError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: method.to_string(),
            params,
        };
        let response = self.transport.call(method, request.params)?;
        Ok(response)
    }
}

impl<T> WalletSigner for LocalRpcSigner<T>
where
    T: RpcTransport,
{
    fn descriptor(&self) -> WalletDescriptor {
        self.descriptor.clone()
    }

    fn sign(&self, request: &SignRequest) -> Result<SignResult, WalletError> {
        let params = serde_json::json!({
            "domain": match request.domain {
                SignDomain::Warrant => "warrant",
                SignDomain::Proof => "proof",
                SignDomain::Approval => "approval",
                SignDomain::Payment => "payment",
            },
            "message": base64_encode(&request.message),
            "key": request.key.as_ref().map(|key| serde_json::json!({
                "alg": format!("{:?}", key.alg),
                "public_key": base64_encode(&key.public_key),
                "key_id": key.key_id,
            })),
        });
        let value = self.call("ledgerflow_sign", params)?;
        parse_sign_result(&value)
    }

    fn keys(&self) -> Result<Vec<SignerRef>, WalletError> {
        let value = self.call("ledgerflow_keys", serde_json::Value::Null)?;
        let keys = value
            .as_array()
            .ok_or_else(|| WalletError::InvalidPayload("expected array".to_string()))?;
        keys.iter()
            .map(|key| {
                let alg = key.get("alg").and_then(|v| v.as_str()).unwrap_or("ed25519");
                let public_key = key
                    .get("public_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| WalletError::InvalidPayload("missing public_key".to_string()))?;
                let key_id = key.get("key_id").and_then(|v| v.as_str()).map(str::to_string);
                Ok(SignerRef {
                    alg: match alg {
                        "secp256k1" => SigningAlgorithm::Secp256k1,
                        _ => SigningAlgorithm::Ed25519,
                    },
                    public_key: base64_decode(public_key)?,
                    key_id,
                })
            })
            .collect()
    }

    fn sign_payment(&self, request: &SignPaymentRequest) -> Result<SignedPayment, WalletError> {
        let params = serde_json::json!({
            "chain_id": request.chain_id,
            "asset": request.asset,
            "amount": request.amount.to_string(),
            "payee": request.payee,
            "nonce": request.nonce,
        });
        let value = self.call("ledgerflow_sign_payment", params)?;
        Ok(SignedPayment {
            signer: SignerRef::new(SigningAlgorithm::Ed25519, Vec::new()),
            raw_transaction: value
                .get("raw_transaction")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            tx_hash: value.get("tx_hash").and_then(|v| v.as_str()).map(str::to_string),
        })
    }
}

fn parse_sign_result(value: &serde_json::Value) -> Result<SignResult, WalletError> {
    let signer = value
        .get("signer")
        .ok_or_else(|| WalletError::InvalidPayload("missing signer".to_string()))?;
    let alg = signer.get("alg").and_then(|v| v.as_str()).unwrap_or("ed25519");
    let public_key = signer
        .get("public_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::InvalidPayload("missing public_key".to_string()))?;
    let signature = value
        .get("signature")
        .ok_or_else(|| WalletError::InvalidPayload("missing signature".to_string()))?;
    let sig_value = signature
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WalletError::InvalidPayload("missing signature.value".to_string()))?;
    Ok(SignResult {
        signer: SignerRef {
            alg: match alg {
                "secp256k1" => SigningAlgorithm::Secp256k1,
                _ => SigningAlgorithm::Ed25519,
            },
            public_key: base64_decode(public_key)?,
            key_id: signer.get("key_id").and_then(|v| v.as_str()).map(str::to_string),
        },
        signature: SignatureEnvelope {
            alg: SigningAlgorithm::Ed25519,
            value: base64_decode(sig_value)?,
        },
    })
}

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub(crate) fn base64_decode(value: &str) -> Result<Vec<u8>, WalletError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|error| WalletError::InvalidPayload(format!("invalid base64: {error}")))
}

/// HTTP JSON-RPC transport (feature-gated; uses hpx).
///
/// Bridges the synchronous [`RpcTransport::call`] seam to hpx's async HTTP
/// client via a single process-wide current-thread tokio runtime (see
/// [`blocking_runtime`]). The runtime is created lazily and reused for every
/// one-shot call, avoiding the cost of spinning up a fresh runtime per
/// request.
#[cfg(feature = "http")]
pub struct HttpJsonRpcTransport {
    config: LocalRpcConfig,
}

#[cfg(feature = "http")]
impl std::fmt::Debug for HttpJsonRpcTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpJsonRpcTransport")
            .field("url", &self.config.url)
            .field("timeout_ms", &self.config.timeout_ms)
            .finish()
    }
}

#[cfg(feature = "http")]
impl HttpJsonRpcTransport {
    /// Creates an HTTP transport for a local wallet daemon.
    #[must_use]
    pub const fn new(config: LocalRpcConfig) -> Self {
        Self { config }
    }
}

/// Process-wide current-thread tokio runtime bridging the synchronous
/// [`RpcTransport`] seam to hpx's async HTTP client.
///
/// Built lazily via [`OnceLock`] and reused for the lifetime of the process.
/// Using `new_current_thread` keeps the overhead minimal: hpx's connection
/// pool and the JSON-RPC request finish within the same task, so no
/// multi-threaded scheduler is required.
#[cfg(feature = "http")]
static WALLET_HTTP_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> =
    std::sync::OnceLock::new();

/// Returns the process-wide blocking runtime, creating it on first use.
///
/// A fast-path `get()` avoids re-acquiring the initialization lock on the
/// common path; on a rare concurrent init race one extra runtime may be
/// built and discarded, which is harmless.
#[cfg(feature = "http")]
fn blocking_runtime() -> Result<&'static tokio::runtime::Runtime, WalletError> {
    if let Some(runtime) = WALLET_HTTP_RUNTIME.get() {
        return Ok(runtime);
    }
    let runtime =
        tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|error| {
            WalletError::Transport(format!("failed to build HTTP blocking runtime: {error}"))
        })?;
    let _ = WALLET_HTTP_RUNTIME.set(runtime);
    WALLET_HTTP_RUNTIME
        .get()
        .ok_or_else(|| WalletError::Transport("HTTP blocking runtime unavailable".to_string()))
}

#[cfg(feature = "http")]
impl RpcTransport for HttpJsonRpcTransport {
    fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, WalletError> {
        let runtime = blocking_runtime()?;
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let url = self.config.url.clone();
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);

        runtime.block_on(async move {
            let client = hpx::Client::new();
            let fut = async {
                let resp = client
                    .post(&url)
                    .header("content-type", "application/json")
                    .body(body.to_string())
                    .send()
                    .await
                    .map_err(|error| {
                        WalletError::Unreachable(format!(
                            "wallet JSON-RPC request to {url} failed: {error}"
                        ))
                    })?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let text =
                        resp.text().await.unwrap_or_else(|_| "<unreadable body>".to_string());
                    return Err(WalletError::Transport(format!(
                        "wallet JSON-RPC server returned HTTP {status}: {text}"
                    )));
                }

                let value: serde_json::Value = resp.json().await.map_err(|error| {
                    WalletError::InvalidPayload(format!("invalid JSON-RPC response body: {error}"))
                })?;
                let response: JsonRpcResponse = serde_json::from_value(value).map_err(|error| {
                    WalletError::InvalidPayload(format!(
                        "response is not a valid JSON-RPC 2.0 object: {error}"
                    ))
                })?;

                if let Some(error) = response.error {
                    return Err(WalletError::Rejected(format!(
                        "wallet JSON-RPC error {}: {}",
                        error.code, error.message
                    )));
                }

                response.result.ok_or_else(|| {
                    WalletError::InvalidPayload(
                        "JSON-RPC response has neither result nor error".to_string(),
                    )
                })
            };
            tokio::time::timeout(timeout, fut).await.map_err(|_| {
                WalletError::Unreachable(format!(
                    "wallet JSON-RPC request to {url} timed out after {} ms",
                    timeout.as_millis()
                ))
            })?
        })
    }
}

#[cfg(feature = "http")]
impl LocalRpcSigner<HttpJsonRpcTransport> {
    /// Creates a local RPC signer that talks HTTP to the given wallet daemon
    /// URL, using the JSON-RPC wiring this crate defines.
    #[must_use]
    pub fn new_http(config: LocalRpcConfig) -> Self {
        Self::new(HttpJsonRpcTransport::new(config))
    }
}
