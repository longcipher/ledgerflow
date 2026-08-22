//! Stateless, decidable constraints for LedgerFlow warrants.
//!
//! v1 constraints are **stateless predicates only**: merchant allowlist,
//! resource (method/path) allowlist, payment (asset + per-charge cap), and
//! optional AI tool allowlist. Period limits and sponsorship are deliberately
//! excluded from v1 and live behind the accounting point (P2+).

use serde::{Deserialize, Serialize};

use crate::{
    error::{AuthorizationError, Result},
    warrant::{AssetRef, PaymentSubjectRef, SignerRef},
};

/// Authorization request context used to evaluate constraints.
///
/// This is the merchant-side view of an incoming payment: which merchant,
/// which resource, which quote was selected, and so on. All fields are
/// required; missing context should be rejected by the caller before it
/// reaches constraint evaluation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationContext {
    pub merchant_id: String,
    pub merchant_host: String,
    pub tool_name: String,
    pub model_provider: String,
    pub action_label: String,
    pub http_method: String,
    pub path_and_query: String,
    /// Selected quote amount, in the asset's base units.
    pub selected_amount: u128,
    /// Asset string of the selected quote (CAIP-19 when available).
    pub asset: String,
    pub asset_network: Option<String>,
    pub scheme: String,
    pub payee_id: String,
    pub rail: crate::warrant::PaymentRail,
    pub challenge_id: String,
    /// Digest of the canonical request.
    pub request_hash: String,
    /// Digest of the accepted quote.
    pub accepted_hash: String,
    pub now_ms: u64,
    pub freshness_window_ms: u64,
    pub clock_skew_ms: u64,
    pub payment_subject: PaymentSubjectRef,
    /// The holder key that presented the proof-of-possession.
    pub presenter: SignerRef,
    /// Whether the merchant requires human presence for this interaction
    /// (AP2-style human-in-the-loop semantics).
    ///
    /// When `true`, the verification pipeline requires valid m-of-n approvals
    /// bound to the presented PoP, even when no approval gate fires. A
    /// warrant without a configured approver set therefore cannot satisfy a
    /// human-present challenge (fail-closed).
    pub human_present: bool,
}

/// Typed warrant constraints for v1 (stateless predicates).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub enum Constraint {
    Merchant(MerchantConstraint),
    Resource(ResourceConstraint),
    Tool(ToolConstraint),
    Payment(PaymentConstraint),
}

/// Merchant allowlist constraint (exact ids and/or host suffixes).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct MerchantConstraint {
    pub merchant_ids: Vec<String>,
    pub host_suffixes: Vec<String>,
}

impl MerchantConstraint {
    #[must_use]
    pub const fn new() -> Self {
        Self { merchant_ids: Vec::new(), host_suffixes: Vec::new() }
    }

    #[must_use]
    pub fn with_ids(ids: impl IntoIterator<Item = String>) -> Self {
        Self { merchant_ids: ids.into_iter().collect(), host_suffixes: Vec::new() }
    }

    #[must_use]
    pub fn with_host_suffixes(suffixes: impl IntoIterator<Item = String>) -> Self {
        Self { merchant_ids: Vec::new(), host_suffixes: suffixes.into_iter().collect() }
    }

    /// Returns `true` when this constraint is satisfied by the context.
    pub fn allows(&self, merchant_id: &str, merchant_host: &str) -> bool {
        let id_ok =
            self.merchant_ids.is_empty() || self.merchant_ids.iter().any(|id| id == merchant_id);
        let host_ok = self.host_suffixes.is_empty() ||
            self.host_suffixes.iter().any(|suffix| merchant_host.ends_with(suffix));
        id_ok && host_ok
    }
}

/// Resource (HTTP method / path prefix) constraint.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceConstraint {
    pub http_methods: Vec<String>,
    pub path_prefixes: Vec<String>,
}

impl ResourceConstraint {
    #[must_use]
    pub const fn new() -> Self {
        Self { http_methods: Vec::new(), path_prefixes: Vec::new() }
    }

