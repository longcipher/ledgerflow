//! Approval signing through a [`WalletSigner`].
//!
//! An m-of-n approval is bound to a request hash and an approver key. Remote
//! wallets hold the approver's private key, so the approval signature is
//! produced by the wallet (domain [`SignDomain::Approval`]) rather than by
//! the caller directly.

use ledgerflow_core::{SignedApproval, SignerRef};

use crate::{
    error::WalletError,
    signer::{SignDomain, SignRequest, WalletSigner},
};

/// Default approval TTL (300 seconds).
pub const DEFAULT_APPROVAL_TTL_SECS: u64 = 300;

/// Requests an approval signature from a wallet and constructs a
/// [`SignedApproval`].
///
/// The wallet signs the domain-separated approval preimage; the resulting
/// [`SignedApproval`] can then be verified with
/// [`SignedApproval::verify_signature`].
pub fn request_approval(
    signer: &dyn WalletSigner,
    approver: SignerRef,
    request_hash: &str,
    now_ms: u64,
) -> Result<SignedApproval, WalletError> {
    let expires_at = now_ms / 1000 + DEFAULT_APPROVAL_TTL_SECS;
    let preimage = approval_preimage(request_hash, &approver, expires_at);
    let result = signer.sign(&SignRequest {
        domain: SignDomain::Approval,
        message: preimage,
        key: Some(approver.clone()),
    })?;
    Ok(SignedApproval {
        request_hash: request_hash.to_string(),
        approver,
        expires_at,
        signature: result.signature,
    })
}

/// Computes the domain-separated approval preimage (mirrors core semantics).
fn approval_preimage(request_hash: &str, approver: &SignerRef, expires_at: u64) -> Vec<u8> {
    const APPROVAL_SIGN_DOMAIN: &[u8] = ledgerflow_core::approval::APPROVAL_SIGN_DOMAIN;
    let mut preimage = Vec::with_capacity(APPROVAL_SIGN_DOMAIN.len() + 128);
    preimage.extend_from_slice(APPROVAL_SIGN_DOMAIN);
    preimage.extend_from_slice(request_hash.as_bytes());
    preimage.extend_from_slice(&approver.public_key);
    preimage.extend_from_slice(&expires_at.to_le_bytes());
    preimage
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use ledgerflow_core::SigningKeyPair;

    use super::*;
    use crate::embedded::EmbeddedSigner;

    #[test]
    fn request_approval_computes_expiry_from_now() {
        let key = SigningKeyPair::from_bytes(&[0x99; 32]);
        let wallet = EmbeddedSigner::new(key.clone());
        let approval =
            request_approval(&wallet, key.signer_ref(), "sha256:req", 10_000).expect("approval");
        // now_ms/1000 + TTL = 10 + 300 = 310.
        assert_eq!(approval.expires_at, 10 + DEFAULT_APPROVAL_TTL_SECS);
        assert_eq!(approval.request_hash, "sha256:req");
        assert!(approval.verify_signature());
    }

    #[test]
    fn request_approval_verifies_with_core() {
        let key = SigningKeyPair::from_bytes(&[0x98; 32]);
        let wallet = EmbeddedSigner::new(key.clone());
        let approval =
            request_approval(&wallet, key.signer_ref(), "sha256:req", 5_000).expect("approval");
        assert!(
            ledgerflow_core::SignedApproval {
                request_hash: approval.request_hash.clone(),
                approver: approval.approver.clone(),
                expires_at: approval.expires_at,
                signature: approval.signature,
            }
            .verify_signature()
        );
    }

    #[test]
    fn request_approval_rejects_wrong_key() {
        let key = SigningKeyPair::from_bytes(&[0x97; 32]);
        let other = SigningKeyPair::from_bytes(&[0x96; 32]);
        let wallet = EmbeddedSigner::new(key);
        let error = request_approval(&wallet, other.signer_ref(), "sha256:req", 5_000)
            .expect_err("key mismatch");
        assert!(matches!(error, crate::error::WalletError::NoMatchingKey));
    }
}
