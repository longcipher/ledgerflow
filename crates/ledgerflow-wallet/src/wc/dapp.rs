//! WC v2 dApp role client — spec-compliant.
//!
//! Connects to a relay, binds to a session topic, sends JSON-RPC requests,
//! and awaits responses. Follows the official WalletConnect 2.0 flow:
//!
//! 1. The pairing `symKey` from the URI encrypts pairing-phase messages with a **type-0 envelope**.
//! 2. `wc_sessionPropose` is sent as a **type-1 envelope** carrying the dApp's X25519 public key.
//! 3. On approval, the dApp derives the session key via `deriveSymKey(dapp_priv, responder_pub)`
//!    (X25519 + HKDF) and binds it.

#[cfg(test)]
use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde_json::Value;
use tokio::sync::Mutex;

#[cfg(test)]
use super::mock_relay::MockRelay;
use super::{
    crypto::{self, WcCipher, WcKeyPair, WcSymKey},
    jsonrpc::{JsonRpcRequest, JsonRpcResponse, method, relay_method},
    relay::{RelayClient, RelayConfig},
    uri::PairingUri,
};
use crate::error::WalletError;

/// Receive timeout for a single relay read during request/response loops.
const RELAY_RECV_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// WC v2 dApp client state.
pub(crate) struct WcDappClient {
    /// Current session topic.
    topic: Mutex<Option<String>>,
    /// Next JSON-RPC request id.
    next_id: Mutex<i64>,
    /// Current symmetric key (pairing key before settle, session key after).
    sym_key: Mutex<Option<WcSymKey>>,
    /// Live relay connection.
    relay: Mutex<Option<RelayClient>>,
    /// Relay config retained for reconnects.
    relay_cfg: Mutex<Option<RelayConfig>>,
    /// Topics currently subscribed on the relay (for resubscribe after reconnect).
    topics: Mutex<Vec<String>>,
    /// The dApp's X25519 keypair used for session-key derivation.
    keypair: Mutex<Option<WcKeyPair>>,
    /// In-memory relay for tests (bypasses the real WebSocket relay).
    #[cfg(test)]
    mock_relay: Option<Arc<MockRelay>>,
}

impl WcDappClient {
    /// Creates a new dApp client.
    pub(crate) fn new() -> Self {
        Self {
            topic: Mutex::new(None),
            next_id: Mutex::new(1),
            sym_key: Mutex::new(None),
            relay: Mutex::new(None),
            relay_cfg: Mutex::new(None),
            topics: Mutex::new(Vec::new()),
            keypair: Mutex::new(Some(WcKeyPair::generate())),
            #[cfg(test)]
            mock_relay: None,
        }
    }

    /// Attaches an in-memory relay for tests.
    #[cfg(test)]
    pub(crate) fn attach_mock_relay(&mut self, relay: Arc<MockRelay>) {
        self.mock_relay = Some(relay);
    }

    /// Connects to the relay and subscribes to the pairing topic from `uri`.
    ///
    /// The relay URL is taken from `relay_url` (or the default
    /// `wss://relay.walletconnect.com`). The pairing `symKey` becomes the
    /// client's initial symmetric key.
    pub(crate) async fn connect(
        &self,
        uri: &PairingUri,
        relay_url: Option<&str>,
    ) -> Result<(), WalletError> {
        let project_id = uri.project_id.clone();
        let base_url =
            relay_url.map_or_else(|| "wss://relay.walletconnect.com".to_string(), str::to_string);
        let relay_url = super::relay::apply_project_id(&base_url, project_id.as_deref());
        let relay_cfg = RelayConfig { url: relay_url, reconnect_max_ms: 60_000 };
        let mut relay = RelayClient::connect(&relay_cfg).await?;

        let sub_msg = serde_json::json!({
            "id": relay_id(1),
            "jsonrpc": "2.0",
            "method": relay_method::IRN_SUBSCRIBE,
            "params": { "topic": uri.topic },
        });
        relay
            .send_text(serde_json::to_string(&sub_msg).map_err(|e| {
                WalletError::Transport(format!("subscribe serialization failed: {e}"))
            })?)
            .await?;

        let sym_key = uri.sym_key.as_ref().and_then(|hex_str| {
            let bytes = hex::decode(hex_str).ok()?;
            (bytes.len() == 32).then(|| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                WcSymKey::from_bytes(arr)
            })
        });

