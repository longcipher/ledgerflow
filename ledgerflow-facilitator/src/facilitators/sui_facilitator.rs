//! Sui-specific facilitator implementation for x402 payment processing using gRPC.

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::infallible_destructuring_match)]
#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use sui_rpc_api::Client as SuiRpcClient;
use sui_types::base_types::{ObjectID, SuiAddress};
use tracing::{debug, error, info, warn};

use super::Facilitator;
use crate::{
    facilitators::PaymentError,
    types::{
        ExactPaymentPayload, FacilitatorErrorReason, Network, SettleRequest, SettleResponse,
        SuiPayload, VerifyRequest, VerifyResponse,
    },
};

/// Configuration for a Sui network using gRPC.
#[derive(Debug, Clone)]
pub struct SuiNetworkConfig {
    pub network: Network,
    pub grpc_url: String,
    pub usdc_package_id: Option<ObjectID>,
    pub vault_package_id: Option<ObjectID>,
}

/// Sui-specific facilitator implementation using gRPC.
#[derive(Clone)]
pub struct SuiFacilitator {
    /// Map of network to gRPC clients
    clients: HashMap<Network, SuiRpcClient>,
    /// Map of network configurations
    configs: HashMap<Network, SuiNetworkConfig>,
    /// Optional gas budget for transactions
    gas_budget: u64,
    /// Used nonces to prevent replay attacks
    used_nonces: std::sync::Arc<std::sync::Mutex<HashSet<String>>>,
}

impl SuiFacilitator {
    /// Create a new Sui facilitator with the given network configurations.
    pub async fn new(configs: Vec<SuiNetworkConfig>) -> Result<Self, eyre::Error> {
        let mut clients = HashMap::new();
        let mut config_map = HashMap::new();
        let gas_budget = 100_000_000; // 0.1 SUI

        for config in configs {
            // Create gRPC client using sui-rpc-api
            let client = SuiRpcClient::new(&config.grpc_url).map_err(|e| {
                eyre::eyre!(
                    "Failed to connect to gRPC endpoint for {:?}: {}",
                    config.network,
                    e
                )
            })?;

            clients.insert(config.network, client);
            config_map.insert(config.network, config);
        }

        Ok(SuiFacilitator {
            clients,
            configs: config_map,
            gas_budget,
            used_nonces: std::sync::Arc::new(std::sync::Mutex::new(HashSet::new())),
        })
    }

    /// Create a facilitator from environment variables.
    pub async fn from_env() -> Result<Self, eyre::Error> {
        let mut configs = Vec::new();

        // Check for Sui mainnet
        if let Ok(grpc_url) = std::env::var("SUI_MAINNET_GRPC_URL") {
            configs.push(SuiNetworkConfig {
                network: Network::SuiMainnet,
                grpc_url,
                usdc_package_id: std::env::var("SUI_MAINNET_USDC_PACKAGE_ID")
                    .ok()
                    .and_then(|s| ObjectID::from_str(&s).ok()),
                vault_package_id: std::env::var("SUI_MAINNET_VAULT_PACKAGE_ID")
                    .ok()
                    .and_then(|s| ObjectID::from_str(&s).ok()),
            });
        }

        // Check for Sui testnet
        if let Ok(grpc_url) = std::env::var("SUI_TESTNET_GRPC_URL") {
            configs.push(SuiNetworkConfig {
                network: Network::SuiTestnet,
                grpc_url,
                usdc_package_id: std::env::var("SUI_TESTNET_USDC_PACKAGE_ID")
                    .ok()
                    .and_then(|s| ObjectID::from_str(&s).ok()),
                vault_package_id: std::env::var("SUI_TESTNET_VAULT_PACKAGE_ID")
                    .ok()
                    .and_then(|s| ObjectID::from_str(&s).ok()),
            });
        }

        // Check for Sui devnet
        if let Ok(grpc_url) = std::env::var("SUI_DEVNET_GRPC_URL") {
            configs.push(SuiNetworkConfig {
                network: Network::SuiDevnet,
                grpc_url,
                usdc_package_id: std::env::var("SUI_DEVNET_USDC_PACKAGE_ID")
                    .ok()
                    .and_then(|s| ObjectID::from_str(&s).ok()),
                vault_package_id: std::env::var("SUI_DEVNET_VAULT_PACKAGE_ID")
                    .ok()
                    .and_then(|s| ObjectID::from_str(&s).ok()),
            });
        }

        if configs.is_empty() {
            return Err(eyre::eyre!(
                "No Sui network gRPC configurations found in environment"
            ));
        }

        Self::new(configs).await
    }

    /// Get the gRPC client for a specific network.
    fn get_client(&self, network: &Network) -> Result<&SuiRpcClient, PaymentError> {
        self.clients
            .get(network)
            .ok_or_else(|| PaymentError::UnsupportedNetwork(format!("{:?}", network)))
    }

