//! EIP-8004 reputation feedback emission (post-settlement).
//!
//! After a successful settlement the Facilitator can emit an EIP-8004-shaped
//! off-chain feedback artifact toward a pluggable [`FeedbackSink`]. The
//! merchant's on-chain identity rides in the leaf warrant extension key
//! `ledgerflow.agent_id` (an EIP-8004 reference such as
//! `eip155:1:0x8004…/22`); settlements without that claim are silently
//! skipped.
//!
//! The emitted document follows the EIP-8004 feedback-file convention:
//! `agentRegistry`, `agentId`, `clientAddress`, `createdAt`, `value`,
//! `valueDecimals`, `tag1`, and a `proofOfPayment` block carrying the
//! settlement transaction hash — giving reputation aggregators verifiable
//! proof that the rated interaction was a real, authorized payment.

use std::sync::Arc;

use ledgerflow_core::{
    VerifiedAuthorization,
    agent_identity::{AgentIdParseError, agent_id_from_warrant},
};
use serde::{Deserialize, Serialize};

use crate::rails::SettlementReceipt;

/// Fixed positive feedback value emitted for settled payments
/// (`value = 100`, `valueDecimals = 0`).
const SETTLED_FEEDBACK_VALUE: i64 = 100;

/// Tag identifying the feedback kind for aggregation filters.
const SETTLED_FEEDBACK_TAG: &str = "paymentSettled";

/// EIP-8004 `proofOfPayment` block proving a real paid interaction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProofOfPayment {
    /// Payer address (the presenting agent's payment subject).
    #[serde(rename = "fromAddress")]
    pub from_address: String,
    /// Payee address / identifier.
    #[serde(rename = "toAddress")]
    pub to_address: String,
    /// Chain id of the settlement transaction.
    #[serde(rename = "chainId")]
    pub chain_id: String,
    /// Settlement transaction hash.
    #[serde(rename = "txHash")]
    pub tx_hash: String,
}

/// An EIP-8004 off-chain feedback file describing one settled payment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementFeedback {
    /// `{namespace}:{chainId}:{identityRegistry}` coordinate of the agent.
    pub agent_registry: String,
    /// The rated agent's token id.
    pub agent_id: u64,
    /// Payer address (CAIP-10 prefix stripped when present).
    pub client_address: String,
    /// RFC 3339 UTC creation timestamp.
    pub created_at: String,
    /// Fixed-point feedback value.
    pub value: i64,
    /// Decimals of [`Self::value`].
    pub value_decimals: u8,
    /// Aggregation tag.
    pub tag1: String,
    /// Optional interaction endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Payment provenance.
    #[serde(rename = "proofOfPayment")]
    pub proof_of_payment: ProofOfPayment,
}

impl SettlementFeedback {
    /// Serializes to the canonical compact JSON form.
    ///
    /// # Errors
    /// Returns the serialization error message on failure (practically
    /// infallible).
    pub fn to_feedback_json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|error| error.to_string())
    }
}

/// Destination for emitted feedback artifacts.
pub trait FeedbackSink: Send + Sync {
    /// Delivers one feedback document.
    ///
    /// # Errors
    /// Implementations return the failure reason; reporting never propagates
    /// errors into settlement.
    fn submit(&self, feedback: &SettlementFeedback) -> Result<(), String>;
}

/// Sink that logs feedback documents via `tracing`.
#[derive(Clone, Debug, Default)]
pub struct LoggingSink;

impl FeedbackSink for LoggingSink {
    fn submit(&self, feedback: &SettlementFeedback) -> Result<(), String> {
        let json = feedback.to_feedback_json()?;
        tracing::info!(
            target: "ledgerflow::reputation",
            feedback = %json,
            "emitted EIP-8004 settlement feedback"
        );
        Ok(())
    }
}

/// Builds and dispatches EIP-8004 feedback artifacts after settlement.
#[derive(Clone)]
pub struct ReputationReporter {
    sink: Arc<dyn FeedbackSink>,
    enabled: bool,
}

impl std::fmt::Debug for ReputationReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The sink is intentionally opaque (dyn trait); only the gate shows.
        f.debug_struct("ReputationReporter")
            .field("enabled", &self.enabled)
            .field("sink", &"Arc<dyn FeedbackSink>")
            .finish()
    }
}

