//! Merchant-side integration kit for LedgerFlow Cloud.
//!
//! - [`CloudClient`]: signed REST client. The secret key never leaves this process — requests carry
//!   the public key id plus an HMAC-SHA256 proof keyed by `SHA-256(secret)`, matching the cloud's
//!   stored digest.
//! - [`WebhookVerifier`]: constant-time webhook signature verification with timestamp tolerance and
//!   schema-version gating.
//!
//! Wire rules (language-neutral): amounts are decimal strings of u128 base
//! units, timestamps are Unix seconds, signatures are lowercase hex.
#![forbid(unsafe_code)]

use hmac::{Hmac, KeyInit as _, Mac as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// SDK result alias.
pub type Result<T> = std::result::Result<T, SdkError>;

/// Canonical MAC: `HMAC-SHA256(SHA-256(key), message)`.
fn mac(key_material: &[u8], message: &[u8]) -> Vec<u8> {
    let key_digest = Sha256::digest(key_material);
    let mut instance = match HmacSha256::new_from_slice(key_digest.as_slice()) {
        Ok(mac) => mac,
        // Unreachable: HMAC accepts any key length.
        Err(_) => return Sha256::digest(message).to_vec(),
    };
    instance.update(message);
    instance.finalize().into_bytes().to_vec()
}

/// Lowercase hex encoding.
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// SDK error taxonomy.
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    /// Builder/input configuration problem.
    #[error("configuration: {0}")]
    Config(String),
    /// Transport failure.
    #[error("http transport: {0}")]
    Http(String),
    /// Cloud returned a structured API error.
    #[error("api {status}: {code}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Stable error code from the envelope.
        code: String,
        /// Human-readable message.
        message: String,
    },
    /// Envelope/protocol mismatch.
    #[error("protocol: {0}")]
    Protocol(String),
    /// Webhook verification failure.
    #[error("webhook: {0}")]
    Webhook(String),
}

/// Serializes `u128` amounts as decimal strings (wire rule).
pub mod serde_amount {
    use serde::{Deserialize, Deserializer, Serializer};

    /// Serializes as a decimal string.
    pub fn serialize<S: Serializer>(value: &u128, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    /// Deserializes from a decimal string.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u128, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse::<u128>().map_err(serde::de::Error::custom)
    }
}

/// Body for `POST /v1/merchant/charges`.
#[derive(Debug, Serialize)]
pub struct CreateCharge {
    /// Amount in base units (serialized as a decimal string).
    #[serde(with = "serde_amount")]
    pub amount: u128,
    /// CAIP-19 asset reference.
    pub asset: String,
    /// Payment scheme: `x402` or `mpp`.
    pub scheme: String,
    /// Optional human description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Time-to-live in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_secs: Option<u64>,
    /// Free-form metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Charge lifecycle status tags returned by the cloud.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeStatus {
    /// Awaiting a verified payment payload.
    Pending,
    /// Payload verified; settlement may arrive.
    Authorized,
    /// Fully settled.
    Settled,
    /// Terminal settle failure.
    Failed,
    /// TTL elapsed while pending.
    Expired,
    /// Canceled before payment.
    Canceled,
    /// Reversed by a tenant admin.
    Refunded,
}

impl ChargeStatus {
    /// Parses a status tag.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "authorized" => Some(Self::Authorized),
            "settled" => Some(Self::Settled),
            "failed" => Some(Self::Failed),
            "expired" => Some(Self::Expired),
            "canceled" => Some(Self::Canceled),
            "refunded" => Some(Self::Refunded),
            _ => None,
        }
    }
}

/// A charge as returned by the cloud.
#[derive(Clone, Debug, Deserialize)]
pub struct Charge {
    /// Public number (`chg_…`).
    pub charge_no: String,
    /// Lifecycle tag.
    pub status: String,
    /// Amount (decimal string).
    pub amount: String,
    /// CAIP-19 asset.
    pub asset: String,
    /// `x402` or `mpp`.
    pub scheme: String,
    /// `test` or `live`.
    pub mode: String,
    /// Expiry instant.
    pub expires_at: String,
}

