//! W3C Verifiable Credential (JSON-LD) dual representation of warrants.
//!
//! The canonical LedgerFlow wire format is deterministic CBOR. This module
//! adds an **interoperability view**: the same signed warrant rendered as a
//! W3C VC 2.0-style JSON-LD document (`LedgerFlowWarrant`), suitable for
//! AP2-style ecosystems and generic VC tooling to *read and verify*.
//!
//! Design rules:
//!
//! - The CBOR payload travels **verbatim** inside `credentialSubject.warrantCbor` (base64url,
//!   unpadded), so the envelope signature remains the single source of truth. The mirrored
//!   human-readable fields are informational.
//! - `proof.proofValue` mirrors the envelope signature bytes; verification re-checks it against the
//!   decoded warrant (fail-closed).
//! - Identities are expressed as `did:key` values (Ed25519 `0xed01`, secp256k1 `0xe701`). Signers
//!   identified only by a 20-byte Ethereum address claim render as `did:ledgerflow:addr:0x…`.
//! - Parsing rejects unknown top-level members and missing context/type entries, matching the
//!   protocol's fail-closed style.

use base64::Engine as _;
use ledgerflow_core::{SigningAlgorithm, Warrant, warrant::SignerRef};
use serde::{Deserialize, Serialize};

use crate::error::ProtocolError;

/// LedgerFlow JSON-LD context term appended to the W3C credentials context.
pub const VC_CONTEXT_WARRANT_V1: &str = "https://ledgerflow.org/contexts/warrant-v1.jsonld";

/// Credential type added next to `VerifiableCredential`.
pub const CREDENTIAL_TYPE_WARRANT: &str = "LedgerFlowWarrant";

/// Multicodec prefix for Ed25519 public keys.
const MULTICODEC_ED25519: [u8; 2] = [0xED, 0x01];
/// Multicodec prefix for secp256k1 public keys.
const MULTICODEC_SECP256K1: [u8; 2] = [0xE7, 0x01];

/// Key family returned by [`parse_did_key`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerFlowAlg {
    /// Ed25519 (`did:key` multicodec `0xed01`).
    Ed25519,
    /// secp256k1 (`did:key` multicodec `0xe701`).
    Secp256k1,
}

/// The `credentialSubject` block: mirrored display fields plus the verbatim
/// CBOR warrant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CredentialSubject {
    /// Holder identity as a DID string.
    #[serde(rename = "holderDid")]
    pub holder_did: String,
    /// Delegation depth of this warrant (0 = root).
    pub depth: u32,
    /// Maximum delegation depth allowed for descendants.
    #[serde(rename = "maxDepth")]
    pub max_depth: u8,
    /// Expiry as unix seconds (numeric mirror of `validUntil`).
    #[serde(rename = "expiresAtEpoch")]
    pub expires_at_epoch: u64,
    /// Merchant allowlist mirror.
    pub merchant: serde_json::Value,
    /// Resource scope mirror.
    pub resource: serde_json::Value,
    /// Payment constraint mirror.
    pub payment: serde_json::Value,
    /// Optional tool allowlist mirror.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<serde_json::Value>,
    /// Verbatim base64url (unpadded) CBOR encoding of the full warrant.
    #[serde(rename = "warrantCbor")]
    pub warrant_cbor: String,
}

/// The DataIntegrity-style proof block mirroring the envelope signature.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Proof {
    /// Fixed to `DataIntegrityProof`.
    #[serde(rename = "type")]
    pub proof_type: String,
    /// LedgerFlow cryptosuite identifier for the envelope algorithm.
    pub cryptosuite: String,
    /// DID of the verifying method (the issuer).
    #[serde(rename = "verificationMethod")]
    pub verification_method: String,
    /// Creation time (mirrors `validFrom`).
    pub created: String,
    /// Base64url (unpadded) envelope signature bytes.
    #[serde(rename = "proofValue")]
    pub proof_value: String,
}

/// A `LedgerFlowWarrant` verifiable credential.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WarrantCredential {
    /// JSON-LD contexts (W3C credentials v2 + LedgerFlow warrant context).
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    /// Credential types (`VerifiableCredential` + `LedgerFlowWarrant`).
    #[serde(rename = "type")]
    pub types: Vec<String>,
    /// `urn:uuid:` id derived from the warrant id bytes.
    pub id: String,
    /// Issuer DID.
    pub issuer: String,
    /// Start of validity (RFC 3339).
    #[serde(rename = "validFrom")]
    pub valid_from: String,
    /// End of validity (RFC 3339).
    #[serde(rename = "validUntil")]
    pub valid_until: String,
    /// Subject block.
    #[serde(rename = "credentialSubject")]
    pub subject: CredentialSubject,
    /// Proof block.
    pub proof: Proof,
}

