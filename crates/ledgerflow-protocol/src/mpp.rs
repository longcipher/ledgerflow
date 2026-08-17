//! MPP (Machine Payments Protocol) binding for LedgerFlow authorization.
//!
//! MPP is built on the `draft-ietf-httpauth-payment` "Payment" HTTP
//! authentication scheme: `WWW-Authenticate: Payment` challenges and
//! `Authorization: Payment` responses. LedgerFlow rides along as a
//! `ledgerflow` challenge parameter without modifying charge/session
//! semantics.
//!
//! Header size policy (design §7.2): the header parameter carries at most a
//! single-node or digest reference; the full chain travels in the body. See
//! [`crate::carrier`].

use ledgerflow_core::{PaymentSubjectRef, PopProof, SignerRef, SignedApproval, Warrant};

use crate::{
    carrier::MAX_HEADER_CBOR_BYTES,
    error::ProtocolError,
    wire::{base64url_decode, base64url_encode, cbor_decode, cbor_encode},
    x402::{LedgerFlowAuthorizationExtension, LedgerFlowChallenge},
};

/// LedgerFlow MPP parameter key.
pub const LEDGERFLOW_PARAM: &str = "ledgerflow";

/// Encodes a challenge as a base64url header parameter value.
pub fn encode_challenge_param(challenge: &LedgerFlowChallenge) -> Result<String, ProtocolError> {
    let bytes = cbor_encode(challenge, MAX_HEADER_CBOR_BYTES)?;
    Ok(base64url_encode(&bytes))
}

/// Decodes a challenge from a base64url header parameter value.
pub fn decode_challenge_param(value: &str) -> Result<LedgerFlowChallenge, ProtocolError> {
    let bytes = base64url_decode(value)?;
    cbor_decode(&bytes, MAX_HEADER_CBOR_BYTES)
}

/// Encodes an authorization (header-slim: single warrant + proof + signer).
pub fn encode_authorization_param(
    chain: &[Warrant],
    proof: &PopProof,
    signer: &SignerRef,
    payment_subject: &PaymentSubjectRef,
    approvals: &[SignedApproval],
) -> Result<String, ProtocolError> {
    let leaf = chain.last().ok_or(ProtocolError::EmptyChain)?;
    let slim = SlimAuthorization {
        leaf: leaf.clone(),
        proof: proof.clone(),
        signer: signer.clone(),
        payment_subject: payment_subject.clone(),
        approvals: approvals.to_vec(),
    };
    let bytes = cbor_encode(&slim, MAX_HEADER_CBOR_BYTES)?;
    Ok(base64url_encode(&bytes))
}

/// Decodes a header-slim authorization parameter.
pub fn decode_authorization_param(value: &str) -> Result<SlimAuthorization, ProtocolError> {
    let bytes = base64url_decode(value)?;
    cbor_decode(&bytes, MAX_HEADER_CBOR_BYTES)
}

/// The header-slim authorization payload carried in MPP `ledgerflow` params.
///
/// Contains only the leaf warrant (plus proof/signer); the full chain is
/// fetched from the body or an established cache.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SlimAuthorization {
    pub leaf: Warrant,
    pub proof: PopProof,
    pub signer: SignerRef,
    pub payment_subject: PaymentSubjectRef,
    pub approvals: Vec<SignedApproval>,
}

impl SlimAuthorization {
    /// Expands the slim header payload into a full extension by prepending the
    /// cached parent warrants (root-first).
    #[must_use]
    pub fn into_extension(self, parents: Vec<Warrant>) -> LedgerFlowAuthorizationExtension {
        let mut chain = parents;
        chain.push(self.leaf);
        LedgerFlowAuthorizationExtension {
            version: crate::x402::LEDGERFLOW_EXTENSION_VERSION.to_string(),
            challenge_id: self.proof.tuple.challenge_id.clone(),
            warrant_chain: chain,
            proof: self.proof,
            signer: self.signer,
            payment_subject: self.payment_subject,
            approvals: self.approvals,
        }
    }
}
