use std::{
    collections::HashMap,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{extract::State, response::Json};
use eyre::WrapErr;
use serde::Deserialize;
use serde_json::Value;
use tracing::{error, info};
use x402_types::{
    chain::ChainId,
    proto::{self, SupportedPaymentKind, v1, v2},
};

use crate::{AppState, config::EvmX402Config, error::AppError};

#[derive(Debug, Deserialize)]
struct PaymentPayloadExactEvm {
    signature: String,
    authorization: Authorization3009,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Authorization3009 {
    from: String,
    to: String,
    value: String,
    valid_after: String,
    valid_before: String,
    nonce: String,
}

type V2PaymentRequirements = v2::PaymentRequirements<String, String, String, Option<Value>>;
type V2PaymentPayload = v2::PaymentPayload<V2PaymentRequirements, Value>;
type V2VerifyRequest = v2::VerifyRequest<V2PaymentPayload, V2PaymentRequirements>;

pub async fn supported(
    State(state): State<AppState>,
) -> Result<Json<proto::SupportedResponse>, AppError> {
    use alloy::signers::local::PrivateKeySigner;

    let mut kinds: Vec<SupportedPaymentKind> = Vec::new();
    let mut signers: HashMap<ChainId, Vec<String>> = HashMap::new();

    if let Some(x402cfg) = state.config.x402.as_ref() {
        for kind in &x402cfg.kinds {
            kinds.push(SupportedPaymentKind {
                x402_version: 2,
                scheme: kind.scheme.clone(),
                network: normalize_kind_network(&kind.network, x402cfg.evm.as_ref()),
                extra: None,
            });
        }

        if let Some(evm) = x402cfg.evm.as_ref()
            && let Ok(signer) = PrivateKeySigner::from_str(&evm.facilitator_private_key)
        {
            signers.insert(
                ChainId::new("eip155", evm.chain_id.to_string()),
                vec![signer.address().to_string()],
            );
        }
    }

    Ok(Json(proto::SupportedResponse {
        kinds,
        extensions: vec!["exact-eip3009".to_string()],
        signers,
    }))
}

pub async fn verify(
    State(state): State<AppState>,
    Json(req): Json<proto::VerifyRequest>,
) -> Result<Json<proto::VerifyResponse>, AppError> {
    let parsed = match V2VerifyRequest::try_from(&req) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(
                    None::<String>,
                    format!("invalid_request: invalid x402 v2 verify request: {e}"),
                )
                .into(),
            ));
        }
    };

    if parsed.payment_payload.accepted != parsed.payment_requirements {
        return Ok(Json(
            v1::VerifyResponse::invalid(
                None::<String>,
                "accepted_requirements_mismatch".to_string(),
            )
            .into(),
        ));
    }

    let requirements = &parsed.payment_requirements;
    let network = requirements.network.to_string();
    if let Err(e) = assert_scheme_exact(requirements) {
        return Ok(Json(
            v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
        ));
    }
    if let Err(e) = assert_asset_transfer_method(requirements.extra.as_ref()) {
        return Ok(Json(
            v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
        ));
    }
    if let Err(e) = assert_eip155_network(
        requirements,
        state.config.x402.as_ref().and_then(|cfg| cfg.evm.as_ref()),
    ) {
        return Ok(Json(
            v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
        ));
    }

    let payload = match parse_evm_payload(&parsed.payment_payload.payload) {
        Ok(payload) => payload,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    let auth = &payload.authorization;

    let to = match parse_address(&auth.to) {
        Ok(to) => to,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    let pay_to = match parse_address(&requirements.pay_to) {
        Ok(pay_to) => pay_to,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    if to != pay_to {
        return Ok(Json(
            v1::VerifyResponse::invalid(
                None::<String>,
                format!("receiver mismatch: authorization.to={to} != payTo={pay_to}"),
            )
            .into(),
        ));
    }

    if let Err(e) = parse_address(&requirements.asset) {
        return Ok(Json(
            v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
        ));
    }
    if let Err(e) = parse_nonce_32(&auth.nonce) {
        return Ok(Json(
            v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
        ));
    }
    if let Err(e) = parse_signature_vrs(&payload.signature) {
        return Ok(Json(
            v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
        ));
    }
    let value = match parse_u256(&auth.value) {
        Ok(value) => value,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    let required = match parse_u256(&requirements.amount) {
        Ok(required) => required,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    if value != required {
        return Ok(Json(
            v1::VerifyResponse::invalid(
                None::<String>,
                format!("amount mismatch: authorization.value={value} != amount={required}"),
            )
            .into(),
        ));
    }

    let valid_after = match parse_u64(&auth.valid_after) {
        Ok(valid_after) => valid_after,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    let valid_before = match parse_u64(&auth.valid_before) {
        Ok(valid_before) => valid_before,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    let is_valid_window = match is_authorization_time_valid(valid_after, valid_before) {
        Ok(is_valid) => is_valid,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };
    if !is_valid_window {
        return Ok(Json(
            v1::VerifyResponse::invalid(
                None::<String>,
                "authorization not within valid window".to_string(),
            )
            .into(),
        ));
    }

    let from = match parse_address(&auth.from) {
        Ok(from) => from,
        Err(e) => {
            return Ok(Json(
                v1::VerifyResponse::invalid(None::<String>, format!("invalid_request: {e}")).into(),
            ));
        }
    };

    let Some(evm) = state.config.x402.as_ref().and_then(|cfg| cfg.evm.as_ref()) else {
        return Ok(Json(
            v1::VerifyResponse::invalid(
                None::<String>,
                "EVM not configured for x402 verification".to_string(),
            )
            .into(),
        ));
    };

    if let Err(e) = simulate_evm_exact(&payload, requirements, evm).await {
        return Ok(Json(
            v1::VerifyResponse::invalid(
                Some(from.to_string()),
                format!("transaction simulation failed: {e}"),
            )
            .into(),
        ));
    }

    info!(payer = %from, network = %network, "x402 v2 verify passed");
    Ok(Json(v1::VerifyResponse::valid(from.to_string()).into()))
}

pub async fn settle(
    State(state): State<AppState>,
    Json(req): Json<proto::SettleRequest>,
) -> Result<Json<proto::SettleResponse>, AppError> {
    let parsed = match V2VerifyRequest::try_from(&req) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Ok(Json(settle_error(
                &format!("invalid_request: invalid x402 v2 settle request: {e}"),
                "unknown",
            )));
        }
    };

    let requirements = &parsed.payment_requirements;
    let network = requirements.network.to_string();
    if let Err(e) = assert_scheme_exact(requirements) {
        return Ok(Json(settle_error(
            &format!("invalid_request: {e}"),
            &network,
        )));
    }

    let Some(x402cfg) = &state.config.x402 else {
        return Ok(Json(settle_error("x402 not configured", &network)));
    };
    let Some(evm) = &x402cfg.evm else {
        return Ok(Json(settle_error("EVM not configured for x402", &network)));
    };

    if parsed.payment_payload.accepted != parsed.payment_requirements {
        return Ok(Json(settle_error(
            "accepted_requirements_mismatch",
            &network,
        )));
    }

    if let Err(e) = assert_asset_transfer_method(requirements.extra.as_ref()) {
        return Ok(Json(settle_error(
            &format!("invalid_request: {e}"),
            &network,
        )));
    }

    let payload = match parse_evm_payload(&parsed.payment_payload.payload) {
        Ok(payload) => payload,
        Err(e) => {
            return Ok(Json(settle_error(
                &format!("invalid_request: {e}"),
                &network,
            )));
        }
    };

    match settle_evm_exact(&payload, requirements, evm).await {
        Ok((tx_hash, payer)) => Ok(Json(
            v1::SettleResponse::Success {
                payer,
                transaction: tx_hash,
                network,
            }
            .into(),
        )),
        Err(e) => {
            error!(error = %e, "x402 settle failed");
            Ok(Json(settle_error(&e.to_string(), &network)))
        }
    }
}

fn settle_error(reason: &str, network: &str) -> proto::SettleResponse {
    v1::SettleResponse::Error {
        reason: reason.to_string(),
        network: network.to_string(),
    }
    .into()
}

fn assert_scheme_exact(requirements: &V2PaymentRequirements) -> Result<(), AppError> {
    if !requirements.scheme.eq_ignore_ascii_case("exact") {
        return Err(AppError::InvalidInput(format!(
            "unsupported scheme '{}', expected 'exact'",
            requirements.scheme
        )));
    }
    Ok(())
}

fn normalize_kind_network(network: &str, evm: Option<&EvmX402Config>) -> String {
    if network == "evm"
        && let Some(evm) = evm
    {
        return format!("eip155:{}", evm.chain_id);
    }
    network.to_string()
}

fn parse_evm_payload(value: &Value) -> Result<PaymentPayloadExactEvm, AppError> {
    serde_json::from_value(value.clone())
        .map_err(|e| AppError::InvalidInput(format!("invalid EVM payload: {e}")))
}

fn assert_asset_transfer_method(extra: Option<&Value>) -> Result<(), AppError> {
    if let Some(method) = extra
        .and_then(|value| value.get("assetTransferMethod"))
        .and_then(Value::as_str)
    {
        let method = method.to_ascii_lowercase();
        if method != "eip3009" {
            return Err(AppError::InvalidInput(format!(
                "unsupported assetTransferMethod '{method}', expected 'eip3009'"
            )));
        }
    }
    Ok(())
}

fn assert_eip155_network(
    requirements: &V2PaymentRequirements,
    evm: Option<&EvmX402Config>,
) -> Result<(), AppError> {
    if requirements.network.namespace() != "eip155" {
        return Err(AppError::InvalidInput(format!(
            "unsupported network namespace '{}'",
            requirements.network.namespace()
        )));
    }

    if let Some(evm) = evm {
        let configured = evm.chain_id.to_string();
        if requirements.network.reference() != configured {
            return Err(AppError::InvalidInput(format!(
                "chain id mismatch: requirements={} configured={configured}",
                requirements.network.reference()
            )));
        }
    }

    Ok(())
}

fn parse_address(input: &str) -> Result<alloy::primitives::Address, AppError> {
    alloy::primitives::Address::from_str(input)
        .map_err(|e| AppError::InvalidInput(format!("invalid address '{input}': {e}")))
}

fn parse_u256(input: &str) -> Result<alloy::primitives::U256, AppError> {
    if let Some(hex_str) = input.strip_prefix("0x") {
        alloy::primitives::U256::from_str_radix(hex_str, 16)
            .map_err(|e| AppError::InvalidInput(format!("invalid hex value '{input}': {e}")))
    } else {
        input
            .parse::<alloy::primitives::U256>()
            .map_err(|e| AppError::InvalidInput(format!("invalid value '{input}': {e}")))
    }
}

fn parse_u64(input: &str) -> Result<u64, AppError> {
    input
        .parse::<u64>()
        .map_err(|e| AppError::InvalidInput(format!("invalid integer '{input}': {e}")))
}

fn is_authorization_time_valid(valid_after: u64, valid_before: u64) -> Result<bool, AppError> {
    if valid_before <= valid_after {
        return Ok(false);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AppError::Internal(format!("system clock error: {e}")))?
        .as_secs();
    Ok(valid_after <= now && valid_before >= now.saturating_add(6))
}

fn parse_nonce_32(input: &str) -> Result<alloy::primitives::FixedBytes<32>, AppError> {
    let hex_str = input.strip_prefix("0x").unwrap_or(input);
    let bytes = hex::decode(hex_str)
        .map_err(|e| AppError::InvalidInput(format!("invalid nonce hex '{input}': {e}")))?;
    if bytes.len() != 32 {
        return Err(AppError::InvalidInput(format!(
            "invalid nonce length: expected 32 bytes, got {}",
            bytes.len()
        )));
    }
    Ok(alloy::primitives::FixedBytes::<32>::from_slice(&bytes))
}

fn parse_signature_vrs(
    signature: &str,
) -> Result<
    (
        u8,
        alloy::primitives::FixedBytes<32>,
        alloy::primitives::FixedBytes<32>,
    ),
    AppError,
> {
    let sig_hex = signature.trim_start_matches("0x");
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| AppError::InvalidInput(format!("invalid signature hex: {e}")))?;
    if sig_bytes.len() != 65 {
        return Err(AppError::InvalidInput(format!(
            "invalid signature length: expected 65 bytes, got {}",
            sig_bytes.len()
        )));
    }
    let r = alloy::primitives::FixedBytes::<32>::from_slice(&sig_bytes[0..32]);
    let s = alloy::primitives::FixedBytes::<32>::from_slice(&sig_bytes[32..64]);
    let mut v = sig_bytes[64];
    if v < 27 {
        v += 27;
    }
    Ok((v, r, s))
}

async fn simulate_evm_exact(
    payload: &PaymentPayloadExactEvm,
    requirements: &V2PaymentRequirements,
    evm: &EvmX402Config,
) -> eyre::Result<()> {
    use alloy::{
        network::TransactionBuilder,
        primitives::{Address, Bytes, U256},
        providers::{Provider, ProviderBuilder},
        rpc::types::TransactionRequest,
        sol,
        sol_types::SolCall,
    };

    assert_eip155_network(requirements, Some(evm)).map_err(|e| eyre::eyre!(e.to_string()))?;

    let from = Address::from_str(&payload.authorization.from)?;
    let to = Address::from_str(&payload.authorization.to)?;
    let pay_to = Address::from_str(&requirements.pay_to)?;
    if to != pay_to {
        eyre::bail!("receiver mismatch: authorization.to={to} != payTo={pay_to}");
    }

    let value = parse_u256(&payload.authorization.value).map_err(|e| eyre::eyre!(e.to_string()))?;
    let required = parse_u256(&requirements.amount).map_err(|e| eyre::eyre!(e.to_string()))?;
    if value != required {
        eyre::bail!("amount mismatch: authorization.value={value} != amount={required}");
    }

    let valid_after =
        parse_u64(&payload.authorization.valid_after).map_err(|e| eyre::eyre!(e.to_string()))?;
    let valid_before =
        parse_u64(&payload.authorization.valid_before).map_err(|e| eyre::eyre!(e.to_string()))?;
    if !is_authorization_time_valid(valid_after, valid_before)
        .map_err(|e| eyre::eyre!(e.to_string()))?
    {
        eyre::bail!("authorization not within valid window");
    }

    let _: Address = Address::from_str(&requirements.asset)?;
    let order_id =
        parse_nonce_32(&payload.authorization.nonce).map_err(|e| eyre::eyre!(e.to_string()))?;
    let (v, r, s) =
        parse_signature_vrs(&payload.signature).map_err(|e| eyre::eyre!(e.to_string()))?;

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

    let provider = ProviderBuilder::new().connect(&evm.rpc_http).await?;
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
    let data: Bytes = call.abi_encode().into();
    let tx = TransactionRequest::default()
        .with_to(vault_addr)
        .with_input(data);

    let _ = provider.call(tx).await.wrap_err("simulation failed")?;
    Ok(())
}

async fn settle_evm_exact(
    payload: &PaymentPayloadExactEvm,
    requirements: &V2PaymentRequirements,
    evm: &EvmX402Config,
) -> eyre::Result<(String, String)> {
    use alloy::{
        network::TransactionBuilder,
        primitives::{Address, Bytes, U256},
        providers::{Provider, ProviderBuilder},
        rpc::types::TransactionRequest,
        signers::local::PrivateKeySigner,
        sol,
        sol_types::SolCall,
    };

    assert_eip155_network(requirements, Some(evm)).map_err(|e| eyre::eyre!(format!("{}", e)))?;

    let from = Address::from_str(&payload.authorization.from)?;
    let to = Address::from_str(&payload.authorization.to)?;
    let pay_to = Address::from_str(&requirements.pay_to)?;
    if to != pay_to {
        eyre::bail!("receiver mismatch: authorization.to={to} != payTo={pay_to}");
    }

    let value = parse_u256(&payload.authorization.value).map_err(|e| eyre::eyre!(e.to_string()))?;
    let required = parse_u256(&requirements.amount).map_err(|e| eyre::eyre!(e.to_string()))?;
    if value != required {
        eyre::bail!("amount mismatch: authorization.value={value} != amount={required}");
    }

    let valid_after =
        parse_u64(&payload.authorization.valid_after).map_err(|e| eyre::eyre!(e.to_string()))?;
    let valid_before =
        parse_u64(&payload.authorization.valid_before).map_err(|e| eyre::eyre!(e.to_string()))?;
    if !is_authorization_time_valid(valid_after, valid_before)
        .map_err(|e| eyre::eyre!(format!("{}", e)))?
    {
        eyre::bail!("authorization not within valid window");
    }

    let _: Address = Address::from_str(&requirements.asset)?;

    let nonce =
        parse_nonce_32(&payload.authorization.nonce).map_err(|e| eyre::eyre!(e.to_string()))?;
    let (v, r, s) =
        parse_signature_vrs(&payload.signature).map_err(|e| eyre::eyre!(e.to_string()))?;

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

    let signer = PrivateKeySigner::from_str(evm.facilitator_private_key.as_str())?;
    let wallet = alloy::network::EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect(&evm.rpc_http)
        .await?;

    let order_id = nonce;
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
    let data: Bytes = call.abi_encode().into();
    let tx = TransactionRequest::default()
        .with_to(vault_addr)
        .with_input(data);

    let _ = provider
        .call(tx.clone())
        .await
        .wrap_err("simulation failed")?;

    let pending = provider.send_transaction(tx).await?;
    let tx_hash = format!("0x{}", hex::encode(pending.tx_hash()));
    info!(%tx_hash, vault = %evm.vault_address, to = %to, value = %value, "sent x402 v2 exact-eip3009 settlement");

    Ok((tx_hash, from.to_string()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use x402_types::{chain::ChainId, proto::v2};

    use super::{
        assert_asset_transfer_method, assert_scheme_exact, normalize_kind_network, parse_nonce_32,
        parse_signature_vrs,
    };

    #[test]
    fn normalize_legacy_evm_network_to_caip2() {
        let network = normalize_kind_network("evm", None);
        assert_eq!(network, "evm");
    }

    #[test]
    fn accept_explicit_eip3009_method() {
        let extra = json!({"assetTransferMethod":"eip3009"});
        assert!(assert_asset_transfer_method(Some(&extra)).is_ok());
    }

    #[test]
    fn reject_unsupported_asset_transfer_method() {
        let extra = json!({"assetTransferMethod":"permit2"});
        assert!(assert_asset_transfer_method(Some(&extra)).is_err());
    }

    #[test]
    fn reject_non_exact_scheme() {
        let requirements = v2::PaymentRequirements {
            scheme: "permit".to_string(),
            network: ChainId::new("eip155", "84532"),
            amount: "1".to_string(),
            pay_to: "0x0000000000000000000000000000000000000001".to_string(),
            max_timeout_seconds: 60,
            asset: "0x0000000000000000000000000000000000000010".to_string(),
            extra: None,
        };
        assert!(assert_scheme_exact(&requirements).is_err());
    }

    #[test]
    fn parse_nonce_rejects_short_bytes() {
        assert!(parse_nonce_32("0xdeadbeef").is_err());
    }

    #[test]
    fn parse_signature_rejects_invalid_length() {
        let short = format!("0x{}", "aa".repeat(64));
        assert!(parse_signature_vrs(&short).is_err());
    }
}
