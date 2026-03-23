//! x402-compatible LedgerFlow extension types and helpers.

use std::io::Cursor;

use ciborium::{de::from_reader, ser::into_writer};
use ledgerflow_core::{
    PaymentSubjectRef, Proof, SignerRef, SigningKeyPair, Warrant, sha256_prefixed,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

/// Maximum accepted size for serialized LedgerFlow extension payloads.
pub const MAX_LEDGERFLOW_EXTENSION_BYTES: usize = 32 * 1024;

/// Errors returned while encoding or decoding LedgerFlow extension payloads.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExtensionCodecError {
    #[error("payload size {size} exceeds the maximum supported size {max}")]
    PayloadTooLarge { size: usize, max: usize },
    #[error("failed to encode the extension as CBOR: {0}")]
    Serialization(String),
    #[error("failed to decode the extension from CBOR: {0}")]
    Deserialization(String),
}

/// Minimal HTTP request context needed for canonical request binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRequest {
    pub method: String,
    pub authority: String,
    pub path_and_query: String,
    pub body: Vec<u8>,
}

impl HttpRequest {
    #[must_use]
    pub fn new(
        method: impl Into<String>,
        authority: impl Into<String>,
        path_and_query: impl Into<String>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            method: method.into(),
            authority: authority.into(),
            path_and_query: path_and_query.into(),
            body: body.into(),
        }
    }
}

/// Minimal x402 `accepted` quote representation used by the MVP.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedQuote {
    pub scheme: String,
    pub asset: String,
    pub amount: u64,
    pub payee_id: String,
    pub network: Option<String>,
}

impl AcceptedQuote {
    #[must_use]
    pub fn exact(
        asset: impl Into<String>,
        amount: u64,
        payee_id: impl Into<String>,
        network: Option<String>,
    ) -> Self {
        Self {
            scheme: "exact".to_string(),
            asset: asset.into(),
            amount,
            payee_id: payee_id.into(),
            network,
        }
    }

    #[must_use]
    pub fn canonical(&self) -> String {
        let network = self.network.as_deref().unwrap_or("-");
        format!(
            "scheme={};asset={};amount={};payee_id={};network={network}",
            self.scheme, self.asset, self.amount, self.payee_id
        )
    }
}

/// Merchant `402 Payment Required` response with a LedgerFlow challenge extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRequiredResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub accepted: Vec<AcceptedQuote>,
    pub ledgerflow: Option<LedgerFlowChallenge>,
}

/// Agent payment payload that remains x402-shaped while carrying LedgerFlow data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentPayload {
    pub accepted: AcceptedQuote,
    pub settlement_payload: String,
    pub payment_identifier: Option<String>,
    pub ledgerflow: Option<LedgerFlowAuthorizationExtension>,
}

impl PaymentPayload {
    #[must_use]
    pub fn payment_identifier(&self) -> Option<&str> {
        self.payment_identifier.as_deref()
    }
}

/// Merchant-advertised LedgerFlow challenge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LedgerFlowChallenge {
    pub version: String,
    pub challenge_id: String,
    pub merchant_id: String,
    pub resource: String,
    pub proof_freshness_ms: u64,
    pub required_subject_kinds: Vec<String>,
}

impl LedgerFlowChallenge {
    pub fn encode_cbor(&self) -> Result<Vec<u8>, ExtensionCodecError> {
        encode_cbor(self)
    }

    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, ExtensionCodecError> {
        decode_cbor(bytes)
    }
}

/// Agent-sent LedgerFlow authorization extension.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LedgerFlowAuthorizationExtension {
    pub version: String,
    pub challenge_id: String,
    pub warrant: WarrantTransport,
    pub proof: Proof,
    pub signer: SignerRef,
    pub payment_subject: PaymentSubjectRef,
}

impl LedgerFlowAuthorizationExtension {
    pub fn encode_cbor(&self) -> Result<Vec<u8>, ExtensionCodecError> {
        encode_cbor(self)
    }

    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, ExtensionCodecError> {
        decode_cbor(bytes)
    }
}

/// Inline-or-digest warrant transport used by LedgerFlow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WarrantTransport {
    pub digest: String,
    pub inline: Option<Warrant>,
}

fn encode_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, ExtensionCodecError> {
    let mut bytes = Vec::new();
    into_writer(value, &mut bytes)
        .map_err(|error| ExtensionCodecError::Serialization(error.to_string()))?;
    Ok(bytes)
}

fn decode_cbor<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ExtensionCodecError> {
    if bytes.len() > MAX_LEDGERFLOW_EXTENSION_BYTES {
        return Err(ExtensionCodecError::PayloadTooLarge {
            size: bytes.len(),
            max: MAX_LEDGERFLOW_EXTENSION_BYTES,
        });
    }

    let mut cursor = Cursor::new(bytes);
    from_reader(&mut cursor)
        .map_err(|error| ExtensionCodecError::Deserialization(error.to_string()))
}

impl WarrantTransport {
    #[must_use]
    pub fn inline(warrant: Warrant) -> Self {
        let digest = warrant.digest();
        Self { digest, inline: Some(warrant) }
    }

