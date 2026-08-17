//! Warrant, proof, and signing types for LedgerFlow.
//!
//! This module defines the signed capability token at the heart of the
//! LedgerFlow authorization layer. A warrant grants a *holder* the right to
//! pay for specific merchants/resources within stateless limits, for a bounded
//! lifetime, with an optional delegation chain.

use std::{
    collections::BTreeMap,
    fmt::{self, Display, Write as _},
};

use ciborium::{de::from_reader, ser::into_writer};
use ed25519_dalek::{Signature, Signer as _, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{WireError, WireResult};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Warrant schema version used by v1.
pub const WARRANT_VERSION_V1: u8 = 1;

/// Default delegation depth cap for newly issued warrants.
pub const DEFAULT_MAX_DEPTH: u8 = 4;

/// Hard ceiling on delegation depth (protocol-enforced).
pub const MAX_DELEGATION_DEPTH: u8 = 8;

/// Default warrant lifetime (7 days, in seconds).
pub const DEFAULT_WARRANT_TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Hard ceiling on warrant lifetime (90 days, in seconds).
pub const MAX_WARRANT_TTL_SECS: u64 = 90 * 24 * 60 * 60;

/// Maximum accepted size for serialized warrant payloads.
pub const MAX_WARRANT_CBOR_BYTES: usize = 64 * 1024;

/// Default proof-of-possession freshness window (60 seconds).
pub const DEFAULT_PROOF_FRESHNESS_MS: u64 = 60_000;

/// Default clock-skew tolerance applied to PoP timestamps (30 seconds).
pub const DEFAULT_CLOCK_SKEW_MS: u64 = 30_000;

/// Default challenge lifetime (5 minutes).
pub const DEFAULT_CHALLENGE_TTL_MS: u64 = 300_000;

/// Domain-separation prefix for warrant envelope signatures.
pub const WARRANT_SIGN_DOMAIN: &[u8] = b"ledgerflow-warrant-v1";

// ---------------------------------------------------------------------------
// CborCodec
// ---------------------------------------------------------------------------

/// Extension trait for CBOR encode/decode with size limits.
pub trait CborCodec: serde::Serialize + serde::de::DeserializeOwned {
    /// Maximum payload size accepted by [`decode_cbor`](Self::decode_cbor).
    fn max_cbor_bytes() -> usize {
        MAX_WARRANT_CBOR_BYTES
    }

    /// Encodes this value as CBOR bytes.
    fn encode_cbor(&self) -> WireResult<Vec<u8>> {
        let mut bytes = Vec::new();
        into_writer(self, &mut bytes)
            .map_err(|error| WireError::Serialization(error.to_string()))?;
        Ok(bytes)
    }

    /// Decodes a value from CBOR bytes, enforcing the size limit.
    fn decode_cbor(bytes: &[u8]) -> WireResult<Self> {
        if bytes.len() > Self::max_cbor_bytes() {
            return Err(WireError::PayloadTooLarge {
                size: bytes.len(),
                max: Self::max_cbor_bytes(),
            });
        }
        from_reader(bytes).map_err(|error| WireError::Deserialization(error.to_string()))
    }
}

/// Hashes bytes as a lowercase hexadecimal SHA-256 digest with a `sha256:` prefix.
#[must_use]
pub fn sha256_prefixed<T: AsRef<[u8]>>(input: T) -> String {
    let digest = Sha256::digest(input.as_ref());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("sha256:{encoded}")
}

/// Generates a 16-byte UUIDv7-style warrant identifier.
///
/// Layout: 48-bit unix-epoch-milliseconds, version bits (0111), variant bits
/// (10), then 62 bits of randomness supplied by the caller.
#[must_use]
pub const fn generate_warrant_id(now_ms: u64, random: [u8; 8]) -> [u8; 16] {
    let mut id = [0_u8; 16];
    let timestamp = now_ms & 0x0000_FFFF_FFFF_FFFF;
    id[0] = (timestamp >> 40) as u8;
    id[1] = (timestamp >> 32) as u8;
    id[2] = (timestamp >> 24) as u8;
    id[3] = (timestamp >> 16) as u8;
    id[4] = (timestamp >> 8) as u8;
    id[5] = timestamp as u8;
    id[6] = 0x70 | ((random[0] >> 4) & 0x0F);
    id[7] = random[1];
    id[8] = 0x80 | (random[2] >> 4);
    id[9] = random[3];
    id[10] = random[4];
    id[11] = random[5];
    id[12] = random[6];
    id[13] = random[7];
    // Remaining 2 bytes (14, 15) stay zero (variant bits already set).
    id
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

/// Supported signer algorithms for warrants and proofs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[non_exhaustive]
pub enum SigningAlgorithm {
    Ed25519,
    Secp256k1,
}

impl SigningAlgorithm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::Secp256k1 => "secp256k1",
        }
    }
}

