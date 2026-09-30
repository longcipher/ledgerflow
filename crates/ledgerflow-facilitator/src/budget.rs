//! Budget accounting for the Accounting Point mode (P2+).
//!
//! This module implements periodic and cumulative budget enforcement
//! for warrants. The accounting Facilitator tracks spending against
//! configured limits and rejects payments that would exceed them.
//!
//! Design:
//! - `BudgetLimit`: defines periodic and cumulative spending caps
//! - `BudgetTracker`: tracks spending against limits
//! - `BudgetAccounting`: the accounting point that enforces budgets

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Budget period for periodic limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BudgetPeriod {
    /// Daily budget.
    Daily,
    /// Weekly budget.
    Weekly,
    /// Monthly budget.
    Monthly,
}

/// Budget limit configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BudgetLimit {
    /// Maximum amount per period (base units).
    pub periodic_limit: Option<u128>,
    /// Period for the periodic limit.
    pub period: Option<BudgetPeriod>,
    /// Maximum cumulative amount over the warrant's lifetime.
    pub cumulative_limit: Option<u128>,
}

impl BudgetLimit {
    /// Creates a budget limit with no restrictions.
    #[must_use]
    pub const fn unrestricted() -> Self {
        Self { periodic_limit: None, period: None, cumulative_limit: None }
    }

    /// Creates a daily budget limit.
    #[must_use]
    pub const fn daily(limit: u128) -> Self {
        Self {
            periodic_limit: Some(limit),
            period: Some(BudgetPeriod::Daily),
            cumulative_limit: None,
        }
    }

    /// Creates a cumulative budget limit.
    #[must_use]
    pub const fn cumulative(limit: u128) -> Self {
        Self { periodic_limit: None, period: None, cumulative_limit: Some(limit) }
    }
}

/// Budget tracking state for a single warrant.
#[derive(Clone, Debug, Default)]
pub struct BudgetState {
    /// Total amount spent (cumulative).
    total_spent: u128,
    /// Amount spent in the current period.
    period_spent: u128,
    /// Current period identifier (e.g., day number).
    current_period: u64,
}

/// Budget enforcement errors.
#[derive(Debug, Error)]
pub enum BudgetError {
    #[error("periodic budget exceeded: spent {spent}, limit {limit}")]
    PeriodicExceeded { spent: u128, limit: u128 },
    #[error("cumulative budget exceeded: spent {spent}, limit {limit}")]
    CumulativeExceeded { spent: u128, limit: u128 },
}

