//! Payment constraint: allowed asset(s) and a stateless per-charge cap.

use serde::{Deserialize, Serialize};

use crate::warrant::AssetRef;

/// Payment constraint: allowed asset(s) and a **stateless** per-charge cap.
///
/// Amounts are expressed in the asset's base units (smallest on-chain unit).
/// Period limits and cumulative budgets are deliberately **not** part of v1:
/// they are stateful predicates handled by the accounting Facilitator (P2+).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PaymentConstraint {
    pub allowed_assets: Vec<AssetRef>,
    /// Maximum authorized amount per charge, in base units.
    pub max_per_charge: u128,
    pub allowed_rails: Vec<crate::warrant::PaymentRail>,
    pub allowed_schemes: Vec<String>,
    pub payee_ids: Vec<String>,
}

impl PaymentConstraint {
    /// Creates an "any asset, any rail, any scheme" payment constraint with a
    /// per-charge cap. Callers MUST set at least one allowed asset before use.
    #[must_use]
    pub const fn new(max_per_charge: u128) -> Self {
        Self {
            allowed_assets: Vec::new(),
            max_per_charge,
            allowed_rails: Vec::new(),
            allowed_schemes: Vec::new(),
            payee_ids: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_asset(mut self, asset: AssetRef) -> Self {
        self.allowed_assets.push(asset);
        self
    }

    #[must_use]
    pub fn with_rails(
        mut self,
        rails: impl IntoIterator<Item = crate::warrant::PaymentRail>,
    ) -> Self {
        self.allowed_rails.extend(rails);
        self
    }

    #[must_use]
    pub fn with_schemes(mut self, schemes: impl IntoIterator<Item = String>) -> Self {
        self.allowed_schemes.extend(schemes);
        self
    }

    #[must_use]
    pub fn with_payees(mut self, payees: impl IntoIterator<Item = String>) -> Self {
        self.payee_ids.extend(payees);
        self
    }

    /// Returns `true` when this constraint is satisfied by the context.
    pub fn allows(
        &self,
        amount: u128,
        asset: &str,
        asset_network: Option<&str>,
        rail: &crate::warrant::PaymentRail,
        scheme: &str,
        payee_id: &str,
    ) -> bool {
        if amount > self.max_per_charge {
            return false;
        }
        if !self.allowed_assets.is_empty() &&
            !self.allowed_assets.iter().any(|a| a.matches(asset, asset_network))
        {
            return false;
        }
        if !self.allowed_rails.is_empty() && !self.allowed_rails.iter().any(|r| r == rail) {
            return false;
        }
        if !self.allowed_schemes.is_empty() && !self.allowed_schemes.iter().any(|s| s == scheme) {
            return false;
        }
        if !self.payee_ids.is_empty() && !self.payee_ids.iter().any(|p| p == payee_id) {
            return false;
        }
        true
    }
}
