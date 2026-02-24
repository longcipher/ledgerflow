use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;
use x402_types::{
    chain::{ChainId, ChainIdPattern},
    proto::{self, AsPaymentProblem, ErrorReason, PaymentProblem, SupportedPaymentKind},
    scheme::SchemeHandlerSlug,
};

pub mod offchain;

pub use offchain::{OffchainAdapter, OffchainAdapterConfig, OffchainBackendConfig};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdapterDescriptor {
    pub id: String,
    pub x402_version: u8,
    pub scheme: String,
    pub networks: Vec<ChainIdPattern>,
}

impl AdapterDescriptor {
    pub fn supports_slug(&self, slug: &SchemeHandlerSlug) -> bool {
        self.x402_version == slug.x402_version
            && self.scheme == slug.name
            && self
                .networks
                .iter()
                .any(|pattern| pattern.matches(&slug.chain_id))
    }

    pub fn supported_kinds(&self) -> Vec<SupportedPaymentKind> {
        self.networks
            .iter()
            .map(|pattern| SupportedPaymentKind {
                x402_version: self.x402_version,
                scheme: self.scheme.clone(),
                network: pattern.to_string(),
                extra: None,
            })
            .collect()
    }
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("{0}")]
    Verification(#[from] proto::PaymentVerificationError),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("upstream error: {0}")]
    Upstream(String),
}

impl AsPaymentProblem for AdapterError {
    fn as_payment_problem(&self) -> PaymentProblem {
        match self {
            AdapterError::Verification(err) => err.as_payment_problem(),
            AdapterError::InvalidRequest(details) => {
                PaymentProblem::new(ErrorReason::InvalidFormat, details.clone())
            }
            AdapterError::Upstream(details) => {
                PaymentProblem::new(ErrorReason::UnexpectedError, details.clone())
            }
        }
    }
}

#[async_trait]
pub trait PaymentAdapter: Send + Sync {
    fn descriptor(&self) -> &AdapterDescriptor;

    async fn verify(
        &self,
        request: &proto::VerifyRequest,
    ) -> Result<proto::VerifyResponse, AdapterError>;

    async fn settle(
        &self,
        request: &proto::SettleRequest,
    ) -> Result<proto::SettleResponse, AdapterError>;

    fn signer_hints(&self) -> HashMap<ChainId, Vec<String>> {
        HashMap::new()
    }
}

#[derive(Clone, Default)]
pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn PaymentAdapter>>,
}

impl AdapterRegistry {
    pub fn new(adapters: Vec<Arc<dyn PaymentAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn resolve_by_request(
        &self,
        request: &proto::VerifyRequest,
    ) -> Result<Arc<dyn PaymentAdapter>, AdapterError> {
        let slug = request.scheme_handler_slug().ok_or_else(|| {
            AdapterError::Verification(proto::PaymentVerificationError::InvalidFormat(
                "unable to detect x402 version/scheme/network".to_string(),
            ))
        })?;

        self.resolve_by_slug(&slug)
    }

    pub fn resolve_settle(
        &self,
        request: &proto::SettleRequest,
    ) -> Result<Arc<dyn PaymentAdapter>, AdapterError> {
        // settle request is an alias of verify request on the wire.
        self.resolve_by_request(request)
    }

    fn resolve_by_slug(
        &self,
        slug: &SchemeHandlerSlug,
    ) -> Result<Arc<dyn PaymentAdapter>, AdapterError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.descriptor().supports_slug(slug))
            .cloned()
            .ok_or(AdapterError::Verification(
                proto::PaymentVerificationError::UnsupportedScheme,
            ))
    }

    pub fn supported(&self) -> proto::SupportedResponse {
        let mut kinds = Vec::new();
        let mut signers: HashMap<ChainId, Vec<String>> = HashMap::new();

        for adapter in &self.adapters {
            kinds.extend(adapter.descriptor().supported_kinds());
            for (chain_id, signer_addresses) in adapter.signer_hints() {
                signers
                    .entry(chain_id)
                    .or_default()
                    .extend(signer_addresses);
            }
        }

        proto::SupportedResponse {
            kinds,
            extensions: Vec::new(),
            signers,
        }
    }
}
