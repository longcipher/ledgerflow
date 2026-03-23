//! Warrant, proof, and verification types for LedgerFlow.

use std::{
    collections::BTreeMap,
    fmt::{self, Display, Write as _},
};

use ciborium::{de::from_reader, ser::into_writer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AuthorizationError, Result, WireError, WireResult};

/// Default proof freshness window for merchant verification.
pub const DEFAULT_PROOF_FRESHNESS_MS: u64 = 60_000;

/// Warrant schema version used by the MVP.
pub const WARRANT_VERSION_V1: u16 = 1;

/// Maximum accepted size for serialized warrant payloads.
pub const MAX_WARRANT_CBOR_BYTES: usize = 64 * 1024;

/// Hashes bytes as a lowercase hexadecimal SHA-256 digest with a `sha256:` prefix.
#[must_use]
pub fn sha256_prefixed(input: impl AsRef<[u8]>) -> String {
    let digest = Sha256::digest(input.as_ref());
    let mut encoded = String::with_capacity(digest.len() * 2);

    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }

    format!("sha256:{encoded}")
}

/// Supported signer algorithms for warrants and proofs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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
    pub public_key: String,
    pub key_id: Option<String>,
}

impl SignerRef {
    #[must_use]
    pub fn new(alg: SigningAlgorithm, public_key: impl Into<String>) -> Self {
        Self { alg, public_key: public_key.into(), key_id: None }
    }

    #[must_use]
    pub fn sign_message(&self, message: &str) -> SignatureEnvelope {
        SignatureEnvelope {
            alg: self.alg,
            value: format!("sig:{}:{}", self.public_key, sha256_prefixed(message)),
        }
    }
}

/// Signature container for warrants and proofs.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SignatureEnvelope {
    pub alg: SigningAlgorithm,
    pub value: String,
}

impl SignatureEnvelope {
    #[must_use]
    pub fn verify(&self, signer: &SignerRef, message: &str) -> bool {
        self.alg == signer.alg && self.value == signer.sign_message(message).value
    }
}

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

    #[must_use]
    fn canonical(&self) -> String {
        format!("{}:{}", self.kind, self.value)
    }
}

/// Supported payment subject kinds in the MVP.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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

/// Narrow merchant audience scope for a warrant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AudienceScope {
    MerchantIds(Vec<String>),
    MerchantHosts(Vec<String>),
    Any,
}

impl AudienceScope {
    #[must_use]
    pub fn allows(&self, merchant_id: &str, merchant_host: &str) -> bool {
        match self {
            Self::MerchantIds(allowed) => allowed.iter().any(|candidate| candidate == merchant_id),
            Self::MerchantHosts(allowed) => {
                allowed.iter().any(|candidate| merchant_host.ends_with(candidate))
            }
            Self::Any => true,
        }
    }

    #[must_use]
    fn canonical(&self) -> String {
        match self {
            Self::MerchantIds(values) => format!("merchant_ids={}", canonical_list(values)),
            Self::MerchantHosts(values) => format!("merchant_hosts={}", canonical_list(values)),
            Self::Any => "any".to_string(),
        }
    }
}

/// Delegation policy for a warrant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DelegationPolicy {
    pub can_delegate: bool,
    pub max_depth: u8,
}

impl DelegationPolicy {
    #[must_use]
    fn canonical(self) -> String {
        format!("can_delegate={};max_depth={}", u8::from(self.can_delegate), self.max_depth)
    }
}

/// A payment asset allowed by the warrant.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AssetRef {
    pub asset: String,
    pub network: Option<String>,
}

impl AssetRef {
    #[must_use]
    pub fn new(asset: impl Into<String>, network: Option<String>) -> Self {
        Self { asset: asset.into(), network }
    }

    #[must_use]
    fn canonical(&self) -> String {
        let network = self.network.as_deref().unwrap_or("-");
        format!("{}@{network}", self.asset)
    }
}

/// Maximum amount a warrant can authorize per request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AmountLimit {
    pub amount: u64,
}

/// Optional windowed spending limit placeholder for future use.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PeriodLimit {
    pub amount: u64,
    pub window_ms: u64,
}

/// High-level settlement rails allowed by a warrant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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