/// A merchant balance row.
#[derive(Clone, Debug, Deserialize)]
pub struct Balance {
    /// CAIP-19 asset.
    pub asset: String,
    /// Available amount (decimal string).
    pub available: String,
    /// Authorized-not-final amount (decimal string).
    pub pending: String,
}

/// A ledger entry.
#[derive(Clone, Debug, Deserialize)]
pub struct LedgerEntry {
    /// Monotonic id.
    pub id: i64,
    /// Signed delta (decimal string).
    pub amount: String,
    /// Balance after (decimal string).
    pub balance_after: String,
    /// Entry type tag.
    pub entry_type: String,
    /// Reference discriminator.
    pub ref_type: String,
    /// Reference id.
    pub ref_id: String,
    /// Entry time.
    pub created_at: String,
}

/// Metadata about an issued warrant.
#[derive(Clone, Debug, Deserialize)]
pub struct WarrantIssued {
    /// Warrant digest (hex).
    pub digest: String,
    /// Protocol warrant id.
    pub warrant_id: String,
    /// Expiry instant.
    pub expires_at: String,
}

/// Keyset-pagination page.
#[derive(Debug, Deserialize)]
pub struct Page<T> {
    /// Items in this page.
    pub data: Vec<T>,
    /// Opaque cursor for the next page.
    pub next_cursor: Option<String>,
}

/// Builds the signature headers for one request.
///
/// `signature = hex(HMAC-SHA256(SHA-256(sk), pk \n t \n METHOD \n path \n hex(SHA-256(body))))`.
#[must_use]
pub fn signature_headers(
    secret_key: &str,
    public_key: &str,
    timestamp: i64,
    method: &str,
    path_and_query: &str,
    body: &[u8],
) -> Vec<(&'static str, String)> {
    let body_hash = Sha256::digest(body);
    let mut payload = Vec::with_capacity(96 + body.len());
    payload.extend_from_slice(public_key.as_bytes());
    payload.push(b'\n');
    payload.extend_from_slice(timestamp.to_string().as_bytes());
    payload.push(b'\n');
    payload.extend_from_slice(method.as_bytes());
    payload.push(b'\n');
    payload.extend_from_slice(path_and_query.as_bytes());
    payload.push(b'\n');
    payload.extend_from_slice(hex_lower(&body_hash).as_bytes());
    let digest = mac(secret_key.as_bytes(), &payload);
    vec![
        ("authorization", format!("Bearer {public_key}")),
        ("x-lf-timestamp", timestamp.to_string()),
        ("x-lf-signature", format!("v1={}", hex_lower(&digest))),
    ]
}

/// Cloud REST client.
#[derive(Clone)]
pub struct CloudClient {
    http: hpx::Client,
    base_url: String,
    secret_key: String,
    public_key: String,
}

impl std::fmt::Debug for CloudClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret key must never appear in debug output.
        f.debug_struct("CloudClient")
            .field("base_url", &self.base_url)
            .field("public_key", &self.public_key)
            .finish_non_exhaustive()
    }
}

impl CloudClient {
    /// Starts building a client.
    #[must_use]
    pub const fn builder() -> CloudClientBuilder {
        CloudClientBuilder {
            base_url: None,
            secret_key: None,
            timeout: std::time::Duration::from_secs(15),
        }
    }

