//! `GET /v1/packs/:key/:file` — region packs for on-device routing.
//!
//! ```text
//! /v1/packs/z12_1092_378_1095_381/pack.toml
//! /v1/packs/z12_1092_378_1095_381/norway.dem
//! ```
//!
//! # No version in the path
//!
//! An earlier sketch had one, on the theory that a data rebuild must
//! orphan old packs. It must — and it already does: the edge Worker
//! prefixes **every** R2 key with its own `DATA_VERSION`, so bumping
//! that orphans pack objects exactly as it orphans tiles, with no help
//! from the path. Putting the version in the URL as well would make the
//! client responsible for knowing the server's data version before it
//! could ask for anything, which is a round trip to learn a string.
//!
//! # The manifest is the build trigger
//!
//! `pack.toml` is a few hundred bytes and every client fetches it first
//! — it carries the file list and sizes a download needs. So it is the
//! natural place to pay for the build: a slow response on a tiny file
//! reads as "preparing your area", where a slow response on a 30 MB DEM
//! reads as a stalled download. By the time the client asks for the
//! artifacts, they are on disk and stream at line rate.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;
use crate::packs::PackError;
use crate::state::ApiState;

/// Packs are immutable for a given key: same key, same bytes, forever.
/// A rebuild changes the edge's `DATA_VERSION`, not this response.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub async fn file(
    State(state): State<ApiState>,
    Path((key, file)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let packs = state
        .packs
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("packs"))?;

    let parsed = packs.parse_key(&key).map_err(to_api)?;

    match packs.file(&parsed, &file).await {
        Ok(path) => serve(path, &file).await,
        Err(PackError::Building) => Ok((
            StatusCode::ACCEPTED,
            // Seconds, not a promise. The client re-requests the same
            // URL; there is no second endpoint and no job id to track.
            [(header::RETRY_AFTER, "10")],
            "building\n",
        )
            .into_response()),
        Err(e) => Err(to_api(e)),
    }
}

async fn serve(path: std::path::PathBuf, name: &str) -> Result<Response, ApiError> {
    let f = tokio::fs::File::open(&path)
        .await
        .map_err(|e| ApiError::Internal(format!("pack file: {e}")))?;
    let len = f
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(format!("pack file: {e}")))?
        .len();

    // Streamed, never buffered. The DEM is tens of megabytes and this
    // process is also solving routes; reading one into a Vec to hand it
    // to axum would spend that memory for nothing.
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(f));

    let content_type = if name.ends_with(".toml") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    };

    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, IMMUTABLE),
            (header::CONTENT_LENGTH, &len.to_string()[..]),
        ],
        body,
    )
        .into_response())
}

fn to_api(e: PackError) -> ApiError {
    match e {
        // A malformed key or an unknown file is the client's mistake and
        // retrying will not help — 400, not 404, so it is distinguishable
        // from "this region has no data".
        PackError::BadKey(m) => ApiError::BadRequest(m),
        PackError::TooLarge { .. } => ApiError::BadRequest(e.to_string()),
        PackError::Build(m) => ApiError::Internal(format!("pack build: {m}")),
        PackError::Building => ApiError::Internal("unreachable: handled by caller".into()),
    }
}
