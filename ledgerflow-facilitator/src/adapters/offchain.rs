use std::{collections::HashMap, env, time::Duration};

use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use x402_types::{
    chain::{ChainId, ChainIdPattern},
    proto::{self, v1, v2},
};

use super::{AdapterDescriptor, AdapterError, PaymentAdapter};

fn default_mock_payer() -> String {
    "cex:user:mock".to_string()
}

fn default_tx_prefix() -> String {
    "offchain-tx".to_string()
}

fn default_timeout_seconds() -> u64 {
    8
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffchainAdapterConfig {
    pub descriptor: AdapterDescriptor,
    pub backend: OffchainBackendConfig,
    #[serde(default)]
    pub signers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum OffchainBackendConfig {
    Mock {
        #[serde(default = "default_mock_payer")]
        payer: String,
        #[serde(default = "default_tx_prefix")]
        transaction_prefix: String,
    },
    Http {
        base_url: String,
        #[serde(default)]
        verify_path: Option<String>,
        #[serde(default)]
        settle_path: Option<String>,
        #[serde(default)]
        api_key_env: Option<String>,
        #[serde(default = "default_timeout_seconds")]
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone)]
struct VerifyOutcome {
    valid: bool,
    payer: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Clone)]
struct SettleOutcome {
    success: bool,
    payer: Option<String>,
    transaction: Option<String>,
    reason: Option<String>,
}

#[async_trait]
trait OffchainBackend: Send + Sync {
    async fn verify(
        &self,
        network: &ChainId,
        payment_payload: &Value,
        payment_requirements: &V2PaymentRequirements,
    ) -> Result<VerifyOutcome, AdapterError>;

    async fn settle(
        &self,
        network: &ChainId,
        payment_payload: &Value,
        payment_requirements: &V2PaymentRequirements,
    ) -> Result<SettleOutcome, AdapterError>;
}

pub struct OffchainAdapter {
    descriptor: AdapterDescriptor,
    backend: Box<dyn OffchainBackend>,
    signers: Vec<String>,
}

impl OffchainAdapter {
    pub fn try_new(config: OffchainAdapterConfig) -> Result<Self, AdapterError> {
        let backend: Box<dyn OffchainBackend> = match config.backend {
            OffchainBackendConfig::Mock {
                payer,
                transaction_prefix,
            } => Box::new(MockOffchainBackend {
                payer,
                transaction_prefix,
            }),
            OffchainBackendConfig::Http {
                base_url,
                verify_path,
                settle_path,
                api_key_env,
                timeout_seconds,
            } => Box::new(HttpOffchainBackend::new(
                base_url,
                verify_path.unwrap_or_else(|| "/verify".to_string()),
                settle_path.unwrap_or_else(|| "/settle".to_string()),
                api_key_env,
                timeout_seconds,
            )?),
        };

        Ok(Self {
            descriptor: config.descriptor,
            backend,
            signers: config.signers,
        })
    }

    fn parse_v2_verify(request: &proto::VerifyRequest) -> Result<V2VerifyRequest, AdapterError> {
        v2::VerifyRequest::try_from(request).map_err(AdapterError::Verification)
    }

    fn validate_consistency(request: &V2VerifyRequest) -> Result<(), &'static str> {
        if request.payment_payload.accepted != request.payment_requirements {
            return Err("accepted_requirements_mismatch");
        }

        if request.payment_requirements.max_timeout_seconds == 0 {
            return Err("invalid_timeout");
        }

        match request.payment_requirements.amount.parse::<u128>() {
            Ok(value) if value > 0 => {}
            _ => return Err("invalid_payment_amount"),
        }

        Ok(())
    }

    fn verify_invalid(reason: impl Into<String>) -> proto::VerifyResponse {
        v1::VerifyResponse::invalid(None::<String>, reason.into()).into()
    }

    fn settle_error(
        reason: impl Into<String>,
        network: impl Into<String>,
    ) -> proto::SettleResponse {
        v1::SettleResponse::Error {
            reason: reason.into(),
            network: network.into(),
        }
        .into()
    }
}

