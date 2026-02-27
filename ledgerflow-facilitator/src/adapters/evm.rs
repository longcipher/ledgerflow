#![allow(clippy::too_many_arguments)]

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use alloy::{
    network::EthereumWallet,
    primitives::{Address, Bytes, FixedBytes, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{info, instrument, warn};
use x402_types::{
    chain::{ChainId, ChainIdPattern},
    proto::{self, v1, v2},
};

use super::{AdapterDescriptor, AdapterError, PaymentAdapter};

// ERC-3009 interface for `transferWithAuthorization` + balance/nonce checks.
sol! {
    #[sol(rpc)]
    interface IERC3009 {
        function transferWithAuthorization(
            address from,
            address to,
            uint256 value,
            uint256 validAfter,
            uint256 validBefore,
            bytes32 nonce,
            uint8 v,
            bytes32 r,
            bytes32 s
        ) external;

        function balanceOf(address account) external view returns (uint256);

        function authorizationState(
            address authorizer,
            bytes32 nonce
        ) external view returns (bool);
    }
}

// PaymentVault wrapper used for settlement to preserve DepositReceived linkage.
sol! {
    #[sol(rpc)]
    interface IPaymentVault {
        function depositWithAuthorization(
            bytes32 orderId,
            address from,
            uint256 value,
            uint256 validAfter,
            uint256 validBefore,
            bytes32 nonce,
            uint8 v,
            bytes32 r,
            bytes32 s
        ) external;
    }
}

// ── Payload types ────────────────────────────────────────────────────

/// Parsed EIP-3009 authorization from the payment payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Eip3009Authorization {
    from: String,
    to: String,
    value: String,
    valid_after: String,
    valid_before: String,
    nonce: String,
}

/// Parsed EVM payload from the x402 payment.
#[derive(Debug, Clone, Deserialize)]
struct EvmPayload {
    signature: String,
    authorization: Eip3009Authorization,
}

// ── Adapter config ───────────────────────────────────────────────────

/// Runtime configuration for the EVM adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmAdapterConfig {
    pub descriptor: AdapterDescriptor,
    /// JSON-RPC endpoint URL (e.g. `https://sepolia.base.org`).
    pub rpc_url: String,
    /// Numeric chain ID (e.g. `84532` for Base Sepolia).
    pub chain_id: u64,
    /// Settlement vault address. Required for settlement.
    pub vault_address: Option<String>,
    /// Hex-encoded private key for settlement (optional; verify-only if absent).
    pub signer_key: Option<String>,
    /// Facilitator signer addresses reported in `/supported`.
    pub signers: Vec<String>,
}

// ── Adapter ──────────────────────────────────────────────────────────

/// EVM on-chain adapter implementing EIP-3009 `transferWithAuthorization`.
///
/// **Verify** — validates authorization parameters (timing, amounts, addresses),
/// checks on-chain balance and nonce state, and simulates the transfer via
/// `eth_call`.
///
/// **Settle** — sends the `transferWithAuthorization` transaction on-chain
/// (requires a `signer_key`).
pub struct EvmAdapter {
    descriptor: AdapterDescriptor,
    rpc_url: String,
    chain_id: u64,
    vault_address: Option<String>,
    signer_key: Option<String>,
    signers: Vec<String>,
}

impl EvmAdapter {
    /// Create a new EVM adapter.
    ///
    /// Validates the RPC URL format but does **not** open a connection yet;
    /// providers are created lazily per request.
    pub fn try_new(config: EvmAdapterConfig) -> Result<Self, AdapterError> {
        // Validate RPC URL format eagerly.
        let _: url::Url = config.rpc_url.parse().map_err(|e| {
            AdapterError::InvalidRequest(format!("invalid RPC URL '{}': {e}", config.rpc_url))
        })?;

        if let Some(vault_address) = config.vault_address.as_ref() {
            let _: Address = vault_address.parse().map_err(|e| {
                AdapterError::InvalidRequest(format!(
                    "invalid vault_address '{vault_address}': {e}",
                ))
            })?;
        }

        Ok(Self {
            descriptor: config.descriptor,
            rpc_url: config.rpc_url,
            chain_id: config.chain_id,
            vault_address: config.vault_address,
            signer_key: config.signer_key,
            signers: config.signers,
        })
    }

