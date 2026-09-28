//! Subject and asset types for warrants.

use std::{
    collections::BTreeMap,
    fmt::{self, Display},
};

use serde::{Deserialize, Serialize};

/// Opaque settlement subject that only the Facilitator interprets.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PaymentSubjectRef {
    pub kind: PaymentSubjectKind,
    pub value: String,
}

impl PaymentSubjectRef {
    #[must_use]
    pub fn new(kind: PaymentSubjectKind, value: impl Into<String>) -> Self {
        Self { kind, value: value.into() }
    }
}

/// Supported payment subject kinds.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[non_exhaustive]
pub enum PaymentSubjectKind {
    Caip10,
    FacilitatorAccount,
    ExchangeAccount,
    Opaque,
}

impl Display for PaymentSubjectKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Caip10 => "caip10",
            Self::FacilitatorAccount => "facilitator_account",
            Self::ExchangeAccount => "exchange_account",
            Self::Opaque => "opaque",
        };
        formatter.write_str(value)
    }
}

/// A payment asset allowed by the warrant (CAIP-19 when available).
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AssetRef {
    /// Asset identifier. Prefer CAIP-19 (`eip155:8453/slip44:60:0x8335...`).
    pub asset: String,
    /// Optional network hint (kept for compatibility with legacy fixtures).
    pub network: Option<String>,
}

impl AssetRef {
    #[must_use]
    pub fn new(asset: impl Into<String>, network: Option<String>) -> Self {
        Self { asset: asset.into(), network }
    }

    /// Returns `true` when `candidate` matches this asset.
    #[must_use]
    pub fn matches(&self, candidate: &str, candidate_network: Option<&str>) -> bool {
        if self.asset != candidate {
            return false;
        }
        match (&self.network, candidate_network) {
            (Some(expected), Some(given)) => expected == given,
            _ => true,
        }
    }
}

/// High-level settlement rails allowed by a warrant.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[non_exhaustive]
pub enum PaymentRail {
    Onchain,
    Exchange,
    Custodial,
    TraditionalGateway,
}

impl Display for PaymentRail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Onchain => "onchain",
            Self::Exchange => "exchange",
            Self::Custodial => "custodial",
            Self::TraditionalGateway => "traditional_gateway",
        };
        formatter.write_str(value)
    }
}

/// Additional metadata carried in a warrant (application-specific).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct WarrantMetadata {
    pub entries: BTreeMap<String, String>,
}
