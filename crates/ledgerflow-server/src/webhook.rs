//! Webhook event emission and delivery.
//!
//! Events are buffered in-memory for the tenant-scoped audit endpoint, and —
//! when a delivery URL is configured — delivered to an HTTP webhook endpoint
//! with a bounded retry. Delivery is best-effort and non-blocking: it runs on
//! a detached thread so handler latency is unaffected. A real, durable,
//! at-least-once fan-out (persistent queue + idempotency keys) is a deployment
//! concern for the platform (design §10.3); this module provides the
//! synchronous in-process delivery path.

use std::sync::Arc;

/// Webhook event kinds emitted by the server.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub enum WebhookEvent {
    /// A warrant was issued.
    WarrantIssued { tenant_id: String, warrant_id: String },
    /// A warrant was revoked.
    WarrantRevoked { tenant_id: String, warrant_id: String },
    /// A payment was settled.
    PaymentSettled { tenant_id: String, transaction_id: String, amount: u128 },
    /// An approval was requested.
    ApprovalRequested { tenant_id: String, request_hash: String },
}

impl WebhookEvent {
    /// Returns the tenant this event belongs to.
    #[must_use]
    pub fn tenant_id(&self) -> &str {
        match self {
            Self::WarrantIssued { tenant_id, .. } |
            Self::WarrantRevoked { tenant_id, .. } |
            Self::PaymentSettled { tenant_id, .. } |
            Self::ApprovalRequested { tenant_id, .. } => tenant_id,
        }
    }

    /// Returns a stable event type tag for delivery routing.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::WarrantIssued { .. } => "warrant.issued",
            Self::WarrantRevoked { .. } => "warrant.revoked",
            Self::PaymentSettled { .. } => "payment.settled",
            Self::ApprovalRequested { .. } => "approval.requested",
        }
    }
}

/// Webhook sender.
///
/// Buffers events for the audit endpoint and, when a delivery URL is
/// configured, delivers each event to that URL with a bounded retry on a
/// detached thread.
#[derive(Clone, Debug)]
pub struct WebhookSender {
    sink: Arc<std::sync::Mutex<Vec<WebhookEvent>>>,
    delivery_url: Option<String>,
}

impl WebhookSender {
    /// Creates a disabled sender (no delivery; events are buffered for tests).
    #[must_use]
    pub fn disabled() -> Self {
        Self { sink: Arc::new(std::sync::Mutex::new(Vec::new())), delivery_url: None }
    }

    /// Creates a sender that buffers events and delivers them to `delivery_url`
    /// (best-effort, with bounded retry).
    #[must_use]
    pub fn with_delivery(delivery_url: String) -> Self {
        Self { sink: Arc::new(std::sync::Mutex::new(Vec::new())), delivery_url: Some(delivery_url) }
    }

    /// Emits an event: buffers it and, if a delivery URL is configured, kicks
    /// off a best-effort delivery on a detached thread.
    pub fn emit(&self, event: WebhookEvent) {
        if let Ok(mut sink) = self.sink.lock() {
            sink.push(event.clone());
        }
        if let Some(url) = &self.delivery_url {
            let url = url.clone();
            let payload = serde_json::json!({
                "type": event.kind(),
                "tenant_id": event.tenant_id(),
                "event": event,
            });
            std::thread::spawn(move || {
                deliver_with_retry(&url, &payload);
            });
        }
    }

    /// Returns the buffered events (for tests and audit).
    pub fn buffered(&self) -> Vec<WebhookEvent> {
        self.sink.lock().map(|sink| sink.clone()).unwrap_or_default()
    }
}

/// Delivers a webhook payload with a bounded number of retries.
///
/// Best-effort: failures are logged and dropped (no durable queue in v1).
fn deliver_with_retry(url: &str, payload: &serde_json::Value) {
    const MAX_ATTEMPTS: usize = 3;
    for attempt in 0..MAX_ATTEMPTS {
        match deliver_once(url, payload) {
            Ok(()) => return,
            Err(error) => {
                if attempt + 1 < MAX_ATTEMPTS {
                    std::thread::sleep(std::time::Duration::from_millis(
                        200 * (attempt as u64 + 1),
                    ));
                } else {
                    tracing::warn!(url, error = %error, "webhook delivery failed after retries");
                }
            }
        }
    }
}

/// Performs a single synchronous webhook POST.
fn deliver_once(url: &str, payload: &serde_json::Value) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(async move {
        let client = hpx::Client::new();
        let response = client
            .post(url)
            .header("content-type", "application/json")
            .body(payload.to_string())
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("webhook returned status {}", response.status()))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_sender_buffers_events() {
        let sender = WebhookSender::disabled();
        assert!(sender.buffered().is_empty(), "new sender must have no buffered events");
        sender.emit(WebhookEvent::WarrantIssued {
            tenant_id: "t1".to_string(),
            warrant_id: "w1".to_string(),
        });
        sender.emit(WebhookEvent::WarrantRevoked {
            tenant_id: "t1".to_string(),
            warrant_id: "w1".to_string(),
        });
        sender.emit(WebhookEvent::PaymentSettled {
            tenant_id: "t1".to_string(),
            transaction_id: "tx-1".to_string(),
            amount: 100,
        });
        sender.emit(WebhookEvent::ApprovalRequested {
            tenant_id: "t1".to_string(),
            request_hash: "sha256:req".to_string(),
        });
        let events = sender.buffered();
        assert_eq!(events.len(), 4);
        assert_eq!(
            events[0],
            WebhookEvent::WarrantIssued {
                tenant_id: "t1".to_string(),
                warrant_id: "w1".to_string()
            }
        );
        assert_eq!(
            events[3],
            WebhookEvent::ApprovalRequested {
                tenant_id: "t1".to_string(),
                request_hash: "sha256:req".to_string()
            }
        );
    }

    #[test]
    fn webhook_sender_clone_shares_buffer() {
        let sender = WebhookSender::disabled();
        let clone = sender.clone();
        sender.emit(WebhookEvent::WarrantRevoked {
            tenant_id: "t1".to_string(),
            warrant_id: "w2".to_string(),
        });
        assert_eq!(clone.buffered().len(), 1);
    }

    #[test]
    fn webhook_events_are_distinct() {
        assert_ne!(
            WebhookEvent::WarrantIssued { tenant_id: "t".to_string(), warrant_id: "w".to_string() },
            WebhookEvent::WarrantRevoked {
                tenant_id: "t".to_string(),
                warrant_id: "w".to_string()
            }
        );
        assert_ne!(
            WebhookEvent::PaymentSettled {
                tenant_id: "t".to_string(),
                transaction_id: "t".to_string(),
                amount: 1
            },
            WebhookEvent::PaymentSettled {
                tenant_id: "t".to_string(),
                transaction_id: "t".to_string(),
                amount: 2
            }
        );
    }

    #[test]
    fn event_kind_tags_are_stable() {
        assert_eq!(
            WebhookEvent::WarrantIssued { tenant_id: "t".into(), warrant_id: "w".into() }.kind(),
            "warrant.issued"
        );
        assert_eq!(
            WebhookEvent::PaymentSettled {
                tenant_id: "t".into(),
                transaction_id: "x".into(),
                amount: 1
            }
            .kind(),
            "payment.settled"
        );
    }
}
