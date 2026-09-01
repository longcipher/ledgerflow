//! In-memory relay for testing the WC v2 dApp client without a real relay.
//!
//! Simulates the IRN pub/sub model: subscribers register per topic and
//! published messages are delivered to every subscriber of that topic. The
//! dApp client's mock paths use this directly (no WebSocket).

use std::collections::HashMap;
use std::sync::Arc;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use tokio::sync::Mutex;

/// In-memory pub/sub relay.
pub(crate) struct MockRelay {
    subscribers: Mutex<HashMap<String, Vec<UnboundedSender<Vec<u8>>>>>,
}

impl MockRelay {
    /// Creates a new mock relay.
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self { subscribers: Mutex::new(HashMap::new()) })
    }

    /// Subscribes to a topic, returning a receiver for published messages.
    pub(crate) async fn subscribe(&self, topic: &str) -> UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = unbounded();
        self.subscribers.lock().await.entry(topic.to_string()).or_default().push(tx);
        rx
    }

    /// Publishes raw bytes to all subscribers of a topic.
    pub(crate) async fn publish(&self, topic: &str, bytes: &[u8]) {
        let subs = self.subscribers.lock().await;
        if let Some(senders) = subs.get(topic) {
            for tx in senders {
                let _ = tx.unbounded_send(bytes.to_vec());
            }
        }
    }
}