    async fn sign_and_send(
        &self,
        method: &str,
        path_and_query: &str,
        body: Option<&serde_json::Value>,
        idempotency_key: Option<&str>,
    ) -> Result<serde_json::Value> {
        let bytes = body.map(|value| value.to_string()).unwrap_or_default();
        let now = chrono_now();
        let mut request = match method {
            "POST" => self.http.post(format!("{}{path_and_query}", self.base_url)),
            _ => self.http.get(format!("{}{path_and_query}", self.base_url)),
        };
        for (name, value) in signature_headers(
            &self.secret_key,
            &self.public_key,
            now,
            method,
            path_and_query,
            bytes.as_bytes(),
        ) {
            request = request.header(name, value);
        }
        if let Some(key) = idempotency_key {
            request = request.header("idempotency-key", key);
        }
        if body.is_some() {
            request = request.header("content-type", "application/json").body(bytes);
        }
        let response = request.send().await.map_err(|error| SdkError::Http(error.to_string()))?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|error| SdkError::Http(error.to_string()))?;
        let envelope: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| SdkError::Protocol(format!("non-json envelope: {error}")))?;
        if envelope["ok"] == serde_json::Value::Bool(true) {
            Ok(envelope["data"].clone())
        } else {
            let error = &envelope["error"];
            Err(SdkError::Api {
                status,
                code: error["code"].as_str().unwrap_or("unknown").to_string(),
                message: error["message"].as_str().unwrap_or_default().to_string(),
            })
        }
    }

    /// Creates a charge; generates an `Idempotency-Key` automatically.
    pub async fn create_charge(&self, charge: &CreateCharge) -> Result<Charge> {
        let digest = hex_lower(&Sha256::digest(
            serde_json::to_string(charge).unwrap_or_default().as_bytes(),
        ));
        let key = format!("auto-{}", &digest[..24]);
        let value = self
            .sign_and_send(
                "POST",
                "/v1/merchant/charges",
                Some(&serde_json::to_value(charge).map_err(|e| SdkError::Protocol(e.to_string()))?),
                Some(&key),
            )
            .await?;
        serde_json::from_value(value).map_err(|e| SdkError::Protocol(e.to_string()))
    }

    /// Fetches one charge.
    pub async fn get_charge(&self, charge_no: &str) -> Result<Charge> {
        let value = self
            .sign_and_send("GET", &format!("/v1/merchant/charges/{charge_no}"), None, None)
            .await?;
        serde_json::from_value(value).map_err(|e| SdkError::Protocol(e.to_string()))
    }

    /// Lists charges (keyset cursor).
    pub async fn list_charges(&self, cursor: Option<&str>) -> Result<Page<Charge>> {
        let suffix = cursor.map(|c| format!("?cursor={c}")).unwrap_or_default();
        let value =
            self.sign_and_send("GET", &format!("/v1/merchant/charges{suffix}"), None, None).await?;
        serde_json::from_value(value).map_err(|e| SdkError::Protocol(e.to_string()))
    }

    /// Cancels a pending charge.
    pub async fn cancel_charge(&self, charge_no: &str) -> Result<Charge> {
        let value = self
            .sign_and_send(
                "POST",
                &format!("/v1/merchant/charges/{charge_no}/cancel"),
                Some(&serde_json::json!({})),
                None,
            )
            .await?;
        serde_json::from_value(value).map_err(|e| SdkError::Protocol(e.to_string()))
    }

    /// Fetches merchant balances.
    pub async fn balances(&self) -> Result<Vec<Balance>> {
        let value = self.sign_and_send("GET", "/v1/merchant/balances", None, None).await?;
        serde_json::from_value(value).map_err(|e| SdkError::Protocol(e.to_string()))
    }

    /// Fetches recent ledger entries.
    pub async fn ledger(&self) -> Result<Vec<LedgerEntry>> {
        let value = self.sign_and_send("GET", "/v1/merchant/ledger", None, None).await?;
        serde_json::from_value(value).map_err(|e| SdkError::Protocol(e.to_string()))
    }

    /// Issues an agent spending warrant.
    pub async fn issue_warrant(&self, request: &serde_json::Value) -> Result<WarrantIssued> {
        let value =
            self.sign_and_send("POST", "/v1/merchant/warrants", Some(request), None).await?;
        serde_json::from_value(value).map_err(|e| SdkError::Protocol(e.to_string()))
    }

    /// Revokes a warrant by digest.
    pub async fn revoke_warrant(&self, digest_hex: &str) -> Result<()> {
        self.sign_and_send(
            "POST",
            &format!("/v1/merchant/warrants/{digest_hex}/revoke"),
            Some(&serde_json::json!({})),
            None,
        )
        .await?;
        Ok(())
    }
}