/// Typed warrant constraints for the MVP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Constraint {
    Merchant(MerchantConstraint),
    Resource(ResourceConstraint),
    Tool(ToolConstraint),
    Payment(PaymentConstraint),
    Sponsorship(SponsorshipConstraint),
}

impl Constraint {
    #[must_use]
    fn canonical(&self) -> String {
        match self {
            Self::Merchant(constraint) => format!("merchant({})", constraint.canonical()),
            Self::Resource(constraint) => format!("resource({})", constraint.canonical()),
            Self::Tool(constraint) => format!("tool({})", constraint.canonical()),
            Self::Payment(constraint) => format!("payment({})", constraint.canonical()),
            Self::Sponsorship(constraint) => {
                format!("sponsorship({})", constraint.canonical())
            }
        }
    }
}

/// Merchant allowlist constraint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MerchantConstraint {
    pub merchant_ids: Vec<String>,
    pub host_suffixes: Vec<String>,
}

impl MerchantConstraint {
    #[must_use]
    fn canonical(&self) -> String {
        format!(
            "merchant_ids={};host_suffixes={}",
            canonical_list(&self.merchant_ids),
            canonical_list(&self.host_suffixes)
        )
    }
}

/// HTTP method and path constraint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceConstraint {
    pub http_methods: Vec<String>,
    pub path_prefixes: Vec<String>,
}

impl ResourceConstraint {
    #[must_use]
    fn canonical(&self) -> String {
        format!(
            "methods={};paths={}",
            canonical_uppercase_list(&self.http_methods),
            canonical_list(&self.path_prefixes)
        )
    }
}

/// AI-native tool constraint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolConstraint {
    pub tool_names: Vec<String>,
    pub model_providers: Vec<String>,
    pub action_labels: Vec<String>,
}

impl ToolConstraint {
    #[must_use]
    fn canonical(&self) -> String {
        format!(
            "tools={};providers={};labels={}",
            canonical_list(&self.tool_names),
            canonical_list(&self.model_providers),
            canonical_list(&self.action_labels)
        )
    }
}

/// Payment authorization constraint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PaymentConstraint {
    pub max_per_request: AmountLimit,
    pub period_limit: Option<PeriodLimit>,
    pub allowed_assets: Vec<AssetRef>,
    pub allowed_rails: Vec<PaymentRail>,
    pub allowed_schemes: Vec<String>,
    pub payee_ids: Vec<String>,
}

impl PaymentConstraint {
    #[must_use]
    fn canonical(&self) -> String {
        let period = self.period_limit.map_or_else(
            || "-".to_string(),
            |limit| format!("{}@{}", limit.amount, limit.window_ms),
        );
        let rails: Vec<String> = self.allowed_rails.iter().map(ToString::to_string).collect();
        let assets: Vec<String> = self.allowed_assets.iter().map(AssetRef::canonical).collect();

        format!(
            "max={};period={period};assets={};rails={};schemes={};payees={}",
            self.max_per_request.amount,
            canonical_list(&assets),
            canonical_list(&rails),
            canonical_list(&self.allowed_schemes),
            canonical_list(&self.payee_ids)
        )
    }
}

/// Sponsorship placeholder kept typed but minimal in the MVP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SponsorshipConstraint {
    pub allow_sponsored_execution: bool,
    pub sponsor_ids: Vec<String>,
}

impl SponsorshipConstraint {
    #[must_use]
    fn canonical(&self) -> String {
        format!(
            "allow={};sponsors={}",
            u8::from(self.allow_sponsored_execution),
            canonical_list(&self.sponsor_ids)
        )
    }
}

/// Additional metadata carried in a warrant.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WarrantMetadata {
    pub entries: BTreeMap<String, String>,
}

