//! `/v1/basemap/{z}/{x}/{y}.mvt` — the multi-layer N50 topo basemap tile.
//!
//! Unlike the per-resource `/v1/{resource}/tiles/...` endpoint (one MVT layer
//! of curated paths), this stitches every basemap feature class
//! (water/landcover/contour/building/transportation/place…) into one vector
//! tile, as defined by `tools/basemap-layers.toml`. The renderer styles it
//! client-side, so the same tiles back any number of map styles.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::Json;
use serde_json::{json, Value};
use turbo_tiles_core::tile::TileCoord;

use crate::error::ApiError;
use crate::state::ApiState;

pub async fn tile(
    State(state): State<ApiState>,
    Path((z, x, y_ext)): Path<(u8, u32, String)>,
) -> Result<Response, ApiError> {
    // Axum captures `:y.mvt` as one segment; strip the extension ourselves.
    let y_str = y_ext
        .strip_suffix(".mvt")
        .ok_or_else(|| ApiError::BadRequest("tile path must end in .mvt".into()))?;
    let y: u32 = y_str
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid tile y `{y_str}`")))?;
    let coord = TileCoord::new(z, x, y).map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let bytes = turbo_tiles_mvt::render_basemap_tile(&state.db, &state.basemap, coord)
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
    // Long-lived + revalidate: basemap data changes on rebuild cadence, and
    // the edge cache keys on the URL. (Versioned URLs land with the R2 tier.)
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400, stale-while-revalidate=604800"),
    );
    Ok(resp)
}

/// `/v1/basemap` — a TileJSON-ish descriptor: the tile URL template, zoom
/// bounds, and the layers (with their zoom range + advertised attributes) so
/// a client/style author can introspect the basemap without reading the TOML.
pub async fn describe(State(state): State<ApiState>) -> Json<Value> {
    let layers: Vec<Value> = state
        .basemap
        .layer
        .iter()
        .map(|l| {
            json!({
                "id": l.name,
                "geometry": match l.kind {
                    turbo_tiles_mvt::GeomKind::Polygon => "polygon",
                    turbo_tiles_mvt::GeomKind::Line => "line",
                    turbo_tiles_mvt::GeomKind::Point => "point",
                },
                "minzoom": l.min_zoom,
                "maxzoom": l.max_zoom,
                "fields": l.attrs.iter().map(|a| a.name.clone()).collect::<Vec<_>>(),
            })
        })
        .collect();

    let min = state.basemap.layer.iter().map(|l| l.min_zoom).min().unwrap_or(0);
    let max = state.basemap.layer.iter().map(|l| l.max_zoom).max().unwrap_or(22);

    Json(json!({
        "tilejson": "3.0.0",
        "name": "N50 Topo",
        "scheme": "xyz",
        "attribution": "© Kartverket",
        "minzoom": min,
        "maxzoom": max,
        "tiles": [format!("{}/v1/basemap/{{z}}/{{x}}/{{y}}.mvt", state.public_base_url)],
        "vector_layers": layers,
    }))
}
