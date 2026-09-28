//! Key rotation with dual-signature window.
//!
//! Implements the key rotation mechanism from the design doc:
//! - Old and new roots are both valid for N days during transition
//! - Warrants issued by the new root carry the key id
//! - The old root leaves the set after expiry
//!
//! This module provides the `KeyRotationManager` that tracks multiple
//! trusted issuers and handles the transition period.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    error::{AuthorizationError, Result},
    trust::TrustedIssuer,
    warrant::Warrant,
};

/// Default dual-signature window duration (7 days).
pub const DEFAULT_ROTATION_WINDOW_SECS: u64 = 7 * 24 * 60 * 60;

/// Key rotation state for a single issuer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyRotationState {
    /// The current active key.
    pub current: TrustedIssuer,
    /// The previous key during transition (if any).
    pub previous: Option<TrustedIssuer>,
    /// Timestamp when the rotation started (unix seconds).
    pub rotation_started_at: u64,
    /// Duration of the dual-signature window (seconds).
    pub window_secs: u64,
}

impl KeyRotationState {
    /// Creates a new key rotation state with a single key.
    #[must_use]
    pub const fn new(current: TrustedIssuer) -> Self {
        Self {
            current,
            previous: None,
            rotation_started_at: 0,
            window_secs: DEFAULT_ROTATION_WINDOW_SECS,
        }
    }

    /// Starts a rotation to a new key.
    pub fn rotate(&mut self, new_key: TrustedIssuer, now_secs: u64) {
        self.previous = Some(self.current.clone());
        self.current = new_key;
        self.rotation_started_at = now_secs;
    }

    /// Checks if the dual-signature window has expired.
    #[must_use]
    pub const fn is_window_expired(&self, now_secs: u64) -> bool {
        self.previous.is_none() ||
            now_secs >= self.rotation_started_at + self.window_secs
    }

    /// Finalizes the rotation by removing the old key.
    pub fn finalize(&mut self) {
        self.previous = None;
        self.rotation_started_at = 0;
    }
}

/// Manages key rotation for multiple trusted issuers.
#[derive(Clone, Debug, Default)]
pub struct KeyRotationManager {
    /// Rotation states keyed by issuer key_id.
    states: BTreeMap<String, KeyRotationState>,
}