    #[must_use]
    pub fn with_methods(methods: impl IntoIterator<Item = String>) -> Self {
        Self { http_methods: methods.into_iter().collect(), path_prefixes: Vec::new() }
    }

    #[must_use]
    pub fn with_path_prefixes(paths: impl IntoIterator<Item = String>) -> Self {
        Self { http_methods: Vec::new(), path_prefixes: paths.into_iter().collect() }
    }

    /// Returns `true` when this constraint is satisfied by the context.
    pub fn allows(&self, method: &str, path_and_query: &str) -> bool {
        let method_ok = self.http_methods.is_empty() ||
            self.http_methods.iter().any(|m| m.eq_ignore_ascii_case(method));
        let path_ok = self.path_prefixes.is_empty() ||
            self.path_prefixes.iter().any(|prefix| path_and_query.starts_with(prefix));
        method_ok && path_ok
    }
}

/// Optional AI-native tool constraint.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolConstraint {
    pub tool_names: Vec<String>,
    pub model_providers: Vec<String>,
    pub action_labels: Vec<String>,
}

impl ToolConstraint {
    #[must_use]
    pub const fn new() -> Self {
        Self { tool_names: Vec::new(), model_providers: Vec::new(), action_labels: Vec::new() }
    }

    /// Returns `true` when this constraint is satisfied by the context.
    pub fn allows(&self, tool_name: &str, model_provider: &str, action_label: &str) -> bool {
        let tool_ok = self.tool_names.is_empty() || self.tool_names.iter().any(|t| t == tool_name);
        let provider_ok = self.model_providers.is_empty() ||
            self.model_providers.iter().any(|p| p == model_provider);
        let action_ok =
            self.action_labels.is_empty() || self.action_labels.iter().any(|a| a == action_label);
        tool_ok && provider_ok && action_ok
    }
}

/// Payment constraint: allowed asset(s) and a **stateless** per-charge cap.
///
/// Amounts are expressed in the asset's base units (smallest on-chain unit).
/// Period limits and cumulative budgets are deliberately **not** part of v1:
/// they are stateful predicates handled by the accounting Facilitator (P2+).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

