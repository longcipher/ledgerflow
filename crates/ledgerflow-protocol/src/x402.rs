//! x402 protocol binding for LedgerFlow authorization.
//!
//! Uses the x402 v2 **extensions** mechanism: the merchant advertises a
//! LedgerFlow challenge in `PaymentRequired`, and the agent echoes the
//! challenge plus its authorization data (warrant chain, PoP, approvals) in
//! `PaymentPayload`. The wire protocol stays standard x402; LedgerFlow only
//! occupies the extension slot.

use ledgerflow_core::{
    PaymentSubjectRef, PopProof, PopTuple, ProofBuilder, SignerRef, SigningKeyPair, Warrant,
    WarrantChain, sha256_prefixed,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::ProtocolError,
    wire::{cbor_decode, cbor_encode},
};

/// LedgerFlow extension version for x402.
pub const LEDGERFLOW_EXTENSION_VERSION: &str = "lfx402/v1";

/// Maximum accepted size for serialized LedgerFlow extension payloads.
pub const MAX_LEDGERFLOW_EXTENSION_BYTES: usize = 32 * 1024;

/// A selected payment quote (x402 `accepted` block).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedQuote {
    pub scheme: String,
    pub asset: String,
    pub amount: u128,
    pub payee_id: String,
    pub network: Option<String>,
}

impl AcceptedQuote {
    #[must_use]
    pub fn exact(
        asset: impl Into<String>,
        amount: u128,
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

    /// Canonical representation used for the accepted-quote binding hash.
    #[must_use]
    pub fn canonical(&self) -> String {
        let network = self.network.as_deref().unwrap_or("-");
        format!(
            "scheme={};asset={};amount={};payee_id={};network={network}",
            self.scheme, self.asset, self.amount, self.payee_id
        )
    }
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

/// Merchant-advertised LedgerFlow challenge (x402 extension `info`).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LedgerFlowChallenge {
    pub version: String,
    pub challenge_id: String,
    pub merchant_id: String,
    pub resource: String,
    pub proof_freshness_ms: u64,
    pub clock_skew_ms: u64,
    pub challenge_ttl_ms: u64,
    pub required_subject_kinds: Vec<String>,
    /// Accounting point for budget execution (P2+; null in v1).
    pub ledger: Option<String>,
}

impl LedgerFlowChallenge {
    /// Encodes the challenge as CBOR bytes.
    pub fn encode_cbor(&self) -> Result<Vec<u8>, ProtocolError> {
        cbor_encode(self, MAX_LEDGERFLOW_EXTENSION_BYTES)
    }

    /// Decodes a challenge from CBOR bytes.
    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, ProtocolError> {
        cbor_decode(bytes, MAX_LEDGERFLOW_EXTENSION_BYTES)
    }
}

/// Agent-sent LedgerFlow authorization extension (x402 extension echo).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LedgerFlowAuthorizationExtension {
    pub version: String,
    pub challenge_id: String,
    /// Root-first warrant chain, transmitted **inline** (v1 rule).
    pub warrant_chain: Vec<Warrant>,
    pub proof: PopProof,
    pub signer: SignerRef,
    pub payment_subject: PaymentSubjectRef,
    pub approvals: Vec<ledgerflow_core::SignedApproval>,
}

impl LedgerFlowAuthorizationExtension {
    /// Encodes the extension as CBOR bytes.
    pub fn encode_cbor(&self) -> Result<Vec<u8>, ProtocolError> {
        cbor_encode(self, MAX_LEDGERFLOW_EXTENSION_BYTES)
    }

    /// Decodes an extension from CBOR bytes.
    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, ProtocolError> {
        cbor_decode(bytes, MAX_LEDGERFLOW_EXTENSION_BYTES)
    }

    /// Assembles the presented chain into a [`WarrantChain`].
    #[must_use]
    pub fn chain(&self) -> WarrantChain {
        WarrantChain { warrants: self.warrant_chain.clone() }
    }
}

/// An x402 `402 Payment Required` response with a LedgerFlow challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRequiredResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub accepted: Vec<AcceptedQuote>,
    pub ledgerflow: Option<LedgerFlowChallenge>,
}

/// x402 payment payload that echoes the quote and adds LedgerFlow authz data.
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