impl Display for SigningAlgorithm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Public signer identity used for warrant issuance and proof verification.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SignerRef {
    pub alg: SigningAlgorithm,
    #[serde(with = "serde_bytes")]
    pub public_key: Vec<u8>,
    pub key_id: Option<String>,
}

impl SignerRef {
    #[must_use]
    pub const fn new(alg: SigningAlgorithm, public_key: Vec<u8>) -> Self {
        Self { alg, public_key, key_id: None }
    }

    #[must_use]
    pub fn with_key_id(mut self, key_id: String) -> Self {
        self.key_id = Some(key_id);
        self
    }
}

/// Ed25519 signing key pair for warrant issuance, proof creation, and approvals.
#[derive(Clone)]
pub struct SigningKeyPair {
    signing_key: ed25519_dalek::SigningKey,
}

impl fmt::Debug for SigningKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKeyPair")
            .field("public_key_hex", &hex_encode(&self.signing_key.verifying_key().to_bytes()))
            .finish()
    }
}

impl SigningKeyPair {
    /// Creates a key pair from raw Ed25519 secret key bytes.
    #[must_use]
    pub fn from_bytes(secret_key: &[u8; 32]) -> Self {
        Self { signing_key: ed25519_dalek::SigningKey::from_bytes(secret_key) }
    }

    /// Returns the public key bytes.
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Creates a `SignerRef` from this key pair.
    #[must_use]
    pub fn signer_ref(&self) -> SignerRef {
        SignerRef::new(SigningAlgorithm::Ed25519, self.public_key_bytes().to_vec())
    }

    /// Signs a message, producing a [`SignatureEnvelope`].
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> SignatureEnvelope {
        let signature = self.signing_key.sign(message);
        SignatureEnvelope { alg: SigningAlgorithm::Ed25519, value: signature.to_bytes().to_vec() }
    }
}

/// Signature container for warrants, proofs, and approvals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureEnvelope {
    pub alg: SigningAlgorithm,
    pub value: Vec<u8>,
}

impl SignatureEnvelope {
    /// Verifies this signature against a signer and message using Ed25519
    /// **strict** verification (rejects non-canonical signatures).
    pub fn verify_strict(&self, signer: &SignerRef, message: &[u8]) -> bool {
        if self.alg != signer.alg || self.alg != SigningAlgorithm::Ed25519 {
            return false;
        }
        let Ok(pk_array) = <&[u8; 32]>::try_from(signer.public_key.as_slice()) else {
            return false;
        };
        let Ok(sig_array) = <&[u8; 64]>::try_from(self.value.as_slice()) else {
            return false;
        };
        let Ok(verifying_key) = VerifyingKey::from_bytes(pk_array) else {
            return false;
        };
        let signature = Signature::from_bytes(sig_array);
        verifying_key.verify_strict(message, &signature).is_ok()
    }
}

impl Serialize for SignatureEnvelope {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("SignatureEnvelope", 2)?;
        state.serialize_field("alg", &self.alg)?;
        state.serialize_field("value", &serde_bytes::ByteBuf::from(self.value.clone()))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SignatureEnvelope {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Inner {
            alg: SigningAlgorithm,
            value: serde_bytes::ByteBuf,
        }
        let inner = Inner::deserialize(deserializer)?;
        Ok(Self { alg: inner.alg, value: inner.value.into_vec() })
    }
}

// ---------------------------------------------------------------------------
// Subjects and assets
// ---------------------------------------------------------------------------

/// Opaque settlement subject that only the Facilitator interprets.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PaymentSubjectRef {
    pub kind: PaymentSubjectKind,
    pub value: String,
}