    // ── Parsing helpers ──────────────────────────────────────────────

    fn parse_payload(payload_value: &Value) -> Result<EvmPayload, AdapterError> {
        serde_json::from_value(payload_value.clone())
            .map_err(|e| AdapterError::InvalidRequest(format!("invalid EVM payload: {e}")))
    }

    fn parse_address(s: &str) -> Result<Address, AdapterError> {
        s.parse::<Address>()
            .map_err(|e| AdapterError::InvalidRequest(format!("invalid address '{s}': {e}")))
    }

    fn parse_u256(s: &str) -> Result<U256, AdapterError> {
        if let Some(hex_str) = s.strip_prefix("0x") {
            U256::from_str_radix(hex_str, 16)
                .map_err(|e| AdapterError::InvalidRequest(format!("invalid hex value '{s}': {e}")))
        } else {
            s.parse::<U256>()
                .map_err(|e| AdapterError::InvalidRequest(format!("invalid value '{s}': {e}")))
        }
    }

    fn parse_nonce(s: &str) -> Result<FixedBytes<32>, AdapterError> {
        let hex_str = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(hex_str)
            .map_err(|e| AdapterError::InvalidRequest(format!("invalid nonce hex '{s}': {e}")))?;
        if bytes.len() != 32 {
            return Err(AdapterError::InvalidRequest(format!(
                "nonce must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        Ok(FixedBytes::from_slice(&bytes))
    }

    fn parse_signature(s: &str) -> Result<Bytes, AdapterError> {
        let hex_str = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(hex_str)
            .map_err(|e| AdapterError::InvalidRequest(format!("invalid signature hex: {e}")))?;
        Ok(Bytes::from(bytes))
    }

    fn parse_signature_vrs(
        signature: &Bytes,
    ) -> Result<(u8, FixedBytes<32>, FixedBytes<32>), AdapterError> {
        let bytes = signature.as_ref();
        if bytes.len() != 65 {
            return Err(AdapterError::InvalidRequest(format!(
                "invalid signature length: expected 65 bytes, got {}",
                bytes.len()
            )));
        }

        let r = FixedBytes::from_slice(&bytes[0..32]);
        let s = FixedBytes::from_slice(&bytes[32..64]);
        let mut v = bytes[64];
        if v < 27 {
            v += 27;
        }
        Ok((v, r, s))
    }

    // ── Validation helpers ───────────────────────────────────────────

    /// Validate that the current timestamp is within the `validAfter..validBefore`
    /// window, with a 6-second grace buffer for network latency (matching the
    /// x402-rs reference implementation).
    fn assert_time(valid_after: u64, valid_before: u64) -> Result<(), AdapterError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AdapterError::Upstream(format!("system clock error: {e}")))?
            .as_secs();

        if valid_before < now.saturating_add(6) {
            return Err(AdapterError::Verification(
                proto::PaymentVerificationError::Expired,
            ));
        }

        if valid_after > now {
            return Err(AdapterError::Verification(
                proto::PaymentVerificationError::Early,
            ));
        }

        Ok(())
    }

    fn verify_invalid(reason: impl Into<String>) -> proto::VerifyResponse {
        v1::VerifyResponse::invalid(None::<String>, reason.into()).into()
    }

    fn assert_scheme_exact(requirements: &V2PaymentRequirements) -> Result<(), AdapterError> {
        if !requirements.scheme.eq_ignore_ascii_case("exact") {
            return Err(AdapterError::InvalidRequest(format!(
                "unsupported scheme '{}', expected 'exact'",
                requirements.scheme
            )));
        }
        Ok(())
    }

    fn assert_asset_transfer_method(extra: Option<&Value>) -> Result<(), AdapterError> {
        if let Some(method) = extra
            .and_then(|value| value.get("assetTransferMethod"))
            .and_then(Value::as_str)
        {
            let method = method.to_ascii_lowercase();
            if method != "eip3009" {
                return Err(AdapterError::InvalidRequest(format!(
                    "unsupported assetTransferMethod '{method}', expected 'eip3009'"
                )));
            }
        }
        Ok(())
    }

    fn assert_requirements_network(&self, network: &ChainId) -> Result<(), AdapterError> {
        if network.namespace() != "eip155" {
            return Err(AdapterError::Verification(
                proto::PaymentVerificationError::UnsupportedChain,
            ));
        }

        let requested_chain_id = network.reference().parse::<u64>().map_err(|_| {
            AdapterError::Verification(proto::PaymentVerificationError::ChainIdMismatch)
        })?;

        if requested_chain_id != self.chain_id {
            return Err(AdapterError::Verification(
                proto::PaymentVerificationError::ChainIdMismatch,
            ));
        }

        Ok(())
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
impl PaymentAdapter for EvmAdapter {
    fn descriptor(&self) -> &AdapterDescriptor {
        &self.descriptor
    }

    #[instrument(skip_all, fields(
        adapter_id = %self.descriptor.id,
        chain_id = %self.chain_id
    ))]
    async fn verify(
        &self,
        request: &proto::VerifyRequest,
    ) -> Result<proto::VerifyResponse, AdapterError> {
        let parsed = V2VerifyRequest::try_from(request).map_err(AdapterError::Verification)?;

        // Validate accepted == requirements
        if parsed.payment_payload.accepted != parsed.payment_requirements {
            return Ok(Self::verify_invalid("accepted_requirements_mismatch"));
        }

        let requirements = &parsed.payment_requirements;
        Self::assert_scheme_exact(requirements)?;
        self.assert_requirements_network(&requirements.network)?;
        Self::assert_asset_transfer_method(requirements.extra.as_ref())?;
        let payload_value = &parsed.payment_payload.payload;

        // Parse EVM-specific payload
        let evm_payload = Self::parse_payload(payload_value)?;
        let auth = &evm_payload.authorization;

        let from = Self::parse_address(&auth.from)?;
        let to = Self::parse_address(&auth.to)?;
        let value = Self::parse_u256(&auth.value)?;
        let valid_after = Self::parse_u256(&auth.valid_after)?;
        let valid_before = Self::parse_u256(&auth.valid_before)?;
        let nonce = Self::parse_nonce(&auth.nonce)?;
        let signature = Self::parse_signature(&evm_payload.signature)?;
        let (v, r, s) = Self::parse_signature_vrs(&signature)?;
        let _ = Self::parse_address(&requirements.asset)?;

        // Validate receiver matches payTo
        let pay_to = Self::parse_address(&requirements.pay_to)?;
        if to != pay_to {
            return Ok(Self::verify_invalid(format!(
                "receiver mismatch: authorization.to={to} != payTo={pay_to}"
            )));
        }

        // Validate exact amount
        let required_amount = Self::parse_u256(&requirements.amount)?;
        if value != required_amount {
            return Ok(Self::verify_invalid(format!(
                "amount mismatch: authorization.value={value} != amount={required_amount}"
            )));
        }

        // Validate timing
        let va: u64 = valid_after
            .try_into()
            .map_err(|_| AdapterError::InvalidRequest("validAfter overflow".to_string()))?;
        let vb: u64 = valid_before
            .try_into()
            .map_err(|_| AdapterError::InvalidRequest("validBefore overflow".to_string()))?;
        Self::assert_time(va, vb)?;

        // Connect to RPC for on-chain checks
        let provider = ProviderBuilder::new()
            .connect(&self.rpc_url)
            .await
            .map_err(|e| AdapterError::Upstream(format!("RPC connection failed: {e}")))?;

        let rpc_chain_id = provider
            .get_chain_id()
            .await
            .map_err(|e| AdapterError::Upstream(format!("chainId call failed: {e}")))?;
        if rpc_chain_id != self.chain_id {
            return Err(AdapterError::Verification(
                proto::PaymentVerificationError::ChainIdMismatch,
            ));
        }

        let asset_address = Self::parse_address(&requirements.asset)?;
        let token = IERC3009::new(asset_address, &provider);

        // Check on-chain balance
        let balance = token
            .balanceOf(from)
            .call()
            .await
            .map_err(|e| AdapterError::Upstream(format!("balanceOf call failed: {e}")))?;

        if balance < required_amount {
            return Ok(Self::verify_invalid(format!(
                "insufficient balance: {balance} < required {required_amount}",
            )));
        }

        // Check authorization nonce has not been used
        let nonce_used: bool = token
            .authorizationState(from, nonce)
            .call()
            .await
            .map_err(|e| AdapterError::Upstream(format!("authorizationState call failed: {e}")))?;

        if nonce_used {
            return Ok(Self::verify_invalid("authorization nonce already used"));
        }

        if let Some(vault_address) = &self.vault_address {
            let vault = IPaymentVault::new(Self::parse_address(vault_address)?, &provider);
            let order_id = nonce;

            vault
                .depositWithAuthorization(
                    order_id,
                    from,
                    value,
                    valid_after,
                    valid_before,
                    order_id,
                    v,
                    r,
                    s,
                )
                .call()
                .await
                .map_err(|e| {
                    warn!(error = %e, "depositWithAuthorization simulation failed");
                    AdapterError::Verification(
                        proto::PaymentVerificationError::TransactionSimulation(format!(
                            "depositWithAuthorization simulation failed: {e}"
                        )),
                    )
                })?;
        } else {
            // Fallback for legacy deployments without vault wrapper.
            token
                .transferWithAuthorization(
                    from,
                    to,
                    value,
                    valid_after,
                    valid_before,
                    nonce,
                    v,
                    r,
                    s,
                )
                .call()
                .await
                .map_err(|e| {
                    warn!(error = %e, "transferWithAuthorization simulation failed");
                    AdapterError::Verification(
                        proto::PaymentVerificationError::TransactionSimulation(format!(
                            "transferWithAuthorization simulation failed: {e}"
                        )),
                    )
                })?;
        }

        info!(payer = %from, "EVM payment verified");
        Ok(v1::VerifyResponse::valid(format!("{from}")).into())
    }

    #[instrument(skip_all, fields(
        adapter_id = %self.descriptor.id,
        chain_id = %self.chain_id
    ))]
    async fn settle(
        &self,
        request: &proto::SettleRequest,
    ) -> Result<proto::SettleResponse, AdapterError> {
        let parsed = V2VerifyRequest::try_from(request).map_err(AdapterError::Verification)?;

        if parsed.payment_payload.accepted != parsed.payment_requirements {
            return Ok(Self::settle_error(
                "accepted_requirements_mismatch",
                parsed.payment_requirements.network.to_string(),
            ));
        }

        let requirements = &parsed.payment_requirements;
        Self::assert_scheme_exact(requirements)?;
        self.assert_requirements_network(&requirements.network)?;
        Self::assert_asset_transfer_method(requirements.extra.as_ref())?;
        let network = requirements.network.to_string();
        let payload_value = &parsed.payment_payload.payload;

        // Parse EVM-specific payload
        let evm_payload = Self::parse_payload(payload_value)?;
        let auth = &evm_payload.authorization;

        let from = Self::parse_address(&auth.from)?;
        let to = Self::parse_address(&auth.to)?;
        let value = Self::parse_u256(&auth.value)?;
        let valid_after = Self::parse_u256(&auth.valid_after)?;
        let valid_before = Self::parse_u256(&auth.valid_before)?;
        let nonce = Self::parse_nonce(&auth.nonce)?;
        let signature = Self::parse_signature(&evm_payload.signature)?;
        let _ = Self::parse_address(&requirements.asset)?;

        // Validate receiver
        let pay_to = Self::parse_address(&requirements.pay_to)?;
        if to != pay_to {
            return Ok(Self::settle_error(
                format!("receiver mismatch: authorization.to={to} != payTo={pay_to}"),
                &network,
            ));
        }

        // Validate exact amount
        let required_amount = Self::parse_u256(&requirements.amount)?;
        if value != required_amount {
            return Ok(Self::settle_error(
                format!("amount mismatch: authorization.value={value} != amount={required_amount}"),
                &network,
            ));
        }

        // Validate timing
        let va: u64 = valid_after
            .try_into()
            .map_err(|_| AdapterError::InvalidRequest("validAfter overflow".to_string()))?;
        let vb: u64 = valid_before
            .try_into()
            .map_err(|_| AdapterError::InvalidRequest("validBefore overflow".to_string()))?;
        Self::assert_time(va, vb)?;

        // Create wallet-backed provider for settlement
        let signer_key = self.signer_key.as_ref().ok_or_else(|| {
            AdapterError::InvalidRequest(
                "settlement requires a signer key; configure signer_key_env".to_string(),
            )
        })?;

        let signer: PrivateKeySigner = signer_key
            .parse()
            .map_err(|e| AdapterError::InvalidRequest(format!("invalid signer key: {e}")))?;
        let wallet = EthereumWallet::from(signer);

        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect(&self.rpc_url)
            .await
            .map_err(|e| AdapterError::Upstream(format!("RPC connection failed: {e}")))?;

        let rpc_chain_id = provider
            .get_chain_id()
            .await
            .map_err(|e| AdapterError::Upstream(format!("chainId call failed: {e}")))?;
        if rpc_chain_id != self.chain_id {
            return Err(AdapterError::Verification(
                proto::PaymentVerificationError::ChainIdMismatch,
            ));
        }

        let vault_address = self.vault_address.as_ref().ok_or_else(|| {
            AdapterError::InvalidRequest(
                "settlement requires vault_address; configure vault_address in adapter config"
                    .to_string(),
            )
        })?;
        let vault = IPaymentVault::new(Self::parse_address(vault_address)?, &provider);
        let (v, r, s) = Self::parse_signature_vrs(&signature)?;
        let order_id = nonce;

        // Send depositWithAuthorization on-chain through PaymentVault wrapper.
        let pending = vault
            .depositWithAuthorization(
                order_id,
                from,
                value,
                valid_after,
                valid_before,
                order_id,
                v,
                r,
                s,
            )
            .send()
            .await
            .map_err(|e| {
                warn!(error = %e, "depositWithAuthorization send failed");
                AdapterError::Upstream(format!("settlement transaction failed: {e}"))
            })?;

        let receipt = pending.get_receipt().await.map_err(|e| {
            AdapterError::Upstream(format!("failed to get transaction receipt: {e}"))
        })?;

        if receipt.status() {
            info!(
                payer = %from,
                tx_hash = %receipt.transaction_hash,
                "EVM settlement succeeded"
            );
            Ok(v1::SettleResponse::Success {
                payer: format!("{from}"),
                transaction: format!("{}", receipt.transaction_hash),
                network,
            }
            .into())
        } else {
            warn!(
                tx_hash = %receipt.transaction_hash,
                "EVM settlement reverted"
            );
            Ok(v1::SettleResponse::Error {
                reason: format!("transaction reverted: {}", receipt.transaction_hash),
                network,
            }
            .into())
        }
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

    fn protocol_extensions(&self) -> Vec<String> {
        vec!["exact-eip3009".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_adapter(chain_id: u64) -> EvmAdapter {
        EvmAdapter::try_new(EvmAdapterConfig {
            descriptor: AdapterDescriptor {
                id: "unit-test-evm".to_string(),
                x402_version: 2,
                scheme: "exact".to_string(),
                networks: vec!["eip155:*".parse().expect("valid pattern")],
            },
            rpc_url: "http://127.0.0.1:8545".to_string(),
            chain_id,
            vault_address: Some("0x00000000000000000000000000000000000000F1".to_string()),
            signer_key: None,
            signers: vec![],
        })
        .expect("build test adapter")
    }

    #[test]
    fn parse_nonce_requires_exactly_32_bytes() {
        let error = EvmAdapter::parse_nonce("0xdeadbeef").expect_err("nonce should be rejected");
        assert!(matches!(error, AdapterError::InvalidRequest(_)));
    }

    #[test]
    fn assert_requirements_network_rejects_non_eip155_namespace() {
        let adapter = test_adapter(84532);
        let network: ChainId = "solana:mainnet".parse().expect("valid chain id");

        let error = adapter
            .assert_requirements_network(&network)
            .expect_err("network should be rejected");
        assert!(matches!(
            error,
            AdapterError::Verification(proto::PaymentVerificationError::UnsupportedChain)
        ));
    }

    #[test]
    fn assert_requirements_network_rejects_chain_id_mismatch() {
        let adapter = test_adapter(84532);
        let network: ChainId = "eip155:1".parse().expect("valid chain id");

        let error = adapter
            .assert_requirements_network(&network)
            .expect_err("network should be rejected");
        assert!(matches!(
            error,
            AdapterError::Verification(proto::PaymentVerificationError::ChainIdMismatch)
        ));
    }

    #[test]
    fn parse_signature_vrs_requires_65_bytes() {
        let signature = Bytes::from(vec![0_u8; 64]);
        let error = EvmAdapter::parse_signature_vrs(&signature)
            .expect_err("invalid signature length should fail");
        assert!(matches!(error, AdapterError::InvalidRequest(_)));
    }

    #[test]
    fn assert_asset_transfer_method_rejects_non_eip3009() {
        let extra = serde_json::json!({"assetTransferMethod":"permit2"});
        let error = EvmAdapter::assert_asset_transfer_method(Some(&extra))
            .expect_err("unsupported method must fail");
        assert!(matches!(error, AdapterError::InvalidRequest(_)));
    }

    #[test]
    fn assert_scheme_exact_rejects_non_exact() {
        let requirements = V2PaymentRequirements {
            scheme: "permit".to_string(),
            network: "eip155:84532".parse().expect("valid chain id"),
            amount: "1".to_string(),
            pay_to: "0x0000000000000000000000000000000000000001".to_string(),
            max_timeout_seconds: 60,
            asset: "0x0000000000000000000000000000000000000010".to_string(),
            extra: None,
        };

        let error = EvmAdapter::assert_scheme_exact(&requirements)
            .expect_err("non-exact scheme should fail");
        assert!(matches!(error, AdapterError::InvalidRequest(_)));
    }

    #[test]
    fn assert_time_rejects_expired_authorization() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_secs();

        let error = EvmAdapter::assert_time(now.saturating_sub(60), now.saturating_sub(1))
            .expect_err("expired authorization must fail");
        assert!(matches!(
            error,
            AdapterError::Verification(proto::PaymentVerificationError::Expired)
        ));
    }

    #[test]
    fn assert_time_rejects_future_authorization() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_secs();

        let error = EvmAdapter::assert_time(now + 60, now + 120)
            .expect_err("future authorization must fail");
        assert!(matches!(
            error,
            AdapterError::Verification(proto::PaymentVerificationError::Early)
        ));
    }
}

// ── v2 type aliases ──────────────────────────────────────────────────

type V2PaymentRequirements = v2::PaymentRequirements<String, String, String, Option<Value>>;
type V2PaymentPayload = v2::PaymentPayload<V2PaymentRequirements, Value>;
type V2VerifyRequest = v2::VerifyRequest<V2PaymentPayload, V2PaymentRequirements>;