/// Tracks budget spending for warrants.
#[derive(Clone, Debug)]
pub struct BudgetTracker {
    /// Budget states keyed by warrant digest.
    states: BTreeMap<String, BudgetState>,
    /// Default budget limit (used when warrant has no specific limit).
    default_limit: BudgetLimit,
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl BudgetTracker {
    /// Creates a new budget tracker with no restrictions.
    #[must_use]
    pub const fn new() -> Self {
        Self { states: BTreeMap::new(), default_limit: BudgetLimit::unrestricted() }
    }

    /// Creates a new budget tracker with a default limit.
    #[must_use]
    pub const fn with_default_limit(default_limit: BudgetLimit) -> Self {
        Self { states: BTreeMap::new(), default_limit }
    }

    /// Records a payment and checks against budget limits.
    pub fn record_payment(
        &mut self,
        warrant_digest: &str,
        amount: u128,
        limit: &BudgetLimit,
        now_secs: u64,
    ) -> Result<(), BudgetError> {
        // Use the provided limit, or fall back to the default limit
        let effective_limit = if limit.periodic_limit.is_none() && limit.cumulative_limit.is_none()
        {
            &self.default_limit
        } else {
            limit
        };

        let period = current_period(now_secs, effective_limit.period);

        let state = self.states.entry(warrant_digest.to_string()).or_default();

        // Reset period spending if we've moved to a new period
        if state.current_period != period {
            state.current_period = period;
            state.period_spent = 0;
        }

        // Check periodic limit
        if let Some(periodic_limit) = effective_limit.periodic_limit {
            let new_period_spent = state.period_spent + amount;
            if new_period_spent > periodic_limit {
                return Err(BudgetError::PeriodicExceeded {
                    spent: new_period_spent,
                    limit: periodic_limit,
                });
            }
        }

        // Check cumulative limit
        if let Some(cumulative_limit) = effective_limit.cumulative_limit {
            let new_total = state.total_spent + amount;
            if new_total > cumulative_limit {
                return Err(BudgetError::CumulativeExceeded {
                    spent: new_total,
                    limit: cumulative_limit,
                });
            }
        }

        // Record the payment
        state.total_spent += amount;
        state.period_spent += amount;

        Ok(())
    }

    /// Returns the total amount spent for a warrant.
    #[must_use]
    pub fn total_spent(&self, warrant_digest: &str) -> u128 {
        self.states.get(warrant_digest).map_or(0, |s| s.total_spent)
    }

    /// Returns the amount spent in the current period for a warrant.
    #[must_use]
    pub fn period_spent(
        &self,
        warrant_digest: &str,
        now_secs: u64,
        period: Option<BudgetPeriod>,
    ) -> u128 {
        let current_period = current_period(now_secs, period);
        self.states
            .get(warrant_digest)
            .filter(|s| s.current_period == current_period)
            .map_or(0, |s| s.period_spent)
    }

    /// Returns the remaining budget for a warrant.
    ///
    /// Considers both periodic and cumulative limits, returning the minimum
    /// of the two remaining amounts.
    #[must_use]
    pub fn remaining_budget(&self, warrant_digest: &str, limit: &BudgetLimit) -> u128 {
        let total_spent = self.total_spent(warrant_digest);
        let cumulative_remaining =
            limit.cumulative_limit.map_or(u128::MAX, |l| l.saturating_sub(total_spent));

        let periodic_remaining = limit.periodic_limit.map_or(u128::MAX, |l| {
            let period_spent = self.period_spent(warrant_digest, 0, limit.period);
            l.saturating_sub(period_spent)
        });

        cumulative_remaining.min(periodic_remaining)
    }
}

/// Computes the current period identifier.
const fn current_period(now_secs: u64, period: Option<BudgetPeriod>) -> u64 {
    match period {
        None => 0,
        Some(BudgetPeriod::Daily) => now_secs / 86_400,
        Some(BudgetPeriod::Weekly) => now_secs / (7 * 86_400),
        Some(BudgetPeriod::Monthly) => now_secs / (30 * 86_400),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    const DAY_SECS: u64 = 86_400;

    #[test]
    fn unrestricted_budget_allows_any_payment() {
        let mut tracker = BudgetTracker::new();
        let limit = BudgetLimit::unrestricted();

        tracker.record_payment("warrant-1", 1_000_000, &limit, 1_000_000).expect("payment 1");
        tracker.record_payment("warrant-1", 2_000_000, &limit, 1_000_001).expect("payment 2");

        assert_eq!(tracker.total_spent("warrant-1"), 3_000_000);
    }

    #[test]
    fn daily_budget_enforced() {
        let mut tracker = BudgetTracker::new();
        let limit = BudgetLimit::daily(1_000_000);

        // First payment within limit
        tracker.record_payment("warrant-1", 600_000, &limit, 1_000_000).expect("payment 1");

        // Second payment exceeds daily limit
        let result = tracker.record_payment("warrant-1", 500_000, &limit, 1_000_001);
        assert!(matches!(result, Err(BudgetError::PeriodicExceeded { .. })));

        // Next day, budget resets
        tracker
            .record_payment("warrant-1", 500_000, &limit, 1_000_000 + DAY_SECS)
            .expect("payment next day");
    }

    #[test]
    fn cumulative_budget_enforced() {
        let mut tracker = BudgetTracker::new();
        let limit = BudgetLimit::cumulative(1_000_000);

        tracker.record_payment("warrant-1", 600_000, &limit, 1_000_000).expect("payment 1");

        let result = tracker.record_payment("warrant-1", 500_000, &limit, 1_000_001);
        assert!(matches!(result, Err(BudgetError::CumulativeExceeded { .. })));
    }

    #[test]
    fn remaining_budget_tracks_correctly() {
        let mut tracker = BudgetTracker::new();
        let limit = BudgetLimit::cumulative(1_000_000);

        assert_eq!(tracker.remaining_budget("warrant-1", &limit), 1_000_000);

        tracker.record_payment("warrant-1", 300_000, &limit, 1_000_000).expect("payment");

        assert_eq!(tracker.remaining_budget("warrant-1", &limit), 700_000);
    }

    #[test]
    fn different_warrants_tracked_independently() {
        let mut tracker = BudgetTracker::new();
        let limit = BudgetLimit::daily(1_000_000);

        tracker.record_payment("warrant-1", 600_000, &limit, 1_000_000).expect("payment 1");
        tracker.record_payment("warrant-2", 600_000, &limit, 1_000_000).expect("payment 2");

        assert_eq!(tracker.total_spent("warrant-1"), 600_000);
        assert_eq!(tracker.total_spent("warrant-2"), 600_000);
    }
}