    #[must_use]
    pub fn digest_ref(digest: impl Into<String>) -> Self {
        Self { digest: digest.into(), inline: None }
    }
}

/// Inputs that vary per payment payload while the x402 shape stays fixed.
#[derive(Clone, Debug)]
pub struct PaymentPayloadSeed {
    pub payment_subject: PaymentSubjectRef,
    pub signer: SigningKeyPair,
    pub created_at_ms: u64,
    pub nonce: String,
    pub payment_identifier: Option<String>,
}

/// Creates a standard x402 `402 Payment Required` response with a LedgerFlow challenge.
#[must_use]
pub fn merchant_payment_required(
    challenge_id: impl Into<String>,
    merchant_id: impl Into<String>,
    resource: impl Into<String>,
    accepted: Vec<AcceptedQuote>,
    proof_freshness_ms: u64,
) -> PaymentRequiredResponse {
    PaymentRequiredResponse {
        status_code: 402,
        headers: vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-payment-required".to_string(), "x402".to_string()),
        ],
        accepted,
        ledgerflow: Some(LedgerFlowChallenge {
            version: "lfx402/v1".to_string(),
            challenge_id: challenge_id.into(),
            merchant_id: merchant_id.into(),
            resource: resource.into(),
            proof_freshness_ms,
            required_subject_kinds: vec!["signer".to_string(), "payment_subject".to_string()],
        }),
    }
}

/// Builds an x402 payment payload that echoes the selected quote and adds LedgerFlow authz data.
#[must_use]
pub fn build_payment_payload(
    challenge: &LedgerFlowChallenge,
    request: &HttpRequest,
    accepted: AcceptedQuote,
    warrant: WarrantTransport,
    seed: PaymentPayloadSeed,
) -> PaymentPayload {
    let accepted_hash = canonical_accepted_hash(&accepted);
    let request_hash = canonical_request_hash(request);
    let warrant_digest = match &warrant.inline {
        Some(inline) => inline.digest(),
        None => warrant.digest.clone(),
    };
    let proof = Proof::new_signed(
        challenge.challenge_id.clone(),
        warrant_digest,
        accepted_hash,
        request_hash,
        seed.created_at_ms,
        seed.nonce.clone(),
        &seed.signer,
    );

    PaymentPayload {
        accepted,
        settlement_payload: "x402-payment-payload".to_string(),
        payment_identifier: seed.payment_identifier.clone(),
        ledgerflow: Some(LedgerFlowAuthorizationExtension {
            version: "lfx402/v1".to_string(),
            challenge_id: challenge.challenge_id.clone(),
            warrant,
            proof,
            signer: seed.signer.signer_ref(),
            payment_subject: seed.payment_subject,
        }),
    }
}

/// Computes the canonical request hash used by LedgerFlow proof binding.
#[must_use]
pub fn canonical_request_hash(request: &HttpRequest) -> String {
    let body_hash = sha256_prefixed(&request.body);
    let preimage = format!(
        "{}\n{}\n{}\n{body_hash}",
        request.method.to_uppercase(),
        request.authority.to_lowercase(),
        request.path_and_query
    );
    sha256_prefixed(preimage)
}

/// Computes the canonical digest of the selected x402 `accepted` quote.
#[must_use]
pub fn canonical_accepted_hash(accepted: &AcceptedQuote) -> String {
    sha256_prefixed(accepted.canonical())
}

#[cfg(test)]
mod tests {
    use ledgerflow_core::{
        AmountLimit, AssetRef, AudienceScope, Constraint, DelegationPolicy, MerchantConstraint,
        PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef, ResourceConstraint,
        SigningKeyPair, SponsorshipConstraint, ToolConstraint, WARRANT_VERSION_V1, Warrant,
        WarrantMetadata,
    };
    use proptest::prelude::*;

    use super::{
        AcceptedQuote, HttpRequest, PaymentPayloadSeed, WarrantTransport, build_payment_payload,
        canonical_accepted_hash, canonical_request_hash, merchant_payment_required,
    };

