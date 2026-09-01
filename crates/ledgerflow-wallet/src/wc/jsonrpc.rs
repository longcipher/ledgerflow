//! JSON-RPC 2.0 codec for WalletConnect v2.
//!
//! Per the WC v2 spec, all messages use JSON-RPC 2.0 with a non-null `id`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JsonRpcRequest {
    pub(crate) jsonrpc: String,
    pub(crate) method: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub(crate) params: Value,
    pub(crate) id: i64,
}

impl JsonRpcRequest {
    /// Constructs a new request.
    pub(crate) fn new(method: impl Into<String>, params: Value, id: i64) -> Self {
        Self { jsonrpc: "2.0".to_string(), method: method.into(), params, id }
    }
}

/// JSON-RPC 2.0 response object (success or error, never both).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JsonRpcResponse {
    pub(crate) jsonrpc: String,
    pub(crate) id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<Value>,
}

/// Standard WC v2 JSON-RPC method names.
///
/// The session-lifecycle constants are part of the protocol surface and are
/// exercised as the client gains session-management support.
#[allow(dead_code)]
pub(crate) mod method {
    /// Session proposal (dApp → wallet, on the pairing topic).
    pub(crate) const SESSION_PROPOSE: &str = "wc_sessionPropose";
    /// Session settle (wallet → dApp, on the session topic).
    pub(crate) const SESSION_SETTLE: &str = "wc_sessionSettle";
    /// Session request (dApp → wallet, on the session topic).
    pub(crate) const SESSION_REQUEST: &str = "wc_sessionRequest";
    /// Session delete.
    pub(crate) const SESSION_DELETE: &str = "wc_sessionDelete";
    /// Session ping.
    pub(crate) const SESSION_PING: &str = "wc_sessionPing";
    /// Session update.
    pub(crate) const SESSION_UPDATE: &str = "wc_sessionUpdate";
    /// Session event (wallet → dApp).
    pub(crate) const SESSION_EVENT: &str = "wc_sessionEvent";

    /// EVM personal sign (EIP-191).
    pub(crate) const PERSONAL_SIGN: &str = "personal_sign";
    /// EVM typed-data sign (EIP-712).
    pub(crate) const ETH_SIGN_TYPED_DATA_V4: &str = "eth_signTypedData_v4";
    /// EVM sign transaction (returns the signed raw tx without broadcasting).
    pub(crate) const ETH_SIGN_TRANSACTION: &str = "eth_signTransaction";
    /// EVM request accounts.
    pub(crate) const ETH_REQUEST_ACCOUNTS: &str = "eth_requestAccounts";
}

/// Relay JSON-RPC method names (IRN protocol).
#[allow(dead_code)]
pub(crate) mod relay_method {
    /// Subscribe to a topic.
    pub(crate) const IRN_SUBSCRIBE: &str = "irn_subscribe";
    /// Publish a message to a topic.
    pub(crate) const IRN_PUBLISH: &str = "irn_publish";
    /// Inbound subscription delivery envelope.
    pub(crate) const IRN_SUBSCRIPTION: &str = "irn_subscription";
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn request_serializes_with_jsonrpc_version() {
        let req = JsonRpcRequest::new("personal_sign", serde_json::json!({"x": 1}), 7);
        let v: Value = serde_json::to_value(&req).expect("serialize");
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["method"], "personal_sign");
        assert_eq!(v["id"], 7);
    }

    #[test]
    fn response_round_trips() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!(["0xabc"])),
            error: None,
        };
        let bytes = serde_json::to_vec(&resp).expect("serialize");
        let parsed: JsonRpcResponse = serde_json::from_slice(&bytes).expect("parse");
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.result, Some(serde_json::json!(["0xabc"])));
    }
}
