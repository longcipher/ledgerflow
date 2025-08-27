//! Type definitions for the x402 payment facilitator protocol.

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unwrap_used)]
//! This module provides all the core data structures needed for x402 payment
//! verification and settlement on the Sui blockchain.

use std::{fmt, str::FromStr};

use alloy::primitives::{Address as AlloyAddress, U256};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use once_cell::sync::Lazy;
use regex::Regex;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sui_types::base_types::{ObjectID, SuiAddress};
use url::Url;

/// Unix timestamp type
pub type UnixTimestamp = u64;

/// EVM address type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EvmAddress(pub AlloyAddress);

impl EvmAddress {
    pub fn new(address: AlloyAddress) -> Self {
        Self(address)
    }
}

impl fmt::Display for EvmAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl FromStr for EvmAddress {
    type Err = alloy::primitives::AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(EvmAddress(AlloyAddress::from_str(s)?))
    }
}

impl Serialize for EvmAddress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:#x}", self.0))
    }
}

impl<'de> Deserialize<'de> for EvmAddress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        EvmAddress::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl From<AlloyAddress> for EvmAddress {
    fn from(address: AlloyAddress) -> Self {
        EvmAddress(address)
    }
}

impl From<EvmAddress> for AlloyAddress {
    fn from(address: EvmAddress) -> Self {
        address.0
    }
}

/// EVM signature type (64 bytes + recovery id)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSignature(pub [u8; 65]);

impl EvmSignature {
    pub fn new(signature: [u8; 65]) -> Self {
        Self(signature)
    }
}

impl Serialize for EvmSignature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("0x{}", hex::encode(self.0)))
    }
}

impl<'de> Deserialize<'de> for EvmSignature {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let hex_str = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(hex_str).map_err(serde::de::Error::custom)?;
        if bytes.len() != 65 {
            return Err(serde::de::Error::custom(format!(
                "Invalid signature length: expected 65 bytes, got {}",
                bytes.len()
            )));
        }
        let mut array = [0u8; 65];
        array.copy_from_slice(&bytes);
        Ok(EvmSignature(array))
    }
}

/// Mixed address type that can represent either Sui or EVM addresses
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MixedAddress {
    Sui(SuiAddress),
    Evm(EvmAddress),
}

impl fmt::Display for MixedAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MixedAddress::Sui(addr) => write!(f, "{}", addr),
            MixedAddress::Evm(addr) => write!(f, "{}", addr),
        }
    }
}

impl From<SuiAddress> for MixedAddress {
    fn from(address: SuiAddress) -> Self {
        MixedAddress::Sui(address)
    }
}

impl From<EvmAddress> for MixedAddress {
    fn from(address: EvmAddress) -> Self {
        MixedAddress::Evm(address)
    }
}

impl From<AlloyAddress> for MixedAddress {
    fn from(address: AlloyAddress) -> Self {
        MixedAddress::Evm(EvmAddress(address))
    }
}

/// Represents the protocol version. Currently only version 1 is supported.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum X402Version {
    V1,
}

impl Serialize for X402Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            X402Version::V1 => serializer.serialize_u8(1),
        }
    }
}

impl<'de> Deserialize<'de> for X402Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let num = u8::deserialize(deserializer)?;
        match num {
            1 => Ok(X402Version::V1),
            _ => Err(serde::de::Error::custom(format!(
                "Unsupported x402Version: {}",
                num
            ))),
        }
    }
}

impl fmt::Display for X402Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            X402Version::V1 => write!(f, "1"),
        }
    }
}

/// Enumerates payment schemes. Only "exact" is supported in this implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Exact,
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scheme::Exact => write!(f, "exact"),
        }
    }
}

