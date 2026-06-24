//! `/v1/raster/n50/{z}/{x}/{y}.png` — the raster fallback basemap.
//!
//! The same PostGIS layers and the same `n50-topo` style as the vector
//! `/v1/basemap` pipeline, rasterised server-side (`turbo-tiles-raster`).
//! This is the drop-in XYZ source `flutter_map` consumes today, replacing
//! the Kartverket Norgeskart WMTS dependency without a client renderer
//! change. The R2 worker's allowlist covers this path, so each tile is
//! rendered once per data version and served from cache thereafter.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use turbo_tiles_core::tile::TileCoord;

use crate::error::ApiError;
use crate::state::ApiState;

const TILE_PX: u32 = 256;
/// The basemap is N50-scale data; past z16 there is no more detail to
/// rasterise (clients overzoom).
const MAX_Z: u8 = 16;

pub async fn tile(
    State(state): State<ApiState>,
    Path((z, x, y_ext)): Path<(u8, u32, String)>,
) -> Result<Response, ApiError> {
    let y_str = y_ext
        .strip_suffix(".png")
        .ok_or_else(|| ApiError::BadRequest("raster tile path must end in .png".into()))?;
    let y: u32 = y_str
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid tile y `{y_str}`")))?;
    if z > MAX_Z {
        return Err(ApiError::BadRequest(format!(
            "raster zoom {z} exceeds max {MAX_Z}; overzoom client-side"
        )));
    }
    let coord = TileCoord::new(z, x, y).map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Serve from the rendered-raster cache (RAM → SSD) when warm; otherwise
    // render once under a permit and cache through both tiers. `get_or_render`
    // collapses a thundering herd for one cold tile to a single DB render, and
    // the render runs with a longer statement-timeout budget (see
    // `turbo_tiles_raster`) so a low-zoom tile can complete + cache instead of
    // re-timing-out on every request.
    let key = format!("n50/{z}/{x}/{y}");
    let png = state
        .raster_tiles
        .get_or_render(key, || async {
            turbo_tiles_raster::render_tile(
                &state.db,
                &state.basemap,
                &state.raster_style,
                coord,
                TILE_PX,
                // Composite hillshade when the DEM artifact is loaded; flat otherwise.
                state.dem.as_deref(),
            )
            .await
            .map_err(|e| ApiError::Db(e.to_string()))
        })
        .await?;

    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(png.to_vec()))
        .unwrap();
    let headers = resp.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400, stale-while-revalidate=604800"),
    );
    Ok(resp)
}
