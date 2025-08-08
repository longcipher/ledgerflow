use std::str::FromStr;

use axum::{extract::State, response::Json};
use base64::Engine;
use eyre::WrapErr;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{AppState, config::EvmX402Config, error::AppError};

// ===== Types matching x402 spec (reduced for exact/EVM) =====

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub max_amount_required: String,
    pub resource: String,
    pub description: String,
    pub mime_type: String,
    pub output_schema: Option<serde_json::Value>,
    pub pay_to: String,
    pub max_timeout_seconds: u32,
    pub asset: String,
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentPayloadExactEvm {
    pub signature: String,
    pub authorization: Authorization3009,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Authorization3009 {
    pub from: String,
    pub to: String,
    pub value: String,
    pub valid_after: String,
    pub valid_before: String,
    pub nonce: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentHeader {
    pub x402_version: u32,
    pub scheme: String,
    pub network: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    pub x402_version: u32,
    pub payment_header: String, // base64 json string
    pub payment_requirements: PaymentRequirements,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResponse {
    pub is_valid: bool,
    pub invalid_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    pub x402_version: u32,
    pub payment_header: String, // base64 json string
    pub payment_requirements: PaymentRequirements,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    pub success: bool,
    pub error: Option<String>,
    pub tx_hash: Option<String>,
    pub network_id: Option<String>,
}

// ===== Handlers =====

pub async fn supported(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let kinds = state
        .config
        .x402
        .as_ref()
        .map(|c| &c.kinds)
        .cloned()
        .unwrap_or_default();

    Ok(Json(serde_json::json!({"kinds": kinds})))
}

pub async fn verify(
    State(_state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, AppError> {
    let (_header, payload) = parse_header_exact_evm(&req.payment_header)?;

    // Basic checks
    // Check paymentRequirements alignment
    if payload.authorization.to.to_lowercase() != req.payment_requirements.pay_to.to_lowercase() {
        return Ok(Json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some("authorization.to != payTo".into()),
        }));
    }
    if req.payment_requirements.asset.to_lowercase() == payload.authorization.from.to_lowercase() {
        // logically cannot happen; keep placeholder for future checks
    }

    // Optionally check value vs maxAmountRequired
    // Parsing to avoid panics; treat overflow as invalid
    let Ok(value_u256) = alloy::primitives::U256::from_str_radix(&payload.authorization.value, 10)
    else {
        return Ok(Json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some("invalid value".into()),
        }));
    };
    let Ok(max_u256) =
        alloy::primitives::U256::from_str_radix(&req.payment_requirements.max_amount_required, 10)
    else {
        return Ok(Json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some("invalid maxAmountRequired".into()),
        }));
    };
    if value_u256 > max_u256 {
        return Ok(Json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some("value exceeds maxAmountRequired".into()),
        }));
    }

    // Basic time window check as strings (facilitator will rely on token validation in settle)
    let now_sec = chrono::Utc::now().timestamp() as u64;
    let Ok(valid_after) = payload.authorization.valid_after.parse::<u64>() else {
        return Ok(Json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some("invalid validAfter".into()),
        }));
    };
    let Ok(valid_before) = payload.authorization.valid_before.parse::<u64>() else {
        return Ok(Json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some("invalid validBefore".into()),
        }));
    };
    if now_sec < valid_after || now_sec > valid_before {
        return Ok(Json(VerifyResponse {
            is_valid: false,
            invalid_reason: Some("authorization not within valid window".into()),
        }));
    }

    Ok(Json(VerifyResponse {
        is_valid: true,
        invalid_reason: None,
    }))
}

pub async fn settle(
    State(state): State<AppState>,
    Json(req): Json<SettleRequest>,
) -> Result<Json<SettleResponse>, AppError> {
    // Parse header
    let (header, payload) = parse_header_exact_evm(&req.payment_header)?;

    // Resolve config
    let Some(x402cfg) = &state.config.x402 else {
        return Ok(Json(SettleResponse {
            success: false,
            error: Some("x402 not configured".into()),
            tx_hash: None,
            network_id: None,
        }));
    };
    let Some(evm) = &x402cfg.evm else {
        return Ok(Json(SettleResponse {
            success: false,
            error: Some("EVM not configured for x402".into()),
            tx_hash: None,
            network_id: None,
        }));
    };

    // Submit transaction via alloy signer
    match settle_evm_exact(&header, &payload, &req.payment_requirements, evm).await {
        Ok(tx) => Ok(Json(SettleResponse {
            success: true,
            error: None,
            tx_hash: Some(tx),
            network_id: Some(evm.chain_id.to_string()),
        })),
        Err(e) => {
            error!(error = %e, "x402 settle failed");
            Ok(Json(SettleResponse {
                success: false,
                error: Some(format!("{e}")),
                tx_hash: None,
                network_id: Some(evm.chain_id.to_string()),
            }))
        }
    }
}