/// Renders a warrant as its verifiable-credential view.
#[must_use]
pub fn to_credential(warrant: &Warrant) -> WarrantCredential {
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let subject = CredentialSubject {
        holder_did: signer_did(&warrant.holder),
        depth: warrant.depth,
        max_depth: warrant.max_depth,
        expires_at_epoch: warrant.expires_at,
        merchant: serde_json::json!({
            "merchantIds": warrant.merchant.merchant_ids,
            "hostSuffixes": warrant.merchant.host_suffixes,
        }),
        resource: serde_json::json!({
            "httpMethods": warrant.resource.http_methods,
            "pathPrefixes": warrant.resource.path_prefixes,
        }),
        payment: serde_json::json!({
            "allowedAssets": warrant.payment.allowed_assets.iter().map(|asset| asset.asset.clone()).collect::<Vec<_>>(),
            "maxPerCharge": warrant.payment.max_per_charge.to_string(),
            "payeeIds": warrant.payment.payee_ids,
        }),
        tool: warrant.tool.as_ref().map(|tool| {
            serde_json::json!({
                "toolNames": tool.tool_names,
                "modelProviders": tool.model_providers,
                "actionLabels": tool.action_labels,
            })
        }),
        warrant_cbor: engine.encode(warrant.full_cbor_bytes()),
    };
    let issuer_did = signer_did(&warrant.issuer);
    WarrantCredential {
        context: vec![
            "https://www.w3.org/ns/credentials/v2".to_string(),
            VC_CONTEXT_WARRANT_V1.to_string(),
        ],
        types: vec!["VerifiableCredential".to_string(), CREDENTIAL_TYPE_WARRANT.to_string()],
        id: format!("urn:uuid:{}", hyphenate_uuid(&warrant.id)),
        issuer: issuer_did.clone(),
        valid_from: unix_secs_to_rfc3339(warrant.issued_at),
        valid_until: unix_secs_to_rfc3339(warrant.expires_at),
        proof: Proof {
            proof_type: "DataIntegrityProof".to_string(),
            cryptosuite: cryptosuite_for(warrant.signature.alg).to_string(),
            verification_method: issuer_did,
            created: unix_secs_to_rfc3339(warrant.issued_at),
            proof_value: engine.encode(&warrant.signature.value),
        },
        subject,
    }
}

/// Serializes a credential to compact JSON.
///
/// # Errors
/// Returns [`ProtocolError::Serialization`] when the value cannot be
/// serialized (practically infallible).
pub fn credential_to_json(credential: &WarrantCredential) -> Result<String, ProtocolError> {
    serde_json::to_string(credential)
        .map_err(|error| ProtocolError::Serialization(error.to_string()))
}

/// Parses a credential from JSON, rejecting unknown top-level members and
/// missing required context/type entries (fail-closed).
///
/// # Errors
/// Returns [`ProtocolError::VcInvalid`] for structural violations and
/// [`ProtocolError::Deserialization`] for malformed JSON.
pub fn credential_from_json(json: &str) -> Result<WarrantCredential, ProtocolError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|error| ProtocolError::Deserialization(error.to_string()))?;
    let map = value
        .as_object()
        .ok_or_else(|| ProtocolError::VcInvalid("credential must be a JSON object".to_string()))?;
    const KNOWN: &[&str] = &[
        "@context",
        "type",
        "id",
        "issuer",
        "validFrom",
        "validUntil",
        "credentialSubject",
        "proof",
    ];
    for key in map.keys() {
        if !KNOWN.contains(&key.as_str()) {
            return Err(ProtocolError::VcInvalid(format!("unknown member `{key}`")));
        }
    }
    let credential: WarrantCredential = serde_json::from_value(value)
        .map_err(|error| ProtocolError::Deserialization(error.to_string()))?;
    if !credential.context.contains(&"https://www.w3.org/ns/credentials/v2".to_string()) ||
        !credential.context.contains(&VC_CONTEXT_WARRANT_V1.to_string())
    {
        return Err(ProtocolError::VcInvalid("missing required @context entry".to_string()));
    }
    if !credential.types.contains(&"VerifiableCredential".to_string()) ||
        !credential.types.contains(&CREDENTIAL_TYPE_WARRANT.to_string())
    {
        return Err(ProtocolError::VcInvalid("missing required type entry".to_string()));
    }
    Ok(credential)
}

