//! x402 protocol types for LedgerFlow authorization.

use ledgerflow_core::{PaymentSubjectRef, PopProof, Warrant};
use serde::{Deserialize, Serialize};

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
    /// Whether this resource requires human presence (AP2-style
    /// human-in-the-loop). When `true`, the presented authorization must
    /// carry valid m-of-n approvals bound to the PoP.
    #[serde(default)]
    pub human_present: bool,
}

impl LedgerFlowChallenge {
    /// Encodes the challenge as CBOR bytes.
    pub fn encode_cbor(&self) -> Result<Vec<u8>, crate::error::ProtocolError> {
        crate::wire::cbor_encode(self, crate::x402::MAX_LEDGERFLOW_EXTENSION_BYTES)
    }

    /// Decodes a challenge from CBOR bytes.
    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, crate::error::ProtocolError> {
        crate::wire::cbor_decode(bytes, crate::x402::MAX_LEDGERFLOW_EXTENSION_BYTES)
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
    pub signer: ledgerflow_core::SignerRef,
    pub payment_subject: PaymentSubjectRef,
    pub approvals: Vec<ledgerflow_core::SignedApproval>,
    /// Digest-referenced warrants (header-slim mode, design §7.1).
    /// When `warrant_chain` is empty, the verifier loads each digest from
    /// [`crate::middleware::WarrantRepository`]. Kept `#[serde(default)]` for
    /// backward compatibility with inline-only payloads.
    #[serde(default)]
    pub warrant_digests: Vec<String>,
}

impl LedgerFlowAuthorizationExtension {
    /// Encodes the extension as CBOR bytes.
    pub fn encode_cbor(&self) -> Result<Vec<u8>, crate::error::ProtocolError> {
        crate::wire::cbor_encode(self, crate::x402::MAX_LEDGERFLOW_EXTENSION_BYTES)
    }

    /// Decodes an extension from CBOR bytes.
    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, crate::error::ProtocolError> {
        crate::wire::cbor_decode(bytes, crate::x402::MAX_LEDGERFLOW_EXTENSION_BYTES)
    }

    /// Assembles the presented chain into a [`WarrantChain`].
    #[must_use]
    pub fn chain(&self) -> ledgerflow_core::WarrantChain {
        ledgerflow_core::WarrantChain { warrants: self.warrant_chain.clone() }
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
    pub signer: ledgerflow_core::SigningKeyPair,
    pub created_at_ms: u64,
    pub nonce: String,
    pub payment_identifier: Option<String>,
    /// Tool-call arguments to bind into the PoP (defends against
    /// confused-deputy at the tool layer). HTTP-only callers may leave this
    /// empty.
    pub tool_args: ledgerflow_core::ToolArguments,
    pub approvals: Vec<ledgerflow_core::SignedApproval>,
}