impl WarrantMetadata {
    #[must_use]
    fn canonical(&self) -> String {
        self.entries
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Compact warrant model for the MVP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Warrant {
    pub version: u16,
    pub warrant_id: String,
    pub issuer: SignerRef,
    pub subject_signer: SignerRef,
    pub payment_subjects: Vec<PaymentSubjectRef>,
    pub audience: AudienceScope,
    pub not_before_ms: u64,
    pub expires_at_ms: u64,
    pub delegation: DelegationPolicy,
    pub constraints: Vec<Constraint>,
    pub metadata: WarrantMetadata,
    pub signature: SignatureEnvelope,
}

impl Warrant {
    #[must_use]
    pub fn sign(mut self) -> Self {
        self.signature = self.issuer.sign_message(&self.canonical_unsigned_payload());
        self
    }

    #[must_use]
    pub fn digest(&self) -> String {
        sha256_prefixed(self.canonical_signed_payload())
    }

    pub fn encode_cbor(&self) -> WireResult<Vec<u8>> {
        let mut bytes = Vec::new();
        into_writer(self, &mut bytes)
            .map_err(|error| WireError::Serialization(error.to_string()))?;
        Ok(bytes)
    }

    pub fn decode_cbor(bytes: &[u8]) -> WireResult<Self> {
        if bytes.len() > MAX_WARRANT_CBOR_BYTES {
            return Err(WireError::PayloadTooLarge {
                size: bytes.len(),
                max: MAX_WARRANT_CBOR_BYTES,
            });
        }

        from_reader(bytes).map_err(|error| WireError::Deserialization(error.to_string()))
    }

    fn canonical_unsigned_payload(&self) -> String {
        let subjects: Vec<String> =
            self.payment_subjects.iter().map(PaymentSubjectRef::canonical).collect();
        let constraints: Vec<String> = self.constraints.iter().map(Constraint::canonical).collect();

        format!(
            "version={};warrant_id={};issuer={};subject_signer={};payment_subjects={};audience={};not_before_ms={};expires_at_ms={};delegation={};constraints={};metadata={}",
            self.version,
            self.warrant_id,
            canonical_signer(&self.issuer),
            canonical_signer(&self.subject_signer),
            canonical_list(&subjects),
            self.audience.canonical(),
            self.not_before_ms,
            self.expires_at_ms,
            self.delegation.canonical(),
            canonical_list(&constraints),
            self.metadata.canonical()
        )
    }

    fn canonical_signed_payload(&self) -> String {
        format!(
            "{};signature={}:{}",
            self.canonical_unsigned_payload(),
            self.signature.alg,
            self.signature.value
        )
    }

    fn verify_signature(&self) -> bool {
        self.signature.verify(&self.issuer, &self.canonical_unsigned_payload())
    }
}

/// Proof of authorization bound to one challenge and one x402 quote/request pair.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Proof {
    pub challenge_id: String,
    pub warrant_digest: String,
    pub accepted_hash: String,
    pub request_hash: String,
    pub created_at_ms: u64,
    pub nonce: String,
    pub signer_key: String,
    pub signature: SignatureEnvelope,
}

impl Proof {
    #[must_use]
    pub fn new_signed(
        challenge_id: impl Into<String>,
        warrant_digest: impl Into<String>,
        accepted_hash: impl Into<String>,
        request_hash: impl Into<String>,
        created_at_ms: u64,
        nonce: impl Into<String>,
        signer: &SignerRef,
    ) -> Self {
        let mut proof = Self {
            challenge_id: challenge_id.into(),
            warrant_digest: warrant_digest.into(),
            accepted_hash: accepted_hash.into(),
            request_hash: request_hash.into(),
            created_at_ms,
            nonce: nonce.into(),
            signer_key: signer.public_key.clone(),
            signature: signer.sign_message("pending"),
        };
        proof.signature = signer.sign_message(&proof.preimage());
        proof
    }

    #[must_use]
    pub fn preimage(&self) -> String {
        format!(
            "domain=ledgerflow-pop/v1;challenge_id={};warrant_digest={};accepted_hash={};request_hash={};created_at_ms={};nonce={};signer_key={}",
            self.challenge_id,
            self.warrant_digest,
            self.accepted_hash,
            self.request_hash,
            self.created_at_ms,
            self.nonce,
            self.signer_key
        )
    }

    fn verify_signature(&self, signer: &SignerRef) -> bool {
        self.signer_key == signer.public_key && self.signature.verify(signer, &self.preimage())
    }
}

/// Merchant-facing context used when verifying a warrant and proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationContext {
    pub merchant_id: String,
    pub merchant_host: String,
    pub tool_name: String,
    pub http_method: String,
    pub path_and_query: String,
    pub selected_quote_amount: u64,
    pub asset: String,
    pub scheme: String,
    pub payee_id: String,
    pub rail: PaymentRail,
    pub challenge_id: String,
    pub request_hash: String,
    pub accepted_hash: String,
    pub now_ms: u64,
    pub freshness_window_ms: u64,
    pub presented_delegation_depth: u8,
    pub payment_subject: PaymentSubjectRef,
}