/// Reconstructs and fully verifies the warrant embedded in a credential.
///
/// Verification steps (each failure is distinct):
///
/// 1. Decode `warrantCbor` and run the frozen-extension-aware [`Warrant::decode_cbor`].
/// 2. Verify the warrant envelope signature.
/// 3. Check `proof.proofValue` equals the current signature bytes.
/// 4. Check `issuer` and `credentialSubject.holderDid` match re-derived DIDs.
///
/// # Errors
/// Returns [`ProtocolError`] variants describing the failed step.
pub fn warrant_from_credential(credential: &WarrantCredential) -> Result<Warrant, ProtocolError> {
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let cbor = engine
        .decode(credential.subject.warrant_cbor.as_bytes())
        .map_err(|error| ProtocolError::VcInvalid(format!("bad warrantCbor: {error}")))?;
    let warrant =
        Warrant::decode_cbor(&cbor).map_err(|error| ProtocolError::VcInvalid(error.to_string()))?;
    if !warrant.verify_signature() {
        return Err(ProtocolError::VcInvalid("warrant envelope signature invalid".to_string()));
    }
    let expected_proof = engine.encode(&warrant.signature.value);
    if credential.proof.proof_value != expected_proof {
        return Err(ProtocolError::VcInvalid("proofValue does not match signature".to_string()));
    }
    if credential.issuer != signer_did(&warrant.issuer) {
        return Err(ProtocolError::VcInvalid("issuer does not match warrant".to_string()));
    }
    if credential.subject.holder_did != signer_did(&warrant.holder) {
        return Err(ProtocolError::VcInvalid("holderDid does not match warrant".to_string()));
    }
    Ok(warrant)
}

/// Composes [`to_credential`] + [`credential_to_json`].
///
/// # Errors
/// See [`credential_to_json`].
pub fn warrant_to_vc_json(warrant: &Warrant) -> Result<String, ProtocolError> {
    credential_to_json(&to_credential(warrant))
}

/// Composes [`credential_from_json`] + [`warrant_from_credential`].
///
/// # Errors
/// See the composed functions.
pub fn warrant_from_vc_json(json: &str) -> Result<Warrant, ProtocolError> {
    warrant_from_credential(&credential_from_json(json)?)
}

/// Encodes an Ed25519 public key as `did:key:z<multibase>` (multicodec
/// `0xed01`, base58btc).
#[must_use]
pub fn did_key_ed25519(public_key: &[u8; 32]) -> String {
    did_key(MULTICODEC_ED25519, public_key)
}

/// Encodes a compressed secp256k1 public key as `did:key:z<multibase>`
/// (multicodec `0xe701`, base58btc).
#[must_use]
pub fn did_key_secp256k1(compressed: &[u8; 33]) -> String {
    did_key(MULTICODEC_SECP256K1, compressed)
}

fn did_key(prefix: [u8; 2], key: &[u8]) -> String {
    let mut bytes = Vec::with_capacity(2 + key.len());
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(key);
    format!("did:key:z{}", bs58::encode(bytes).into_string())
}

/// Parses a `did:key:z…` value into its key family and raw key bytes.
///
/// # Errors
/// Returns [`ProtocolError::VcInvalid`] for malformed DIDs or unknown
/// multicodec prefixes.
pub fn parse_did_key(did: &str) -> Result<(LedgerFlowAlg, Vec<u8>), ProtocolError> {
    let encoded = did
        .strip_prefix("did:key:z")
        .ok_or_else(|| ProtocolError::VcInvalid(format!("not a did:key value: `{did}`")))?;
    let bytes = bs58::decode(encoded)
        .into_vec()
        .map_err(|error| ProtocolError::VcInvalid(format!("bad multibase: {error}")))?;
    if bytes.starts_with(&MULTICODEC_ED25519) && bytes.len() == 34 {
        return Ok((LedgerFlowAlg::Ed25519, bytes[2..].to_vec()));
    }
    if bytes.starts_with(&MULTICODEC_SECP256K1) && bytes.len() == 35 {
        return Ok((LedgerFlowAlg::Secp256k1, bytes[2..].to_vec()));
    }
    Err(ProtocolError::VcInvalid("unsupported multicodec prefix".to_string()))
}

