//! SaaS integration: gateway-injected internal headers + service token.
//!
//! The server never parses user JWTs; it trusts only the headers the gateway
//! injects (after verifying the shared service token with constant-time
//! comparison). In `standalone` mode a fixed tenant context is injected.

use axum::response::IntoResponse as _;

use crate::config::SaasMode;

/// SaaS request context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaaSContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub roles: Vec<String>,
    pub principal: Option<String>,
}

impl SaaSContext {
    /// Creates the fixed standalone context.
    #[must_use]
    pub fn standalone(tenant_id: impl Into<String>) -> Self {
        Self { tenant_id: tenant_id.into(), user_id: None, roles: Vec::new(), principal: None }
    }
}

/// Internal header names injected by the gateway (design §10.1).
pub const HEADER_TENANT_ID: &str = "x-internal-tenant-id";
pub const HEADER_USER_ID: &str = "x-internal-user-id";
pub const HEADER_ROLES: &str = "x-internal-roles";
pub const HEADER_PRINCIPAL: &str = "x-internal-principal";
pub const HEADER_AUTHORIZATION: &str = "authorization";
pub const BEARER_PREFIX: &str = "Bearer ";

/// SaaS authentication failures.
#[derive(Debug, thiserror::Error)]
pub enum SaasAuthError {
    #[error("missing or malformed service token")]
    InvalidToken,
    #[error("missing tenant id header")]
    MissingTenant,
}

/// An extractor configuration; call [`Self::extract`] from middleware or
/// handlers.
#[derive(Clone, Debug)]
pub struct SaasAuthExtractor {
    pub mode: SaasMode,
    pub service_token: Option<String>,
    pub standalone_tenant: String,
}

impl SaasAuthExtractor {
    /// Extracts the SaaS context from HTTP headers.
    ///
    /// In `saas` mode the service token is verified (constant-time) and the
    /// internal headers are read. In `standalone` mode the fixed tenant
    /// context is returned.
    pub fn extract_from_headers(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Result<SaaSContext, SaasAuthError> {
        match self.mode {
            SaasMode::Standalone => Ok(SaaSContext::standalone(self.standalone_tenant.clone())),
            SaasMode::Saas => {
                let expected = self.service_token.as_deref().ok_or(SaasAuthError::InvalidToken)?;
                let provided = headers
                    .get(HEADER_AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix(BEARER_PREFIX))
                    .ok_or(SaasAuthError::InvalidToken)?;
                if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
                    return Err(SaasAuthError::InvalidToken);
                }
                let tenant_id = headers
                    .get(HEADER_TENANT_ID)
                    .and_then(|v| v.to_str().ok())
                    .filter(|v| !v.is_empty())
                    .ok_or(SaasAuthError::MissingTenant)?;
                let user_id =
                    headers.get(HEADER_USER_ID).and_then(|v| v.to_str().ok()).map(str::to_string);
                let roles = headers
                    .get(HEADER_ROLES)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.split(',').map(str::to_string).collect())
                    .unwrap_or_default();
                let principal =
                    headers.get(HEADER_PRINCIPAL).and_then(|v| v.to_str().ok()).map(str::to_string);
                Ok(SaaSContext { tenant_id: tenant_id.to_string(), user_id, roles, principal })
            }
        }
    }
}

/// Constant-time string equality.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Middleware function for axum that extracts the SaaS context and stores it
/// in request extensions.
///
/// Use with `axum::middleware::from_fn_with_state` in the binary:
/// ```ignore
/// .layer(axum::middleware::from_fn_with_state(
///     extractor.clone(),
///     saas_auth_middleware,
/// ))
/// ```
pub async fn saas_auth_middleware(
    extractor: axum::extract::State<SaasAuthExtractor>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    match extractor.extract_from_headers(request.headers()) {
        Ok(ctx) => {
            let mut request = request;
            request.extensions_mut().insert(ctx);
            next.run(request).await
        }
        Err(error) => {
            let status = axum::http::StatusCode::UNAUTHORIZED;
            (status, error.to_string()).into_response()
        }
    }
}

/// Returns the SaaS auth middleware with the given extractor as state.
///
/// Use with `.layer(axum::middleware::from_fn_with_state(extractor, saas_auth_middleware))`
/// in the binary.
#[must_use]
pub const fn saas_auth_extractor_state(
    extractor: SaasAuthExtractor,
) -> axum::extract::State<SaasAuthExtractor> {
    axum::extract::State(extractor)
}

/// Axum extractor for the per-request [`SaaSContext`].
///
/// The [`saas_auth_middleware`] inserts the context into request extensions on
/// every request, so handlers can depend on it directly. This closes the gap
/// where the context was computed but never consumed (design §10.2 tenant
/// isolation).
impl axum::extract::FromRequestParts<crate::state::AppState> for SaaSContext {
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &crate::state::AppState,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let context =
            parts.extensions.get::<Self>().cloned().unwrap_or_else(|| Self::standalone("default"));
        std::future::ready(Ok(context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_handles_length_mismatch() {
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"ab", b"abc"));
        assert!(!constant_time_eq(&[], b"a"));
    }

    #[test]
    fn constant_time_eq_detects_any_byte_difference() {
        // Equal-length strings differing in ANY byte position must be unequal.
        assert!(!constant_time_eq(b"secret1", b"secret2"));
        assert!(!constant_time_eq(b"secret1", b"Secret1"));
        assert!(!constant_time_eq(b"aaaa", b"aaab"));
        assert!(!constant_time_eq(b"aaaa", b"baaa"));
        assert!(!constant_time_eq(b"aaaa", b"aaba"));
    }

    #[test]
    fn constant_time_eq_accepts_identical() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"token-value", b"token-value"));
    }
}
