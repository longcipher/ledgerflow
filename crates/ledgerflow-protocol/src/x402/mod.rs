//! x402 protocol binding for LedgerFlow authorization.
//!
//! Uses the x402 v2 **extensions** mechanism: the merchant advertises a
//! LedgerFlow challenge in `PaymentRequired`, and the agent echoes the
//! challenge plus its authorization data (warrant chain, PoP, approvals) in
//! `PaymentPayload`. The wire protocol stays standard x402; LedgerFlow only
//! occupies the extension slot.

use ledgerflow_core::{PopTuple, ProofBuilder, WarrantChain, sha256_prefixed};

use crate::error::ProtocolError;

mod types;

pub use types::{
    AcceptedQuote, HttpRequest, LedgerFlowAuthorizationExtension, LedgerFlowChallenge,
    PaymentPayload, PaymentPayloadSeed, PaymentRequiredResponse,
};

/// LedgerFlow extension version for x402.
pub const LEDGERFLOW_EXTENSION_VERSION: &str = "lfx402/v1";

/// Maximum accepted size for serialized LedgerFlow extension payloads.
pub const MAX_LEDGERFLOW_EXTENSION_BYTES: usize = 32 * 1024;

/// Creates a standard x402 `402 Payment Required` response with a LedgerFlow
/// challenge extension (human presence not required).
#[must_use]
pub fn merchant_payment_required(
    challenge_id: impl Into<String>,
    merchant_id: impl Into<String>,
    resource: impl Into<String>,
    accepted: Vec<AcceptedQuote>,
    proof_freshness_ms: u64,
) -> PaymentRequiredResponse {
    merchant_payment_required_with(
        challenge_id,
        merchant_id,
        resource,
        accepted,
        proof_freshness_ms,
        false,
    )
}

/// Creates a 402 response whose LedgerFlow challenge declares whether human
/// presence is required for this resource.
///
/// With `human_present = true`, the verifier will demand valid m-of-n
/// approvals bound to the presented PoP (AP2-style human-in-the-loop flow).
#[must_use]
pub fn merchant_payment_required_with(
    challenge_id: impl Into<String>,
    merchant_id: impl Into<String>,
    resource: impl Into<String>,
    accepted: Vec<AcceptedQuote>,
    proof_freshness_ms: u64,
    human_present: bool,
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
            human_present,
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
) -> Result<PaymentPayload, ProtocolError> {
    let accepted_hash = canonical_accepted_hash(&accepted);
    let request_hash = canonical_request_hash(request);
    let leaf = chain.leaf().cloned().ok_or(ProtocolError::EmptyChain)?;
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
        warrant_id: leaf.id.to_vec(),
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
            warrant_digests: Vec::new(),
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