/// Builder for [`CloudClient`].
#[derive(Debug)]
pub struct CloudClientBuilder {
    base_url: Option<String>,
    secret_key: Option<String>,
    timeout: std::time::Duration,
}

impl CloudClientBuilder {
    /// Sets the cloud base URL (required).
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the merchant secret key `sk_test_…` / `sk_live_…` (required).
    #[must_use]
    pub fn secret_key(mut self, key: impl Into<String>) -> Self {
        self.secret_key = Some(key.into());
        self
    }

    /// Overrides the request timeout (default 15 s).
    #[must_use]
    pub const fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Builds the client, deriving the public key id from the secret prefix.
    ///
    /// # Errors
    /// [`SdkError::Config`] when required fields are missing or the secret
    /// does not start with `sk_test_`/`sk_live_`.
    pub fn build(self) -> Result<CloudClient> {
        let base_url =
            self.base_url.ok_or_else(|| SdkError::Config("base_url is required".into()))?;
        let secret_key =
            self.secret_key.ok_or_else(|| SdkError::Config("secret_key is required".into()))?;
        if !secret_key.starts_with("sk_test_") && !secret_key.starts_with("sk_live_") {
            return Err(SdkError::Config("secret must start with sk_test_ or sk_live_".into()));
        }
        // The public key id is embedded in the secret after the mode prefix:
        // `sk_<mode>_<pkid>_<random>` is NOT assumed; instead callers pass the
        // pk via the conventional `LF_PK` companion env var. For ergonomics we
        // accept `sk` strings of the form `sk_live_<pkid>:<random>` too.
        let public_key =
            std::env::var("LF_PK").unwrap_or_else(|_| match secret_key.split_once(':') {
                Some((_, pk)) => pk.to_string(),
                None => String::new(),
            });
        if public_key.is_empty() ||
            (!public_key.starts_with("pk_test_") && !public_key.starts_with("pk_live_"))
        {
            return Err(SdkError::Config(
                "public key required: set LF_PK or embed it as sk_<mode>_<pkid>:<random>".into(),
            ));
        }
        let http = hpx::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|error| SdkError::Http(error.to_string()))?;
        Ok(CloudClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            secret_key,
            public_key,
        })
    }
}

/// Verified webhook event envelope.
#[derive(Clone, Debug, Deserialize)]
pub struct Event {
    /// Schema version date tag.
    pub schema_version: String,
    /// Event id (`evt_…`).
    pub id: String,
    /// Event type tag.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Unix seconds.
    pub created: i64,
    /// Owning tenant.
    pub tenant_id: String,
    /// Type-specific payload.
    pub data: serde_json::Value,
}

/// Verifies and parses webhook deliveries.
#[derive(Debug)]
pub struct WebhookVerifier {
    secret: String,
    tolerance_secs: i64,
    accepted_versions: Vec<String>,
}

const DEFAULT_SCHEMA_VERSION: &str = "2026-08-22";