impl ReputationReporter {
    /// Creates a reporter bound to `sink`; emission is gated by `enabled`.
    #[must_use]
    pub const fn new(sink: Arc<dyn FeedbackSink>, enabled: bool) -> Self {
        Self { sink, enabled }
    }

    /// Builds and submits the feedback artifact for one settled payment.
    ///
    /// Skipped silently (debug-logged) when disabled or when the leaf warrant
    /// carries no parseable `ledgerflow.agent_id` extension. Sink failures are
    /// warn-logged; they never affect settlement outcomes.
    pub fn report_settlement(
        &self,
        authorization: &VerifiedAuthorization,
        receipt: &SettlementReceipt,
    ) {
        if !self.enabled {
            tracing::debug!(target: "ledgerflow::reputation", "reputation reporting disabled");
            return;
        }
        let agent_ref = match agent_id_from_warrant(&authorization.leaf_warrant) {
            Ok(Some(agent_ref)) => agent_ref,
            Ok(None) => {
                tracing::debug!(
                    target: "ledgerflow::reputation",
                    "warrant carries no agent identity; skipping feedback"
                );
                return;
            }
            Err(AgentIdParseError::Malformed(value)) => {
                tracing::warn!(
                    target: "ledgerflow::reputation",
                    value = %value,
                    "malformed agent identity reference"
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    target: "ledgerflow::reputation",
                    error = %error,
                    "invalid agent identity reference"
                );
                return;
            }
        };
        let client_address = strip_caip10_prefix(&authorization.payment_subject.value);
        let feedback = SettlementFeedback {
            agent_registry: agent_ref.agent_registry(),
            agent_id: agent_ref.agent_id,
            client_address: client_address.clone(),
            created_at: unix_secs_to_rfc3339(now_unix_secs()),
            value: SETTLED_FEEDBACK_VALUE,
            value_decimals: 0,
            tag1: SETTLED_FEEDBACK_TAG.to_string(),
            endpoint: None,
            proof_of_payment: ProofOfPayment {
                from_address: client_address,
                to_address: authorization.payee_id.clone(),
                chain_id: agent_ref.chain_id,
                tx_hash: receipt.transaction_id.clone(),
            },
        };
        if let Err(error) = self.sink.submit(&feedback) {
            tracing::warn!(
                target: "ledgerflow::reputation",
                error = %error,
                "feedback sink rejected the settlement feedback"
            );
        }
    }
}

/// Strips a leading `caip10:` scheme prefix when present.
fn strip_caip10_prefix(subject: &str) -> String {
    subject.strip_prefix("caip10:").map_or_else(|| subject.to_string(), str::to_string)
}

