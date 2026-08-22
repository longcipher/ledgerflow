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

/// Reserved LedgerFlow warrant extension keys (frozen in v1).
///
/// The extensions map is fail-closed: any key not listed here is rejected
/// when a warrant is decoded from the wire. Reserved prefixes are documented
/// in the design doc (§6.1); applications MUST NOT add new keys without a
/// protocol-version bump.
pub const KNOWN_EXTENSION_KEYS: &[&str] = &[
    // Audit / provenance hints (application metadata, non-authoritative).
    "ledgerflow.agent_id",
    "ledgerflow.session_id",
    "ledgerflow.client_id",
    // Budget-accounting point (P2+; reserved now so the field is forward
    // compatible).
    "ledgerflow.ledger",
    // Human-readable merchant display name (non-authoritative).
    "ledgerflow.merchant_display_name",
    // Issuance bounds constraining what the holder may delegate.
    // See [`crate::issue_bounds::ISSUE_BOUNDS_EXTENSION`].
    crate::issue_bounds::ISSUE_BOUNDS_EXTENSION,
];

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
/// Layout: 48-bit unix-epoch-milliseconds (bytes 0-5), version bits `0111`
/// (byte 6 high nibble), variant bits `10` (byte 8 high nibble), then 62 bits
/// of randomness supplied by the caller (bytes 6 low nibble, 7, 8 low nibble,
/// 9-15). All 16 bytes are filled: 48 timestamp + 4 version + 2 variant +
/// 62 random = 128 bits.
///
/// This is the 128-bit-entropy variant; [`generate_warrant_id`] is retained
/// as a 64-bit-entropy compatibility shim for deterministic fixtures.
#[must_use]
pub const fn generate_warrant_id_128(now_ms: u64, random: [u8; 16]) -> [u8; 16] {
    let mut id = [0_u8; 16];
    let timestamp = now_ms & 0x0000_FFFF_FFFF_FFFF;
    id[0] = (timestamp >> 40) as u8;
    id[1] = (timestamp >> 32) as u8;
    id[2] = (timestamp >> 24) as u8;
    id[3] = (timestamp >> 16) as u8;
    id[4] = (timestamp >> 8) as u8;
    id[5] = timestamp as u8;
    // Byte 6: version 7 (high nibble) plus four random bits; the operands
    // are disjoint so addition and bitwise-or are equivalent here — addition
    // is used to keep the mutation surface unambiguous.
    id[6] = 0x70 + (random[0] & 0x0F);
    id[7] = random[1];
    // Byte 8: variant `10` in the top two bits, six bits from random[2].
    id[8] = 0x80 + (random[2] & 0x3F);
    id[9] = random[3];
    id[10] = random[4];
    id[11] = random[5];
    id[12] = random[6];
    id[13] = random[7];
    id[14] = random[8];
    id[15] = random[9];
    id
}

/// Generates a 16-byte UUIDv7-style warrant identifier from 8 bytes of
/// caller-supplied randomness.
///
/// The 8 random bytes fill the 62 random bits of the UUIDv7 layout; the
/// remaining 6 bits (high nibble of byte 6, high two bits of byte 8) are
/// deterministic version/variant bits. This variant exists for compatibility
/// with deterministic fixtures; production code should use
/// [`generate_warrant_id_128`] for full entropy.
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
    // `random[0] >> 4` is already within the low nibble for u8, so the
    // addition stays disjoint from the version bits above.
    id[6] = 0x70 + (random[0] >> 4);
    id[7] = random[1];
    id[8] = 0x80 + (random[2] >> 4);
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
///
/// The EVM-family variants enable native wallet integration and EIP-8004
/// interop:
///
/// - [`SigningAlgorithm::Secp256k1`]: strict (low-s) ECDSA over `SHA-256(message)`.
///   `SignerRef::public_key` is the 33-byte compressed SEC1 encoding.
/// - [`SigningAlgorithm::EthPersonalSign`]: EIP-191 `personal_sign` semantics. The verification
///   preimage is `keccak256("\x19Ethereum Signed Message:\n" + len(message) + message)`.
///   `SignerRef::public_key` is either the 33-byte compressed pubkey or a 20-byte Ethereum address
///   claim.
/// - [`SigningAlgorithm::EthTypedData`]: EIP-712 semantics. The `message` passed to verification
///   MUST already be the 32-byte typed-data digest (`keccak256(domainSeparator || structHash)`).
///   Key conventions match [`SigningAlgorithm::EthPersonalSign`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[non_exhaustive]
pub enum SigningAlgorithm {
    Ed25519,
    Secp256k1,
    EthPersonalSign,
    EthTypedData,
}

