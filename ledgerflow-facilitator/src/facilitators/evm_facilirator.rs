//! EVM facilitator implementation (simplified version).

use std::collections::HashMap;

use async_trait::async_trait;
use tracing::{info, instrument};

use super::{Facilitator, PaymentError};
use crate::types::{
    EvmAddress, EvmPayload, ExactPaymentPayload, Network, PayToAddress, SettleRequest,
    SettleResponse, TransactionHash, VerifyRequest, VerifyResponse,
};

#[derive(Clone, Debug)]
pub struct EvmChain {
    pub network: Network,
    pub chain_id: u64,
}

impl EvmChain {
    pub fn new(network: Network, chain_id: u64) -> Self {
        Self { network, chain_id }
    }
}

impl TryFrom<Network> for EvmChain {
    type Error = PaymentError;

    fn try_from(value: Network) -> Result<Self, Self::Error> {
        match value {
            Network::BaseSepolia => Ok(EvmChain::new(value, 84532)),
            Network::Base => Ok(EvmChain::new(value, 8453)),
            Network::XdcMainnet => Ok(EvmChain::new(value, 50)),
            Network::AvalancheFuji => Ok(EvmChain::new(value, 43113)),
            Network::Avalanche => Ok(EvmChain::new(value, 43114)),
            _ => Err(PaymentError::UnsupportedNetwork(format!(
                "Network {value:?} not supported by EVM facilitator"
            ))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EvmProvider {
    chain: EvmChain,
}

impl EvmProvider {
    pub fn new(network: Network) -> Result<Self, PaymentError> {
        let chain = EvmChain::try_from(network)?;
        Ok(Self { chain })
    }

    pub fn network(&self) -> Network {
        self.chain.network
    }

    /// Validates an EVM payment payload against requirements (simplified version)
    #[instrument(skip_all, err)]
    async fn validate_payment(
        &self,
        request: &VerifyRequest,
    ) -> Result<ExactEvmPayment, PaymentError> {
        let payload = &request.payment_payload;
        let requirements = &request.payment_requirements;

        // Extract EVM payload
        let evm_payload = match &payload.payload {
            ExactPaymentPayload::Evm(payload) => payload,
            ExactPaymentPayload::Sui(_) => {
                return Err(PaymentError::UnsupportedNetwork(
                    "Sui payloads not supported by EVM facilitator".to_string(),
                ));
            }
        };

        let _payer = evm_payload.authorization.from;

        // Network validation
        if payload.network != self.network() {
            return Err(PaymentError::IncompatibleNetwork {
                expected: format!("{:?}", self.network()),
                actual: format!("{:?}", payload.network),
            });
        }

        // Scheme validation
        if payload.scheme != requirements.scheme {
            return Err(PaymentError::IncompatibleScheme {
                expected: format!("{:?}", requirements.scheme),
                actual: format!("{:?}", payload.scheme),
            });
        }

        // Receiver validation
        let payload_to = evm_payload.authorization.to;
        let requirements_to: EvmAddress = match &requirements.pay_to {
            PayToAddress::Evm(addr) => *addr,
            PayToAddress::Sui(_) => {
                return Err(PaymentError::InvalidAddress(
                    "Sui address not supported for EVM payments".to_string(),
                ));
            }
        };

        if payload_to != requirements_to {
            return Err(PaymentError::IncompatibleReceivers {
                expected: requirements_to.to_string(),
                actual: payload_to.to_string(),
            });
        }

        // Time validation
        self.validate_time(evm_payload)?;

        // Value validation (simplified - just check if value meets requirements)
        let value_sent = evm_payload.authorization.value.0;
        let amount_required = requirements.max_amount_required.0;
        if value_sent < amount_required {
            return Err(PaymentError::InsufficientAmount);
        }

        let payment = ExactEvmPayment {
            chain: self.chain.clone(),
            from: evm_payload.authorization.from,
            to: evm_payload.authorization.to,
            value: evm_payload.authorization.value,
            valid_after: evm_payload.authorization.valid_after,
            valid_before: evm_payload.authorization.valid_before,
            nonce: evm_payload.authorization.nonce,
            signature: evm_payload.signature.clone(),
        };

        Ok(payment)
    }

    /// Validates timing constraints
    fn validate_time(&self, payload: &EvmPayload) -> Result<(), PaymentError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| PaymentError::ClockError(format!("System time error: {e}")))?
            .as_secs();

        let valid_after = payload.authorization.valid_after;
        let valid_before = payload.authorization.valid_before;

        // Add 6 second grace buffer for expiration
        if valid_before < now + 6 {
            return Err(PaymentError::InvalidTiming(format!(
                "Expired: now {} > valid_before {}",
                now + 6,
                valid_before
            )));
        }

        if valid_after > now {
            return Err(PaymentError::InvalidTiming(format!(
                "Not active yet: valid_after {valid_after} > now {now}"
            )));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct EvmFacilitator {
    providers: HashMap<Network, EvmProvider>,
}

impl EvmFacilitator {
    pub async fn from_env() -> eyre::Result<Self> {
        // Initialize with default supported networks
        let mut providers = HashMap::new();

        let supported_networks = vec![
            Network::BaseSepolia,
            Network::Base,
            Network::XdcMainnet,
            Network::AvalancheFuji,
            Network::Avalanche,
        ];

        for network in supported_networks {
            if let Ok(provider) = EvmProvider::new(network) {
                providers.insert(network, provider);
            }
        }

        Ok(Self { providers })
    }

    pub fn add_provider(&mut self, network: Network, provider: EvmProvider) {
        self.providers.insert(network, provider);
    }

    fn get_provider(&self, network: Network) -> Result<&EvmProvider, PaymentError> {
        self.providers.get(&network).ok_or_else(|| {
            PaymentError::UnsupportedNetwork(format!("No provider for network {network:?}"))
        })
    }
}

#[async_trait]
impl Facilitator for EvmFacilitator {
    fn supported_networks(&self) -> Vec<Network> {
        self.providers.keys().cloned().collect()
    }

    async fn verify(&self, request: &VerifyRequest) -> Result<VerifyResponse, PaymentError> {
        let network = request.payment_payload.network;
        let provider = self.get_provider(network)?;

        let payment = provider.validate_payment(request).await?;

        // For now, just return valid if all checks pass
        // TODO: Add actual on-chain verification with proper RPC calls
        info!(
            "EVM payment verification passed for payer: {:?}",
            payment.from
        );

        Ok(VerifyResponse::valid(payment.from))
    }

    async fn settle(&self, request: &SettleRequest) -> Result<SettleResponse, PaymentError> {
        let network = request.payment_payload.network;
        let provider = self.get_provider(network)?;

        let payment = provider.validate_payment(request).await?;

        // For now, simulate successful settlement
        // TODO: Add actual on-chain settlement with proper transaction submission
        info!(
            "EVM payment settlement simulated for payer: {:?}",
            payment.from
        );

        // Simulate a transaction hash
        let fake_tx_hash = [0u8; 32];

        Ok(SettleResponse {
            success: true,
            error_reason: None,
            payer: payment.from.into(),
            transaction: Some(TransactionHash::Evm(fake_tx_hash)),
            network,
        })
    }
}

/// Represents an exact EVM payment ready for execution
#[derive(Clone, Debug)]
pub struct ExactEvmPayment {
    pub chain: EvmChain,
    pub from: EvmAddress,
    pub to: EvmAddress,
    pub value: crate::types::TokenAmount,
    pub valid_after: crate::types::UnixTimestamp,
    pub valid_before: crate::types::UnixTimestamp,
    pub nonce: crate::types::HexEncodedNonce,
    pub signature: crate::types::EvmSignature,
}