/// Derives the DID string for a signer reference.
///
/// Ed25519 signers require 32-byte keys; secp256k1-family signers accept
/// either a 33-byte compressed key (`did:key`) or a 20-byte Ethereum address
/// claim (`did:ledgerflow:addr:0x…`).
#[must_use]
pub fn signer_did(signer: &SignerRef) -> String {
    match signer.alg {
        SigningAlgorithm::Ed25519 => {
            let Ok(key) = <&[u8; 32]>::try_from(signer.public_key.as_slice()) else {
                return opaque_did(signer);
            };
            did_key_ed25519(key)
        }
        SigningAlgorithm::Secp256k1 |
        SigningAlgorithm::EthPersonalSign |
        SigningAlgorithm::EthTypedData => match signer.public_key.len() {
            33 => {
                // Unchecked conversion is safe: length verified above.
                let mut key = [0_u8; 33];
                key.copy_from_slice(&signer.public_key);
                did_key_secp256k1(&key)
            }
            20 => format!(
                "did:ledgerflow:addr:0x{}",
                ledgerflow_core::hex_encode_bytes(&signer.public_key)
            ),
            _ => opaque_did(signer),
        },
        _ => opaque_did(signer),
    }
}

fn opaque_did(signer: &SignerRef) -> String {
    format!(
        "did:ledgerflow:opaque:{}:{}",
        signer.alg.as_str(),
        ledgerflow_core::hex_encode_bytes(&signer.public_key)
    )
}

const fn cryptosuite_for(alg: SigningAlgorithm) -> &'static str {
    match alg {
        SigningAlgorithm::Ed25519 => "ledgerflow-ed25519-v1",
        SigningAlgorithm::Secp256k1 |
        SigningAlgorithm::EthPersonalSign |
        SigningAlgorithm::EthTypedData => "ledgerflow-secp256k1-v1",
        _ => "ledgerflow-unknown-v1",
    }
}