/// Normalized authorization output handed to the x402 layer and Facilitator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAuthorization {
    pub merchant_id: String,
    pub tool_name: String,
    pub payment_subject: PaymentSubjectRef,
    pub payer: SignerRef,
    pub warrant_digest: String,
    pub accepted_hash: String,
    pub request_hash: String,
    pub amount: u64,
    pub asset: String,
    pub scheme: String,
    pub payee_id: String,
    pub rail: PaymentRail,
}

/// Verifies a warrant and proof against merchant request context.
pub fn verify_authorization(
    warrant: &Warrant,
    proof: &Proof,
    context: &AuthorizationContext,
) -> Result<VerifiedAuthorization> {
    if warrant.version != WARRANT_VERSION_V1 {
        return Err(AuthorizationError::UnsupportedVersion(warrant.version));
    }

    if !warrant.verify_signature() {
        return Err(AuthorizationError::InvalidWarrantSignature);
    }

    if warrant.not_before_ms > context.now_ms {
        return Err(AuthorizationError::WarrantNotYetValid { now_ms: context.now_ms });
    }

    if warrant.expires_at_ms < context.now_ms {
        return Err(AuthorizationError::WarrantExpired { expires_at_ms: warrant.expires_at_ms });
    }

    if !warrant.audience.allows(&context.merchant_id, &context.merchant_host) {
        return Err(AuthorizationError::MerchantNotAllowed {
            merchant_id: context.merchant_id.clone(),
        });
    }

    if !proof.verify_signature(&warrant.subject_signer) {
        return Err(AuthorizationError::InvalidProofSignature);
    }

    if proof.challenge_id != context.challenge_id {
        return Err(AuthorizationError::ChallengeMismatch);
    }

    if proof.warrant_digest != warrant.digest() {
        return Err(AuthorizationError::WarrantDigestMismatch);
    }

    if proof.accepted_hash != context.accepted_hash {
        return Err(AuthorizationError::AcceptedHashMismatch);
    }

    if proof.request_hash != context.request_hash {
        return Err(AuthorizationError::RequestHashMismatch);
    }

    if proof.signer_key != warrant.subject_signer.public_key {
        return Err(AuthorizationError::SignerMismatch);
    }

    let freshest_allowed = proof.created_at_ms.saturating_add(context.freshness_window_ms);
    if proof.created_at_ms > context.now_ms || freshest_allowed < context.now_ms {
        return Err(AuthorizationError::ProofOutsideFreshnessWindow);
    }

    if !warrant.payment_subjects.contains(&context.payment_subject) {
        return Err(AuthorizationError::PaymentSubjectNotAllowed {
            subject: context.payment_subject.value.clone(),
        });
    }

    if context.presented_delegation_depth > 0 && !warrant.delegation.can_delegate {
        return Err(AuthorizationError::DelegationNotAllowed);
    }

    if context.presented_delegation_depth > warrant.delegation.max_depth {
        return Err(AuthorizationError::DelegationDepthExceeded {
            presented: context.presented_delegation_depth,
            allowed: warrant.delegation.max_depth,
        });
    }

    for constraint in &warrant.constraints {
        match constraint {
            Constraint::Merchant(constraint) => verify_merchant_constraint(constraint, context)?,
            Constraint::Resource(constraint) => verify_resource_constraint(constraint, context)?,
            Constraint::Tool(constraint) => verify_tool_constraint(constraint, context)?,
            Constraint::Payment(constraint) => verify_payment_constraint(constraint, context)?,
            Constraint::Sponsorship(_) => {}
        }
    }

    Ok(VerifiedAuthorization {
        merchant_id: context.merchant_id.clone(),
        tool_name: context.tool_name.clone(),
        payment_subject: context.payment_subject.clone(),
        payer: warrant.subject_signer.clone(),
        warrant_digest: warrant.digest(),
        accepted_hash: context.accepted_hash.clone(),
        request_hash: context.request_hash.clone(),
        amount: context.selected_quote_amount,
        asset: context.asset.clone(),
        scheme: context.scheme.clone(),
        payee_id: context.payee_id.clone(),
        rail: context.rail,
    })
}