#[async_trait]
impl PaymentAdapter for OffchainAdapter {
    fn descriptor(&self) -> &AdapterDescriptor {
        &self.descriptor
    }

    async fn verify(
        &self,
        request: &proto::VerifyRequest,
    ) -> Result<proto::VerifyResponse, AdapterError> {
        let parsed = Self::parse_v2_verify(request)?;
        if let Err(reason) = Self::validate_consistency(&parsed) {
            return Ok(Self::verify_invalid(reason));
        }

        let network = &parsed.payment_requirements.network;
        let outcome = self
            .backend
            .verify(
                network,
                &parsed.payment_payload.payload,
                &parsed.payment_requirements,
            )
            .await?;

        if !outcome.valid {
            return Ok(Self::verify_invalid(
                outcome
                    .reason
                    .unwrap_or_else(|| "verification_failed".to_string()),
            ));
        }

        let payer = outcome
            .payer
            .unwrap_or_else(|| "offchain:payer:unknown".to_string());
        Ok(v1::VerifyResponse::valid(payer).into())
    }

    async fn settle(
        &self,
        request: &proto::SettleRequest,
    ) -> Result<proto::SettleResponse, AdapterError> {
        let parsed = Self::parse_v2_verify(request)?;
        if let Err(reason) = Self::validate_consistency(&parsed) {
            return Ok(Self::settle_error(
                reason,
                parsed.payment_requirements.network.to_string(),
            ));
        }

        let network = &parsed.payment_requirements.network;
        let outcome = self
            .backend
            .settle(
                network,
                &parsed.payment_payload.payload,
                &parsed.payment_requirements,
            )
            .await?;

        if !outcome.success {
            return Ok(Self::settle_error(
                outcome
                    .reason
                    .unwrap_or_else(|| "settlement_failed".to_string()),
                network.to_string(),
            ));
        }

        Ok(v1::SettleResponse::Success {
            payer: outcome
                .payer
                .unwrap_or_else(|| "offchain:payer:unknown".to_string()),
            transaction: outcome
                .transaction
                .unwrap_or_else(|| format!("offchain-{}", Uuid::new_v4())),
            network: network.to_string(),
        }
        .into())
    }

    fn signer_hints(&self) -> HashMap<ChainId, Vec<String>> {
        let mut hints = HashMap::new();
        for pattern in &self.descriptor.networks {
            if let ChainIdPattern::Exact {
                namespace,
                reference,
            } = pattern
            {
                hints.insert(
                    ChainId::new(namespace.clone(), reference.clone()),
                    self.signers.clone(),
                );
            }
        }
        hints
    }
}

struct MockOffchainBackend {
    payer: String,
    transaction_prefix: String,
}

#[async_trait]
impl OffchainBackend for MockOffchainBackend {
    async fn verify(
        &self,
        _network: &ChainId,
        payment_payload: &Value,
        _payment_requirements: &V2PaymentRequirements,
    ) -> Result<VerifyOutcome, AdapterError> {
        let has_signature = payment_payload
            .get("signature")
            .and_then(Value::as_str)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let has_intent = payment_payload
            .get("paymentIntentId")
            .and_then(Value::as_str)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        if has_signature || has_intent {
            Ok(VerifyOutcome {
                valid: true,
                payer: Some(self.payer.clone()),
                reason: None,
            })
        } else {
            Ok(VerifyOutcome {
                valid: false,
                payer: None,
                reason: Some("missing_signature_or_intent_id".to_string()),
            })
        }
    }

    async fn settle(
        &self,
        network: &ChainId,
        payment_payload: &Value,
        payment_requirements: &V2PaymentRequirements,
    ) -> Result<SettleOutcome, AdapterError> {
        let verification = self
            .verify(network, payment_payload, payment_requirements)
            .await?;

        if !verification.valid {
            return Ok(SettleOutcome {
                success: false,
                payer: None,
                transaction: None,
                reason: verification.reason,
            });
        }

        Ok(SettleOutcome {
            success: true,
            payer: Some(self.payer.clone()),
            transaction: Some(format!(
                "{}-{}",
                self.transaction_prefix,
                Uuid::new_v4().simple()
            )),
            reason: None,
        })
    }
}

