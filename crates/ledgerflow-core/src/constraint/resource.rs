//! Resource (HTTP method / path prefix) constraint.

use serde::{Deserialize, Serialize};

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
