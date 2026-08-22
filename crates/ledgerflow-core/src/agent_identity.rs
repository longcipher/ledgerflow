//! EIP-8004 agent identity references and the resolver seam.
//!
//! LedgerFlow binds opaque public keys; EIP-8004 gives agents portable
//! on-chain identities. This module bridges the two without coupling the
//! core to any chain client:
//!
//! - [`AgentIdRef`] parses and renders the global agent identifier
//!   `{namespace}:{chainId}:{identityRegistry}/{agentId}` (e.g.
//!   `eip155:1:0x8004A169FB4a3325136EB29fA0ceB6D2e539a432/22`).
//! - The reserved warrant extension key [`AGENT_ID_EXTENSION_KEY`] carries a serialized
//!   `AgentIdRef`; [`agent_id_from_warrant`] extracts it.
//! - [`IdentityResolver`] is the I/O seam: downstream crates resolve an `AgentIdRef` to its
//!   currently valid signer keys (from the on-chain registration file, `agentWallet`, or cached
//!   metadata), enabling discoverable trust anchors ([`crate::trust`]).

use serde::{Deserialize, Serialize};

use crate::warrant::{SignerRef, Warrant};

/// Reserved warrant extension key carrying an EIP-8004 agent reference.
pub const AGENT_ID_EXTENSION_KEY: &str = "ledgerflow.agent_id";

/// A parsed EIP-8004 global agent identifier.
///
/// Canonical form: `{namespace}:{chainId}:{registry}/{agentId}` where
/// `namespace` is the CAIP-2 chain family (`eip155`), `chainId` the network
/// identifier, `registry` the lowercase hex IdentityRegistry address, and
/// `agentId` the ERC-721 token id.
///
/// Serialized as its canonical string form so anchored trust configurations
/// stay human-readable and match the `ledgerflow.agent_id` extension value
/// byte-for-byte.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AgentIdRef {
    /// Chain family namespace (currently always `eip155`).
    pub namespace: String,
    /// Network identifier within the namespace (e.g. `1`, `8453`).
    pub chain_id: String,
    /// Lowercase hex IdentityRegistry contract address (`0x…`, 40 hex chars).
    pub registry: String,
    /// The agent's ERC-721 token id.
    pub agent_id: u64,
}

impl Serialize for AgentIdRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for AgentIdRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// Errors produced while parsing an [`AgentIdRef`].
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum AgentIdParseError {
    /// The value does not match `{ns}:{chain}:{registry}/{id}`.
    #[error("malformed agent identity reference `{0}`")]
    Malformed(String),
    /// The registry component is not a 40-digit lowercase hex address.
    #[error("invalid identity registry address in `{0}`")]
    InvalidRegistry(String),
    /// The agent id component is not a decimal integer.
    #[error("invalid agent id in `{0}`")]
    InvalidAgentId(String),
}

impl AgentIdRef {
    /// Parses a canonical `namespace:chainId:registry/agentId` reference.
    ///
    /// # Errors
    /// Returns [`AgentIdParseError`] when any component is malformed;
    /// parsing is strict (lowercase hex registry, decimal id) so that
    /// round-tripping is lossless.
    pub fn parse(value: &str) -> Result<Self, AgentIdParseError> {
        let malformed = || AgentIdParseError::Malformed(value.to_string());
        let (head, agent_id) = value.split_once('/').ok_or_else(malformed)?;
        let mut parts = head.split(':');
        let namespace = parts.next().ok_or_else(malformed)?;
        let chain_id = parts.next().ok_or_else(malformed)?;
        let registry = parts.next().ok_or_else(malformed)?;
        if parts.next().is_some() ||
            namespace.is_empty() ||
            chain_id.is_empty() ||
            agent_id.is_empty()
        {
            return Err(malformed());
        }
        if !is_lowercase_hex_address(registry) {
            return Err(AgentIdParseError::InvalidRegistry(value.to_string()));
        }
        let agent_id: u64 =
            agent_id.parse().map_err(|_| AgentIdParseError::InvalidAgentId(value.to_string()))?;
        Ok(Self {
            namespace: namespace.to_string(),
            chain_id: chain_id.to_string(),
            registry: registry.to_string(),
            agent_id,
        })
    }

    /// Returns the `namespace:chainId:registry` registry coordinate (the
    /// EIP-8004 `agentRegistry` string, sans token id).
    #[must_use]
    pub fn agent_registry(&self) -> String {
        format!("{}:{}:{}", self.namespace, self.chain_id, self.registry)
    }
}

impl std::fmt::Display for AgentIdRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}/{}", self.agent_registry(), self.agent_id)
    }
}