/// Supported blockchain networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Network {
    SuiMainnet,
    SuiTestnet,
    SuiDevnet,
    // EVM networks
    BaseSepolia,
    Base,
    XdcMainnet,
    AvalancheFuji,
    Avalanche,
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Network::SuiMainnet => write!(f, "sui-mainnet"),
            Network::SuiTestnet => write!(f, "sui-testnet"),
            Network::SuiDevnet => write!(f, "sui-devnet"),
            Network::BaseSepolia => write!(f, "base-sepolia"),
            Network::Base => write!(f, "base"),
            Network::XdcMainnet => write!(f, "xdc-mainnet"),
            Network::AvalancheFuji => write!(f, "avalanche-fuji"),
            Network::Avalanche => write!(f, "avalanche"),
        }
    }
}

impl Network {
    pub fn variants() -> &'static [Network] {
        &[
            Network::SuiMainnet,
            Network::SuiTestnet,
            Network::SuiDevnet,
            Network::BaseSepolia,
            Network::Base,
            Network::XdcMainnet,
            Network::AvalancheFuji,
            Network::Avalanche,
        ]
    }
}

/// A precise on-chain token amount in base units.
/// Represented as a stringified u64 in JSON to prevent precision loss.
#[derive(Debug, Copy, Clone, PartialEq, Ord, PartialOrd, Eq, Hash)]
pub struct TokenAmount(pub u64);

impl TokenAmount {
    pub fn new(amount: u64) -> Self {
        Self(amount)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl From<TokenAmount> for U256 {
    fn from(amount: TokenAmount) -> Self {
        U256::from(amount.0)
    }
}

impl From<u64> for TokenAmount {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<TokenAmount> for u64 {
    fn from(value: TokenAmount) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for TokenAmount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let string = String::deserialize(deserializer)?;
        let value = string
            .parse::<u64>()
            .map_err(|e| serde::de::Error::custom(format!("Invalid token amount: {}", e)))?;
        Ok(TokenAmount(value))
    }
}

impl Serialize for TokenAmount {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents a transaction hash for different networks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionHash {
    Sui(String),
    Evm([u8; 32]),
}

impl TransactionHash {
    pub fn sui(hash: String) -> Self {
        Self::Sui(hash)
    }

    pub fn evm(hash: [u8; 32]) -> Self {
        Self::Evm(hash)
    }
}

impl fmt::Display for TransactionHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionHash::Sui(hash) => write!(f, "{}", hash),
            TransactionHash::Evm(hash) => write!(f, "0x{}", hex::encode(hash)),
        }
    }
}

impl<'de> Deserialize<'de> for TransactionHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        // Try to determine if it's an EVM hash (0x + 64 hex chars) or Sui hash
        if s.starts_with("0x") && s.len() == 66 {
            let hex_str = &s[2..];
            let bytes = hex::decode(hex_str).map_err(serde::de::Error::custom)?;
            if bytes.len() == 32 {
                let mut array = [0u8; 32];
                array.copy_from_slice(&bytes);
                return Ok(TransactionHash::Evm(array));
            }
        }
        Ok(TransactionHash::Sui(s))
    }
}

impl Serialize for TransactionHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            TransactionHash::Sui(hash) => serializer.serialize_str(hash),
            TransactionHash::Evm(hash) => {
                serializer.serialize_str(&format!("0x{}", hex::encode(hash)))
            }
        }
    }
}

/// Represents a 32-byte random nonce, hex-encoded with 0x prefix.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct HexEncodedNonce(pub [u8; 32]);

impl fmt::Debug for HexEncodedNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HexEncodedNonce(0x{})", hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for HexEncodedNonce {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        static NONCE_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^0x[0-9a-fA-F]{64}$").expect("Invalid nonce regex"));

        if !NONCE_REGEX.is_match(&s) {
            return Err(serde::de::Error::custom("Invalid nonce format"));
        }

        let bytes =
            hex::decode(&s[2..]).map_err(|_| serde::de::Error::custom("Invalid hex in nonce"))?;

        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("Invalid length for nonce"))?;

        Ok(HexEncodedNonce(array))
    }
}

impl Serialize for HexEncodedNonce {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_string = format!("0x{}", hex::encode(self.0));
        serializer.serialize_str(&hex_string)
    }
}