fn verify_merchant_constraint(
    constraint: &MerchantConstraint,
    context: &AuthorizationContext,
) -> Result<()> {
    if !constraint.merchant_ids.is_empty() &&
        !constraint.merchant_ids.iter().any(|candidate| candidate == &context.merchant_id)
    {
        return Err(AuthorizationError::MerchantNotAllowed {
            merchant_id: context.merchant_id.clone(),
        });
    }

    if !constraint.host_suffixes.is_empty() &&
        !constraint
            .host_suffixes
            .iter()
            .any(|candidate| context.merchant_host.ends_with(candidate))
    {
        return Err(AuthorizationError::MerchantNotAllowed {
            merchant_id: context.merchant_id.clone(),
        });
    }

    Ok(())
}

fn verify_resource_constraint(
    constraint: &ResourceConstraint,
    context: &AuthorizationContext,
) -> Result<()> {
    let method = context.http_method.to_uppercase();

    if !constraint.http_methods.is_empty() &&
        !constraint.http_methods.iter().any(|candidate| candidate.eq_ignore_ascii_case(&method))
    {
        return Err(AuthorizationError::HttpMethodNotAllowed { method });
    }

    if !constraint.path_prefixes.is_empty() &&
        !constraint
            .path_prefixes
            .iter()
            .any(|candidate| context.path_and_query.starts_with(candidate))
    {
        return Err(AuthorizationError::ResourcePathNotAllowed {
            path: context.path_and_query.clone(),
        });
    }

    Ok(())
}

fn verify_tool_constraint(
    constraint: &ToolConstraint,
    context: &AuthorizationContext,
) -> Result<()> {
    if !constraint.tool_names.is_empty() &&
        !constraint.tool_names.iter().any(|candidate| candidate == &context.tool_name)
    {
        return Err(AuthorizationError::ToolNotAllowed { tool_name: context.tool_name.clone() });
    }

    Ok(())
}

fn verify_payment_constraint(
    constraint: &PaymentConstraint,
    context: &AuthorizationContext,
) -> Result<()> {
    if context.selected_quote_amount > constraint.max_per_request.amount {
        return Err(AuthorizationError::PaymentAmountExceeded {
            amount: context.selected_quote_amount,
            limit: constraint.max_per_request.amount,
        });
    }

    if !constraint.allowed_assets.is_empty() &&
        !constraint.allowed_assets.iter().any(|asset| asset.asset == context.asset)
    {
        return Err(AuthorizationError::AssetNotAllowed { asset: context.asset.clone() });
    }

    if !constraint.allowed_schemes.is_empty() &&
        !constraint.allowed_schemes.iter().any(|scheme| scheme == &context.scheme)
    {
        return Err(AuthorizationError::SchemeNotAllowed { scheme: context.scheme.clone() });
    }

    if !constraint.allowed_rails.is_empty() &&
        !constraint.allowed_rails.iter().any(|rail| rail == &context.rail)
    {
        return Err(AuthorizationError::RailNotAllowed { rail: context.rail });
    }

    if !constraint.payee_ids.is_empty() &&
        !constraint.payee_ids.iter().any(|payee| payee == &context.payee_id)
    {
        return Err(AuthorizationError::PayeeNotAllowed { payee_id: context.payee_id.clone() });
    }

    Ok(())
}

fn canonical_list(values: &[impl AsRef<str>]) -> String {
    let mut items = values.iter().map(AsRef::as_ref).collect::<Vec<_>>();
    items.sort_unstable();
    items.join("|")
}

fn canonical_uppercase_list(values: &[String]) -> String {
    let mut items = values.iter().map(|value| value.to_uppercase()).collect::<Vec<_>>();
    items.sort_unstable();
    items.join("|")
}