/// Current unix seconds (0 when the clock is before the epoch).
fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// Formats unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (civil-from-days).
fn unix_secs_to_rfc3339(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z",
        h = rem / 3_600,
        m = (rem % 3_600) / 60,
        s = rem % 60
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use std::sync::Mutex;

    use ledgerflow_core::{
        MerchantConstraint, PaymentConstraint, PaymentSubjectKind, PaymentSubjectRef,
        ResourceConstraint, SignerRef, SigningAlgorithm, SigningKeyPair, Warrant,
    };

    use super::*;
    use crate::RailKind;

    const AGENT_REF: &str = "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/22";

    struct CaptureSink(Mutex<Vec<SettlementFeedback>>);

    impl CaptureSink {
        fn new() -> Self {
            Self(Mutex::new(Vec::new()))
        }

        fn captured(&self) -> Vec<SettlementFeedback> {
            self.0.lock().expect("lock").clone()
        }
    }

    impl FeedbackSink for CaptureSink {
        fn submit(&self, feedback: &SettlementFeedback) -> Result<(), String> {
            self.0.lock().expect("lock").push(feedback.clone());
            Ok(())
        }
    }

    struct FailingSink;

    impl FeedbackSink for FailingSink {
        fn submit(&self, _feedback: &SettlementFeedback) -> Result<(), String> {
            Err("sink down".to_string())
        }
    }

    fn warrant(with_agent_ref: bool) -> Warrant {
        let issuer = SigningKeyPair::from_bytes(&[0x81; 32]);
        let holder = SigningKeyPair::from_bytes(&[0x82; 32]);
        let mut warrant = ledgerflow_core::WarrantBuilder::new(1_000)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
            .resource(ResourceConstraint::default())
            .payment(PaymentConstraint::new(1_000))
            .sign_with(&issuer, [0_u8; 8]);
        if with_agent_ref {
            warrant.extensions.insert(
                ledgerflow_core::agent_identity::AGENT_ID_EXTENSION_KEY.to_string(),
                AGENT_REF.as_bytes().to_vec(),
            );
        }
        warrant
    }

    fn authorization(with_agent_ref: bool) -> VerifiedAuthorization {
        let holder = SignerRef::new(SigningAlgorithm::Ed25519, vec![2_u8; 32]);
        VerifiedAuthorization {
            merchant_id: "merchant-a".to_string(),
            tool_name: "web-search".to_string(),
            payment_subject: PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            holder,
            leaf_warrant: warrant(with_agent_ref),
            root_warrant: warrant(false),
            chain_len: 1,
            amount: 100,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: ledgerflow_core::PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            warrant_digest: "sha256:w".to_string(),
        }
    }

    fn receipt() -> SettlementReceipt {
        SettlementReceipt {
            rail: RailKind::Evm,
            transaction_id: "0xtxhash".to_string(),
            settled_amount: 100,
            asset: "USDC".to_string(),
        }
    }

    #[test]
    fn reporter_emits_spec_shaped_feedback() {
        let sink = Arc::new(CaptureSink::new());
        let reporter = ReputationReporter::new(sink.clone(), true);
        reporter.report_settlement(&authorization(true), &receipt());
        let captured = sink.captured();
        assert_eq!(captured.len(), 1);
        let feedback = &captured[0];
        assert_eq!(feedback.agent_registry, "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432");
        assert_eq!(feedback.agent_id, 22);
        assert_eq!(feedback.client_address, "eip155:8453:0xabc123");
        assert_eq!(feedback.value, 100);
        assert_eq!(feedback.value_decimals, 0);
        assert_eq!(feedback.tag1, "paymentSettled");
        assert_eq!(feedback.proof_of_payment.tx_hash, "0xtxhash");
        assert_eq!(feedback.proof_of_payment.from_address, "eip155:8453:0xabc123");
        assert_eq!(feedback.proof_of_payment.to_address, "merchant-a");
        assert_eq!(feedback.proof_of_payment.chain_id, "1");

        // JSON uses exact EIP-8004 camelCase keys.
        let json = feedback.to_feedback_json().expect("json");
        assert!(json.contains("\"agentRegistry\":\"eip155:1:0x8004a169"));
        assert!(json.contains("\"agentId\":22"));
        assert!(json.contains("\"valueDecimals\":0"));
        assert!(json.contains("\"proofOfPayment\":{\"fromAddress\":"));
        assert!(json.contains("\"txHash\":\"0xtxhash\""));
        assert!(!json.contains("endpoint"));
    }

    #[test]
    fn missing_agent_identity_skips_emission() {
        let sink = Arc::new(CaptureSink::new());
        let reporter = ReputationReporter::new(sink.clone(), true);
        reporter.report_settlement(&authorization(false), &receipt());
        assert!(sink.captured().is_empty());
    }

    #[test]
    fn disabled_reporter_never_calls_sink() {
        let sink = Arc::new(CaptureSink::new());
        let reporter = ReputationReporter::new(sink.clone(), false);
        reporter.report_settlement(&authorization(true), &receipt());
        assert!(sink.captured().is_empty());
    }

    #[test]
    fn sink_errors_are_swallowed() {
        let reporter = ReputationReporter::new(Arc::new(FailingSink), true);
        // Must not panic.
        reporter.report_settlement(&authorization(true), &receipt());
    }

    #[test]
    fn rfc3339_vectors() {
        assert_eq!(unix_secs_to_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_secs_to_rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn caip10_prefix_stripped_only_once() {
        assert_eq!(strip_caip10_prefix("caip10:eip155:1:0xa"), "eip155:1:0xa");
        assert_eq!(strip_caip10_prefix("eip155:1:0xa"), "eip155:1:0xa");
    }
}