/// Formats 16 bytes as a lowercase hyphenated UUID (`8-4-4-4-12`).
#[must_use]
pub fn hyphenate_uuid(bytes: &[u8]) -> String {
    let hex = ledgerflow_core::hex_encode_bytes(bytes);
    if hex.len() < 32 {
        return hex;
    }
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// Formats unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC, proleptic Gregorian).
///
/// Uses Howard Hinnant's `civil_from_days` algorithm; no external time
/// dependencies.
#[must_use]
pub fn unix_secs_to_rfc3339(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX);
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = rem / 3_600;
    let minute = (rem % 3_600) / 60;
    let second = rem % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Converts days since 1970-01-01 to a `(year, month, day)` triple.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, u32::try_from(month).unwrap_or(0), u32::try_from(day).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ledgerflow_core::{
        MerchantConstraint, PaymentConstraint, ResourceConstraint, SignatureEnvelope,
        SigningKeyPair, WarrantBuilder,
    };

    use super::*;

    fn fixture_warrant() -> Warrant {
        let issuer = SigningKeyPair::from_bytes(&[0x71; 32]);
        let holder = SigningKeyPair::from_bytes(&[0x72; 32]);
        WarrantBuilder::new(1_700_000_000)
            .ttl_secs(3_600)
            .max_depth(2)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]))
            .resource(ResourceConstraint::default())
            .payment(PaymentConstraint::new(1_000))
            .sign_with(&issuer, [0_u8; 8])
    }

    #[test]
    fn round_trip_preserves_warrant_equality() {
        let warrant = fixture_warrant();
        let json = warrant_to_vc_json(&warrant).expect("serialize");
        let parsed = warrant_from_vc_json(&json).expect("verify");
        assert_eq!(parsed, warrant);
    }

    #[test]
    fn credential_shape_is_json_ld_correct() {
        let json = warrant_to_vc_json(&fixture_warrant()).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert_eq!(value["@context"][0], "https://www.w3.org/ns/credentials/v2");
        assert_eq!(value["@context"][1], VC_CONTEXT_WARRANT_V1);
        assert_eq!(value["type"][1], CREDENTIAL_TYPE_WARRANT);
        assert!(value["id"].as_str().expect("id").starts_with("urn:uuid:"));
        assert!(value["issuer"].as_str().expect("issuer").starts_with("did:key:z"));
        assert_eq!(value["proof"]["type"], "DataIntegrityProof");
        assert_eq!(value["proof"]["cryptosuite"], "ledgerflow-ed25519-v1");
        assert!(value["credentialSubject"]["warrantCbor"].as_str().is_some());
    }

    #[test]
    fn tampered_cbor_fails_signature_check() {
        let warrant = fixture_warrant();
        let mut credential = to_credential(&warrant);
        // Flip one byte inside the CBOR payload region (after the header).
        let mut raw =
            URL_SAFE_NO_PAD.decode(credential.subject.warrant_cbor.as_bytes()).expect("decode");
        let last = raw.len() - 1;
        raw[last] ^= 0xFF;
        credential.subject.warrant_cbor = URL_SAFE_NO_PAD.encode(raw);
        let error = warrant_from_credential(&credential).expect_err("tampered");
        assert!(matches!(error, ProtocolError::VcInvalid(_)));
    }

    #[test]
    fn tampered_proof_value_is_distinct_error() {
        let warrant = fixture_warrant();
        let mut credential = to_credential(&warrant);
        credential.proof.proof_value = URL_SAFE_NO_PAD.encode([0_u8; 64]);
        let error = warrant_from_credential(&credential).expect_err("proof mismatch");
        assert!(matches!(error, ProtocolError::VcInvalid(_)));
    }

    #[test]
    fn swapped_issuer_is_rejected() {
        let warrant = fixture_warrant();
        let mut credential = to_credential(&warrant);
        credential.issuer = signer_did(&warrant.holder);
        let error = warrant_from_credential(&credential).expect_err("issuer swap");
        assert!(matches!(error, ProtocolError::VcInvalid(_)));
    }

    #[test]
    fn unknown_top_level_member_rejected() {
        let warrant = fixture_warrant();
        let mut value: serde_json::Value =
            serde_json::from_str(&warrant_to_vc_json(&warrant).expect("json")).expect("value");
        value["evil"] = serde_json::json!(1);
        let error = credential_from_json(&value.to_string()).expect_err("unknown member");
        assert!(matches!(error, ProtocolError::VcInvalid(_)));
    }

    #[test]
    fn missing_context_entry_rejected() {
        let warrant = fixture_warrant();
        let mut credential = to_credential(&warrant);
        credential.context.remove(1);
        let error = credential_from_json(&credential_to_json(&credential).expect("json"))
            .expect_err("context");
        assert!(matches!(error, ProtocolError::VcInvalid(_)));
    }

    #[test]
    fn did_key_vectors_round_trip() {
        let ed = [0xAB_u8; 32];
        let ed_did = did_key_ed25519(&ed);
        assert!(ed_did.starts_with("did:key:z"));
        let (alg, key) = parse_did_key(&ed_did).expect("parse");
        assert_eq!(alg, LedgerFlowAlg::Ed25519);
        assert_eq!(key, ed);

        let sec = [0xCD_u8; 33];
        let sec_did = did_key_secp256k1(&sec);
        assert!(sec_did.starts_with("did:key:z"));
        let (alg, key) = parse_did_key(&sec_did).expect("parse");
        assert_eq!(alg, LedgerFlowAlg::Secp256k1);
        assert_eq!(key, sec);

        // Distinct prefixes produce distinct DIDs.
        assert_ne!(ed_did, sec_did);
        assert!(parse_did_key("did:key:zzzz").is_err());
        assert!(parse_did_key("did:web:example.com").is_err());
    }

    #[test]
    fn address_claim_signers_render_as_ledgerflow_dids() {
        let signer = SignerRef {
            alg: SigningAlgorithm::EthPersonalSign,
            public_key: vec![0x11; 20],
            key_id: None,
        };
        assert_eq!(
            signer_did(&signer),
            "did:ledgerflow:addr:0x1111111111111111111111111111111111111111"
        );
        // An unsigned literal warrant still renders (display-only view).
        let holder = signer.clone();
        let warrant = Warrant {
            version: 1,
            id: vec![0_u8; 16],
            holder,
            issuer: signer,
            issued_at: 0,
            expires_at: 1,
            depth: 0,
            max_depth: 0,
            parent_hash: None,
            merchant: MerchantConstraint::default(),
            resource: ResourceConstraint::default(),
            payment: PaymentConstraint::new(1),
            tool: None,
            approval_gates: std::collections::BTreeMap::new(),
            required_approvers: Vec::new(),
            min_approvals: 0,
            extensions: std::collections::BTreeMap::new(),
            signature: SignatureEnvelope {
                alg: SigningAlgorithm::EthPersonalSign,
                value: vec![0; 65],
            },
        };
        let credential = to_credential(&warrant);
        assert!(credential.issuer.starts_with("did:ledgerflow:addr:"));
        assert_eq!(credential.proof.cryptosuite, "ledgerflow-secp256k1-v1");
    }

    #[test]
    fn uuid_hyphenation_layout() {
        let bytes = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ];
        assert_eq!(hyphenate_uuid(&bytes), "01234567-89ab-cdef-fedc-ba9876543210");
        assert_eq!(hyphenate_uuid(&[0_u8; 16]), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn rfc3339_formatting_vectors() {
        assert_eq!(unix_secs_to_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_secs_to_rfc3339(86_399), "1970-01-01T23:59:59Z");
        assert_eq!(unix_secs_to_rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(unix_secs_to_rfc3339(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(unix_secs_to_rfc3339(2_145_830_400), "2037-12-31T00:00:00Z");
        assert_eq!(unix_secs_to_rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
