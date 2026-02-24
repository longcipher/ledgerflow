use thiserror::Error;
use x402_types::proto;

use crate::adapters::{AdapterError, AdapterRegistry};

#[derive(Clone, Default)]
pub struct FacilitatorService {
    registry: AdapterRegistry,
}

impl FacilitatorService {
    pub fn new(registry: AdapterRegistry) -> Self {
        Self { registry }
    }

    pub async fn verify(
        &self,
        request: &proto::VerifyRequest,
    ) -> Result<proto::VerifyResponse, ServiceError> {
        let adapter = self.registry.resolve_by_request(request)?;
        Ok(adapter.verify(request).await?)
    }

    pub async fn settle(
        &self,
        request: &proto::SettleRequest,
    ) -> Result<proto::SettleResponse, ServiceError> {
        let adapter = self.registry.resolve_settle(request)?;
        Ok(adapter.settle(request).await?)
    }

    pub fn supported(&self) -> proto::SupportedResponse {
        self.registry.supported()
    }
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),
}
