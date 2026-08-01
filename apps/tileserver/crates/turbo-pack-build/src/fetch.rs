//! The one thing this crate cannot do for itself.
//!
//! Everything else here — decoding GeoTIFF, parsing GML, rasterising,
//! noding, writing artifacts — is pure computation over bytes. Only the
//! three source modules (`wcs`, `wfs`, `n50`) need a network, and they
//! need very little of one: GET a URL, POST a JSON body, read the
//! status and the bytes back.
//!
//! # Why this is a trait and not a `reqwest::Client`
//!
//! Because of what it costs on a phone. Linking the HTTP stack into the
//! Android library measured **+3.2 MB per ABI** — `reqwest` plus
//! `rustls` plus `tokio`, a second TLS implementation and a second async
//! runtime inside an app that already ships OkHttp. The parsers this
//! crate exists for are a rounding error next to that.
//!
//! Inverting it means the host fetches. On Android that reuses OkHttp's
//! connection pool, retry policy, certificate pinning and proxy
//! handling — all of which the app already configures and none of which
//! a second HTTP client would have inherited.
//!
//! # Why it is blocking
//!
//! The async-ness was never load-bearing; it existed because `reqwest`
//! is async. A blocking trait is what a uniffi callback can implement
//! without either side owning a runtime, and the concurrency that
//! actually mattered — several DEM tiles in flight — is a handful of
//! threads (see `build_dem`). Dropping the runtime is most of the
//! saving.

use crate::BuildError;

/// A response, with the status kept.
///
/// Not flattened into "bytes or error": the callers build their own
/// diagnostics from the status *and* the first bytes of the body — a
/// WCS 400 carries an XML explanation worth surfacing — and an
/// implementation that collapsed non-2xx into an error string would
/// throw that away.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The first bytes of the body as text, for an error message.
    pub fn head(&self, n: usize) -> String {
        String::from_utf8_lossy(&self.body[..self.body.len().min(n)]).to_string()
    }
}

/// How this crate reaches Kartverket.
///
/// Implementations must be usable from several threads at once —
/// `build_dem` runs a few requests in parallel — and are expected to
/// apply their own timeout. The services here are slow: a cold WCS
/// coverage can take minutes, so a default 30 s client will fail builds
/// that would otherwise have worked.
pub trait Fetch: Send + Sync {
    fn get(&self, url: &str) -> Result<Response, BuildError>;

    /// POST a JSON body. Used only by the N50 order API.
    fn post_json(&self, url: &str, body: &str) -> Result<Response, BuildError>;
}

impl<T: Fetch + ?Sized> Fetch for &T {
    fn get(&self, url: &str) -> Result<Response, BuildError> {
        (**self).get(url)
    }
    fn post_json(&self, url: &str, body: &str) -> Result<Response, BuildError> {
        (**self).post_json(url, body)
    }
}

impl<T: Fetch + ?Sized> Fetch for std::sync::Arc<T> {
    fn get(&self, url: &str) -> Result<Response, BuildError> {
        (**self).get(url)
    }
    fn post_json(&self, url: &str, body: &str) -> Result<Response, BuildError> {
        (**self).post_json(url, body)
    }
}

/// A `reqwest`-backed [`Fetch`], for callers that want one.
///
/// Behind the `net` feature so the Android build can leave the entire
/// HTTP stack out. The server and the CLI keep it — they are already
/// linked against `reqwest` for other reasons, so it costs them
/// nothing.
#[cfg(feature = "net")]
pub mod net {
    use super::{Fetch, Response};
    use crate::BuildError;

    /// Blocking `reqwest` client with the timeout these services need.
    pub struct HttpFetch {
        client: reqwest::blocking::Client,
    }

    impl HttpFetch {
        pub fn new() -> Result<Self, BuildError> {
            let client = reqwest::blocking::Client::builder()
                // Five minutes, not the default thirty seconds. A cold
                // WCS coverage genuinely takes minutes, and the failure
                // when it does not is a build that dies most of the way
                // through a region.
                .timeout(std::time::Duration::from_secs(300))
                .user_agent(concat!("turbo-pack-build/", env!("CARGO_PKG_VERSION")))
                // The native trust store matters behind a
                // TLS-intercepting proxy, which is how CI reaches the
                // internet here.
                .tls_built_in_native_certs(true)
                .build()
                .map_err(|e| BuildError::Fetch(format!("build http client: {e}")))?;
            Ok(Self { client })
        }
    }

    impl Fetch for HttpFetch {
        fn get(&self, url: &str) -> Result<Response, BuildError> {
            let r = self
                .client
                .get(url)
                .send()
                .map_err(|e| BuildError::Fetch(format!("GET {}: {e}", elide(url))))?;
            into_response(r, url)
        }

        fn post_json(&self, url: &str, body: &str) -> Result<Response, BuildError> {
            let r = self
                .client
                .post(url)
                .header("content-type", "application/json")
                .body(body.to_string())
                .send()
                .map_err(|e| BuildError::Fetch(format!("POST {}: {e}", elide(url))))?;
            into_response(r, url)
        }
    }

    fn into_response(r: reqwest::blocking::Response, url: &str) -> Result<Response, BuildError> {
        let status = r.status().as_u16();
        let body = r
            .bytes()
            .map_err(|e| BuildError::Fetch(format!("body from {}: {e}", elide(url))))?
            .to_vec();
        Ok(Response { status, body })
    }

    /// Query strings here carry bboxes hundreds of characters long;
    /// pasting one into an error message buries the message.
    fn elide(url: &str) -> &str {
        url.split('?').next().unwrap_or(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_is_2xx_only() {
        let ok = Response {
            status: 200,
            body: vec![],
        };
        assert!(ok.is_success());
        for s in [199u16, 300, 400, 500] {
            assert!(
                !Response {
                    status: s,
                    body: vec![]
                }
                .is_success(),
                "{s} must not be success"
            );
        }
    }

    /// A short body must not panic the slice in [`Response::head`] —
    /// error paths run on exactly the responses that are malformed.
    #[test]
    fn head_is_clamped_to_the_body() {
        let r = Response {
            status: 400,
            body: b"nope".to_vec(),
        };
        assert_eq!(r.head(200), "nope");
        assert_eq!(r.head(2), "no");
        assert_eq!(
            Response {
                status: 400,
                body: vec![]
            }
            .head(200),
            ""
        );
    }
}
