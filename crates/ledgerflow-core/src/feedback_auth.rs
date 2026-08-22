//! EIP-8004 `FeedbackAuth` authorization tuples.
//!
//! Before a client may submit reputation feedback for an agent through the
//! EIP-8004 `ReputationRegistry`, the agent (service provider) signs an
//! authorization tuple:
//!
//! ```text
//! abi.encode(
//!     uint256 agentId,
//!     address clientAddress,
//!     uint64 indexLimit,
//!     uint256 expiry,
//!     uint256 chainId,
//!     address identityRegistry,
//!     address signerAddress
//! )
//! ```
//!
//! The digest is `keccak256(abi.encode(...))`, signed under EIP-191
//! `personal_sign` semantics over the 32-byte digest (i.e.
//! `ECDSA.toEthSignedMessageHash(bytes32)` first, matching the reference
//! implementations). LedgerFlow uses this to let settled payments flow into
//! the EIP-8004 reputation layer with verifiable provenance.

use serde::{Deserialize, Serialize};

use crate::{
    crypto::{Secp256k1KeyPair, eip191_hash_of_bytes32, keccak256},
    warrant::SignatureEnvelope,
};

/// ABI word size used by `abi.encode`.
const WORD: usize = 32;

/// An EIP-8004 feedback authorization tuple.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeedbackAuth {
    /// The rated agent's EIP-8004 token id.
    pub agent_id: u64,
    /// The client allowed to submit feedback (20-byte address).
    pub client_address: [u8; 20],
    /// Maximum feedback index the client may use (inclusive anti-replay cap).
    pub index_limit: u64,
    /// Unix seconds after which this authorization is invalid.
    pub expiry: u64,
    /// Chain id of the identity registry.
    pub chain_id: u64,
    /// The EIP-8004 IdentityRegistry contract address.
    pub identity_registry: [u8; 20],
    /// The address expected to have signed this authorization.
    pub signer_address: [u8; 20],
}

impl FeedbackAuth {
    /// Encodes the tuple exactly as Solidity's `abi.encode` would for
    /// `(uint256, address, uint64, uint256, uint256, address, address)`:
    /// seven big-endian 32-byte words with values right-aligned.
    #[must_use]
    pub fn encode_abi(&self) -> [u8; 224] {
        let mut out = [0_u8; 7 * WORD];
        // Word 0 (bytes 0..32): uint256 agentId.
        out[24..32].copy_from_slice(&self.agent_id.to_be_bytes());
        // Word 1 (bytes 32..64): address clientAddress, left-padded.
        out[44..64].copy_from_slice(&self.client_address);
        // Word 2 (bytes 64..96): uint64 indexLimit, right-aligned.
        out[88..96].copy_from_slice(&self.index_limit.to_be_bytes());
        // Word 3 (bytes 96..128): uint256 expiry.
        out[120..128].copy_from_slice(&self.expiry.to_be_bytes());
        // Word 4 (bytes 128..160): uint256 chainId.
        out[152..160].copy_from_slice(&self.chain_id.to_be_bytes());
        // Word 5 (bytes 160..192): address identityRegistry, left-padded.
        out[172..192].copy_from_slice(&self.identity_registry);
        // Word 6 (bytes 192..224): address signerAddress, left-padded.
        out[204..224].copy_from_slice(&self.signer_address);
        out
    }

