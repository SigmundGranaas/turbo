use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use jsonwebtoken::decode;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::claims::Claims;
use crate::config::AuthConfig;

/// State injected into the router so extractors can reach the config.
#[derive(Clone)]
pub struct AuthState(pub Arc<AuthConfig>);

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing or malformed bearer token")]
    MissingToken,
    #[error("token rejected: {0}")]
    Invalid(String),
    #[error("required role missing")]
    MissingRole,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            AuthError::MissingToken | AuthError::Invalid(_) => StatusCode::UNAUTHORIZED,
            AuthError::MissingRole => StatusCode::FORBIDDEN,
        };
        let body = serde_json::json!({"error": self.to_string()});
        (status, axum::Json(body)).into_response()
    }
}

/// Authenticated principal. Inject via Axum extractor on any handler
/// that should require a valid token without role gating.
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    AuthState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthState(cfg) = AuthState::from_ref(state);
        let token = extract_token(parts).ok_or(AuthError::MissingToken)?;
        let data = decode::<Claims>(&token, &cfg.decoding_key, &cfg.validation)
            .map_err(|e| AuthError::Invalid(e.to_string()))?;
        Ok(AuthUser(data.claims))
    }
}

/// Marker trait for required-role extractors. Implement once per role:
/// `pub struct Curator; impl RoleSpec for Curator { const NAME: &'static str = "curator"; }`.
/// Then `RequireRole<Curator>` in a handler enforces it.
pub trait RoleSpec: Send + Sync + 'static {
    const NAME: &'static str;
}

pub struct Curator;
impl RoleSpec for Curator {
    const NAME: &'static str = "curator";
}

pub struct Admin;
impl RoleSpec for Admin {
    const NAME: &'static str = "admin";
}

pub struct RequireRole<R: RoleSpec> {
    pub claims: Claims,
    _role: PhantomData<R>,
}

#[async_trait::async_trait]
impl<S, R: RoleSpec> FromRequestParts<S> for RequireRole<R>
where
    AuthState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if !claims.has_role(R::NAME) {
            return Err(AuthError::MissingRole);
        }
        Ok(RequireRole {
            claims,
            _role: PhantomData,
        })
    }
}

/// Pull the JWT from `Authorization: Bearer ...` or, failing that, from
/// the `access_token` cookie set by the .NET auth service. The latter
/// path is what the HTMX admin panel uses (no JS token plumbing).
fn extract_token(parts: &Parts) -> Option<String> {
    if let Some(h) = parts.headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(value) = h.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    if let Some(cookie_h) = parts.headers.get(axum::http::header::COOKIE) {
        if let Ok(cookies) = cookie_h.to_str() {
            for part in cookies.split(';') {
                let part = part.trim();
                if let Some(value) = part.strip_prefix("access_token=") {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}