fn canonical_signer(signer: &SignerRef) -> String {
    let key_id = signer.key_id.as_deref().unwrap_or("-");
    format!("{}:{}:{key_id}", signer.alg, signer.public_key)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{
        AmountLimit, AssetRef, AudienceScope, AuthorizationContext, Constraint, DelegationPolicy,
        MerchantConstraint, PaymentConstraint, PaymentRail, PaymentSubjectKind, PaymentSubjectRef,
        Proof, ResourceConstraint, SignerRef, SigningAlgorithm, SponsorshipConstraint,
        ToolConstraint, Warrant, WarrantMetadata, sha256_prefixed, verify_authorization,
    };
    use crate::error::AuthorizationError;

    fn issuer() -> SignerRef {
        SignerRef::new(SigningAlgorithm::Ed25519, "issuer-key")
    }

    fn subject_signer() -> SignerRef {
        SignerRef::new(SigningAlgorithm::Ed25519, "agent-key")
    }

    fn subject_ref() -> PaymentSubjectRef {
        PaymentSubjectRef::new(PaymentSubjectKind::Caip10, "caip10:eip155:8453:0xabc123")
    }

    fn base_warrant() -> Warrant {
        Warrant {
            version: super::WARRANT_VERSION_V1,
            warrant_id: "warrant-1".to_string(),
            issuer: issuer(),
            subject_signer: subject_signer(),
            payment_subjects: vec![subject_ref()],
            audience: AudienceScope::MerchantIds(vec!["merchant-a".to_string()]),
            not_before_ms: 1_000,
            expires_at_ms: 5_000,
            delegation: DelegationPolicy { can_delegate: true, max_depth: 1 },
            constraints: vec![
                Constraint::Merchant(MerchantConstraint {
                    merchant_ids: vec!["merchant-a".to_string()],
                    host_suffixes: vec![],
                }),
                Constraint::Resource(ResourceConstraint {
                    http_methods: vec!["GET".to_string()],
                    path_prefixes: vec!["/search".to_string()],
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
            signature: issuer().sign_message("placeholder"),
        }
        .sign()
    }

    fn base_context() -> AuthorizationContext {
        AuthorizationContext {
            merchant_id: "merchant-a".to_string(),
            merchant_host: "merchant-a.example".to_string(),
            tool_name: "web-search".to_string(),
            http_method: "GET".to_string(),
            path_and_query: "/search?q=ledgerflow".to_string(),
            selected_quote_amount: 200,
            asset: "USDC".to_string(),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: sha256_prefixed(
                "GET\nmerchant-a.example\n/search?q=ledgerflow\nsha256:body",
            ),
            accepted_hash: sha256_prefixed("exact:USDC:200:merchant-a"),
            now_ms: 2_000,
            freshness_window_ms: 60_000,
            presented_delegation_depth: 1,
            payment_subject: subject_ref(),
        }
    }

    fn base_proof(warrant: &Warrant, context: &AuthorizationContext) -> Proof {
        Proof::new_signed(
            context.challenge_id.clone(),
            warrant.digest(),
            context.accepted_hash.clone(),
            context.request_hash.clone(),
            context.now_ms,
            "nonce-1",
            &subject_signer(),
        )
    }

    #[test]
    fn verifies_a_valid_warrant_and_matching_proof() {
        let warrant = base_warrant();
        let context = base_context();
        let proof = base_proof(&warrant, &context);

        let verified = verify_authorization(&warrant, &proof, &context).expect("valid authz");

        assert_eq!(verified.merchant_id, "merchant-a");
        assert_eq!(verified.tool_name, "web-search");
        assert_eq!(verified.payment_subject, subject_ref());
    }

    #[test]
    fn rejects_an_expired_warrant() {
        let mut warrant = base_warrant();
        warrant.expires_at_ms = 1_999;
        let warrant = warrant.sign();
        let context = base_context();
        let proof = base_proof(&warrant, &context);

        let error = verify_authorization(&warrant, &proof, &context).expect_err("expired");

        assert_eq!(error, AuthorizationError::WarrantExpired { expires_at_ms: 1_999 });
    }

    #[test]
    fn rejects_a_warrant_for_a_different_merchant() {
        let warrant = base_warrant();
        let mut context = base_context();
        context.merchant_id = "merchant-b".to_string();
        let proof = base_proof(&warrant, &context);

        let error = verify_authorization(&warrant, &proof, &context).expect_err("merchant scope");

        assert_eq!(
            error,
            AuthorizationError::MerchantNotAllowed { merchant_id: "merchant-b".to_string() }
        );
    }

    #[test]
    fn rejects_a_quote_above_the_payment_limit() {
        let warrant = base_warrant();
        let mut context = base_context();
        context.selected_quote_amount = 201;
        let proof = base_proof(&warrant, &context);

        let error = verify_authorization(&warrant, &proof, &context).expect_err("limit");

        assert_eq!(error, AuthorizationError::PaymentAmountExceeded { amount: 201, limit: 200 });
    }

    #[test]
    fn rejects_a_delegation_chain_that_exceeds_the_policy() {
        let warrant = base_warrant();
        let mut context = base_context();
        context.presented_delegation_depth = 2;
        let proof = base_proof(&warrant, &context);

        let error = verify_authorization(&warrant, &proof, &context).expect_err("delegation");

        assert_eq!(error, AuthorizationError::DelegationDepthExceeded { presented: 2, allowed: 1 });
    }

    #[test]
    fn warrant_cbor_round_trip_preserves_payload_and_digest() {
        let warrant = base_warrant();

        let encoded = warrant.encode_cbor().expect("encode warrant");
        let decoded = Warrant::decode_cbor(&encoded).expect("decode warrant");

        assert_eq!(decoded, warrant);
        assert_eq!(decoded.digest(), warrant.digest());
    }

    #[test]
    fn warrant_decode_rejects_oversized_payloads() {
        let oversized = vec![0_u8; super::MAX_WARRANT_CBOR_BYTES + 1];

        let error = Warrant::decode_cbor(&oversized).expect_err("oversized payload");

        assert_eq!(
            error,
            crate::error::WireError::PayloadTooLarge {
                size: oversized.len(),
                max: super::MAX_WARRANT_CBOR_BYTES,
            }
        );
    }

    proptest! {
        #[test]
        fn warrant_digest_is_stable_for_identical_inputs(
            warrant_id in "[a-z0-9-]{4,16}",
            merchant_id in "[a-z]{3,12}",
            tool_name in "[a-z-]{3,12}",
            amount in 1_u64..10_000,
        ) {
            let subject = PaymentSubjectRef::new(PaymentSubjectKind::Opaque, "opaque:test");
            let warrant = Warrant {
                version: super::WARRANT_VERSION_V1,
                warrant_id,
                issuer: issuer(),
                subject_signer: subject_signer(),
                payment_subjects: vec![subject.clone()],
                audience: AudienceScope::MerchantIds(vec![merchant_id.clone()]),
                not_before_ms: 1_000,
                expires_at_ms: 9_000,
                delegation: DelegationPolicy { can_delegate: false, max_depth: 0 },
                constraints: vec![
                    Constraint::Tool(ToolConstraint {
                        tool_names: vec![tool_name],
                        model_providers: vec![],
                        action_labels: vec![],
                    }),
                    Constraint::Payment(PaymentConstraint {
                        max_per_request: AmountLimit { amount },
                        period_limit: None,
                        allowed_assets: vec![AssetRef::new("USDC", None)],
                        allowed_rails: vec![PaymentRail::Exchange],
                        allowed_schemes: vec!["exact".to_string()],
                        payee_ids: vec![merchant_id],
                    }),
                ],
                metadata: WarrantMetadata::default(),
                signature: issuer().sign_message("placeholder"),
            }.sign();

            prop_assert_eq!(warrant.digest(), warrant.clone().digest());
        }

        #[test]
        fn proof_signature_tracks_binding_inputs(
            challenge_id in "[a-z0-9-]{4,16}",
            nonce in "[a-z0-9-]{4,16}",
            request_value in "[a-z0-9-]{4,16}",
            accepted_value in "[a-z0-9-]{4,16}",
        ) {
            let signer = subject_signer();
            let proof = Proof::new_signed(
                challenge_id,
                "sha256:digest",
                accepted_value.clone(),
                request_value.clone(),
                4_200,
                nonce,
                &signer,
            );

            prop_assert!(proof.signature.verify(&signer, &proof.preimage()));
            prop_assert_eq!(proof.accepted_hash, accepted_value);
            prop_assert_eq!(proof.request_hash, request_value);
        }
    }
}