    /// Computes `keccak256(encode_abi())`.
    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        keccak256(self.encode_abi().as_slice())
    }

    /// Signs the tuple digest under EIP-191 `personal_sign` semantics over
    /// the 32-byte digest (`toEthSignedMessageHash(bytes32)` convention),
    /// producing a 65-byte recoverable envelope.
    #[must_use]
    pub fn sign(&self, keys: &Secp256k1KeyPair) -> SignatureEnvelope {
        keys.sign_eth_personal(&self.digest())
    }

    /// Verifies an envelope against [`Self::signer_address`].
    ///
    /// The verification message is the raw 32-byte tuple digest; the
    /// EthPersonalSign path applies the EIP-191 bytes32 prefix internally.
    #[must_use]
    pub fn verify(&self, signature: &SignatureEnvelope) -> bool {
        let signer = crate::warrant::SignerRef::new(
            crate::warrant::SigningAlgorithm::EthPersonalSign,
            self.signer_address.to_vec(),
        );
        signature.verify_strict(&signer, &self.digest())
    }

    /// Returns the EIP-191 hash that an external (e.g. browser-wallet)
    /// signer would sign for this tuple, useful when signing happens outside
    /// of this crate.
    #[must_use]
    pub fn external_signing_hash(&self) -> [u8; 32] {
        eip191_hash_of_bytes32(&self.digest())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::warrant::SigningAlgorithm;

    fn sample() -> FeedbackAuth {
        FeedbackAuth {
            agent_id: 22,
            client_address: [0x11; 20],
            index_limit: 5,
            expiry: 1_900_000_000,
            chain_id: 1,
            identity_registry: [0x80; 20],
            signer_address: [0x99; 20],
        }
    }

    #[test]
    fn encode_abi_layout_is_exact() {
        let encoded = sample().encode_abi();
        assert_eq!(encoded.len(), 224);
        // Word 0: agentId right-aligned.
        assert!(encoded[0..24].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[24..32], &22_u64.to_be_bytes());
        // Word 1: address left-padded with 12 zero bytes.
        assert!(encoded[32..44].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[44..64], &[0x11; 20]);
        // Word 2: uint64 indexLimit occupies only the low 8 bytes.
        assert!(encoded[64..88].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[88..96], &5_u64.to_be_bytes());
        // Word 3: expiry right-aligned.
        assert!(encoded[96..120].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[120..128], &1_900_000_000_u64.to_be_bytes());
        // Word 4: chainId right-aligned.
        assert!(encoded[128..152].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[152..160], &1_u64.to_be_bytes());
        // Word 5/6 addresses left-padded.
        assert!(encoded[160..172].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[172..192], &[0x80; 20]);
        assert!(encoded[192..204].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[204..224], &[0x99; 20]);
    }

    #[test]
    fn digest_binds_every_field() {
        let base = sample();
        let baseline = base.digest();
        let mutated = |mutate: &dyn Fn(&mut FeedbackAuth)| {
            let mut auth = sample();
            mutate(&mut auth);
            auth.digest()
        };
        assert_ne!(baseline, mutated(&|auth| auth.agent_id = 23));
        assert_ne!(baseline, mutated(&|auth| auth.client_address = [0x12; 20]));
        assert_ne!(baseline, mutated(&|auth| auth.index_limit = 6));
        assert_ne!(baseline, mutated(&|auth| auth.expiry += 1));
        assert_ne!(baseline, mutated(&|auth| auth.chain_id = 8453));
        assert_ne!(baseline, mutated(&|auth| auth.identity_registry = [0x81; 20]));
        assert_ne!(baseline, mutated(&|auth| auth.signer_address = [0x9A; 20]));
    }

    #[test]
    fn sign_verify_roundtrip_and_tamper() {
        let keys = Secp256k1KeyPair::from_bytes(&[0x7A; 32]).expect("valid key");
        let mut auth = sample();
        auth.signer_address = keys.ethereum_address();
        let envelope = auth.sign(&keys);
        assert_eq!(envelope.alg, SigningAlgorithm::EthPersonalSign);
        assert_eq!(envelope.value.len(), 65);
        assert!(auth.verify(&envelope));

        // Tampered tuple no longer verifies.
        let mut tampered = auth.clone();
        tampered.agent_id += 1;
        assert!(!tampered.verify(&envelope));

        // Wrong signer key fails.
        let other = Secp256k1KeyPair::from_bytes(&[0x7B; 32]).expect("valid key");
        assert!(!auth.verify(&other.sign_eth_personal(&auth.digest())));

        // External signing hash composes: signing it via typed-data-style
        // raw-digest path equals the internal preimage convention.
        let external = auth.external_signing_hash();
        assert_eq!(external, eip191_hash_of_bytes32(&auth.digest()));
    }
}