impl WebhookVerifier {
    /// Creates a verifier with default tolerance (300 s).
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            tolerance_secs: 300,
            accepted_versions: vec![DEFAULT_SCHEMA_VERSION.to_string()],
        }
    }

    /// Sets the timestamp tolerance window.
    #[must_use]
    pub const fn tolerance_secs(mut self, secs: i64) -> Self {
        self.tolerance_secs = secs;
        self
    }

    /// Replaces the accepted schema versions.
    #[must_use]
    pub fn acceptable_schema_versions(mut self, versions: &[&str]) -> Self {
        self.accepted_versions = versions.iter().map(|version| (*version).to_string()).collect();
        self
    }

    /// Verifies `signature_header` over `body` and returns the parsed event.
    ///
    /// Header format: `t=<unix>,v1=<hex>[,v1=<hex>…]`.
    ///
    /// # Errors
    /// [`SdkError::Webhook`] on malformed headers, stale timestamps, signature
    /// mismatch, or unknown schema-version major.
    pub fn verify(&self, signature_header: &str, body: &[u8]) -> Result<Event> {
        let mut timestamp: Option<i64> = None;
        let mut matched = false;
        for part in signature_header.split(',') {
            let Some((key, value)) = part.split_once('=') else { continue };
            match key.trim() {
                "t" => timestamp = value.trim().parse::<i64>().ok(),
                "v1" => {
                    let Some(ts) = timestamp else { continue };
                    if (chrono_now() - ts).abs() > self.tolerance_secs {
                        continue;
                    }
                    let expected = sign_payload_value(&self.secret, ts, body);
                    if constant_time_eq_str(&format!("t={ts},v1={}", value.trim()), &expected) {
                        matched = true;
                    }
                }
                _ => {}
            }
            if matched {
                break;
            }
        }
        if !matched {
            return Err(SdkError::Webhook("signature verification failed".into()));
        }
        let event: Event = serde_json::from_slice(body)
            .map_err(|error| SdkError::Webhook(format!("invalid envelope: {error}")))?;
        let known_major = event.schema_version.split('-').next().unwrap_or_default().to_string();
        let accepted = self
            .accepted_versions
            .iter()
            .any(|version| version.split('-').next() == Some(known_major.as_str()));
        if !accepted {
            return Err(SdkError::Webhook(format!(
                "unsupported schema_version `{}`",
                event.schema_version
            )));
        }
        Ok(event)
    }
}

fn sign_payload_value(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut signed = Vec::with_capacity(24 + body.len());
    signed.extend_from_slice(timestamp.to_string().as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(body);
    let digest = mac(secret.as_bytes(), &signed);
    format!("t={timestamp},v1={}", hex_lower(&digest))
}

fn constant_time_eq_str(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes().zip(right.bytes()).fold(0_u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// Current unix seconds (indirection keeps tests deterministic-friendly).
fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // Test assertions only.
    use super::*;

    #[test]
    fn signature_headers_are_deterministic() {
        let first = signature_headers("sk_live_a:b:c", "pk_live_x", 100, "GET", "/v1/x", b"");
        let second = signature_headers("sk_live_a:b:c", "pk_live_x", 100, "GET", "/v1/x", b"");
        assert_eq!(first, second);
        assert_eq!(first[2].1.len(), 3 + 64); // "v1=" + 64 hex chars
    }

    #[test]
    fn webhook_verifier_accepts_and_rejects() {
        let verifier = WebhookVerifier::new("whsec");
        let now = chrono_now();
        let header = sign_payload_value("whsec", now, br#"{"schema_version":"2026-08-22","id":"evt_1","type":"charge.settled","created":1,"tenant_id":"t","data":{}}"#);
        let body = br#"{"schema_version":"2026-08-22","id":"evt_1","type":"charge.settled","created":1,"tenant_id":"t","data":{}}"#;
        assert!(verifier.verify(&header, body).is_ok());
        assert!(WebhookVerifier::new("other").verify(&header, body).is_err());
        assert!(verifier.verify(&header, b"tampered").is_err());
        let stale = sign_payload_value("whsec", now - 4000, body);
        assert!(verifier.verify(&stale, body).is_err());
    }

    #[test]
    fn schema_major_gate_rejects_unknown() {
        let verifier = WebhookVerifier::new("whsec");
        let body = br#"{"schema_version":"2030-01-01","id":"evt_1","type":"x","created":1,"tenant_id":"t","data":{}}"#;
        let header = sign_payload_value("whsec", chrono_now(), body);
        assert!(verifier.verify(&header, body).is_err());
    }
}