struct HttpOffchainBackend {
    client: reqwest::Client,
    verify_url: String,
    settle_url: String,
}

impl HttpOffchainBackend {
    fn new(
        base_url: String,
        verify_path: String,
        settle_path: String,
        api_key_env: Option<String>,
        timeout_seconds: u64,
    ) -> Result<Self, AdapterError> {
        let verify_url = format!("{}{}", base_url.trim_end_matches('/'), verify_path);
        let settle_url = format!("{}{}", base_url.trim_end_matches('/'), settle_path);

        let mut headers = HeaderMap::new();
        if let Some(api_key_env) = api_key_env {
            let api_key = env::var(&api_key_env).map_err(|_| {
                AdapterError::InvalidRequest(format!(
                    "environment variable {api_key_env} is required for offchain http backend"
                ))
            })?;
            let header = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|err| {
                AdapterError::InvalidRequest(format!("invalid api key header: {err}"))
            })?;
            headers.insert(AUTHORIZATION, header);
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|err| AdapterError::InvalidRequest(format!("http client: {err}")))?;

        Ok(Self {
            client,
            verify_url,
            settle_url,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendRequest<'a> {
    network: String,
    payment_payload: &'a Value,
    payment_requirements: &'a V2PaymentRequirements,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendVerifyResponse {
    valid: bool,
    payer: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendSettleResponse {
    success: bool,
    payer: Option<String>,
    transaction: Option<String>,
    reason: Option<String>,
}

#[async_trait]
impl OffchainBackend for HttpOffchainBackend {
    async fn verify(
        &self,
        network: &ChainId,
        payment_payload: &Value,
        payment_requirements: &V2PaymentRequirements,
    ) -> Result<VerifyOutcome, AdapterError> {
        let response = self
            .client
            .post(&self.verify_url)
            .json(&BackendRequest {
                network: network.to_string(),
                payment_payload,
                payment_requirements,
            })
            .send()
            .await
            .map_err(|err| AdapterError::Upstream(format!("verify request failed: {err}")))?;

        if !response.status().is_success() {
            return Err(AdapterError::Upstream(format!(
                "verify endpoint returned status {}",
                response.status()
            )));
        }

        let payload: BackendVerifyResponse = response.json().await.map_err(|err| {
            AdapterError::Upstream(format!("verify response decode failed: {err}"))
        })?;

        Ok(VerifyOutcome {
            valid: payload.valid,
            payer: payload.payer,
            reason: payload.reason,
        })
    }

    async fn settle(
        &self,
        network: &ChainId,
        payment_payload: &Value,
        payment_requirements: &V2PaymentRequirements,
    ) -> Result<SettleOutcome, AdapterError> {
        let response = self
            .client
            .post(&self.settle_url)
            .json(&BackendRequest {
                network: network.to_string(),
                payment_payload,
                payment_requirements,
            })
            .send()
            .await
            .map_err(|err| AdapterError::Upstream(format!("settle request failed: {err}")))?;

        if !response.status().is_success() {
            return Err(AdapterError::Upstream(format!(
                "settle endpoint returned status {}",
                response.status()
            )));
        }

        let payload: BackendSettleResponse = response.json().await.map_err(|err| {
            AdapterError::Upstream(format!("settle response decode failed: {err}"))
        })?;

        Ok(SettleOutcome {
            success: payload.success,
            payer: payload.payer,
            transaction: payload.transaction,
            reason: payload.reason,
        })
    }
}

type V2PaymentRequirements = v2::PaymentRequirements<String, String, String, Option<Value>>;
type V2PaymentPayload = v2::PaymentPayload<V2PaymentRequirements, Value>;
type V2VerifyRequest = v2::VerifyRequest<V2PaymentPayload, V2PaymentRequirements>;