        *self.topic.lock().await = Some(uri.topic.clone());
        *self.sym_key.lock().await = sym_key;
        *self.relay.lock().await = Some(relay);
        *self.relay_cfg.lock().await = Some(relay_cfg);
        self.topics.lock().await.push(uri.topic.clone());
        Ok(())
    }

    /// Reconnects the relay and re-subscribes to all known topics.
    ///
    /// Used to recover from a dropped relay connection between calls.
    pub(crate) async fn reconnect_and_resubscribe(&self) -> Result<(), WalletError> {
        let cfg = self
            .relay_cfg
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Unreachable("relay config missing".to_string()))?;
        let topics = self.topics.lock().await.clone();
        let mut guard = self.relay.lock().await;
        let relay = guard
            .as_mut()
            .ok_or_else(|| WalletError::Unreachable("relay not connected".to_string()))?;
        relay.reconnect(&cfg).await?;
        for topic in &topics {
            let sub_msg = serde_json::json!({
                "id": relay_id(self.next_id().await),
                "jsonrpc": "2.0",
                "method": relay_method::IRN_SUBSCRIBE,
                "params": { "topic": topic },
            });
            relay
                .send_text(serde_json::to_string(&sub_msg).map_err(|e| {
                    WalletError::Transport(format!("resubscribe serialization failed: {e}"))
                })?)
                .await?;
        }
        Ok(())
    }

    /// Sends `wc_sessionPropose` and awaits the approval response.
    ///
    /// The propose request is sent as a **type-1 envelope** carrying the dApp's
    /// X25519 public key; on approval the session key is derived via
    /// `deriveSymKey(dapp_priv, responder_pub)`.
    pub(crate) async fn propose(
        &self,
        dapp_name: &str,
        dapp_url: &str,
    ) -> Result<String, WalletError> {
        #[cfg(test)]
        if self.mock_relay.is_some() {
            return self.propose_mock(dapp_name, dapp_url).await;
        }
        let topic = self
            .topic
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no bound pairing topic".to_string()))?;

        let pairing_key = self
            .sym_key
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no pairing sym key set".to_string()))?;

        let mut relay_guard = self.relay.lock().await;
        let relay = relay_guard
            .as_mut()
            .ok_or_else(|| WalletError::Unreachable("relay not connected".to_string()))?;

        let id = self.next_id().await;

        let kp = WcKeyPair::generate();
        let proposer_pubkey = kp.public_key_hex();

        let propose = serde_json::json!({
            "relays": [{ "protocol": "irn" }],
            "requiredNamespaces": {
                "eip155": {
                    "methods": ["eth_sendTransaction", "personal_sign", "eth_signTypedData_v4", "eth_signTransaction", "eth_requestAccounts"],
                    "chains": ["eip155:1"],
                    "events": ["accountsChanged", "chainChanged"]
                }
            },
            "proposer": {
                "publicKey": proposer_pubkey,
                "metadata": {
                    "name": dapp_name,
                    "description": format!("{dapp_name} via WalletConnect v2"),
                    "url": dapp_url,
                    "icons": []
                }
            }
        });

        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method::SESSION_PROPOSE,
            "params": propose,
            "id": id
        });
        let req_bytes = serde_json::to_vec(&req)
            .map_err(|e| WalletError::Transport(format!("propose serialization failed: {e}")))?;

        // Type-1 envelope: [type(1) ‖ senderPubKey(32) ‖ iv(12) ‖ ciphertext].
        let proposer_bytes = hex::decode(&proposer_pubkey)
            .map_err(|e| WalletError::Transport(format!("bad proposer pubkey hex: {e}")))?;
        let mut proposer_arr = [0u8; 32];
        proposer_arr.copy_from_slice(&proposer_bytes);
        let envelope = WcCipher::seal_type1(&pairing_key, &proposer_arr, &req_bytes)?;

        relay.publish_irn(&relay_id(id), &topic, &BASE64.encode(&envelope), 300, 1108).await?;

        // Store the proposer keypair for session-key derivation.
        *self.keypair.lock().await = Some(kp);

        loop {
            let raw = relay.recv_timeout(RELAY_RECV_TIMEOUT).await?;
            let envelope_val: Value = serde_json::from_str(&raw)
                .map_err(|e| WalletError::Transport(format!("relay envelope parse failed: {e}")))?;

            if envelope_val.get("method").and_then(|m| m.as_str()) !=
                Some(relay_method::IRN_SUBSCRIPTION)
            {
                continue;
            }
            let data = match envelope_val.pointer("/params/data") {
                Some(d) => d,
                None => continue,
            };
            let b64_msg = match data.get("message").and_then(|m| m.as_str()) {
                Some(m) => m,
                None => continue,
            };

            let encrypted = BASE64.decode(b64_msg).unwrap_or_default();
            if encrypted.first() != Some(&crypto::ENVELOPE_TYPE_0) &&
                encrypted.first() != Some(&crypto::ENVELOPE_TYPE_1)
            {
                continue;
            }
            // The approval response is encrypted with the pairing key.
            let plaintext = match encrypted[0] {
                crypto::ENVELOPE_TYPE_0 => WcCipher::open_type0(&pairing_key, &encrypted)?,
                crypto::ENVELOPE_TYPE_1 => WcCipher::open_type1(&pairing_key, &encrypted)?.1,
                _ => continue,
            };
            if plaintext.is_empty() {
                continue;
            }

            let resp: JsonRpcResponse = serde_json::from_slice(&plaintext).map_err(|e| {
                WalletError::Transport(format!("approve response parse failed: {e}"))
            })?;
            if resp.id != id {
                continue;
            }
            if let Some(e) = resp.error {
                return Err(WalletError::Rejected(format!(
                    "session proposal rejected: {} ({})",
                    e.message, e.code
                )));
            }
            // Derive the session key from the responder's public key.
            let responder_pub_hex = resp
                .result
                .as_ref()
                .and_then(|r| r.get("responderPublicKey"))
                .and_then(|k| k.as_str())
                .ok_or_else(|| {
                    WalletError::Transport("approve missing responderPublicKey".to_string())
                })?;
            let responder_bytes = hex::decode(responder_pub_hex)
                .map_err(|e| WalletError::Transport(format!("bad responder publicKey hex: {e}")))?;
            if responder_bytes.len() != 32 {
                return Err(WalletError::Transport(
                    "responder publicKey must be 32 bytes".to_string(),
                ));
            }
            let mut responder_pub = [0u8; 32];
            responder_pub.copy_from_slice(&responder_bytes);

            let kp =
                self.keypair.lock().await.clone().ok_or_else(|| {
                    WalletError::Transport("proposer keypair missing".to_string())
                })?;
            let shared = kp.shared_secret(&x25519_dalek::PublicKey::from(responder_pub));
            let session_key = crypto::derive_sym_key(&shared)?;
            *self.sym_key.lock().await = Some(session_key);

            // The session topic is SHA-256 of the proposer public key
            // (matches the wallet side).
            let session_topic = crypto::hash_bytes(&proposer_bytes);
            *self.topic.lock().await = Some(session_topic.clone());

            // Subscribe to the session topic.
            let sub_msg = serde_json::json!({
                "id": relay_id(self.next_id().await),
                "jsonrpc": "2.0",
                "method": relay_method::IRN_SUBSCRIBE,
                "params": { "topic": session_topic },
            });
            relay
                .send_text(serde_json::to_string(&sub_msg).map_err(|e| {
                    WalletError::Transport(format!("session subscribe failed: {e}"))
                })?)
                .await?;
            self.topics.lock().await.push(session_topic);

            return Ok(proposer_pubkey);
        }
    }

    /// Mock-relay propose path: publish `wc_sessionPropose` and await the
    /// approval response from the in-memory relay.
    #[cfg(test)]
    async fn propose_mock(&self, dapp_name: &str, dapp_url: &str) -> Result<String, WalletError> {
        let relay = self
            .mock_relay
            .clone()
            .ok_or_else(|| WalletError::Unreachable("no mock relay attached".to_string()))?;
        let topic = self
            .topic
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no bound pairing topic".to_string()))?;
        let pairing_key = self
            .sym_key
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no pairing sym key set".to_string()))?;

        let id = self.next_id().await;
        let kp = WcKeyPair::generate();
        let proposer_pubkey = kp.public_key_hex();

        let propose = serde_json::json!({
            "relays": [{ "protocol": "irn" }],
            "requiredNamespaces": {
                "eip155": {
                    "methods": ["personal_sign", "eth_requestAccounts"],
                    "chains": ["eip155:1"],
                    "events": ["accountsChanged", "chainChanged"]
                }
            },
            "proposer": {
                "publicKey": proposer_pubkey,
                "metadata": {
                    "name": dapp_name,
                    "description": format!("{dapp_name} via WalletConnect v2"),
                    "url": dapp_url,
                    "icons": []
                }
            }
        });
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method::SESSION_PROPOSE,
            "params": propose,
            "id": id
        });
        let req_bytes = serde_json::to_vec(&req)
            .map_err(|e| WalletError::Transport(format!("propose serialization failed: {e}")))?;
        let proposer_bytes = hex::decode(&proposer_pubkey)
            .map_err(|e| WalletError::Transport(format!("bad proposer pubkey hex: {e}")))?;
        let mut proposer_arr = [0u8; 32];
        proposer_arr.copy_from_slice(&proposer_bytes);
        let envelope = WcCipher::seal_type1(&pairing_key, &proposer_arr, &req_bytes)?;

        *self.keypair.lock().await = Some(kp);
        let mut sub = relay.subscribe(&topic).await;
        relay.publish(&topic, &envelope).await;

        loop {
            let payload = sub
                .recv()
                .await
                .map_err(|_| WalletError::Unreachable("mock relay stream ended".to_string()))?;
            let plaintext = match payload.first() {
                Some(&crypto::ENVELOPE_TYPE_0) => WcCipher::open_type0(&pairing_key, &payload)?,
                Some(&crypto::ENVELOPE_TYPE_1) => WcCipher::open_type1(&pairing_key, &payload)?.1,
                _ => continue,
            };
            let resp: JsonRpcResponse = serde_json::from_slice(&plaintext).map_err(|e| {
                WalletError::Transport(format!("approve response parse failed: {e}"))
            })?;
            if resp.id != id {
                continue;
            }
            // Skip non-response messages (e.g. the mock relay echoing the
            // dApp's own propose back to its subscription).
            if resp.result.is_none() && resp.error.is_none() {
                continue;
            }
            if let Some(e) = resp.error {
                return Err(WalletError::Rejected(format!(
                    "session proposal rejected: {} ({})",
                    e.message, e.code
                )));
            }
            let responder_pub_hex = resp
                .result
                .as_ref()
                .and_then(|r| r.get("responderPublicKey"))
                .and_then(|k| k.as_str())
                .ok_or_else(|| {
                    WalletError::Transport("approve missing responderPublicKey".to_string())
                })?;
            let responder_bytes = hex::decode(responder_pub_hex)
                .map_err(|e| WalletError::Transport(format!("bad responder publicKey hex: {e}")))?;
            if responder_bytes.len() != 32 {
                return Err(WalletError::Transport(
                    "responder publicKey must be 32 bytes".to_string(),
                ));
            }
            let mut responder_pub = [0u8; 32];
            responder_pub.copy_from_slice(&responder_bytes);

            let kp =
                self.keypair.lock().await.clone().ok_or_else(|| {
                    WalletError::Transport("proposer keypair missing".to_string())
                })?;
            let shared = kp.shared_secret(&x25519_dalek::PublicKey::from(responder_pub));
            let session_key = crypto::derive_sym_key(&shared)?;
            *self.sym_key.lock().await = Some(session_key);
            let session_topic = crypto::hash_bytes(&proposer_bytes);
            *self.topic.lock().await = Some(session_topic.clone());
            self.topics.lock().await.push(session_topic);

            return Ok(proposer_pubkey);
        }
    }

    /// Sends a JSON-RPC request on the bound session topic and awaits the
    /// matching response.
    ///
    /// On a dropped relay connection the client reconnects, re-subscribes, and
    /// retries the request once before surfacing the error.
    pub(crate) async fn request(&self, method: &str, params: Value) -> Result<Value, WalletError> {
        #[cfg(test)]
        if self.mock_relay.is_some() {
            return self.request_mock(method, params).await;
        }
        match self.request_once(method, params.clone()).await {
            Err(e) if is_connection_error(&e) => {
                self.reconnect_and_resubscribe().await?;
                self.request_once(method, params).await
            }
            other => other,
        }
    }

    /// Mock-relay request path: publish an encrypted request and await the
    /// matching response from the in-memory relay.
    #[cfg(test)]
    async fn request_mock(&self, method: &str, params: Value) -> Result<Value, WalletError> {
        let relay = self
            .mock_relay
            .clone()
            .ok_or_else(|| WalletError::Unreachable("no mock relay attached".to_string()))?;
        let topic = self
            .topic
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no bound session topic".to_string()))?;
        let sym_key = self
            .sym_key
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no session sym key set".to_string()))?;

        let id = self.next_id().await;
        let req = JsonRpcRequest::new(method, params, id);
        let req_bytes = serde_json::to_vec(&req)
            .map_err(|e| WalletError::Transport(format!("request serialization failed: {e}")))?;
        let envelope = WcCipher::seal_type0(&sym_key, &req_bytes)?;

        let mut sub = relay.subscribe(&topic).await;
        relay.publish(&topic, &envelope).await;

        loop {
            let payload = sub
                .recv()
                .await
                .map_err(|_| WalletError::Unreachable("mock relay stream ended".to_string()))?;
            let plaintext = match payload.first() {
                Some(&crypto::ENVELOPE_TYPE_0) => WcCipher::open_type0(&sym_key, &payload)?,
                Some(&crypto::ENVELOPE_TYPE_1) => WcCipher::open_type1(&sym_key, &payload)?.1,
                _ => continue,
            };
            let resp: JsonRpcResponse = serde_json::from_slice(&plaintext)
                .map_err(|e| WalletError::Transport(format!("response parse failed: {e}")))?;
            if resp.id != id {
                continue;
            }
            if resp.result.is_none() && resp.error.is_none() {
                continue;
            }
            if let Some(e) = resp.error {
                return Err(WalletError::Rejected(format!("{} ({})", e.message, e.code)));
            }
            return Ok(resp.result.unwrap_or(Value::Null));
        }
    }

    /// Single attempt at a session request (publish + await matching response).
    async fn request_once(&self, method: &str, params: Value) -> Result<Value, WalletError> {
        let topic = self
            .topic
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no bound session topic".to_string()))?;

        let sym_key = self
            .sym_key
            .lock()
            .await
            .clone()
            .ok_or_else(|| WalletError::Transport("no session sym key set".to_string()))?;

        let mut relay_guard = self.relay.lock().await;
        let relay = relay_guard
            .as_mut()
            .ok_or_else(|| WalletError::Unreachable("relay not connected".to_string()))?;

        let id = self.next_id().await;
        let req = JsonRpcRequest::new(method, params, id);
        let req_bytes = serde_json::to_vec(&req)
            .map_err(|e| WalletError::Transport(format!("request serialization failed: {e}")))?;

        let envelope = WcCipher::seal_type0(&sym_key, &req_bytes)?;
        relay.publish_irn(&relay_id(id), &topic, &BASE64.encode(&envelope), 300, 1108).await?;

        loop {
            let raw = relay.recv_timeout(RELAY_RECV_TIMEOUT).await?;
            let envelope_val: Value = serde_json::from_str(&raw)
                .map_err(|e| WalletError::Transport(format!("relay envelope parse failed: {e}")))?;

            if envelope_val.get("method").and_then(|m| m.as_str()) !=
                Some(relay_method::IRN_SUBSCRIPTION)
            {
                continue;
            }
            let data = match envelope_val.pointer("/params/data") {
                Some(d) => d,
                None => continue,
            };
            if data.get("topic").and_then(|t| t.as_str()) != Some(&topic) {
                continue;
            }
            let b64_msg = match data.get("message").and_then(|m| m.as_str()) {
                Some(m) => m,
                None => continue,
            };

            let encrypted = BASE64.decode(b64_msg).unwrap_or_default();
            if encrypted.first() != Some(&crypto::ENVELOPE_TYPE_0) &&
                encrypted.first() != Some(&crypto::ENVELOPE_TYPE_1)
            {
                continue;
            }
            let plaintext = match encrypted[0] {
                crypto::ENVELOPE_TYPE_0 => WcCipher::open_type0(&sym_key, &encrypted)?,
                crypto::ENVELOPE_TYPE_1 => WcCipher::open_type1(&sym_key, &encrypted)?.1,
                _ => continue,
            };
            if plaintext.is_empty() {
                continue;
            }

            let resp: JsonRpcResponse = serde_json::from_slice(&plaintext)
                .map_err(|e| WalletError::Transport(format!("response parse failed: {e}")))?;
            if resp.id != id {
                continue;
            }
            if resp.result.is_none() && resp.error.is_none() {
                continue;
            }
            if let Some(e) = resp.error {
                return Err(WalletError::Rejected(format!("{} ({})", e.message, e.code)));
            }
            return Ok(resp.result.unwrap_or(Value::Null));
        }
    }

    /// Allocates the next JSON-RPC request id.
    async fn next_id(&self) -> i64 {
        let mut n = self.next_id.lock().await;
        let v = *n;
        *n += 1;
        v
    }

    /// Closes the relay connection (best-effort).
    pub(crate) async fn close(&self) {
        let mut guard = self.relay.lock().await;
        if let Some(relay) = guard.as_mut() {
            relay.close().await;
        }
        *guard = None;
    }
}