/// Sui-specific payment authorization following x402 standard format.
/// Similar to EIP-3009 transferWithAuthorization parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiPayloadAuthorization {
    pub from: SuiAddress,
    pub to: SuiAddress,
    pub value: TokenAmount,
    pub valid_after: u64,
    pub valid_before: u64,
    pub nonce: HexEncodedNonce,
    /// The type of coin being transferred (e.g., "0x2::sui::SUI" or "0x5d4b302506645c37ff133b98c4b50a5ae14841659738d6d733d59d0d217a93bf::coin::COIN")
    pub coin_type: String,
}

/// Full payload required for Sui payment authorization including signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiPayload {
    /// Base64-encoded signature following Sui's intent signing format
    pub signature: String,
    /// The authorization data (following x402 standard)
    pub authorization: SuiPayloadAuthorization,
    /// Optional: gas budget for transaction execution
    pub gas_budget: Option<u64>,
}

/// EVM-specific payment authorization following EIP-3009 transferWithAuthorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmPayloadAuthorization {
    pub from: EvmAddress,
    pub to: EvmAddress,
    pub value: TokenAmount,
    pub valid_after: u64,
    pub valid_before: u64,
    pub nonce: HexEncodedNonce,
}

/// Full payload required for EVM payment authorization including signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmPayload {
    /// EIP-712 signature for the transferWithAuthorization
    pub signature: EvmSignature,
    /// The authorization data (following EIP-3009)
    pub authorization: EvmPayloadAuthorization,
}

/// Payment payload variants for different schemes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExactPaymentPayload {
    Sui(SuiPayload),
    Evm(EvmPayload),
}

/// Describes a signed request to transfer a specific amount of funds on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: X402Version,
    pub scheme: Scheme,
    pub network: Network,
    pub payload: ExactPaymentPayload,
}

/// Asset identifier for different networks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AssetId {
    Sui(ObjectID),
    Evm(EvmAddress),
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetId::Sui(id) => write!(f, "{}", id),
            AssetId::Evm(addr) => write!(f, "{}", addr),
        }
    }
}

/// Pay-to address that can be either Sui or EVM
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PayToAddress {
    Sui(SuiAddress),
    Evm(EvmAddress),
}

impl fmt::Display for PayToAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PayToAddress::Sui(addr) => write!(f, "{}", addr),
            PayToAddress::Evm(addr) => write!(f, "{}", addr),
        }
    }
}

impl TryFrom<PayToAddress> for EvmAddress {
    type Error = String;

    fn try_from(value: PayToAddress) -> Result<Self, Self::Error> {
        match value {
            PayToAddress::Evm(addr) => Ok(addr),
            PayToAddress::Sui(_) => Err("Expected EVM address, got Sui address".to_string()),
        }
    }
}

impl TryFrom<PayToAddress> for SuiAddress {
    type Error = String;

    fn try_from(value: PayToAddress) -> Result<Self, Self::Error> {
        match value {
            PayToAddress::Sui(addr) => Ok(addr),
            PayToAddress::Evm(_) => Err("Expected Sui address, got EVM address".to_string()),
        }
    }
}

/// Requirements set by the payment-gated endpoint for an acceptable payment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub scheme: Scheme,
    pub network: Network,
    pub max_amount_required: TokenAmount,
    pub resource: Url,
    pub description: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
    pub pay_to: PayToAddress,
    pub max_timeout_seconds: u64,
    pub asset: AssetId,
    pub extra: Option<serde_json::Value>,
}

/// Wrapper for a payment payload and requirements sent by the client to a facilitator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    pub x402_version: X402Version,
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

impl fmt::Display for VerifyRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VerifyRequest(version={:?}, network={}, scheme={})",
            self.x402_version, self.payment_payload.network, self.payment_payload.scheme
        )
    }
}

impl VerifyRequest {
    pub fn network(&self) -> Network {
        self.payment_payload.network
    }
}

/// Wrapper for a payment payload and requirements sent for settlement.
pub type SettleRequest = VerifyRequest;

