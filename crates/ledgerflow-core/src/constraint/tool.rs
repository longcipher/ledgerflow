//! Optional AI-native tool constraint.

use serde::{Deserialize, Serialize};

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