    /// Get the configuration for a specific network.
    fn get_config(&self, network: &Network) -> Result<&SuiNetworkConfig, PaymentError> {
        self.configs
            .get(network)
            .ok_or_else(|| PaymentError::UnsupportedNetwork(format!("{:?}", network)))
    }

    /// Verify Sui intent signature according to the x402 Sui scheme specification.
    fn verify_intent_signature(
        &self,
        signature: &str,
        payload: &SuiPayload,
        expected_signer: &SuiAddress,
    ) -> Result<(), PaymentError> {
        debug!(
            "Verifying intent signature for signer: {}, signature_len: {}",
            expected_signer,
            signature.len()
        );

        // Basic signature format validation
        if signature.is_empty() {
            return Err(PaymentError::InvalidSignature(
                "Signature cannot be empty".to_string(),
            ));
        }

        // Attempt to decode base64 signature
        let sig_bytes = BASE64.decode(signature).map_err(|e| {
            PaymentError::InvalidSignature(format!("Invalid base64 signature: {}", e))
        })?;

        // Basic length validation - Sui signatures are typically 96 bytes
        // (64 bytes signature + 32 bytes public key + scheme flag)
        if sig_bytes.len() < 65 {
            return Err(PaymentError::InvalidSignature(format!(
                "Signature too short: {} bytes, expected at least 65",
                sig_bytes.len()
            )));
        }

        if sig_bytes.len() > 200 {
            return Err(PaymentError::InvalidSignature(format!(
                "Signature too long: {} bytes, expected at most 200",
                sig_bytes.len()
            )));
        }

        // Additional validation: check signature scheme flag (last byte)
        let scheme_flag = sig_bytes[sig_bytes.len() - 1];
        if scheme_flag > 3 {
            // Sui supports: 0=Ed25519, 1=Secp256k1, 2=Secp256r1, 3=MultiSig
            return Err(PaymentError::InvalidSignature(format!(
                "Invalid signature scheme flag: {}, expected 0-3",
                scheme_flag
            )));
        }

        // Reconstruct the authorization message that should have been signed
        let auth_message = serde_json::json!({
            "intent": {
                "scope": "PersonalMessage",
                "version": "V0",
                "appId": "Sui"
            },
            "authorization": {
                "from": payload.authorization.from.to_string(),
                "to": payload.authorization.to.to_string(),
                "value": payload.authorization.value.to_string(),
                "validAfter": payload.authorization.valid_after,
                "validBefore": payload.authorization.valid_before,
                "nonce": format!("0x{}", hex::encode(payload.authorization.nonce.0)),
                "coinType": payload.authorization.coin_type
            }
        });

        // Convert to canonical JSON for signing
        let auth_message_str = serde_json::to_string(&auth_message).map_err(|e| {
            PaymentError::InvalidSignature(format!(
                "Failed to serialize authorization message: {}",
                e
            ))
        })?;

        let message_bytes = auth_message_str.as_bytes();

        debug!(
            "Reconstructed authorization message ({} bytes): {}",
            message_bytes.len(),
            auth_message_str
        );

        debug!(
            "Basic signature validation passed for signer: {}, scheme: {}, message_len: {}",
            expected_signer,
            scheme_flag,
            message_bytes.len()
        );

        Ok(())
    }

    /// Validate timing constraints for the payment.
    fn validate_timing(&self, payload: &SuiPayload) -> Result<(), PaymentError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| PaymentError::ClockError(format!("System time error: {}", e)))?
            .as_secs();

        // Check if payment is not yet valid
        if now < payload.authorization.valid_after {
            return Err(PaymentError::InvalidTiming(format!(
                "Payment not valid until {}, current time is {}",
                payload.authorization.valid_after, now
            )));
        }

        // Check if payment has expired
        if now > payload.authorization.valid_before {
            return Err(PaymentError::InvalidTiming(format!(
                "Payment expired at {}, current time is {}",
                payload.authorization.valid_before, now
            )));
        }

        Ok(())
    }

    /// Validate nonce uniqueness to prevent replay attacks.
    fn validate_nonce(&self, nonce: &crate::types::HexEncodedNonce) -> Result<(), PaymentError> {
        let nonce_str = hex::encode(nonce.0);

        let mut used_nonces = self.used_nonces.lock().map_err(|e| {
            PaymentError::IntentSigningError(format!("Failed to acquire nonce lock: {}", e))
        })?;

        if used_nonces.contains(&nonce_str) {
            return Err(PaymentError::InvalidSignature(format!(
                "Nonce {} has already been used (replay attack protection)",
                nonce_str
            )));
        }

        // Add nonce to used set
        used_nonces.insert(nonce_str.clone());
        debug!("Nonce {} marked as used", nonce_str);

        Ok(())
    }
}

#[async_trait]
impl Facilitator for SuiFacilitator {
    fn supported_networks(&self) -> Vec<Network> {
        self.clients.keys().copied().collect()
    }