/// Validates that `child` is a valid static attenuation of `parent`.
///
/// This performs a **conservative, decidable** subset check: every value that
/// `parent` allows must also be allowed by `child`. It is used at issuance
/// time (in [`crate::typestate::DelegatedWarrantBuilder`]) to reject a child
/// that would expand capabilities *before* the warrant is signed.
///
/// The check is conservative in two ways:
///
/// - Empty allowlists mean "any", so a child that adds restrictions to a parent with an empty list
///   is valid (narrowing), but a child that empties a parent's non-empty list is rejected
///   (widening).
/// - Unknown/unbounded dimensions (e.g. arbitrary host names under a suffix) are judged by the same
///   allow-list semantics, never by pattern-language containment (which can be undecidable).
pub fn validate_attenuation(parent: &Constraint, child: &Constraint) -> Result<()> {
    match (parent, child) {
        (Constraint::Merchant(p), Constraint::Merchant(c)) => {
            // Child's allowed merchant ids must be a subset of parent's
            // (when the parent restricts them).
            if !p.merchant_ids.is_empty() {
                for id in &c.merchant_ids {
                    if !p.merchant_ids.contains(id) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: "merchant_ids".to_string(),
                            detail: format!("merchant `{id}` not allowed by parent"),
                        });
                    }
                }
            }
            // Child's host suffixes must be a subset of parent's.
            if !p.host_suffixes.is_empty() {
                for suffix in &c.host_suffixes {
                    if !p.host_suffixes.contains(suffix) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: "host_suffixes".to_string(),
                            detail: format!("host suffix `{suffix}` not allowed by parent"),
                        });
                    }
                }
            }
            Ok(())
        }
        (Constraint::Resource(p), Constraint::Resource(c)) => {
            if !p.http_methods.is_empty() {
                for method in &c.http_methods {
                    if !p.http_methods.iter().any(|m| m.eq_ignore_ascii_case(method)) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: "http_methods".to_string(),
                            detail: format!("method `{method}` not allowed by parent"),
                        });
                    }
                }
            }
            if !p.path_prefixes.is_empty() {
                for prefix in &c.path_prefixes {
                    if !p.path_prefixes.iter().any(|pp| prefix.starts_with(pp)) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: "path_prefixes".to_string(),
                            detail: format!("path prefix `{prefix}` not under a parent prefix"),
                        });
                    }
                }
            }
            Ok(())
        }
        (Constraint::Tool(p), Constraint::Tool(c)) => {
            for (dimension, parent_list, child_list) in [
                ("tool_names", &p.tool_names, &c.tool_names),
                ("model_providers", &p.model_providers, &c.model_providers),
                ("action_labels", &p.action_labels, &c.action_labels),
            ] {
                if parent_list.is_empty() {
                    continue;
                }
                for value in child_list {
                    if !parent_list.contains(value) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: dimension.to_string(),
                            detail: format!("`{value}` not allowed by parent"),
                        });
                    }
                }
            }
            Ok(())
        }
        (Constraint::Payment(p), Constraint::Payment(c)) => {
            // Amount cap can only shrink.
            if c.max_per_charge > p.max_per_charge {
                return Err(AuthorizationError::AttenuationViolation {
                    dimension: "max_per_charge".to_string(),
                    detail: format!(
                        "child cap {} exceeds parent cap {}",
                        c.max_per_charge, p.max_per_charge
                    ),
                });
            }
            // Child assets must be a subset of parent assets.
            if !p.allowed_assets.is_empty() {
                for asset in &c.allowed_assets {
                    if !p.allowed_assets.iter().any(|pa| pa == asset) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: "allowed_assets".to_string(),
                            detail: format!("asset `{}` not allowed by parent", asset.asset),
                        });
                    }
                }
            }
            // Rails are a distinct enum type; check them separately.
            if !p.allowed_rails.is_empty() {
                for rail in &c.allowed_rails {
                    if !p.allowed_rails.contains(rail) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: "allowed_rails".to_string(),
                            detail: format!("rail `{rail:?}` not allowed by parent"),
                        });
                    }
                }
            }
            for (dimension, parent_list, child_list) in [
                ("allowed_schemes", &p.allowed_schemes, &c.allowed_schemes),
                ("payee_ids", &p.payee_ids, &c.payee_ids),
            ] {
                if parent_list.is_empty() {
                    continue;
                }
                for value in child_list {
                    if !parent_list.contains(value) {
                        return Err(AuthorizationError::AttenuationViolation {
                            dimension: dimension.to_string(),
                            detail: format!("`{value}` not allowed by parent"),
                        });
                    }
                }
            }
            Ok(())
        }
        // Different constraint kinds are never comparable; treat as invalid.
        _ => Err(AuthorizationError::AttenuationViolation {
            dimension: "constraint_kind".to_string(),
            detail: "parent and child constraint kinds differ".to_string(),
        }),
    }
}

/// Unified `Verify` trait for constraint evaluation.
pub trait Verify {
    /// Checks this constraint against the given context.
    fn verify(&self, context: &AuthorizationContext) -> Result<()>;
}

impl Verify for Constraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        match self {
            Self::Merchant(c) => c.verify(context),
            Self::Resource(c) => c.verify(context),
            Self::Tool(c) => c.verify(context),
            Self::Payment(c) => c.verify(context),
        }
    }
}

impl Verify for MerchantConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        if !self.allows(&context.merchant_id, &context.merchant_host) {
            return Err(AuthorizationError::MerchantNotAllowed {
                merchant_id: context.merchant_id.clone(),
            });
        }
        Ok(())
    }
}

impl Verify for ResourceConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        if !self.allows(&context.http_method, &context.path_and_query) {
            return Err(AuthorizationError::ResourceNotAllowed {
                method: context.http_method.clone(),
                path: context.path_and_query.clone(),
            });
        }
        Ok(())
    }
}

impl Verify for ToolConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        if !self.allows(&context.tool_name, &context.model_provider, &context.action_label) {
            return Err(AuthorizationError::ToolNotAllowed { tool_name: context.tool_name.clone() });
        }
        Ok(())
    }
}