impl PaymentSubjectRef {
    #[must_use]
    pub fn new(kind: PaymentSubjectKind, value: impl Into<String>) -> Self {
        Self { kind, value: value.into() }
    }
}

/// Supported payment subject kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[non_exhaustive]
pub enum PaymentSubjectKind {
    Caip10,
    FacilitatorAccount,
    ExchangeAccount,
    Opaque,
}

impl Display for PaymentSubjectKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Caip10 => "caip10",
            Self::FacilitatorAccount => "facilitator_account",
            Self::ExchangeAccount => "exchange_account",
            Self::Opaque => "opaque",
        };
        formatter.write_str(value)
    }
}

/// A payment asset allowed by the warrant (CAIP-19 when available).
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AssetRef {
    /// Asset identifier. Prefer CAIP-19 (`eip155:8453/slip44:60:0x8335...`).
    pub asset: String,
    /// Optional network hint (kept for compatibility with legacy fixtures).
    pub network: Option<String>,
}

impl AssetRef {
    #[must_use]
    pub fn new(asset: impl Into<String>, network: Option<String>) -> Self {
        Self { asset: asset.into(), network }
    }

    /// Returns `true` when `candidate` matches this asset.
    #[must_use]
    pub fn matches(&self, candidate: &str, candidate_network: Option<&str>) -> bool {
        if self.asset != candidate {
            return false;
        }
        match (&self.network, candidate_network) {
            (Some(expected), Some(given)) => expected == given,
            _ => true,
        }
    }
}

/// High-level settlement rails allowed by a warrant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[non_exhaustive]
pub enum PaymentRail {
    Onchain,
    Exchange,
    Custodial,
    TraditionalGateway,
}

impl Display for PaymentRail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Onchain => "onchain",
            Self::Exchange => "exchange",
            Self::Custodial => "custodial",
            Self::TraditionalGateway => "traditional_gateway",
        };
        formatter.write_str(value)
    }
}

/// Additional metadata carried in a warrant (application-specific).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WarrantMetadata {
    pub entries: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// Warrant
// ---------------------------------------------------------------------------

/// Signed capability token granting a holder scoped payment authority.
///
/// The signature covers `WARRANT_SIGN_DOMAIN || version || payload_bytes`
/// where `payload_bytes` is the CBOR encoding of the warrant without its
/// signature field. `parent_hash` links a delegated warrant to its parent
/// payload for chain verification (invariant I5).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Warrant {
    /// Payload schema version (`WARRANT_VERSION_V1`).
    pub version: u8,
    /// 16-byte UUIDv7 identifier.
    #[serde(with = "serde_bytes")]
    pub id: Vec<u8>,
    /// Authorized holder of this warrant (the agent).
    pub holder: SignerRef,
    /// Issuer who signed this warrant.
    pub issuer: SignerRef,
    /// Unix seconds when the warrant becomes valid.
    pub issued_at: u64,
    /// Unix seconds when the warrant expires.
    pub expires_at: u64,
    /// Delegation depth of this warrant (0 = root).
    pub depth: u32,
    /// Maximum delegation depth allowed for descendants.
    pub max_depth: u8,
    /// SHA-256 of the parent's payload bytes (None for root warrants).
    pub parent_hash: Option<Vec<u8>>,
    /// Merchant allowlist constraint.
    pub merchant: crate::constraint::MerchantConstraint,
    /// Resource (method/path) constraint.
    pub resource: crate::constraint::ResourceConstraint,
    /// Payment (asset + per-charge cap) constraint.
    pub payment: crate::constraint::PaymentConstraint,
    /// Optional AI tool constraint.
    pub tool: Option<crate::constraint::ToolConstraint>,
    /// Approval gates: tool name -> gate configuration.
    pub approval_gates: BTreeMap<String, crate::approval::ApprovalGate>,
    /// Keys that may approve gated executions.
    pub required_approvers: Vec<SignerRef>,
    /// m-of-n approval threshold (default: all required approvers).
    pub min_approvals: u32,
    /// Application extensions. **Frozen in v1: unknown keys are rejected.**
    pub extensions: BTreeMap<String, Vec<u8>>,
    /// Envelope signature.
    pub signature: SignatureEnvelope,
}

impl CborCodec for Warrant {}