impl KeyRotationManager {
    /// Creates a new empty key rotation manager.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            states: BTreeMap::new(),
        }
    }

    /// Adds a trusted issuer (no rotation).
    pub fn add_issuer(&mut self, issuer: TrustedIssuer) {
        let key_id = issuer.key_id.clone();
        self.states
            .entry(key_id)
            .or_insert_with(|| KeyRotationState::new(issuer));
    }

    /// Starts a rotation for an issuer.
    pub fn start_rotation(
        &mut self,
        key_id: &str,
        new_key: TrustedIssuer,
        now_secs: u64,
    ) -> Result<()> {
        let state = self.states.get_mut(key_id).ok_or_else(|| {
            AuthorizationError::UntrustedIssuer {
                key_id: key_id.to_string(),
            }
        })?;

        state.rotate(new_key, now_secs);
        Ok(())
    }

    /// Verifies if a warrant's issuer is trusted (considering rotation).
    #[must_use]
    pub fn verify_issuer(&self, warrant: &Warrant, now_secs: u64) -> bool {
        let issuer = &warrant.issuer;

        for state in self.states.values() {
            // Check current key
            if Self::signer_matches(&state.current.issuer, issuer) {
                return true;
            }

            // Check previous key during window
            if let Some(previous) = &state.previous
                && Self::signer_matches(&previous.issuer, issuer)
            {
                return !state.is_window_expired(now_secs);
            }
        }

        false
    }

    /// Checks if two signer references match, including key_id when present.
    fn signer_matches(
        trusted: &crate::warrant::SignerRef,
        issuer: &crate::warrant::SignerRef,
    ) -> bool {
        if trusted.alg != issuer.alg || trusted.public_key != issuer.public_key {
            return false;
        }
        // Strict key_id policy: both sides must agree when either pins a key_id.
        match (&trusted.key_id, &issuer.key_id) {
            (Some(expected), Some(actual)) => expected == actual,
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
    }

    /// Finalizes rotations whose windows have expired.
    pub fn finalize_expired_rotations(&mut self, now_secs: u64) {
        for state in self.states.values_mut() {
            if state.is_window_expired(now_secs) {
                state.finalize();
            }
        }
    }

    /// Returns the rotation state for a key_id.
    #[must_use]
    pub fn get_state(&self, key_id: &str) -> Option<&KeyRotationState> {
        self.states.get(key_id)
    }

    /// Returns all key ids being managed.
    #[must_use]
    pub fn key_ids(&self) -> Vec<&str> {
        self.states.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::SigningKeyPair;

    fn test_issuer(key_id: &str, secret: [u8; 32]) -> TrustedIssuer {
        let keys = SigningKeyPair::from_bytes(&secret);
        TrustedIssuer::new(key_id.to_string(), keys.signer_ref())
    }

    fn test_warrant(issuer: &crate::warrant::SignerRef) -> Warrant {
        let holder = SigningKeyPair::from_bytes(&[0x99; 32]);
        crate::WarrantBuilder::new(1_000_000)
            .issuer(issuer.clone())
            .holder(holder.signer_ref())
            .merchant(crate::constraint::MerchantConstraint::with_ids(vec!["m".to_string()]))
            .resource(crate::constraint::ResourceConstraint::default())
            .payment(crate::constraint::PaymentConstraint::new(100))
            .sign_with(&SigningKeyPair::from_bytes(&[0x88; 32]), [0_u8; 8])
    }

    #[test]
    fn single_key_is_trusted() {
        let mut manager = KeyRotationManager::new();
        let issuer = test_issuer("key-1", [0x11; 32]);
        manager.add_issuer(issuer.clone());

        let warrant = test_warrant(&issuer.issuer);
        assert!(manager.verify_issuer(&warrant, 1_000_000));
    }

    #[test]
    fn key_rotation_exports_work() {
        // Verify the public API is accessible
        let state = KeyRotationState::new(test_issuer("key-1", [0x11; 32]));
        assert!(state.previous.is_none());
        assert_eq!(state.window_secs, DEFAULT_ROTATION_WINDOW_SECS);
    }

    #[test]
    fn rotation_works_during_window() {
        let mut manager = KeyRotationManager::new();
        let old_issuer = test_issuer("key-1", [0x11; 32]);
        manager.add_issuer(old_issuer.clone());

        let old_warrant = test_warrant(&old_issuer.issuer);
        assert!(manager.verify_issuer(&old_warrant, 1_000_000));

        // Start rotation
        let new_issuer = test_issuer("key-1", [0x22; 32]);
        manager
            .start_rotation("key-1", new_issuer.clone(), 1_000_000)
            .expect("rotation");

        let new_warrant = test_warrant(&new_issuer.issuer);
        assert!(manager.verify_issuer(&new_warrant, 1_000_000));
        // Old key still works during window
        assert!(manager.verify_issuer(&old_warrant, 1_000_000));
    }

    #[test]
    fn old_key_expires_after_window() {
        let mut manager = KeyRotationManager::new();
        let old_issuer = test_issuer("key-1", [0x11; 32]);
        manager.add_issuer(old_issuer.clone());

        let old_warrant = test_warrant(&old_issuer.issuer);

        // Start rotation
        let new_issuer = test_issuer("key-1", [0x22; 32]);
        manager
            .start_rotation("key-1", new_issuer.clone(), 1_000_000)
            .expect("rotation");

        // After window expires, old key no longer works
        let after_window = 1_000_000 + DEFAULT_ROTATION_WINDOW_SECS + 1;
        assert!(!manager.verify_issuer(&old_warrant, after_window));

        // Finalize and verify old key is gone
        manager.finalize_expired_rotations(after_window);
        assert!(manager.get_state("key-1").expect("state").previous.is_none());
    }

    #[test]
    fn untrusted_issuer_is_rejected() {
        let mut manager = KeyRotationManager::new();
        let issuer = test_issuer("key-1", [0x11; 32]);
        manager.add_issuer(issuer);

        let stranger = test_issuer("key-2", [0x33; 32]);
        let warrant = test_warrant(&stranger.issuer);
        assert!(!manager.verify_issuer(&warrant, 1_000_000));
    }
}
