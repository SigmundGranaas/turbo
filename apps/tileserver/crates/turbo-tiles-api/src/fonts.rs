//! `/fonts/{fontstack}/{range}.pbf` — SDF glyph PBFs for MapLibre/Mapbox GL.
//!
//! The basemap style's `glyphs` URL points here. We serve a single embedded
//! face (DejaVu Sans, which covers the Norwegian alphabet) for any requested
//! fontstack — generated on demand by `turbo-tiles-raster` and cached by the
//! edge worker (its allowlist already includes `/fonts/...`).

use axum::body::Body;
use axum::extract::Path;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;

use crate::error::ApiError;

/// `GET /fonts/{fontstack}/{range}.pbf`. `range` is `"{start}-{end}.pbf"`
/// where start is a multiple of 256 (MapLibre requests one 256-codepoint
/// block at a time). The fontstack is ignored — we only ship one face.
pub async fn glyphs(
    Path((_fontstack, range)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let range = range
        .strip_suffix(".pbf")
        .ok_or_else(|| ApiError::BadRequest("font range must end in .pbf".into()))?;
    let start: u32 = range
        .split('-')
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ApiError::BadRequest(format!("invalid glyph range `{range}`")))?;
    if !start.is_multiple_of(256) {
        return Err(ApiError::BadRequest(
            "glyph range start must be a multiple of 256".into(),
        ));
    }

    let pbf = turbo_tiles_raster::render_range(start).map_err(|e| ApiError::Db(e.to_string()))?;

    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(pbf))
        .unwrap();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-protobuf"),
    );
    // Glyphs are immutable per font; cache hard.
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    Ok(resp)
}