/// Inputs that vary per payment payload while the x402 shape stays fixed.
#[derive(Clone, Debug)]
pub struct PaymentPayloadSeed {
    pub payment_subject: PaymentSubjectRef,
    pub signer: SigningKeyPair,
    pub created_at_ms: u64,
    pub nonce: String,
    pub payment_identifier: Option<String>,
    /// Tool-call arguments to bind into the PoP (defends against
    /// confused-deputy at the tool layer). HTTP-only callers may leave this
    /// empty.
    pub tool_args: ledgerflow_core::ToolArguments,
    pub approvals: Vec<ledgerflow_core::SignedApproval>,
}

/// Creates a standard x402 `402 Payment Required` response with a LedgerFlow
/// challenge extension.
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
            version: LEDGERFLOW_EXTENSION_VERSION.to_string(),
            challenge_id: challenge_id.into(),
            merchant_id: merchant_id.into(),
            resource: resource.into(),
            proof_freshness_ms,
            clock_skew_ms: ledgerflow_core::DEFAULT_CLOCK_SKEW_MS,
            challenge_ttl_ms: ledgerflow_core::DEFAULT_CHALLENGE_TTL_MS,
            required_subject_kinds: vec!["signer".to_string(), "payment_subject".to_string()],
            ledger: None,
        }),
    }
}

/// Builds an x402 payment payload that echoes the selected quote and adds
/// LedgerFlow authz data (warrant chain + PoP + approvals).
///
/// Returns an error when the warrant chain is empty.
pub fn build_payment_payload(
    challenge: &LedgerFlowChallenge,
    request: &HttpRequest,
    accepted: AcceptedQuote,
    chain: WarrantChain,
    seed: PaymentPayloadSeed,
) -> Result<PaymentPayload, crate::error::ProtocolError> {
    let accepted_hash = canonical_accepted_hash(&accepted);
    let request_hash = canonical_request_hash(request);
    let leaf = chain.leaf().cloned().ok_or(crate::error::ProtocolError::EmptyChain)?;
    let approvals_digest = if seed.approvals.is_empty() {
        None
    } else {
        Some(PopTuple::approvals_digest(&seed.approvals))
    };

    let tool_args_digest = PopTuple::tool_args_digest(&seed.tool_args);
    // Bind the PoP to the concrete accepted quote (design §6.3). The digest is
    // derived from the canonical quote representation and is later cross-checked
    // by `verify_authorization`, so a valid PoP cannot be reused against a
    // different payment.
    let payment_payload_digest = sha256_prefixed(accepted.canonical());
    let tuple = PopTuple {
        warrant_id: leaf.id,
        challenge_id: challenge.challenge_id.clone(),
        method: request.method.clone(),
        uri: format!("{}{}", request.authority, request.path_and_query),
        request_hash,
        accepted_hash,
        payment_payload_digest,
        tool_args_digest,
        approvals_digest,
        nonce: seed.nonce.clone(),
        created_at_ms: seed.created_at_ms,
    };
    let proof = ProofBuilder::new()
        .warrant_id(tuple.warrant_id.clone())
        .challenge_id(tuple.challenge_id.clone())
        .method(tuple.method.clone())
        .uri(tuple.uri.clone())
        .request_hash(tuple.request_hash.clone())
        .accepted_hash(tuple.accepted_hash.clone())
        .payment_payload_digest(tuple.payment_payload_digest.clone())
        .approvals_digest(tuple.approvals_digest.clone().unwrap_or_default())
        .nonce(tuple.nonce.clone())
        .created_at_ms(tuple.created_at_ms)
        .sign_with(&seed.signer);

    Ok(PaymentPayload {
        accepted: accepted.clone(),
        // The settlement payload carries the canonical quote; the PoP commits
        // to its digest, so the two stay consistent (design §6.3).
        settlement_payload: accepted.canonical(),
        payment_identifier: seed.payment_identifier.clone(),
        ledgerflow: Some(LedgerFlowAuthorizationExtension {
            version: LEDGERFLOW_EXTENSION_VERSION.to_string(),
            challenge_id: challenge.challenge_id.clone(),
            warrant_chain: chain.warrants,
            proof,
            signer: seed.signer.signer_ref(),
            payment_subject: seed.payment_subject,
            approvals: seed.approvals,
        }),
    })
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