/// Returns `true` when the error indicates a dropped relay connection that a
/// reconnect + retry can recover from.
const fn is_connection_error(e: &WalletError) -> bool {
    matches!(e, WalletError::Unreachable(_))
}

/// Generates a relay JSON-RPC `id` matching the official client's
/// recommendation (19-digit value: 13-digit epoch milliseconds + 6-digit
/// entropy).
fn relay_id(counter: i64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let id = (millis << 20) | ((counter as u64) & 0xFFFFF);
    format!("{id:019}")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::wc::mock_relay::MockRelay;

    /// Builds a client with a mock relay and a pre-established session
    /// (topic + session key), returning the client and the shared session key.
    fn client_with_session(relay: Arc<MockRelay>) -> (WcDappClient, WcSymKey) {
        let mut client = WcDappClient::new();
        client.attach_mock_relay(relay);
        let session_key = WcSymKey::from_random();
        let topic = "session-topic".to_string();
        client.topic = Mutex::new(Some(topic));
        client.sym_key = Mutex::new(Some(session_key.clone()));
        (client, session_key)
    }

    #[tokio::test]
    async fn request_round_trips_through_mock_relay() {
        let relay = MockRelay::new();
        let (client, session_key) = client_with_session(Arc::clone(&relay));
        let topic = "session-topic".to_string();

        // Mock wallet: subscribe first (signalled via oneshot), then respond.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let wallet = {
            let relay = Arc::clone(&relay);
            let session_key = session_key.clone();
            let topic = topic.clone();
            tokio::spawn(async move {
                let mut sub = relay.subscribe(&topic).await;
                let _ = ready_tx.send(());
                let payload = sub.recv().await.expect("request envelope");
                let plaintext = WcCipher::open_type0(&session_key, &payload).expect("decrypt");
                let req: JsonRpcRequest = serde_json::from_slice(&plaintext).expect("parse");
                assert_eq!(req.method, "personal_sign");
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(serde_json::json!("0xdeadbeef")),
                    error: None,
                };
                let resp_bytes = serde_json::to_vec(&resp).expect("serialize");
                let envelope = WcCipher::seal_type0(&session_key, &resp_bytes).expect("seal");
                relay.publish(&topic, &envelope).await;
            })
        };
        ready_rx.await.expect("wallet subscribed");

        let result = client
            .request("personal_sign", serde_json::json!(["0x6869", "0xabc"]))
            .await
            .expect("request");
        assert_eq!(result, serde_json::json!("0xdeadbeef"));
        wallet.await.expect("wallet task");
    }

    #[tokio::test]
    async fn request_surfaces_wallet_rejection() {
        let relay = MockRelay::new();
        let (client, session_key) = client_with_session(Arc::clone(&relay));
        let topic = "session-topic".to_string();

        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let wallet = {
            let relay = Arc::clone(&relay);
            let session_key = session_key.clone();
            let topic = topic.clone();
            tokio::spawn(async move {
                let mut sub = relay.subscribe(&topic).await;
                let _ = ready_tx.send(());
                let payload = sub.recv().await.expect("request envelope");
                let plaintext = WcCipher::open_type0(&session_key, &payload).expect("decrypt");
                let req: JsonRpcRequest = serde_json::from_slice(&plaintext).expect("parse");
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: None,
                    error: Some(crate::wc::jsonrpc::JsonRpcError {
                        code: 4001,
                        message: "user rejected".to_string(),
                        data: None,
                    }),
                };
                let resp_bytes = serde_json::to_vec(&resp).expect("serialize");
                let envelope = WcCipher::seal_type0(&session_key, &resp_bytes).expect("seal");
                relay.publish(&topic, &envelope).await;
            })
        };
        ready_rx.await.expect("wallet subscribed");

        let err = client
            .request("personal_sign", serde_json::json!(["0x6869", "0xabc"]))
            .await
            .expect_err("must be rejected");
        assert!(matches!(err, WalletError::Rejected(_)));
        assert!(err.to_string().contains("user rejected"));
        wallet.await.expect("wallet task");
    }

    #[tokio::test]
    async fn propose_derives_session_key_from_approval() {
        let relay = MockRelay::new();
        let mut client = WcDappClient::new();
        client.attach_mock_relay(Arc::clone(&relay));
        let pairing_key = WcSymKey::from_random();
        let pairing_topic = "pairing-topic".to_string();
        client.topic = Mutex::new(Some(pairing_topic.clone()));
        client.sym_key = Mutex::new(Some(pairing_key.clone()));

        // Mock wallet: subscribe to the pairing topic (signalled via oneshot),
        // then approve the proposal.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let wallet = {
            let relay = Arc::clone(&relay);
            let pairing_key = pairing_key.clone();
            let pairing_topic = pairing_topic.clone();
            tokio::spawn(async move {
                let mut sub = relay.subscribe(&pairing_topic).await;
                let _ = ready_tx.send(());
                let payload = sub.recv().await.expect("propose envelope");
                let (proposer_pub, plaintext) =
                    WcCipher::open_type1(&pairing_key, &payload).expect("decrypt propose");
                let req: JsonRpcRequest = serde_json::from_slice(&plaintext).expect("parse");
                assert_eq!(req.method, "wc_sessionPropose");

                let responder_kp = WcKeyPair::generate();
                let shared =
                    responder_kp.shared_secret(&x25519_dalek::PublicKey::from(proposer_pub));
                let _session_key = crypto::derive_sym_key(&shared).expect("derive");

                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(serde_json::json!({
                        "relay": { "protocol": "irn" },
                        "responderPublicKey": responder_kp.public_key_hex(),
                        "expiry": u64::MAX,
                    })),
                    error: None,
                };
                let resp_bytes = serde_json::to_vec(&resp).expect("serialize");
                let envelope = WcCipher::seal_type0(&pairing_key, &resp_bytes).expect("seal");
                relay.publish(&pairing_topic, &envelope).await;
            })
        };

        ready_rx.await.expect("wallet subscribed");
        let proposer_pubkey =
            client.propose("test-dapp", "https://test.example").await.expect("propose");
        assert_eq!(proposer_pubkey.len(), 64);
        // Session key must have been derived and the topic switched to the
        // session topic (SHA-256 of the proposer public key).
        let session_key = client.sym_key.lock().await.clone().expect("session key");
        let session_topic = client.topic.lock().await.clone().expect("session topic");
        assert_eq!(session_topic, crypto::hash_bytes(&hex::decode(&proposer_pubkey).expect("hex")));
        assert_ne!(session_key.as_bytes(), pairing_key.as_bytes());
        wallet.await.expect("wallet task");
    }
}