impl Verify for PaymentConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        if !self.allows(
            context.selected_amount,
            &context.asset,
            context.asset_network.as_deref(),
            &context.rail,
            &context.scheme,
            &context.payee_id,
        ) {
            if context.selected_amount > self.max_per_charge {
                return Err(AuthorizationError::PaymentAmountExceeded {
                    amount: context.selected_amount,
                    limit: self.max_per_charge,
                });
            }
            return Err(AuthorizationError::PaymentNotAllowed);
        }
        Ok(())
    }
}

/// Verifies every constraint in a slice against the context.
///
/// Short-circuits on the first failure.
pub fn verify_all(constraints: &[Constraint], context: &AuthorizationContext) -> Result<()> {
    for constraint in constraints {
        constraint.verify(context)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::{
        error::AuthorizationError,
        warrant::{PaymentRail, PaymentSubjectRef, SignerRef},
    };

    #[test]
    fn merchant_with_host_suffixes_rejects_other_hosts() {
        let constraint = MerchantConstraint::with_host_suffixes(vec!["acme.com".to_string()]);
        assert!(constraint.allows("merchant-a", "api.acme.com"));
        assert!(!constraint.allows("merchant-a", "evil.org"));
        // Host suffix matches, so the merchant id is not restricted.
        assert!(constraint.allows("merchant-b", "acme.com"));
    }

    #[test]
    fn merchant_empty_allows_all() {
        let constraint = MerchantConstraint::default();
        assert!(constraint.allows("anything", "any.host"));
    }

    #[test]
    fn merchant_with_ids_matches_exactly() {
        let constraint = MerchantConstraint::with_ids(vec!["merchant-a".to_string()]);
        assert!(constraint.allows("merchant-a", "any.host"));
        assert!(!constraint.allows("merchant-b", "any.host"));
    }

    #[test]
    fn resource_with_methods_and_paths() {
        let methods = ResourceConstraint::with_methods(vec!["POST".to_string()]);
        assert!(methods.allows("POST", "/pay"));
        assert!(!methods.allows("GET", "/pay"));

        let paths = ResourceConstraint::with_path_prefixes(vec!["/pay".to_string()]);
        assert!(paths.allows("POST", "/pay/orders"));
        assert!(!paths.allows("POST", "/admin"));
    }

    #[test]
    fn resource_method_is_case_insensitive() {
        let constraint = ResourceConstraint::with_methods(vec!["post".to_string()]);
        assert!(constraint.allows("POST", "/x"));
        assert!(constraint.allows("Post", "/x"));
        assert!(!constraint.allows("DELETE", "/x"));
    }

    #[test]
    fn resource_empty_allows_all() {
        let constraint = ResourceConstraint::default();
        assert!(constraint.allows("GET", "/anything"));
    }

    #[test]
    fn tool_constraint_requires_all_dimensions() {
        let constraint = ToolConstraint {
            tool_names: vec!["search".to_string()],
            model_providers: vec!["openai".to_string()],
            action_labels: vec!["web-search".to_string()],
        };
        assert!(constraint.allows("search", "openai", "web-search"));
        assert!(!constraint.allows("search", "anthropic", "web-search"));
        assert!(!constraint.allows("search", "openai", "other"));
        assert!(!constraint.allows("other", "openai", "web-search"));
    }

    #[test]
    fn tool_empty_allows_all() {
        let constraint = ToolConstraint::default();
        assert!(constraint.allows("anything", "any-provider", "any-action"));
    }

    #[test]
    fn payment_constraint_builders_collect() {
        let payment = PaymentConstraint::new(100)
            .with_asset(AssetRef::new("USDC", Some("base".to_string())))
            .with_rails(vec![crate::warrant::PaymentRail::Onchain])
            .with_schemes(vec!["exact".to_string()]);
        assert_eq!(payment.max_per_charge, 100);
        assert_eq!(payment.allowed_assets.len(), 1);
        assert_eq!(payment.allowed_rails.len(), 1);
        assert_eq!(payment.allowed_schemes, vec!["exact".to_string()]);
    }

    #[test]
    fn payment_allows_exact_cap() {
        let payment = PaymentConstraint::new(100);
        // amount == max is allowed (strict >).
        assert!(payment.allows(
            100,
            "USDC",
            None,
            &crate::warrant::PaymentRail::Onchain,
            "exact",
            "merchant-a"
        ));
        assert!(!payment.allows(
            101,
            "USDC",
            None,
            &crate::warrant::PaymentRail::Onchain,
            "exact",
            "merchant-a"
        ));
    }

    #[test]
    fn payment_allows_rejects_asset_rail_scheme_payee_mismatch() {
        let payment = PaymentConstraint::new(1_000)
            .with_asset(AssetRef::new("USDC", Some("base".to_string())))
            .with_rails(vec![PaymentRail::Onchain])
            .with_schemes(vec!["exact".to_string()])
            .with_payees(vec!["merchant-a".to_string()]);
        // Asset mismatch.
        assert!(!payment.allows(
            100,
            "DAI",
            None,
            &crate::warrant::PaymentRail::Onchain,
            "exact",
            "merchant-a"
        ));
        // Rail mismatch.
        assert!(!payment.allows(
            100,
            "USDC",
            Some("base"),
            &crate::warrant::PaymentRail::Exchange,
            "exact",
            "merchant-a"
        ));
        // Scheme mismatch.
        assert!(!payment.allows(
            100,
            "USDC",
            Some("base"),
            &crate::warrant::PaymentRail::Onchain,
            "upto",
            "merchant-a"
        ));
        // Payee mismatch.
        assert!(!payment.allows(
            100,
            "USDC",
            Some("base"),
            &crate::warrant::PaymentRail::Onchain,
            "exact",
            "merchant-b"
        ));
        // All match.
        assert!(payment.allows(
            100,
            "USDC",
            Some("base"),
            &crate::warrant::PaymentRail::Onchain,
            "exact",
            "merchant-a"
        ));
    }

    fn context_for(amount: u128, method: &str, path: &str, tool: &str) -> AuthorizationContext {
        AuthorizationContext {
            merchant_id: "merchant-a".to_string(),
            merchant_host: "merchant-a.example".to_string(),
            tool_name: tool.to_string(),
            model_provider: "openai".to_string(),
            action_label: "web-search".to_string(),
            http_method: method.to_string(),
            path_and_query: path.to_string(),
            selected_amount: amount,
            asset: "USDC".to_string(),
            asset_network: Some("base".to_string()),
            scheme: "exact".to_string(),
            payee_id: "merchant-a".to_string(),
            rail: PaymentRail::Onchain,
            challenge_id: "challenge-1".to_string(),
            request_hash: "sha256:req".to_string(),
            accepted_hash: "sha256:acc".to_string(),
            now_ms: 2_000,
            freshness_window_ms: 60_000,
            clock_skew_ms: 30_000,
            payment_subject: PaymentSubjectRef::new(
                crate::warrant::PaymentSubjectKind::Caip10,
                "caip10:eip155:8453:0xabc123",
            ),
            presenter: SignerRef::new(crate::warrant::SigningAlgorithm::Ed25519, vec![1; 32]),
            human_present: false,
        }
    }

    #[test]
    fn merchant_verify_rejects_wrong_id() {
        let constraint = MerchantConstraint::with_ids(vec!["merchant-a".to_string()]);
        let mut ctx = context_for(100, "POST", "/pay", "web-search");
        ctx.merchant_id = "merchant-b".to_string();
        let error = constraint.verify(&ctx).expect_err("merchant mismatch");
        assert!(matches!(error, AuthorizationError::MerchantNotAllowed { .. }));
    }

    #[test]
    fn resource_verify_rejects_wrong_method_and_path() {
        let constraint = ResourceConstraint::with_methods(vec!["POST".to_string()]);
        let mut ctx = context_for(100, "POST", "/pay", "web-search");
        ctx.http_method = "DELETE".to_string();
        let error = constraint.verify(&ctx).expect_err("method mismatch");
        assert!(matches!(error, AuthorizationError::ResourceNotAllowed { .. }));

        let paths = ResourceConstraint::with_path_prefixes(vec!["/pay".to_string()]);
        let ctx = context_for(100, "POST", "/admin", "web-search");
        let error = paths.verify(&ctx).expect_err("path mismatch");
        assert!(matches!(error, AuthorizationError::ResourceNotAllowed { .. }));
    }

    #[test]
    fn tool_verify_rejects_wrong_tool() {
        let constraint = ToolConstraint {
            tool_names: vec!["search".to_string()],
            model_providers: vec!["openai".to_string()],
            action_labels: vec!["web-search".to_string()],
        };
        let ctx = context_for(100, "POST", "/pay", "other-tool");
        let error = constraint.verify(&ctx).expect_err("tool mismatch");
        assert!(matches!(error, AuthorizationError::ToolNotAllowed { .. }));

        let mut ctx = context_for(100, "POST", "/pay", "search");
        ctx.model_provider = "anthropic".to_string();
        let error = constraint.verify(&ctx).expect_err("provider mismatch");
        assert!(matches!(error, AuthorizationError::ToolNotAllowed { .. }));
    }

    #[test]
    fn payment_verify_rejects_over_limit() {
        let constraint = PaymentConstraint::new(100);
        let ctx = context_for(101, "POST", "/pay", "web-search");
        let error = constraint.verify(&ctx).expect_err("over limit");
        assert!(matches!(error, AuthorizationError::PaymentAmountExceeded { .. }));
        // Exactly at cap passes.
        let ok = context_for(100, "POST", "/pay", "web-search");
        assert!(constraint.verify(&ok).is_ok());
    }

    #[test]
    fn constraint_enum_dispatches_to_variants() {
        let merchant =
            Constraint::Merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]));
        let ctx = context_for(100, "POST", "/pay", "web-search");
        assert!(merchant.verify(&ctx).is_ok());

        let resource =
            Constraint::Resource(ResourceConstraint::with_methods(vec!["GET".to_string()]));
        let error = resource.verify(&ctx).expect_err("method mismatch");
        assert!(matches!(error, AuthorizationError::ResourceNotAllowed { .. }));
    }

    #[test]
    fn verify_all_runs_every_constraint() {
        let constraints = vec![
            Constraint::Merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()])),
            Constraint::Resource(ResourceConstraint::with_methods(vec!["POST".to_string()])),
            Constraint::Payment(PaymentConstraint::new(1_000)),
        ];
        let ctx = context_for(100, "POST", "/pay", "web-search");
        assert!(crate::constraint::verify_all(&constraints, &ctx).is_ok());

        let over_limit = vec![
            Constraint::Merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()])),
            Constraint::Payment(PaymentConstraint::new(50)),
        ];
        let error = crate::constraint::verify_all(&over_limit, &ctx).expect_err("over limit");
        assert!(matches!(error, AuthorizationError::PaymentAmountExceeded { .. }));
    }

    // ---------------------------------------------------------------------
    // validate_attenuation: per-dimension guard coverage
    // ---------------------------------------------------------------------

    use crate::warrant::AssetRef;

    #[test]
    fn attenuation_rejects_widening_in_each_dimension() {
        // Merchant ids.
        let parent =
            Constraint::Merchant(MerchantConstraint::with_ids(vec!["merchant-a".to_string()]));
        let child =
            Constraint::Merchant(MerchantConstraint::with_ids(vec!["merchant-b".to_string()]));
        assert!(validate_attenuation(&parent, &child).is_err());

        // Host suffixes (parent restricted, child escapes the set).
        let parent = Constraint::Merchant(MerchantConstraint {
            merchant_ids: Vec::new(),
            host_suffixes: vec![".acme.com".to_string()],
        });
        let child = Constraint::Merchant(MerchantConstraint {
            merchant_ids: Vec::new(),
            host_suffixes: vec![".evil.io".to_string()],
        });
        assert!(validate_attenuation(&parent, &child).is_err());

        // HTTP methods.
        let parent = Constraint::Resource(ResourceConstraint {
            http_methods: vec!["GET".to_string()],
            path_prefixes: Vec::new(),
        });
        let child = Constraint::Resource(ResourceConstraint {
            http_methods: vec!["POST".to_string()],
            path_prefixes: Vec::new(),
        });
        assert!(validate_attenuation(&parent, &child).is_err());

        // Path prefixes.
        let parent = Constraint::Resource(ResourceConstraint {
            http_methods: Vec::new(),
            path_prefixes: vec!["/api".to_string()],
        });
        let child = Constraint::Resource(ResourceConstraint {
            http_methods: Vec::new(),
            path_prefixes: vec!["/admin".to_string()],
        });
        assert!(validate_attenuation(&parent, &child).is_err());

        // Tool names.
        let parent = Constraint::Tool(ToolConstraint {
            tool_names: vec!["search".to_string()],
            model_providers: Vec::new(),
            action_labels: Vec::new(),
        });
        let child = Constraint::Tool(ToolConstraint {
            tool_names: vec!["delete".to_string()],
            model_providers: Vec::new(),
            action_labels: Vec::new(),
        });
        assert!(validate_attenuation(&parent, &child).is_err());

        // Payment assets.
        let parent = Constraint::Payment(PaymentConstraint {
            allowed_assets: vec![AssetRef::new("USDC", None)],
            ..PaymentConstraint::new(1_000)
        });
        let child = Constraint::Payment(PaymentConstraint {
            allowed_assets: vec![AssetRef::new("ETH", None)],
            ..PaymentConstraint::new(1_000)
        });
        assert!(validate_attenuation(&parent, &child).is_err());

        // Payment rails.
        let parent = Constraint::Payment(PaymentConstraint {
            allowed_rails: vec![PaymentRail::Onchain],
            ..PaymentConstraint::new(1_000)
        });
        let child = Constraint::Payment(PaymentConstraint {
            allowed_rails: vec![PaymentRail::Exchange],
            ..PaymentConstraint::new(1_000)
        });
        assert!(validate_attenuation(&parent, &child).is_err());
    }

    #[test]
    fn attenuation_accepts_valid_subsets() {
        // Matching host suffix subset (guards the inner contains-check).
        let parent = Constraint::Merchant(MerchantConstraint {
            merchant_ids: Vec::new(),
            host_suffixes: vec![".acme.com".to_string(), ".acme.dev".to_string()],
        });
        let child = Constraint::Merchant(MerchantConstraint {
            merchant_ids: Vec::new(),
            host_suffixes: vec![".acme.com".to_string()],
        });
        assert!(validate_attenuation(&parent, &child).is_ok());

        // Matching tool subset (guards the inner contains-check).
        let parent = Constraint::Tool(ToolConstraint {
            tool_names: vec!["search".to_string(), "read".to_string()],
            model_providers: vec!["openai".to_string()],
            action_labels: Vec::new(),
        });
        let child = Constraint::Tool(ToolConstraint {
            tool_names: vec!["search".to_string()],
            model_providers: vec!["openai".to_string()],
            action_labels: Vec::new(),
        });
        assert!(validate_attenuation(&parent, &child).is_ok());
    }

    #[test]
    fn payment_error_classification_distinguishes_amount_from_other_causes() {
        // At exactly the cap with a DIFFERENT violating dimension (payee not
        // in the allowlist), the error must be PaymentNotAllowed, not
        // AmountExceeded.
        let constraint = PaymentConstraint {
            payee_ids: vec!["merchant-b".to_string()],
            ..PaymentConstraint::new(100)
        };
        let ctx = context_for(100, "POST", "/pay", "web-search");
        let error = constraint.verify(&ctx).expect_err("payee mismatch");
        assert_eq!(error, AuthorizationError::PaymentNotAllowed);

        // Over the cap with everything else fine → AmountExceeded.
        let open = PaymentConstraint::new(100);
        let ctx_over = context_for(101, "POST", "/pay", "web-search");
        let error = open.verify(&ctx_over).expect_err("over cap");
        assert_eq!(error, AuthorizationError::PaymentAmountExceeded { amount: 101, limit: 100 });
    }
}
