//! Issuance bounds: what a warrant's holder may delegate to descendants.
//!
//! A root warrant can carry an optional `IssueBounds` that constrains the
//! warrants its holder is allowed to issue (delegate). This provides
//! defense-in-depth for the control plane: even if a root signing key is
//! compromised, the attacker can only delegate within the bounds, never
//! arbitrarily.
//!
//! Bounds are carried in the warrant's `extensions` map under the reserved
//! key [`ISSUE_BOUNDS_EXTENSION`], encoded as CBOR (see [`IssueBounds`]).
//! Unknown extension keys are rejected by the decoder, so the reserved key is
//! the only way to express bounds on the wire.

use serde::{Deserialize, Serialize};

use crate::{
    error::WireResult,
    warrant::{AssetRef, CborCodec, PaymentRail, Warrant},
};

/// Reserved extension key carrying the issuance bounds.
pub const ISSUE_BOUNDS_EXTENSION: &str = "ledgerflow.issue_bounds";

/// Limits on the warrants this warrant's holder may issue (delegate).
///
/// Every dimension is a *ceiling*: a delegated warrant's corresponding
/// constraint must be no wider than the bound. Empty lists mean "no
/// restriction" (any value allowed), mirroring the warrant constraint
/// semantics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IssueBounds {
    /// Merchant ids the issuer may delegate to (empty = any).
    pub merchant_ids: Vec<String>,
    /// Host suffixes the issuer may delegate to (empty = any).
    pub host_suffixes: Vec<String>,
    /// HTTP methods the issuer may delegate (empty = any).
    pub http_methods: Vec<String>,
    /// Path prefixes the issuer may delegate (empty = any).
    pub path_prefixes: Vec<String>,
    /// Assets the issuer may delegate (empty = any).
    pub assets: Vec<AssetRef>,
    /// Rails the issuer may delegate (empty = any).
    pub rails: Vec<PaymentRail>,
    /// Schemes the issuer may delegate (empty = any).
    pub schemes: Vec<String>,
    /// Payees the issuer may delegate to (empty = any).
    pub payee_ids: Vec<String>,
    /// Maximum per-charge amount the issuer may delegate (`None` = inherit
    /// the parent's cap, which is already monotonic).
    pub max_per_charge: Option<u128>,
    /// Maximum delegation depth for issued warrants.
    pub max_issue_depth: Option<u8>,
}

impl IssueBounds {
    /// Creates unrestricted bounds (no limits beyond what the parent itself
    /// already constrains).
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self {
            merchant_ids: Vec::new(),
            host_suffixes: Vec::new(),
            http_methods: Vec::new(),
            path_prefixes: Vec::new(),
            assets: Vec::new(),
            rails: Vec::new(),
            schemes: Vec::new(),
            payee_ids: Vec::new(),
            max_per_charge: None,
            max_issue_depth: None,
        }
    }

    /// Encodes the bounds as CBOR bytes (for embedding in `extensions`).
    pub fn encode_cbor(&self) -> WireResult<Vec<u8>> {
        <Self as CborCodec>::encode_cbor(self)
    }

    /// Decodes bounds from CBOR bytes.
    pub fn decode_cbor(bytes: &[u8]) -> WireResult<Self> {
        <Self as CborCodec>::decode_cbor(bytes)
    }
}

impl CborCodec for IssueBounds {}

impl Warrant {
    /// Returns the issuance bounds carried by this warrant, if any.
    pub fn issue_bounds(&self) -> Option<IssueBounds> {
        let bytes = self.extensions.get(ISSUE_BOUNDS_EXTENSION)?;
        IssueBounds::decode_cbor(bytes).ok()
    }
}
