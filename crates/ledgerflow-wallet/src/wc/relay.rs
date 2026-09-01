//! WC v2 relay client (WebSocket Secure).
//!
//! Connects to a Waku v2 / IRN relay endpoint and provides send/recv over
//! text frames. Each JSON-RPC message is one WS text frame, matching the
//! official relay RPC protocol (`irn_publish` / `irn_subscribe`).

use std::time::Duration;

use futures::{SinkExt, StreamExt};
use hpx_yawc::{
    TcpWebSocket,
    frame::{Frame, OpCode},
};

use crate::error::WalletError;

/// Relay connection configuration.
#[derive(Debug, Clone)]
pub(crate) struct RelayConfig {
    /// Relay WSS URL (e.g. `wss://relay.walletconnect.com`).
    pub(crate) url: String,
    /// Maximum reconnect backoff delay.
    #[allow(dead_code)] // reserved for reconnect support
    pub(crate) reconnect_max_ms: u64,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self { url: "wss://relay.walletconnect.com".to_string(), reconnect_max_ms: 60_000 }
    }
}

/// Appends a `projectId` query param when the URL does not already carry one.
///
/// The official WalletConnect Cloud relay requires `?projectId=<id>`; local /
/// self-hosted relays may accept it or not, so the param is only appended when
/// a project ID is explicitly configured.
pub(crate) fn apply_project_id(url: &str, project_id: Option<&str>) -> String {
    match project_id {
        Some(pid) if !pid.is_empty() => {
            if url.contains('?') {
                format!("{url}&projectId={pid}")
            } else {
                format!("{url}?projectId={pid}")
            }
        }
        _ => url.to_string(),
    }
}

/// A live relay connection.
pub(crate) struct RelayClient {
    ws: TcpWebSocket,
}

impl RelayClient {
    /// Connects to the relay at `cfg.url`.
    pub(crate) async fn connect(cfg: &RelayConfig) -> Result<Self, WalletError> {
        let url = cfg
            .url
            .parse()
            .map_err(|e| WalletError::Unreachable(format!("invalid relay url: {e}")))?;
        let ws = TcpWebSocket::connect(url)
            .await
            .map_err(|e| WalletError::Unreachable(format!("relay connect failed: {e}")))?;
        Ok(Self { ws })
    }

    /// Sends a raw text frame.
    pub(crate) async fn send_text(&mut self, s: impl Into<String>) -> Result<(), WalletError> {
        self.ws
            .send(Frame::text(s.into()))
            .await
            .map_err(|e| WalletError::Transport(format!("relay send failed: {e}")))
    }

    /// Publishes a message to a topic using the IRN `irn_publish` JSON-RPC
    /// method. `message` is the base64-encoded envelope.
    pub(crate) async fn publish_irn(
        &mut self,
        id: &str,
        topic: &str,
        message_b64: &str,
        ttl: u32,
        tag: u32,
    ) -> Result<(), WalletError> {
        let msg = serde_json::json!({
            "id": id,
            "jsonrpc": "2.0",
            "method": "irn_publish",
            "params": {
                "topic": topic,
                "message": message_b64,
                "ttl": ttl,
                "tag": tag,
            },
        });
        self.send_text(serde_json::to_string(&msg).map_err(|e| {
            WalletError::Transport(format!("relay publish serialization failed: {e}"))
        })?)
        .await
    }

    /// Receives the next text frame from the relay.
    pub(crate) async fn recv(&mut self) -> Result<String, WalletError> {
        loop {
            match self.ws.next().await {
                Some(frame) => match frame.opcode() {
                    OpCode::Text => {
                        let text = frame
                            .as_str()
                            .map_err(|e| WalletError::Transport(format!("invalid text frame: {e}")))?;
                        return Ok(text.to_owned());
                    }
                    OpCode::Binary => {
                        return Ok(String::from_utf8_lossy(frame.payload()).into_owned());
                    }
                    OpCode::Ping => {
                        self.ws
                            .send(Frame::pong(frame.payload().to_vec()))
                            .await
                            .map_err(|e| WalletError::Transport(format!("pong failed: {e}")))?;
                    }
                    OpCode::Close => {
                        return Err(WalletError::Unreachable("relay connection closed".to_string()));
                    }
                    _ => {}
                },
                None => return Err(WalletError::Unreachable("relay stream ended".to_string())),
            }
        }
    }

    /// Like [`recv`](Self::recv) but bounds the wait with `timeout`.
    #[allow(dead_code)] // reserved for reconnect / heartbeat support
    pub(crate) async fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<String, WalletError> {
        match tokio::time::timeout(timeout, self.recv()).await {
            Ok(res) => res,
            Err(_) => Err(WalletError::Unreachable(format!(
                "no relay message within {timeout:?}"
            ))),
        }
    }

    /// Reconnects the underlying WebSocket to the same relay URL.
    pub(crate) async fn reconnect(&mut self, cfg: &RelayConfig) -> Result<(), WalletError> {
        let url = cfg
            .url
            .parse()
            .map_err(|e| WalletError::Unreachable(format!("invalid relay url: {e}")))?;
        let ws = TcpWebSocket::connect(url)
            .await
            .map_err(|e| WalletError::Unreachable(format!("relay reconnect failed: {e}")))?;
        self.ws = ws;
        Ok(())
    }

    /// Gracefully closes the underlying WebSocket.
    pub(crate) async fn close(&mut self) {
        let _ = self
            .ws
            .send(Frame::close(hpx_yawc::close::CloseCode::Normal, b"shutdown"))
            .await;
    }
}