impl Warrant {
    /// Signs this warrant using the issuer's signing key pair.
    #[must_use]
    pub fn sign_with(mut self, issuer_keys: &SigningKeyPair) -> Self {
        let message = self.signing_message();
        self.signature = issuer_keys.sign(message.as_slice());
        self
    }

    /// Returns the SHA-256 digest of the **signed** warrant (payload + signature).
    #[must_use]
    pub fn digest(&self) -> String {
        sha256_prefixed(self.full_cbor_bytes())
    }

    /// Returns the SHA-256 digest of the **unsigned payload** only.
    #[must_use]
    pub fn payload_digest(&self) -> String {
        sha256_prefixed(self.payload_bytes())
    }

    /// CBOR-encodes the payload (all fields except `signature`).
    #[must_use]
    pub fn payload_bytes(&self) -> Vec<u8> {
        let payload = WarrantPayloadRef::from(self);
        let mut bytes = Vec::new();
        #[allow(clippy::expect_used)]
        into_writer(&payload, &mut bytes).expect("warrant payload serialization is infallible");
        bytes
    }

    /// CBOR-encodes the full warrant (payload + signature).
    #[must_use]
    pub fn full_cbor_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        #[allow(clippy::expect_used)]
        into_writer(self, &mut bytes).expect("warrant serialization is infallible");
        bytes
    }

    /// Domain-separated signing message.
    #[must_use]
    pub fn signing_message(&self) -> Vec<u8> {
        let mut message = Vec::with_capacity(WARRANT_SIGN_DOMAIN.len() + 1 + 256);
        message.extend_from_slice(WARRANT_SIGN_DOMAIN);
        message.push(self.version);
        message.extend_from_slice(&self.payload_bytes());
        message
    }

    /// Verifies the envelope signature using **strict** Ed25519 verification.
    #[must_use]
    pub fn verify_signature(&self) -> bool {
        self.signature.verify_strict(&self.issuer, &self.signing_message())
    }

    /// Encodes the warrant as CBOR bytes (delegates to [`CborCodec::encode_cbor`]).
    pub fn encode_cbor(&self) -> WireResult<Vec<u8>> {
        <Self as CborCodec>::encode_cbor(self)
    }

    /// Decodes a warrant from CBOR bytes.
    pub fn decode_cbor(bytes: &[u8]) -> WireResult<Self> {
        <Self as CborCodec>::decode_cbor(bytes)
    }

    /// Returns the human-readable id (hex-encoded).
    #[must_use]
    pub fn id_hex(&self) -> String {
        hex_encode(&self.id)
    }
}

/// Serialization view of the warrant payload (without the signature).
#[derive(Serialize)]
struct WarrantPayloadRef<'a> {
    version: u8,
    #[serde(with = "serde_bytes")]
    id: &'a [u8],
    holder: &'a SignerRef,
    issuer: &'a SignerRef,
    issued_at: u64,
    expires_at: u64,
    depth: u32,
    max_depth: u8,
    parent_hash: Option<&'a Vec<u8>>,
    merchant: &'a crate::constraint::MerchantConstraint,
    resource: &'a crate::constraint::ResourceConstraint,
    payment: &'a crate::constraint::PaymentConstraint,
    tool: Option<&'a crate::constraint::ToolConstraint>,
    approval_gates: &'a BTreeMap<String, crate::approval::ApprovalGate>,
    required_approvers: &'a [SignerRef],
    min_approvals: u32,
    extensions: &'a BTreeMap<String, Vec<u8>>,
}

impl<'a> From<&'a Warrant> for WarrantPayloadRef<'a> {
    fn from(warrant: &'a Warrant) -> Self {
        Self {
            version: warrant.version,
            id: &warrant.id,
            holder: &warrant.holder,
            issuer: &warrant.issuer,
            issued_at: warrant.issued_at,
            expires_at: warrant.expires_at,
            depth: warrant.depth,
            max_depth: warrant.max_depth,
            parent_hash: warrant.parent_hash.as_ref(),
            merchant: &warrant.merchant,
            resource: &warrant.resource,
            payment: &warrant.payment,
            tool: warrant.tool.as_ref(),
            approval_gates: &warrant.approval_gates,
            required_approvers: &warrant.required_approvers,
            min_approvals: warrant.min_approvals,
            extensions: &warrant.extensions,
        }
    }
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}