impl SigningAlgorithm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::Secp256k1 => "secp256k1",
            Self::EthPersonalSign => "eth_personal_sign",
            Self::EthTypedData => "eth_typed_data",
        }
    }

    /// Returns `true` for the secp256k1/EVM family of algorithms.
    #[must_use]
    pub const fn is_secp256k1_family(self) -> bool {
        matches!(self, Self::Secp256k1 | Self::EthPersonalSign | Self::EthTypedData)
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
    /// Verifies this signature against a signer and message using **strict**
    /// verification semantics for the envelope's algorithm.
    ///
    /// - Ed25519: `verify_strict` (rejects non-canonical signatures).
    /// - Secp256k1: low-s ECDSA over `SHA-256(message)`.
    /// - EthPersonalSign / EthTypedData: EIP-191 recovery with low-s enforcement; see
    ///   [`SigningAlgorithm`] for key conventions.
    pub fn verify_strict(&self, signer: &SignerRef, message: &[u8]) -> bool {
        if self.alg != signer.alg {
            return false;
        }
        match self.alg {
            SigningAlgorithm::Ed25519 => self.verify_ed25519_strict(signer, message),
            SigningAlgorithm::Secp256k1 |
            SigningAlgorithm::EthPersonalSign |
            SigningAlgorithm::EthTypedData => {
                crate::crypto::verify_secp256k1_family(self.alg, signer, message, &self.value)
            }
        }
    }

    /// The original strict Ed25519 verification path.
    fn verify_ed25519_strict(&self, signer: &SignerRef, message: &[u8]) -> bool {
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
    ///
    /// After decoding, the extensions map is validated: only the reserved
    /// LedgerFlow extension keys (see [`KNOWN_EXTENSION_KEYS`]) are accepted.
    /// Unknown keys are rejected (fail-closed) because the extensions map is
    /// frozen in v1.
    pub fn decode_cbor(bytes: &[u8]) -> WireResult<Self> {
        let warrant = <Self as CborCodec>::decode_cbor(bytes)?;
        for key in warrant.extensions.keys() {
            if !KNOWN_EXTENSION_KEYS.contains(&key.as_str()) {
                return Err(WireError::UnknownExtension { key: key.clone() });
            }
        }
        Ok(warrant)
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

/// Hex-encodes bytes (lowercase). Public helper used by SRL and audit code.
#[must_use]
pub fn hex_encode_bytes(bytes: &[u8]) -> String {
    hex_encode(bytes)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn protocol_constants_have_exact_values() {
        assert_eq!(DEFAULT_MAX_DEPTH, 4);
        assert_eq!(MAX_DELEGATION_DEPTH, 8);
        assert_eq!(DEFAULT_WARRANT_TTL_SECS, 7 * 24 * 60 * 60);
        assert_eq!(MAX_WARRANT_TTL_SECS, 90 * 24 * 60 * 60);
        assert_eq!(MAX_WARRANT_CBOR_BYTES, 64 * 1024);
        assert_eq!(DEFAULT_PROOF_FRESHNESS_MS, 60_000);
        assert_eq!(DEFAULT_CLOCK_SKEW_MS, 30_000);
        assert_eq!(DEFAULT_CHALLENGE_TTL_MS, 300_000);
    }

    #[test]
    fn generate_warrant_id_128_pins_every_byte() {
        let now_ms = 0x01_23_45_67_89_AB;
        let random = [
            0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE,
            0xAF, 0xB0,
        ];
        let id = generate_warrant_id_128(now_ms, random);
        // Bytes 0-5: big-endian unix milliseconds.
        assert_eq!(&id[0..6], &now_ms.to_be_bytes()[2..8]);
        // Byte 6: version 7 in the high nibble, low nibble from random[0].
        assert_eq!(id[6], 0x70 | (random[0] & 0x0F));
        assert_eq!(id[7], random[1]);
        // Byte 8: variant `10` in the top bits, six bits from random[2].
        assert_eq!(id[8], 0x80 | (random[2] & 0x3F));
        // Bytes 9-15 carry random[3..=9].
        assert_eq!(&id[9..16], &random[3..10]);
    }

    #[test]
    fn generate_warrant_id_pins_every_byte() {
        let now_ms = 0x01_23_45_67_89_AB;
        let random = [0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8];
        let id = generate_warrant_id(now_ms, random);
        assert_eq!(&id[0..6], &now_ms.to_be_bytes()[2..8]);
        assert_eq!(id[6], 0x70 | (random[0] >> 4));
        assert_eq!(id[7], random[1]);
        assert_eq!(id[8], 0x80 | (random[2] >> 4));
        assert_eq!(id[9], random[3]);
        assert_eq!(id[10], random[4]);
        assert_eq!(id[11], random[5]);
        assert_eq!(id[12], random[6]);
        assert_eq!(id[13], random[7]);
        assert_eq!(id[14], 0);
        assert_eq!(id[15], 0);
    }

    #[test]
    fn algorithm_strings_are_exact() {
        assert_eq!(SigningAlgorithm::Ed25519.as_str(), "ed25519");
        assert_eq!(SigningAlgorithm::Secp256k1.as_str(), "secp256k1");
        assert_eq!(SigningAlgorithm::EthPersonalSign.as_str(), "eth_personal_sign");
        assert_eq!(SigningAlgorithm::EthTypedData.as_str(), "eth_typed_data");
        for alg in [
            SigningAlgorithm::Ed25519,
            SigningAlgorithm::Secp256k1,
            SigningAlgorithm::EthPersonalSign,
            SigningAlgorithm::EthTypedData,
        ] {
            assert_eq!(format!("{alg}"), alg.as_str());
        }
    }

    #[test]
    fn signing_key_pair_debug_exposes_public_material_only() {
        let keys = SigningKeyPair::from_bytes(&[0x5E; 32]);
        let rendered = format!("{keys:?}");
        assert!(rendered.contains("public_key_hex"));
        assert!(rendered.contains(&hex_encode(&keys.public_key_bytes())));
        assert!(!rendered.contains(&hex_encode(&[0x5E; 32])));
    }

    /// Minimal codec probe with a tiny size limit to exercise the exact
    /// boundary of [`CborCodec::decode_cbor`].
    #[derive(Deserialize, Serialize)]
    struct Probe(Vec<u8>);

    impl CborCodec for Probe {
        fn max_cbor_bytes() -> usize {
            4
        }
    }

    #[test]
    fn cbor_codec_rejects_only_strictly_larger_payloads() {
        // A 3-byte CBOR payload (byte-string header + two bytes).
        let probe = Probe(vec![1, 2]);
        let encoded = probe.encode_cbor().expect("small");
        assert_eq!(encoded.len(), 3);
        assert!(Probe::decode_cbor(&encoded).is_ok());

        // Exactly at the limit (4 bytes): accepted — only strictly larger
        // payloads are rejected (`>` semantics, not `>=`).
        let at_limit = Probe(vec![1, 2, 3]); // header + 3 bytes = 4
        let encoded_at_limit = at_limit.encode_cbor().expect("at limit");
        assert_eq!(encoded_at_limit.len(), Probe::max_cbor_bytes());
        assert!(Probe::decode_cbor(&encoded_at_limit).is_ok());

        // One byte beyond the limit: rejected.
        let beyond = Probe(vec![1, 2, 3, 4]); // header + 4 bytes = 5
        let error = beyond.encode_cbor().ok().and_then(|bytes| Probe::decode_cbor(&bytes).err());
        assert!(matches!(error, Some(WireError::PayloadTooLarge { .. })));
    }

    #[test]
    fn subject_kind_display_strings_are_exact() {
        assert_eq!(format!("{}", PaymentSubjectKind::Caip10), "caip10");
        assert_eq!(format!("{}", PaymentSubjectKind::FacilitatorAccount), "facilitator_account");
        assert_eq!(format!("{}", PaymentSubjectKind::ExchangeAccount), "exchange_account");
        assert_eq!(format!("{}", PaymentSubjectKind::Opaque), "opaque");
    }

    #[test]
    fn payment_rail_display_strings_are_exact() {
        assert_eq!(format!("{}", PaymentRail::Onchain), "onchain");
        assert_eq!(format!("{}", PaymentRail::Exchange), "exchange");
        assert_eq!(format!("{}", PaymentRail::Custodial), "custodial");
        assert_eq!(format!("{}", PaymentRail::TraditionalGateway), "traditional_gateway");
    }

    #[test]
    fn asset_ref_network_matching_is_strict_when_both_present() {
        let asset = AssetRef::new("USDC", Some("base".to_string()));
        // Same network: match.
        assert!(asset.matches("USDC", Some("base")));
        // Different network: no match (guards the Some/Some comparison arm).
        assert!(!asset.matches("USDC", Some("solana")));
        // Missing candidate network: permissive.
        assert!(asset.matches("USDC", None));
        // Different asset never matches.
        assert!(!asset.matches("ETH", Some("base")));
        // Asset without a network hint matches any network.
        let hint_free = AssetRef::new("USDC", None);
        assert!(hint_free.matches("USDC", Some("base")));
        assert!(hint_free.matches("USDC", None));
    }

    fn sample_warrant() -> Warrant {
        crate::typestate::WarrantBuilder::new(2_000)
            .issuer(SigningKeyPair::from_bytes(&[0x7A; 32]).signer_ref())
            .holder(SigningKeyPair::from_bytes(&[0x7B; 32]).signer_ref())
            .merchant(crate::constraint::MerchantConstraint::with_ids(vec![
                "merchant-a".to_string(),
            ]))
            .resource(crate::constraint::ResourceConstraint::default())
            .payment(crate::constraint::PaymentConstraint::new(1_000))
            .sign_with(&SigningKeyPair::from_bytes(&[0x7A; 32]), [0_u8; 8])
    }

    #[test]
    fn digests_are_prefixed_sha256_and_content_bound() {
        let warrant = sample_warrant();
        assert!(warrant.digest().starts_with("sha256:"));
        assert!(warrant.payload_digest().starts_with("sha256:"));
        // Digest over the full envelope differs from the payload-only digest.
        assert_ne!(warrant.digest(), warrant.payload_digest());
        // Identical warrants share digests; tampering changes them.
        let mut tampered = warrant.clone();
        tampered.min_approvals = 7;
        tampered.signature = SigningKeyPair::from_bytes(&[0x7A; 32]).sign(b"resign");
        assert_ne!(tampered.digest(), warrant.digest());
        assert_ne!(tampered.payload_digest(), warrant.payload_digest());
    }

    #[test]
    fn id_hex_is_lowercase_full_hex() {
        let mut warrant = sample_warrant();
        warrant.id = vec![0xDE; 16];
        let hex_id = warrant.id_hex();
        assert_eq!(hex_id.len(), 32);
        assert!(hex_id.starts_with("dede"));
        assert_eq!(hex_id.to_lowercase(), hex_id);
    }

    #[test]
    fn signing_message_carries_domain_version_and_payload() {
        let warrant = sample_warrant();
        let message = warrant.signing_message();
        assert!(message.starts_with(WARRANT_SIGN_DOMAIN));
        // One version byte between the domain and the CBOR payload.
        assert_eq!(message[WARRANT_SIGN_DOMAIN.len()], WARRANT_VERSION_V1);
        assert!(message.len() > WARRANT_SIGN_DOMAIN.len() + 1);
        assert_eq!(&message[WARRANT_SIGN_DOMAIN.len() + 1..], &warrant.payload_bytes()[..]);
        // Payload bytes are non-empty and decode back to an equal warrant.
        let payload = warrant.payload_bytes();
        assert!(!payload.is_empty());
        let full = warrant.full_cbor_bytes();
        assert!(!full.is_empty());
        assert_ne!(full, payload);
        let decoded = Warrant::decode_cbor(&full).expect("roundtrip");
        assert_eq!(decoded, warrant);
    }

    #[test]
    fn verify_signature_rejects_tampered_payload() {
        let mut warrant = sample_warrant();
        assert!(warrant.verify_signature());
        warrant.expires_at += 1;
        assert!(!warrant.verify_signature());
    }
}