/// Checks `0x` + exactly 40 lowercase hex characters.
fn is_lowercase_hex_address(value: &str) -> bool {
    let Some(hex_part) = value.strip_prefix("0x") else {
        return false;
    };
    hex_part.len() == 40 &&
        hex_part.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Extracts and parses the optional EIP-8004 agent reference from a
/// warrant's extensions map.
///
/// Returns `Ok(None)` when the extension is absent. A present but malformed
/// value is an error (fail-closed): callers must not silently treat a broken
/// identity claim as no claim.
///
/// # Errors
/// Returns [`AgentIdParseError`] when the extension value exists but cannot
/// be parsed as UTF-8 or as an [`AgentIdRef`].
pub fn agent_id_from_warrant(warrant: &Warrant) -> Result<Option<AgentIdRef>, AgentIdParseError> {
    let Some(bytes) = warrant.extensions.get(AGENT_ID_EXTENSION_KEY) else {
        return Ok(None);
    };
    let value = std::str::from_utf8(bytes)
        .map_err(|_| AgentIdParseError::Malformed(format!("{:?}", bytes)))?;
    AgentIdRef::parse(value).map(Some)
}

/// Resolves an EIP-8004 agent identity to its currently valid signer keys.
///
/// Implemented by downstream crates over chain RPC / IPFS / caches. The core
/// stays stateless and I/O-free.
pub trait IdentityResolver: Send + Sync {
    /// Returns the set of signer keys currently bound to the agent identity.
    ///
    /// # Errors
    /// Implementations return [`AuthorizationError::IdentityResolutionFailed`]
    /// when the identity cannot be resolved (network failure, unknown agent,
    /// malformed registration). An empty key set means "resolved but binds no
    /// keys" and fails trust checks downstream.
    fn resolve_keys(&self, agent: &AgentIdRef) -> crate::error::Result<Vec<SignerRef>>;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::{SigningKeyPair, WarrantBuilder};

    const VALID: &str = "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/22";

    fn sample() -> AgentIdRef {
        AgentIdRef::parse(VALID).expect("valid")
    }

    #[test]
    fn parse_valid_reference() {
        let parsed = sample();
        assert_eq!(parsed.namespace, "eip155");
        assert_eq!(parsed.chain_id, "1");
        assert_eq!(parsed.registry, "0x8004a169fb4a3325136eb29fa0ceb6d2e539a432");
        assert_eq!(parsed.agent_id, 22);
        assert_eq!(parsed.agent_registry(), "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432");
    }

    #[test]
    fn display_round_trips() {
        assert_eq!(sample().to_string(), VALID);
        let l2 = AgentIdRef::parse("eip155:8453:0x0000000000000000000000000000000000000001/7")
            .expect("valid");
        assert_eq!(l2.to_string(), "eip155:8453:0x0000000000000000000000000000000000000001/7");
    }

    #[test]
    fn parse_rejects_malformed_inputs() {
        let addr = "0x8004a169fb4a3325136eb29fa0ceb6d2e539a432";
        for bad in [
            "",
            "eip155:1:0xabc",
            "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432",
            // Uppercase hex rejected (strict canonical form).
            "eip155:1:0x8004A169FB4a3325136EB29fA0ceB6D2e539a432/22",
            // Short address.
            "eip155:1:0x1234/22",
            // Non-decimal id.
            "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/2x",
            // Negative id.
            "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/-1",
            // Trailing junk.
            "eip155:1:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/22/x",
            // Too many colons.
            "eip155:1:extra:0x8004a169fb4a3325136eb29fa0ceb6d2e539a432/22",
            // Empty individual components.
            &format!(":1:{addr}/22"),
            &format!("eip155::{addr}/22"),
            &format!("eip155:1:{addr}/"),
        ] {
            assert!(AgentIdRef::parse(bad).is_err(), "expected reject: {bad}");
        }
    }

    #[test]
    fn extract_from_warrant_extensions() {
        let issuer = SigningKeyPair::from_bytes(&[0x0F; 32]);
        let holder = SigningKeyPair::from_bytes(&[0x10; 32]);
        let warrant = WarrantBuilder::new(1_000)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(crate::constraint::MerchantConstraint::with_ids(vec!["m".to_string()]))
            .resource(crate::constraint::ResourceConstraint::default())
            .payment(crate::constraint::PaymentConstraint::new(100))
            .extension(AGENT_ID_EXTENSION_KEY, VALID.as_bytes().to_vec())
            .sign_with(&issuer, [0_u8; 8]);

        let extracted = agent_id_from_warrant(&warrant).expect("parses");
        assert_eq!(extracted, Some(sample()));

        // Absent extension → None.
        let plain = WarrantBuilder::new(1_000)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(crate::constraint::MerchantConstraint::with_ids(vec!["m".to_string()]))
            .resource(crate::constraint::ResourceConstraint::default())
            .payment(crate::constraint::PaymentConstraint::new(100))
            .sign_with(&issuer, [0_u8; 8]);
        assert_eq!(agent_id_from_warrant(&plain).expect("none"), None);
    }

    #[test]
    fn malformed_extension_value_is_an_error() {
        let issuer = SigningKeyPair::from_bytes(&[0x11; 32]);
        let holder = SigningKeyPair::from_bytes(&[0x12; 32]);
        let warrant = WarrantBuilder::new(1_000)
            .issuer(issuer.signer_ref())
            .holder(holder.signer_ref())
            .merchant(crate::constraint::MerchantConstraint::with_ids(vec!["m".to_string()]))
            .resource(crate::constraint::ResourceConstraint::default())
            .payment(crate::constraint::PaymentConstraint::new(100))
            .extension(AGENT_ID_EXTENSION_KEY, b"not-an-agent-ref".to_vec())
            .sign_with(&issuer, [0_u8; 8]);
        assert!(agent_id_from_warrant(&warrant).is_err());
    }
}