/// Error reasons returned by the facilitator.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum FacilitatorErrorReason {
    /// Payer doesn't have sufficient funds.
    #[error("insufficient_funds")]
    #[serde(rename = "insufficient_funds")]
    InsufficientFunds,
    /// The scheme didn't match expected or settlement failed.
    #[error("invalid_scheme")]
    #[serde(rename = "invalid_scheme")]
    InvalidScheme,
    /// Network didn't match facilitator's expected network.
    #[error("invalid_network")]
    #[serde(rename = "invalid_network")]
    InvalidNetwork,
    /// Unexpected settle error.
    #[error("unexpected_settle_error")]
    #[serde(rename = "unexpected_settle_error")]
    UnexpectedSettleError,
    /// Invalid signature.
    #[error("invalid_signature")]
    #[serde(rename = "invalid_signature")]
    InvalidSignature,
    /// Invalid timing (expired or not yet valid).
    #[error("invalid_timing")]
    #[serde(rename = "invalid_timing")]
    InvalidTiming,
}

/// Response returned from verification.
#[derive(Debug, Clone)]
pub enum VerifyResponse {
    /// The payload matches the requirements and passes all checks.
    Valid { payer: MixedAddress },
    /// The payload was well-formed but failed verification.
    Invalid {
        reason: FacilitatorErrorReason,
        payer: Option<MixedAddress>,
    },
}

impl VerifyResponse {
    pub fn valid(payer: impl Into<MixedAddress>) -> Self {
        VerifyResponse::Valid {
            payer: payer.into(),
        }
    }

    pub fn invalid(payer: Option<impl Into<MixedAddress>>, reason: FacilitatorErrorReason) -> Self {
        VerifyResponse::Invalid {
            reason,
            payer: payer.map(|p| p.into()),
        }
    }
}

impl Serialize for VerifyResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut s = match self {
            VerifyResponse::Valid { .. } => serializer.serialize_struct("VerifyResponse", 2)?,
            VerifyResponse::Invalid { .. } => serializer.serialize_struct("VerifyResponse", 3)?,
        };

        match self {
            VerifyResponse::Valid { payer } => {
                s.serialize_field("isValid", &true)?;
                s.serialize_field("payer", &payer.to_string())?;
            }
            VerifyResponse::Invalid { reason, payer } => {
                s.serialize_field("isValid", &false)?;
                s.serialize_field("invalidReason", reason)?;
                if let Some(payer) = payer {
                    s.serialize_field("payer", &payer.to_string())?;
                }
            }
        }

        s.end()
    }
}

impl<'de> Deserialize<'de> for VerifyResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Raw {
            is_valid: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            payer: Option<String>,
            #[serde(default)]
            invalid_reason: Option<FacilitatorErrorReason>,
        }

        let raw = Raw::deserialize(deserializer)?;

        match (raw.is_valid, raw.invalid_reason) {
            (true, None) => match raw.payer {
                None => Err(serde::de::Error::custom(
                    "`payer` must be present when `isValid` is true",
                )),
                Some(payer_str) => {
                    // Try to parse as Sui address first, then EVM
                    let payer = if let Ok(sui_addr) = SuiAddress::from_str(&payer_str) {
                        MixedAddress::Sui(sui_addr)
                    } else if let Ok(evm_addr) = EvmAddress::from_str(&payer_str) {
                        MixedAddress::Evm(evm_addr)
                    } else {
                        return Err(serde::de::Error::custom(format!(
                            "Invalid payer address: {}",
                            payer_str
                        )));
                    };
                    Ok(VerifyResponse::Valid { payer })
                }
            },
            (false, Some(reason)) => {
                let payer = if let Some(payer_str) = raw.payer {
                    // Try to parse as Sui address first, then EVM
                    let payer = if let Ok(sui_addr) = SuiAddress::from_str(&payer_str) {
                        MixedAddress::Sui(sui_addr)
                    } else if let Ok(evm_addr) = EvmAddress::from_str(&payer_str) {
                        MixedAddress::Evm(evm_addr)
                    } else {
                        return Err(serde::de::Error::custom(format!(
                            "Invalid payer address: {}",
                            payer_str
                        )));
                    };
                    Some(payer)
                } else {
                    None
                };
                Ok(VerifyResponse::Invalid { payer, reason })
            }
            (true, Some(_)) => Err(serde::de::Error::custom(
                "`invalidReason` must be absent when `isValid` is true",
            )),
            (false, None) => Err(serde::de::Error::custom(
                "`invalidReason` must be present when `isValid` is false",
            )),
        }
    }
}

