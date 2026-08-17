//! Webhook event emission.

/// Webhook event kinds emitted by the server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WebhookEvent {
    /// A warrant was issued.
    WarrantIssued { warrant_id: String },
    /// A warrant was revoked.
    WarrantRevoked { warrant_id: String },
    /// A payment was settled.
    PaymentSettled { transaction_id: String, amount: u128 },
    /// An approval was requested.
    ApprovalRequested { request_hash: String },
}

/// Webhook sender (in-memory in v1; a real HTTP fan-out is a deployment
/// concern for the platform).
#[derive(Clone, Debug, Default)]
pub struct WebhookSender {
    sink: std::sync::Arc<std::sync::Mutex<Vec<WebhookEvent>>>,
}

impl WebhookSender {
    /// Creates a disabled sender (no delivery; events are buffered for tests).
    #[must_use]
    pub fn disabled() -> Self {
        Self { sink: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())) }
    }

    /// Emits an event.
    pub fn emit(&self, event: WebhookEvent) {
        if let Ok(mut sink) = self.sink.lock() {
            sink.push(event);
        }
    }

    /// Returns the buffered events (for tests and audit).
    pub fn buffered(&self) -> Vec<WebhookEvent> {
        self.sink.lock().map(|sink| sink.clone()).unwrap_or_default()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_sender_buffers_events() {
        let sender = WebhookSender::disabled();
        assert!(sender.buffered().is_empty(), "new sender must have no buffered events");
        sender.emit(WebhookEvent::WarrantIssued { warrant_id: "w1".to_string() });
        sender.emit(WebhookEvent::WarrantRevoked { warrant_id: "w1".to_string() });
        sender.emit(WebhookEvent::PaymentSettled { transaction_id: "tx-1".to_string(), amount: 100 });
        sender.emit(WebhookEvent::ApprovalRequested { request_hash: "sha256:req".to_string() });
        let events = sender.buffered();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0], WebhookEvent::WarrantIssued { warrant_id: "w1".to_string() });
        assert_eq!(events[3], WebhookEvent::ApprovalRequested { request_hash: "sha256:req".to_string() });
    }

    #[test]
    fn webhook_sender_clone_shares_buffer() {
        let sender = WebhookSender::disabled();
        let clone = sender.clone();
        sender.emit(WebhookEvent::WarrantRevoked { warrant_id: "w2".to_string() });
        assert_eq!(clone.buffered().len(), 1);
    }

    #[test]
    fn webhook_events_are_distinct() {
        assert_ne!(
            WebhookEvent::WarrantIssued { warrant_id: "w".to_string() },
            WebhookEvent::WarrantRevoked { warrant_id: "w".to_string() }
        );
        assert_ne!(
            WebhookEvent::PaymentSettled { transaction_id: "t".to_string(), amount: 1 },
            WebhookEvent::PaymentSettled { transaction_id: "t".to_string(), amount: 2 }
        );
    }
}