    fn issuer_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[1u8; 32])
    }

    fn agent_keys() -> SigningKeyPair {
        SigningKeyPair::from_bytes(&[2u8; 32])
    }

    fn sample_warrant() -> Warrant {
        let issuer = issuer_keys();
        let agent = agent_keys();
        Warrant {
            version: WARRANT_VERSION_V1,
            warrant_id: "warrant-1".to_string(),
            issuer: issuer.signer_ref(),
            subject_signer: agent.signer_ref(),
            payment_subjects: vec![PaymentSubjectRef::new(
                PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            )],
            audience: AudienceScope::MerchantIds(vec!["merchant-a".to_string()]),
            not_before_ms: 1_000,
            expires_at_ms: 10_000,
            delegation: DelegationPolicy { can_delegate: true, max_depth: 1 },
            constraints: vec![
                Constraint::Merchant(MerchantConstraint {
                    merchant_ids: vec!["merchant-a".to_string()],
                    host_suffixes: vec![],
                }),
                Constraint::Resource(ResourceConstraint {
                    http_methods: vec!["POST".to_string()],
                    path_prefixes: vec!["/pay".to_string()],
                }),
                Constraint::Tool(ToolConstraint {
                    tool_names: vec!["web-search".to_string()],
                    model_providers: vec![],
                    action_labels: vec![],
                }),
                Constraint::Payment(PaymentConstraint {
                    max_per_request: AmountLimit { amount: 200 },
                    period_limit: None,
                    allowed_assets: vec![AssetRef::new("USDC", Some("base".to_string()))],
                    allowed_rails: vec![PaymentRail::Onchain],
                    allowed_schemes: vec!["exact".to_string()],
                    payee_ids: vec!["merchant-a".to_string()],
                }),
                Constraint::Sponsorship(SponsorshipConstraint {
                    allow_sponsored_execution: false,
                    sponsor_ids: vec![],
                }),
            ],
            metadata: WarrantMetadata::default(),
            signature: issuer.sign(b"placeholder"),
        }
        .sign_with(&issuer)
    }

    #[test]
    fn merchant_response_keeps_standard_x402_fields_and_adds_challenge_extension() {
        let accepted =
            vec![AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()))];
        let response = merchant_payment_required(
            "challenge-1",
            "merchant-a",
            "/pay",
            accepted.clone(),
            60_000,
        );

        assert_eq!(response.status_code, 402);
        assert_eq!(response.accepted, accepted);
        assert!(response.headers.iter().any(|(name, _)| name == "x-payment-required"));
        assert_eq!(
            response.ledgerflow.expect("ledgerflow challenge").required_subject_kinds,
            vec!["signer".to_string(), "payment_subject".to_string()]
        );
    }

    #[test]
    fn agent_payload_preserves_selected_quote_and_adds_ledgerflow_extension() {
        let accepted = AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()));
        let request =
            HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec());
        let challenge = merchant_payment_required(
            "challenge-1",
            "merchant-a",
            "/pay",
            vec![accepted.clone()],
            60_000,
        )
        .ledgerflow
        .expect("challenge");
        let payload = build_payment_payload(
            &challenge,
            &request,
            accepted.clone(),
            WarrantTransport::inline(sample_warrant()),
            PaymentPayloadSeed {
                payment_subject: PaymentSubjectRef::new(
                    PaymentSubjectKind::Caip10,
                    "caip10:eip155:8453:0xabc123",
                ),
                signer: agent_keys(),
                created_at_ms: 2_000,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-1".to_string()),
            },
        );

        assert_eq!(payload.accepted, accepted);
        let extension = payload.ledgerflow.expect("ledgerflow extension");
        assert_eq!(extension.challenge_id, "challenge-1");
        assert_eq!(extension.proof.accepted_hash, canonical_accepted_hash(&payload.accepted));
        assert_eq!(extension.proof.request_hash, canonical_request_hash(&request));
    }

    #[test]
    fn challenge_extension_cbor_round_trip_preserves_fields() {
        let challenge = merchant_payment_required(
            "challenge-1",
            "merchant-a",
            "/pay",
            vec![AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()))],
            60_000,
        )
        .ledgerflow
        .expect("challenge");

        let encoded = challenge.encode_cbor().expect("encode challenge");
        let decoded = super::LedgerFlowChallenge::decode_cbor(&encoded).expect("decode challenge");

        assert_eq!(decoded, challenge);
    }

    #[test]
    fn authorization_extension_cbor_round_trip_preserves_fields() {
        let accepted = AcceptedQuote::exact("USDC", 200, "merchant-a", Some("base".to_string()));
        let request =
            HttpRequest::new("POST", "merchant-a.example", "/pay", br#"{"ok":true}"#.to_vec());
        let challenge = merchant_payment_required(
            "challenge-1",
            "merchant-a",
            "/pay",
            vec![accepted.clone()],
            60_000,
        )
        .ledgerflow
        .expect("challenge");
        let payload = build_payment_payload(
            &challenge,
            &request,
            accepted,
            WarrantTransport::inline(sample_warrant()),
            PaymentPayloadSeed {
                payment_subject: PaymentSubjectRef::new(
                    PaymentSubjectKind::Caip10,
                    "caip10:eip155:8453:0xabc123",
                ),
                signer: agent_keys(),
                created_at_ms: 2_000,
                nonce: "nonce-1".to_string(),
                payment_identifier: Some("payment-1".to_string()),
            },
        );
        let extension = payload.ledgerflow.expect("ledgerflow extension");

        let encoded = extension.encode_cbor().expect("encode extension");
        let decoded = super::LedgerFlowAuthorizationExtension::decode_cbor(&encoded)
            .expect("decode extension");

        assert_eq!(decoded, extension);
    }

    proptest! {
        #[test]
        fn canonical_hashes_are_stable_for_the_same_request(
            body in proptest::collection::vec(any::<u8>(), 0..64),
            path in "/[a-z0-9/_-]{1,16}",
            authority in "[a-z0-9.-]{3,16}",
        ) {
            let request = HttpRequest::new("POST", authority, path, body);
            let left = canonical_request_hash(&request);
            let right = canonical_request_hash(&request.clone());
            prop_assert_eq!(left, right);
        }
    }
}
