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
    Path((resource, z, x, y)): Path<(String, u8, u32, u32)>,
) -> Result<Response, ApiError> {
    let resource: Resource = resource
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("unknown resource `{resource}`")))?;
    let coord = TileCoord::new(z, x, y).map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let bytes = turbo_tiles_mvt::render_tile(&state.db, resource, coord)
        .await
        .map_err(|e| ApiError::Db(e.to_string()))?;

    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(bytes))
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
