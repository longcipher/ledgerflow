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
        handler: impl Fn(&str, serde_json::Value) -> Result<serde_json::Value, WalletError> + Send + Sync + 'static,
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

    fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, WalletError> {
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
    let signer = value.get("signer").ok_or_else(|| WalletError::InvalidPayload("missing signer".to_string()))?;
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

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn base64_decode(value: &str) -> Result<Vec<u8>, WalletError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|error| WalletError::InvalidPayload(format!("invalid base64: {error}")))
}

/// HTTP JSON-RPC transport (feature-gated; uses hpx).
#[cfg(feature = "http")]
pub struct HttpJsonRpcTransport {
    config: LocalRpcConfig,
}

#[cfg(feature = "http")]
impl HttpJsonRpcTransport {
    /// Creates an HTTP transport for a local wallet daemon.
    #[must_use]
    pub fn new(config: LocalRpcConfig) -> Self {
        Self { config }
    }
}

#[cfg(feature = "http")]
impl RpcTransport for HttpJsonRpcTransport {
    fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, WalletError> {
        // hpx is a low-level HTTP client; a full integration would build the
        // request and parse the response here. v1 keeps the transport seam
        // and defers the concrete HTTP implementation (hpx wiring) to the
        // server crate (P3), where the runtime is present.
        let _ = (method, params, &self.config);
        Err(WalletError::Unreachable(
            "HTTP JSON-RPC transport is not yet wired; use MockJsonRpcTransport or implement RpcTransport".to_string(),
        ))
    }
}