// ===== Helpers =====

fn parse_header_exact_evm(
    b64_header: &str,
) -> Result<(PaymentHeader, PaymentPayloadExactEvm), AppError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64_header)
        .map_err(|e| AppError::InvalidInput(format!("invalid base64 header: {e}")))?;
    let header: PaymentHeader = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::InvalidInput(format!("invalid header json: {e}")))?;

    if header.scheme != "exact" {
        return Err(AppError::InvalidInput("only exact scheme supported".into()));
    }
    let payload: PaymentPayloadExactEvm = serde_json::from_value(header.payload.clone())
        .map_err(|e| AppError::InvalidInput(format!("invalid payload: {e}")))?;

    Ok((header, payload))
}

async fn settle_evm_exact(
    _header: &PaymentHeader,
    payload: &PaymentPayloadExactEvm,
    _reqs: &PaymentRequirements,
    evm: &EvmX402Config,
) -> eyre::Result<String> {
    use alloy::{
        network::TransactionBuilder,
        primitives::{Address, Bytes, FixedBytes, U256},
        providers::{Provider, ProviderBuilder},
        rpc::types::TransactionRequest,
        signers::local::PrivateKeySigner,
        sol,
        sol_types::SolCall,
    };

    // Parse inputs
    let from = Address::from_str(&payload.authorization.from)?;
    let to = Address::from_str(&payload.authorization.to)?;
    let value = U256::from_str_radix(&payload.authorization.value, 10)?;
    let valid_after: u64 = payload.authorization.valid_after.parse()?;
    let valid_before: u64 = payload.authorization.valid_before.parse()?;
    let nonce_hex = payload.authorization.nonce.trim_start_matches("0x");
    let nonce_bytes = hex::decode(nonce_hex)?;
    if nonce_bytes.len() != 32 {
        eyre::bail!("invalid nonce length: expected 32 bytes");
    }
    let nonce = FixedBytes::<32>::from_slice(&nonce_bytes);

    // Signature split (supports 0x... concatenated)
    let sig_hex = payload.signature.trim_start_matches("0x");
    let sig_bytes = hex::decode(sig_hex)?;
    if sig_bytes.len() != 65 {
        eyre::bail!("invalid signature length");
    }
    let r = FixedBytes::<32>::from_slice(&sig_bytes[0..32]);
    let s = FixedBytes::<32>::from_slice(&sig_bytes[32..64]);
    let mut v = sig_bytes[64];
    if v < 27 {
        v += 27;
    }

    // Vault ABI
    sol! {
        interface PaymentVault {
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

    // Provider + wallet
    let signer = PrivateKeySigner::from_str(evm.facilitator_private_key.as_str())?;
    let wallet = alloy::network::EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect(&evm.rpc_http)
        .await?;

    // Build call
    let order_id = nonce; // by design
    let call = PaymentVault::depositWithAuthorizationCall {
        orderId: order_id,
        from,
        value,
        validAfter: U256::from(valid_after),
        validBefore: U256::from(valid_before),
        nonce: order_id,
        v,
        r,
        s,
    };

    let vault_addr = Address::from_str(&evm.vault_address)?;

    // Build tx request (to vault, input = call data)
    let data: Bytes = call.abi_encode().into();
    let tx = TransactionRequest::default()
        .with_to(vault_addr)
        .with_input(data);

    // Optional: simulate
    let _ = provider
        .call(tx.clone())
        .await
        .wrap_err("simulation failed")?;

    // Send tx
    let pending = provider.send_transaction(tx).await?;
    let tx_hash = format!("0x{}", hex::encode(pending.tx_hash()));
    info!(%tx_hash, vault = %evm.vault_address, to = %to, value = %value, "sent x402 exact evm settlement");

    Ok(tx_hash)
}
