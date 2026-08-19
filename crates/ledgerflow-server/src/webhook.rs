//! Webhook event emission and delivery.
//!
//! Events are buffered in-memory for the tenant-scoped audit endpoint, and —
//! when a delivery URL is configured — delivered to an HTTP webhook endpoint
//! with a bounded retry. Delivery is best-effort and non-blocking: handlers
//! enqueue onto a bounded worker queue so request latency and thread count stay
//! bounded under load. A real, durable, at-least-once fan-out (persistent
//! queue + idempotency keys) is a deployment concern for the platform
//! (design §10.3); this module provides the in-process delivery path.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

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
/// configured, enqueues each event onto a bounded background worker.
#[derive(Clone, Debug)]
pub struct WebhookSender {
    sink: Arc<std::sync::Mutex<Vec<WebhookEvent>>>,
    delivery: Option<Arc<DeliveryWorker>>,
}

#[derive(Clone, Debug)]
struct DeliveryConfig {
    queue_capacity: usize,
    max_attempts: usize,
    backoff_base_ms: u64,
    attempt_timeout_ms: u64,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 256,
            max_attempts: 3,
            backoff_base_ms: 200,
            attempt_timeout_ms: 5_000,
        }
    }
}

#[derive(Debug)]
struct DeliveryWorker {
    queue: flume::Sender<DeliveryJob>,
    dropped_events: AtomicUsize,
}

#[derive(Clone, Debug)]
struct DeliveryJob {
    payload: serde_json::Value,
}

impl WebhookSender {
    /// Creates a disabled sender (no delivery; events are buffered for tests).
    #[must_use]
    pub fn disabled() -> Self {
        Self { sink: Arc::new(std::sync::Mutex::new(Vec::new())), delivery: None }
    }

    /// Creates a sender that buffers events and delivers them to `delivery_url`
    /// (best-effort, with bounded retry).
    #[must_use]
    pub fn with_delivery(delivery_url: String) -> Self {
        Self::with_delivery_config(delivery_url, DeliveryConfig::default())
    }

    /// Emits an event: buffers it and, if a delivery URL is configured,
    /// enqueues it for best-effort background delivery.
    pub fn emit(&self, event: WebhookEvent) {
        if let Ok(mut sink) = self.sink.lock() {
            sink.push(event.clone());
        }
        if let Some(delivery) = &self.delivery {
            let payload = serde_json::json!({
                "type": event.kind(),
                "tenant_id": event.tenant_id(),
                "event": event,
            });
            delivery.enqueue(DeliveryJob { payload });
        }
    }

    /// Returns the buffered events (for tests and audit).
    pub fn buffered(&self) -> Vec<WebhookEvent> {
        self.sink.lock().map(|sink| sink.clone()).unwrap_or_default()
    }

    #[must_use]
    fn with_delivery_config(delivery_url: String, config: DeliveryConfig) -> Self {
        Self {
            sink: Arc::new(std::sync::Mutex::new(Vec::new())),
            delivery: Some(Arc::new(DeliveryWorker::spawn(delivery_url, config))),
        }
    }
}

impl DeliveryWorker {
    fn spawn(delivery_url: String, config: DeliveryConfig) -> Self {
        let queue_capacity = config.queue_capacity.max(1);
        let (queue, receiver) = flume::bounded(queue_capacity);
        if let Err(error) = std::thread::Builder::new()
            .name("ledgerflow-webhook".to_string())
            .spawn(move || run_delivery_worker(delivery_url, receiver, config))
        {
            tracing::warn!(error = %error, "failed to start webhook worker");
        }
        Self { queue, dropped_events: AtomicUsize::new(0) }
    }

    fn enqueue(&self, job: DeliveryJob) {
        match self.queue.try_send(job) {
            Ok(()) => {}
            Err(flume::TrySendError::Full(_)) => {
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
                tracing::warn!("webhook delivery queue is full; dropping event");
            }
            Err(flume::TrySendError::Disconnected(_)) => {
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
                tracing::warn!("webhook delivery worker stopped; dropping event");
            }
        }
    }
}

fn run_delivery_worker(
    delivery_url: String,
    receiver: flume::Receiver<DeliveryJob>,
    config: DeliveryConfig,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::warn!(error = %error, "failed to initialize webhook runtime");
            return;
        }
    };
    let client = hpx::Client::new();
    while let Ok(job) = receiver.recv() {
        if let Err(error) =
            runtime.block_on(deliver_with_retry(&client, &delivery_url, &job.payload, &config))
        {
            tracing::warn!(url = %delivery_url, error = %error, "webhook delivery failed");
        }
    }
}

/// Delivers a webhook payload with a bounded number of retries.
async fn deliver_with_retry(
    client: &hpx::Client,
    url: &str,
    payload: &serde_json::Value,
    config: &DeliveryConfig,
) -> Result<(), String> {
    let mut last_error = None;
    for attempt in 0..config.max_attempts {
        match deliver_once(client, url, payload, config).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < config.max_attempts {
                    tokio::time::sleep(Duration::from_millis(
                        config.backoff_base_ms * (attempt as u64 + 1),
                    ))
                    .await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| "webhook delivery exhausted retries".to_string()))
}

/// Performs a single webhook POST using the worker-owned client.
async fn deliver_once(
    client: &hpx::Client,
    url: &str,
    payload: &serde_json::Value,
    config: &DeliveryConfig,
) -> Result<(), String> {
    let request =
        client.post(url).header("content-type", "application/json").body(payload.to_string());
    let response =
        tokio::time::timeout(Duration::from_millis(config.attempt_timeout_ms), request.send())
            .await
            .map_err(|_| {
                format!("webhook request timed out after {} ms", config.attempt_timeout_ms)
            })?
            .map_err(|error| error.to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("webhook returned status {}", response.status()))
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::atomic::Ordering, time::Duration};

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

    #[test]
    fn webhook_sender_uses_a_bounded_delivery_queue() {
        let sender = WebhookSender::with_delivery_config(
            "http://127.0.0.1:9".to_string(),
            DeliveryConfig {
                queue_capacity: 1,
                max_attempts: 3,
                backoff_base_ms: 250,
                attempt_timeout_ms: 250,
            },
        );

        for index in 0..16 {
            sender.emit(WebhookEvent::WarrantIssued {
                tenant_id: "t1".to_string(),
                warrant_id: format!("w{index}"),
            });
        }

        std::thread::sleep(Duration::from_millis(50));
        let dropped = sender
            .delivery
            .as_ref()
            .map(|worker| worker.dropped_events.load(Ordering::Relaxed))
            .unwrap_or_default();
        assert!(dropped > 0, "queue saturation should drop excess events");
        assert_eq!(sender.buffered().len(), 16, "audit buffering stays intact");
    }
}
