//! Paymaster / Sponsorship constraint (gasless / sponsored payments).
//!
//! This module implements the `SponsorshipConstraint` that allows
//! gasless/sponsored payments where a paymaster covers the gas fees.
//!
//! Design:
//! - A warrant may declare a sponsorship policy
//! - The paymaster signs a sponsorship commitment
//! - The Facilitator verifies the sponsorship before settlement
//! - Sponsorship is bound to the specific payment (amount, asset, payee)

use serde::{Deserialize, Serialize};

use crate::{
    error::{Result, SponsorshipError},
    warrant::{SignatureEnvelope, SignerRef, SigningKeyPair},
};

/// Domain-separation prefix for sponsorship signatures.
pub const SPONSORSHIP_SIGN_DOMAIN: &[u8] = b"ledgerflow-sponsorship-v1";

/// Sponsorship policy for a warrant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SponsorshipPolicy {
    /// No sponsorship (default).
    None,
    /// Sponsorship allowed with a specific paymaster.
    Allowed { paymaster: SignerRef },
    /// Sponsorship allowed with any registered paymaster.
    Open,
}

/// A signed sponsorship commitment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SponsorshipCommitment {
    /// The paymaster's public key.
    pub paymaster: SignerRef,
    /// The sponsored amount (base units).
    pub amount: u128,
    /// The asset being sponsored.
    pub asset: String,
    /// The payee address.
    pub payee: String,
    /// Unix seconds when the commitment expires.
    pub expires_at: u64,
    /// The paymaster's signature.
    pub signature: SignatureEnvelope,
}

impl SponsorshipCommitment {
    /// Signs a sponsorship commitment.
    #[must_use]
    pub fn sign(
        paymaster: &SignerRef,
        amount: u128,
        asset: impl Into<String>,
        payee: impl Into<String>,
        expires_at: u64,
        paymaster_keys: &SigningKeyPair,
    ) -> Self {
        let asset = asset.into();
        let payee = payee.into();
        let preimage = sponsorship_preimage(paymaster, amount, &asset, &payee, expires_at);
        let signature = paymaster_keys.sign(&preimage);
        Self { paymaster: paymaster.clone(), amount, asset, payee, expires_at, signature }
    }

    /// Verifies the sponsorship signature.
    #[must_use]
    pub fn verify_signature(&self) -> bool {
        let preimage = sponsorship_preimage(
            &self.paymaster,
            self.amount,
            &self.asset,
            &self.payee,
            self.expires_at,
        );
        self.signature.verify_strict(&self.paymaster, &preimage)
    }

    /// Checks if the commitment is expired.
    #[must_use]
    pub const fn is_expired(&self, now_secs: u64) -> bool {
        self.expires_at < now_secs
    }
}

/// Computes the domain-separated sponsorship preimage.
fn sponsorship_preimage(
    paymaster: &SignerRef,
    amount: u128,
    asset: &str,
    payee: &str,
    expires_at: u64,
) -> Vec<u8> {
    let mut preimage = Vec::with_capacity(SPONSORSHIP_SIGN_DOMAIN.len() + 128);
    preimage.extend_from_slice(SPONSORSHIP_SIGN_DOMAIN);
    preimage.extend_from_slice(&paymaster.public_key);
    preimage.extend_from_slice(&amount.to_le_bytes());
    preimage.extend_from_slice(asset.as_bytes());
    preimage.extend_from_slice(payee.as_bytes());
    preimage.extend_from_slice(&expires_at.to_le_bytes());
    preimage
}

/// Verifies a sponsorship commitment against a policy.
pub fn verify_sponsorship(
    commitment: &SponsorshipCommitment,
    policy: &SponsorshipPolicy,
    now_secs: u64,
) -> Result<()> {
    // Check expiry
    if commitment.is_expired(now_secs) {
        return Err(SponsorshipError::Expired.into());
    }

    // Verify signature
    if !commitment.verify_signature() {
        return Err(SponsorshipError::InvalidSignature.into());
    }

    // Check policy
    match policy {
        SponsorshipPolicy::None => Err(SponsorshipError::NotAllowed.into()),
        SponsorshipPolicy::Allowed { paymaster } => {
            if commitment.paymaster.alg != paymaster.alg ||
                commitment.paymaster.public_key != paymaster.public_key
            {
                return Err(SponsorshipError::PaymasterNotAllowed.into());
            }
            Ok(())
        }
        SponsorshipPolicy::Open => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn test_paymaster() -> (SignerRef, SigningKeyPair) {
        let keys = SigningKeyPair::from_bytes(&[0xAA; 32]);
        (keys.signer_ref(), keys)
    }

    #[test]
    fn sponsorship_commitment_roundtrip() {
        let (paymaster, keys) = test_paymaster();
        let commitment = SponsorshipCommitment::sign(
            &paymaster,
            1_000_000,
            "USDC",
            "0xpayee",
            1_900_000_000,
            &keys,
        );

        assert!(commitment.verify_signature());
        assert!(!commitment.is_expired(1_800_000_000));
        assert!(commitment.is_expired(2_000_000_000));
    }

    #[test]
    fn sponsorship_policy_none_rejects() {
        let (paymaster, keys) = test_paymaster();
        let commitment = SponsorshipCommitment::sign(
            &paymaster,
            1_000_000,
            "USDC",
            "0xpayee",
            1_900_000_000,
            &keys,
        );

        let result = verify_sponsorship(&commitment, &SponsorshipPolicy::None, 1_800_000_000);
        assert!(result.is_err());
    }

    #[test]
    fn sponsorship_policy_allowed_accepts_matching_paymaster() {
        let (paymaster, keys) = test_paymaster();
        let commitment = SponsorshipCommitment::sign(
            &paymaster,
            1_000_000,
            "USDC",
            "0xpayee",
            1_900_000_000,
            &keys,
        );

        let policy = SponsorshipPolicy::Allowed { paymaster: paymaster.clone() };
        let result = verify_sponsorship(&commitment, &policy, 1_800_000_000);
        assert!(result.is_ok());
    }

    #[test]
    fn sponsorship_policy_allowed_rejects_wrong_paymaster() {
        let (paymaster, keys) = test_paymaster();
        let commitment = SponsorshipCommitment::sign(
            &paymaster,
            1_000_000,
            "USDC",
            "0xpayee",
            1_900_000_000,
            &keys,
        );

        // Use a different key for the policy
        let other_keys = SigningKeyPair::from_bytes(&[0xBB; 32]);
        let other_paymaster = other_keys.signer_ref();
        let policy = SponsorshipPolicy::Allowed { paymaster: other_paymaster };
        let result = verify_sponsorship(&commitment, &policy, 1_800_000_000);
        assert!(result.is_err());
    }

    #[test]
    fn sponsorship_policy_open_accepts_any() {
        let (paymaster, keys) = test_paymaster();
        let commitment = SponsorshipCommitment::sign(
            &paymaster,
            1_000_000,
            "USDC",
            "0xpayee",
            1_900_000_000,
            &keys,
        );

        let result = verify_sponsorship(&commitment, &SponsorshipPolicy::Open, 1_800_000_000);
        assert!(result.is_ok());
    }

    #[test]
    fn expired_commitment_is_rejected() {
        let (paymaster, keys) = test_paymaster();
        let commitment = SponsorshipCommitment::sign(
            &paymaster,
            1_000_000,
            "USDC",
            "0xpayee",
            1_900_000_000,
            &keys,
        );

        let result = verify_sponsorship(&commitment, &SponsorshipPolicy::Open, 2_000_000_000);
        assert!(result.is_err());
    }
}
