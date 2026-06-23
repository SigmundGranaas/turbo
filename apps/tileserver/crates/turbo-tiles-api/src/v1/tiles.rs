use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use turbo_tiles_core::resource::Resource;
use turbo_tiles_core::tile::TileCoord;

use crate::error::ApiError;
use crate::state::ApiState;

pub async fn tile(
    State(state): State<ApiState>,
    Path((resource, z, x, y_ext)): Path<(String, u8, u32, String)>,
) -> Result<Response, ApiError> {
    let resource: Resource = resource
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("unknown resource `{resource}`")))?;
    // Axum's path matcher captures the `:y.mvt` segment as a single
    // string; strip the extension and parse as a number ourselves.
    let y_str = y_ext
        .strip_suffix(".mvt")
        .ok_or_else(|| ApiError::BadRequest("tile path must end in .mvt".into()))?;
    let y: u32 = y_str
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid tile y `{y_str}`")))?;
    let coord = TileCoord::new(z, x, y).map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Cache-or-render (see basemap::tile): warm tiles serve from memory; cold
    // renders are bounded by a permit and de-duped by the post-permit re-check.
    let key = format!("{}/{z}/{x}/{y}", resource.slug());
    let bytes: std::sync::Arc<[u8]> = match state.mvt_tiles.get(&key) {
        Some(b) => b,
        None => {
            let _permit = state.mvt_tiles.acquire_render().await;
            match state.mvt_tiles.get(&key) {
                Some(b) => b,
                None => {
                    let rendered = turbo_tiles_mvt::render_tile(&state.db, resource, coord)
                        .await
                        .map_err(|e| ApiError::Db(e.to_string()))?;
                    let arc: std::sync::Arc<[u8]> =
                        std::sync::Arc::from(rendered.into_boxed_slice());
                    state.mvt_tiles.put(&key, arc.clone());
                    arc
                }
            }
        }
    };

    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(bytes.to_vec()))
        .unwrap();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.mapbox-vector-tile"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400, stale-while-revalidate=604800"),
    );
    Ok(resp)
}