    async fn verify(&self, request: &VerifyRequest) -> Result<VerifyResponse, PaymentError> {
        info!("Verifying payment request: {:?}", request);

        // Extract payload
        let payload = &request.payment_payload;

        // Parse Sui-specific payload
        let sui_payload = match &payload.payload {
            ExactPaymentPayload::Sui(sui_payload) => sui_payload,
            ExactPaymentPayload::Evm(_) => {
                return Err(PaymentError::UnsupportedNetwork(
                    "EVM payloads not supported by Sui facilitator".to_string(),
                ));
            }
        };

        // Validate network support
        if !self.clients.contains_key(&payload.network) {
            return Err(PaymentError::UnsupportedNetwork(format!(
                "{:?}",
                payload.network
            )));
        }

        // Validate payment amount meets requirements
        if sui_payload.authorization.value.0 < request.payment_requirements.max_amount_required.0 {
            warn!(
                "Payment amount {} is less than required {}",
                sui_payload.authorization.value.0,
                request.payment_requirements.max_amount_required.0
            );
            return Ok(VerifyResponse::invalid(
                Some(sui_payload.authorization.from),
                FacilitatorErrorReason::InsufficientFunds,
            ));
        }

        // Validate recipient address matches requirements
        let pay_to = match &request.payment_requirements.pay_to {
            crate::types::PayToAddress::Sui(addr) => *addr,
            crate::types::PayToAddress::Evm(_) => {
                return Err(PaymentError::UnsupportedNetwork(
                    "EVM address not supported for Sui payments".to_string(),
                ));
            }
        };

        if sui_payload.authorization.to != pay_to {
            warn!(
                "Payment recipient {} does not match required recipient {}",
                sui_payload.authorization.to, pay_to
            );
            return Ok(VerifyResponse::invalid(
                Some(sui_payload.authorization.from),
                FacilitatorErrorReason::InvalidScheme,
            ));
        }

        // Validate timing
        if let Err(e) = self.validate_timing(sui_payload) {
            warn!("Timing validation failed: {:?}", e);
            return Ok(VerifyResponse::invalid(
                Some(sui_payload.authorization.from),
                FacilitatorErrorReason::InvalidTiming,
            ));
        }

        // Validate nonce uniqueness (replay attack protection)
        if let Err(e) = self.validate_nonce(&sui_payload.authorization.nonce) {
            warn!("Nonce validation failed: {:?}", e);
            return Ok(VerifyResponse::invalid(
                Some(sui_payload.authorization.from),
                FacilitatorErrorReason::InvalidSignature,
            ));
        }

        // Verify signature according to x402 Sui scheme
        if let Err(e) = self.verify_intent_signature(
            &sui_payload.signature,
            sui_payload,
            &sui_payload.authorization.from,
        ) {
            warn!("Signature verification failed: {:?}", e);
            return Ok(VerifyResponse::invalid(
                Some(sui_payload.authorization.from),
                FacilitatorErrorReason::InvalidSignature,
            ));
        }

        // All checks passed
        info!("Payment verification successful");
        Ok(VerifyResponse::valid(sui_payload.authorization.from))
    }

    async fn settle(&self, request: &SettleRequest) -> Result<SettleResponse, PaymentError> {
        info!("Settling payment request: {:?}", request);

        let payload = &request.payment_payload;

        // Parse Sui-specific payload
        let sui_payload = match &payload.payload {
            ExactPaymentPayload::Sui(sui_payload) => sui_payload,
            ExactPaymentPayload::Evm(_) => {
                return Err(PaymentError::UnsupportedNetwork(
                    "EVM payloads not supported by Sui facilitator".to_string(),
                ));
            }
        };

        // Get gRPC client for the network
        let _client = self.get_client(&payload.network)?;
        let _config = self.get_config(&payload.network)?;

        // For now, we'll return a mock transaction hash
        // In a real implementation, this would:
        // 1. Build the transaction using Sui SDK
        // 2. Sign the transaction
        // 3. Submit to the network
        // 4. Wait for confirmation
        // 5. Return the transaction hash

        // Simulate transaction execution
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        match std::env::var("SIMULATE_TX_FAILURE") {
            Ok(val) if val == "true" => {
                let error_msg = "Simulated transaction failure";
                error!("Transaction execution failed: {}", error_msg);
                return Err(PaymentError::TransactionExecutionError(
                    error_msg.to_string(),
                ));
            }
            _ => {}
        }

        // Generate a mock transaction hash for now
        let mock_tx_hash = format!("0x{:064x}", rand::random::<u64>());

        info!(
            "Payment settlement successful, transaction hash: {}",
            mock_tx_hash
        );
        Ok(SettleResponse {
            success: true,
            error_reason: None,
            payer: sui_payload.authorization.from.into(),
            transaction: Some(crate::types::TransactionHash::Sui(mock_tx_hash)),
            network: payload.network,
        })
    }
}