/// Response returned from settlement.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<FacilitatorErrorReason>,
    pub payer: MixedAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction: Option<TransactionHash>,
    pub network: Network,
}

/// Simple error response structure.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
}

/// Base64-encoded bytes wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base64Bytes(pub Vec<u8>);

impl Base64Bytes {
    pub fn decode(input: &str) -> Result<Self, base64::DecodeError> {
        let bytes = BASE64.decode(input)?;
        Ok(Base64Bytes(bytes))
    }

    pub fn encode(&self) -> String {
        BASE64.encode(&self.0)
    }
}

/// Supported payment kind for discovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedPaymentKind {
    pub x402_version: X402Version,
    pub scheme: Scheme,
    pub network: Network,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<SupportedPaymentKindExtra>,
}

/// Extra information for supported payment kind.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedPaymentKindExtra {
    pub fee_payer: MixedAddress,
}

/// Response for supported payment kinds endpoint.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedPaymentKindsResponse {
    pub kinds: Vec<SupportedPaymentKind>,
}

/// Response returned when payment is required.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequiredResponse {
    pub error: String,
    pub accepts: Vec<PaymentRequirements>,
    pub x402_version: X402Version,
}

impl fmt::Display for PaymentRequiredResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PaymentRequiredResponse: error='{}', accepts={} requirement(s), version={}",
            self.error,
            self.accepts.len(),
            self.x402_version
        )
    }
}

/// Represents a price-like numeric value in human-readable currency format.
#[derive(Debug, Clone, PartialEq)]
pub struct MoneyAmount(pub Decimal);

impl MoneyAmount {
    pub fn parse(input: &str) -> Result<Self, eyre::Error> {
        // Remove anything that isn't digit, dot, minus
        let cleaned = Regex::new(r"[^\d\.\-]+")
            .unwrap()
            .replace_all(input, "")
            .to_string();

        let parsed = Decimal::from_str(&cleaned)
            .map_err(|e| eyre::eyre!("Invalid money amount format: {}", e))?;

        if parsed.is_sign_negative() {
            return Err(eyre::eyre!("Negative amounts are not allowed"));
        }

        Ok(MoneyAmount(parsed))
    }

    /// Convert to token amount with given decimals.
    pub fn as_token_amount(&self, token_decimals: u8) -> Result<TokenAmount, eyre::Error> {
        let multiplier = 10u64.pow(token_decimals as u32);
        let scaled = self.0 * Decimal::from(multiplier);

        let amount = scaled
            .to_u64()
            .ok_or_else(|| eyre::eyre!("Amount too large for u64"))?;

        Ok(TokenAmount::new(amount))
    }
}

impl FromStr for MoneyAmount {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        MoneyAmount::parse(s)
    }
}

impl fmt::Display for MoneyAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.normalize())
    }
}

/// Error type for payment payload decoding.
#[derive(Debug, thiserror::Error)]
pub enum PaymentPayloadB64DecodingError {
    /// The input bytes were not valid base64.
    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    /// The decoded bytes could not be interpreted as a UTF-8 JSON string.
    #[error("utf-8 decode error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// The JSON structure was invalid.
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl TryFrom<&str> for PaymentPayload {
    type Error = PaymentPayloadB64DecodingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let decoded = BASE64.decode(value)?;
        let json_str = std::str::from_utf8(&decoded)?;
        let payload = serde_json::from_str(json_str)?;
        Ok(payload)
    }
}
