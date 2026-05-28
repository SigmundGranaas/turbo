//! Dev-mode middleware that auto-redirects unauthenticated SPA
//! browser hits to `/admin/dev-login`, which mints a curator JWT
//! and redirects back. Result: opening `/admin/app/plot` in a
//! fresh browser session works without a manual stop at the login
//! shortcut. Only active when `TURBO_DEV_AUTH=1` — the bin's SPA
//! mount wraps it conditionally so production behaviour is
//! unchanged.

use axum::extract::Request;
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

pub async fn dev_auto_login(req: Request, next: Next) -> Response {
    // Only intercept GET requests for HTML — asset requests (.js,
    // .css, fonts) pass straight through. The redirect only makes
    // sense for the document load that initiates the SPA session.
    let is_get = req.method() == axum::http::Method::GET;
    let path = req.uri().path().to_string();
    let looks_like_doc = !path.contains('.')
        || path.ends_with(".html")
        || path.ends_with("/");
    let has_cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').any(|p| p.trim().starts_with("access_token=")))
        .unwrap_or(false);

    if is_get && looks_like_doc && !has_cookie {
        // Build a redirect to /admin/dev-login that preserves the
        // SPA route the user originally wanted — dev_login itself
        // currently lands them on /admin/app/ unconditionally, but
        // we still send the intended path as a `from=…` query
        // string so future versions can honor it.
        // Simple percent-encoding for the `from=` parameter — we only
        // need to handle `/` and a few benign chars from URL paths,
        // and pulling in a crate just for this isn't worth it.
        let mut from_enc = String::with_capacity(path.len() + 8);
        for c in path.chars() {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | '/') {
                from_enc.push(c);
            } else {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    from_enc.push_str(&format!("%{:02X}", b));
                }
            }
        }
        let location = format!("/admin/dev-login?from={from_enc}");
        return (
            StatusCode::FOUND,
            [(
                header::LOCATION,
                HeaderValue::from_str(&location).unwrap_or_else(|_| {
                    HeaderValue::from_static("/admin/dev-login")
                }),
            )],
            "redirecting to dev-login",
        )
            .into_response();
    }
    next.run(req).await
}
