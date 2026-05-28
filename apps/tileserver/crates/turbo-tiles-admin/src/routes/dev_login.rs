//! `GET /admin/dev-login` — mint a curator JWT and set the
//! `access_token` cookie, then redirect to the SPA. **Only
//! registered when `TURBO_DEV_AUTH=1`** so it can never reach
//! production by accident.
//!
//! Purpose: let a developer on a laptop open the admin SPA without
//! standing up the .NET OAuth service. The same JWT the .NET auth
//! service would issue, minted locally with the tileserver's own
//! `JWT_SECRET`.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;

#[derive(Serialize)]
struct Claims {
    sub: &'static str,
    email: &'static str,
    exp: u64,
    /// The .NET claim URI for role. Matches what `mint-curator-state.ts`
    /// emits and what `turbo-tiles-auth` looks for.
    #[serde(rename = "http://schemas.microsoft.com/ws/2008/06/identity/claims/role")]
    role: [&'static str; 2],
}

/// Always succeeds when reached — registration is gated by the
/// env var so the route just doesn't exist in production. One hour
/// validity is enough for any practical dev session; reload to
/// extend.
pub async fn dev_login() -> Response {
    let secret = match std::env::var("JWT_SECRET") {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "JWT_SECRET env var not set",
            )
                .into_response()
        }
    };
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() + 3600)
        .unwrap_or(0);
    let claims = Claims {
        // Valid UUID v4 nil — the auth extractor parses `sub` as a
        // UUID, so the "dev" suffix variant rejects with HTTP 401.
        sub: "00000000-0000-0000-0000-000000000000",
        email: "dev@local",
        exp,
        role: ["curator", "admin"],
    };
    let token = match encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("encode failed: {e}"),
            )
                .into_response()
        }
    };
    // Path=/ so the cookie travels with every request, not just
    // /admin/*. Max-Age 3600 matches the `exp` claim. SameSite=Lax
    // so navigations from typed URLs include it but cross-site
    // POSTs don't. HttpOnly omitted because the SPA never reads
    // the cookie itself — it only relies on `credentials: include`.
    let cookie = format!(
        "access_token={token}; Path=/; Max-Age=3600; SameSite=Lax"
    );
    let mut resp = (
        StatusCode::FOUND,
        [
            (
                header::SET_COOKIE,
                HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
            ),
            (header::LOCATION, HeaderValue::from_static("/admin/app/")),
        ],
        "redirecting to /admin/app/",
    )
        .into_response();
    // Belt-and-braces: set the cookie via the typed header too so
    // proxies that normalise headers don't strip it.
    resp.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    resp
}

/// True when `TURBO_DEV_AUTH=1` — the bin uses this to decide
/// whether to register the route at all.
pub fn enabled() -> bool {
    std::env::var("TURBO_DEV_AUTH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
