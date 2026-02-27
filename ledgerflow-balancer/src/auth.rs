use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{AppState, error::AppError, utils::hash_api_token};

#[derive(Debug, Clone)]
pub enum AuthPrincipal {
    Account { id: i64 },
    Service,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub principal: AuthPrincipal,
    pub is_admin: bool,
}

impl AuthContext {
    pub fn account_id(&self) -> Option<i64> {
        match self.principal {
            AuthPrincipal::Account { id, .. } => Some(id),
            AuthPrincipal::Service => None,
        }
    }
}

impl FromRequestParts<AppState> for AuthContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        app_state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parse_bearer_token(parts)?;
        let token_hash = hash_api_token(token);

        if let Some(account) = app_state
            .db
            .get_account_by_api_token_hash(&token_hash)
            .await?
        {
            return Ok(Self {
                principal: AuthPrincipal::Account { id: account.id },
                is_admin: account.is_admin,
            });
        }

        if let Some(service_token) = app_state
            .config
            .auth
            .service_tokens
            .iter()
            .find(|service_token| hash_api_token(&service_token.token) == token_hash)
        {
            return Ok(Self {
                principal: AuthPrincipal::Service,
                is_admin: service_token.is_admin,
            });
        }

        Err(AppError::Unauthorized("invalid bearer token".to_string()))
    }
}

fn parse_bearer_token(parts: &Parts) -> Result<&str, AppError> {
    let auth_header = parts
        .headers
        .get(AUTHORIZATION)
        .ok_or_else(|| AppError::Unauthorized("missing Authorization header".to_string()))?;

    let auth_header_value = auth_header
        .to_str()
        .map_err(|_| AppError::Unauthorized("invalid Authorization header encoding".to_string()))?;

    let token = auth_header_value
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Authorization scheme must be Bearer".to_string()))?;

    if token.trim().is_empty() {
        return Err(AppError::Unauthorized("empty bearer token".to_string()));
    }

    Ok(token.trim())
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, Request};

    use super::parse_bearer_token;

    fn parts_with_header(header: Option<&str>) -> axum::http::request::Parts {
        let mut request = Request::builder()
            .uri("/orders")
            .body(())
            .expect("request build");
        if let Some(value) = header {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::AUTHORIZATION,
                HeaderValue::from_str(value).expect("header value"),
            );
            *request.headers_mut() = headers;
        }
        let (parts, _) = request.into_parts();
        parts
    }

    #[test]
    fn parse_bearer_token_accepts_valid_header() {
        let parts = parts_with_header(Some("Bearer token-123"));
        let token = parse_bearer_token(&parts).expect("valid bearer token");
        assert_eq!(token, "token-123");
    }

    #[test]
    fn parse_bearer_token_rejects_missing_header() {
        let parts = parts_with_header(None);
        let error = parse_bearer_token(&parts).expect_err("missing header should fail");
        assert!(error.to_string().contains("missing Authorization"));
    }

    #[test]
    fn parse_bearer_token_rejects_wrong_scheme() {
        let parts = parts_with_header(Some("Basic xyz"));
        let error = parse_bearer_token(&parts).expect_err("wrong scheme should fail");
        assert!(
            error
                .to_string()
                .contains("Authorization scheme must be Bearer")
        );
    }
}
