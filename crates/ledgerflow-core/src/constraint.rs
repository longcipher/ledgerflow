//! Constraint verification abstraction.
//!
//! The [`Verify`] trait provides a uniform interface for checking individual
//! warrant constraints against an [`AuthorizationContext`](crate::warrant::AuthorizationContext).
//!
//! # Design Pattern: Trait-Based Polymorphism
//!
//! Each constraint type implements `Verify` independently, so adding a new
//! constraint variant only requires adding an `impl Verify for NewConstraint`
//! — no changes to the verification loop. This replaces the monolithic
//! `match` + free-function pattern in `warrant.rs` with an open/closed design.

use crate::{
    error::Result,
    warrant::{
        AuthorizationContext, Constraint, MerchantConstraint, PaymentConstraint,
        ResourceConstraint, SponsorshipConstraint, ToolConstraint,
    },
};

/// Verifies a single constraint against an authorization context.
///
/// Implementations return `Ok(())` when the constraint is satisfied, or an
/// appropriate [`AuthorizationError`](crate::error::AuthorizationError)
/// variant when it is not.
pub trait Verify {
    /// Checks this constraint against the given context.
    fn verify(&self, context: &AuthorizationContext) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Constraint-level blanket dispatch
// ---------------------------------------------------------------------------

impl Verify for Constraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        match self {
            Self::Merchant(c) => c.verify(context),
            Self::Resource(c) => c.verify(context),
            Self::Tool(c) => c.verify(context),
            Self::Payment(c) => c.verify(context),
            Self::Sponsorship(c) => c.verify(context),
        }
    }
}

// ---------------------------------------------------------------------------
// Individual constraint implementations
// ---------------------------------------------------------------------------

impl Verify for MerchantConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        if !self.merchant_ids.is_empty() &&
            !self.merchant_ids.iter().any(|id| id == &context.merchant_id)
        {
            return Err(crate::error::AuthorizationError::MerchantNotAllowed {
                merchant_id: context.merchant_id.clone(),
            });
        }
        if !self.host_suffixes.is_empty() &&
            !self.host_suffixes.iter().any(|suffix| context.merchant_host.ends_with(suffix))
        {
            return Err(crate::error::AuthorizationError::MerchantNotAllowed {
                merchant_id: context.merchant_id.clone(),
            });
        }
        Ok(())
    }
}

impl Verify for ResourceConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        let method = context.http_method.to_uppercase();
        if !self.http_methods.is_empty() &&
            !self.http_methods.iter().any(|m| m.eq_ignore_ascii_case(&method))
        {
            return Err(crate::error::AuthorizationError::HttpMethodNotAllowed { method });
        }
        if !self.path_prefixes.is_empty() &&
            !self.path_prefixes.iter().any(|p| context.path_and_query.starts_with(p))
        {
            return Err(crate::error::AuthorizationError::ResourcePathNotAllowed {
                path: context.path_and_query.clone(),
            });
        }
        Ok(())
    }
}

impl Verify for ToolConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        if !self.tool_names.is_empty() &&
            !self.tool_names.iter().any(|name| name == &context.tool_name)
        {
            return Err(crate::error::AuthorizationError::ToolNotAllowed {
                tool_name: context.tool_name.clone(),
            });
        }
        if !self.model_providers.is_empty() &&
            !self.model_providers.iter().any(|p| p == &context.model_provider)
        {
            return Err(crate::error::AuthorizationError::ModelProviderNotAllowed {
                model_provider: context.model_provider.clone(),
            });
        }
        if !self.action_labels.is_empty() &&
            !self.action_labels.iter().any(|label| label == &context.action_label)
        {
            return Err(crate::error::AuthorizationError::ActionLabelNotAllowed {
                action_label: context.action_label.clone(),
            });
        }
        Ok(())
    }
}

impl Verify for PaymentConstraint {
    fn verify(&self, context: &AuthorizationContext) -> Result<()> {
        if context.selected_quote_amount > self.max_per_request.amount {
            return Err(crate::error::AuthorizationError::PaymentAmountExceeded {
                amount: context.selected_quote_amount,
                limit: self.max_per_request.amount,
            });
        }
        if !self.allowed_assets.is_empty() &&
            !self.allowed_assets.iter().any(|a| a.asset == context.asset)
        {
            return Err(crate::error::AuthorizationError::AssetNotAllowed {
                asset: context.asset.clone(),
            });
        }
        if !self.allowed_schemes.is_empty() &&
            !self.allowed_schemes.iter().any(|s| s == &context.scheme)
        {
            return Err(crate::error::AuthorizationError::SchemeNotAllowed {
                scheme: context.scheme.clone(),
            });
        }
        if !self.allowed_rails.is_empty() && !self.allowed_rails.iter().any(|r| r == &context.rail)
        {
            return Err(crate::error::AuthorizationError::RailNotAllowed { rail: context.rail });
        }
        if !self.payee_ids.is_empty() && !self.payee_ids.iter().any(|p| p == &context.payee_id) {
            return Err(crate::error::AuthorizationError::PayeeNotAllowed {
                payee_id: context.payee_id.clone(),
            });
        }
        Ok(())
    }
}

impl Verify for SponsorshipConstraint {
    fn verify(&self, _context: &AuthorizationContext) -> Result<()> {
        if !self.allow_sponsored_execution && !self.sponsor_ids.is_empty() {
            return Err(crate::error::AuthorizationError::SponsorshipNotAllowed);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Convenience: verify a slice of constraints
// ---------------------------------------------------------------------------

/// Verifies every constraint in a slice against the context.
///
/// Short-circuits on the first failure.
pub fn verify_all(constraints: &[Constraint], context: &AuthorizationContext) -> Result<()> {
    for constraint in constraints {
        constraint.verify(context)?;
    }
    Ok(())
}
