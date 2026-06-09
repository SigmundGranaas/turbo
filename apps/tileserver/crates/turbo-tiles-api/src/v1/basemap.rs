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

/// The house `n50-topo` MapLibre style, embedded at compile time. Disk copy
/// (when running from the repo) wins so style edits are live without a
/// rebuild.
const EMBEDDED_STYLE: &str = include_str!("../../../../styles/n50-topo.json");

/// `/v1/basemap/style.json` — a MapLibre Style Spec document wired to this
/// server's tile URL. MapLibre GL (web/Flutter) consumes it directly;
/// `turbomap-style-maplibre` lowers the same document onto turbomap's
/// `VectorStyle` for the native renderer. `{BASE_URL}` placeholders are
/// resolved against `PUBLIC_BASE_URL` at serve time so one style document
/// works across dev/staging/prod.
pub async fn style(State(state): State<ApiState>) -> Response {
    let text = std::fs::read_to_string("styles/n50-topo.json")
        .unwrap_or_else(|_| EMBEDDED_STYLE.to_string());
    let body = text.replace("{BASE_URL}", &state.public_base_url);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=3600, stale-while-revalidate=86400"),
        )
        .body(Body::from(body))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::EMBEDDED_STYLE;

    /// Style ↔ tiles consistency guard: every `source-layer` the style
    /// references must exist in the basemap layer config, and every filter
    /// property must be an attribute that layer actually ships. Catches the
    /// silent-blank-layer class of bug (style names drifting from the TOML).
    #[test]
    fn style_source_layers_and_fields_match_basemap_config() {
        let style: serde_json::Value = serde_json::from_str(EMBEDDED_STYLE).expect("style parses");
        let cfg = turbo_tiles_mvt::BasemapConfig::load_or_default();
        assert!(!cfg.layer.is_empty(), "basemap config must define layers");

        for layer in style["layers"].as_array().expect("layers array") {
            let Some(source_layer) = layer["source-layer"].as_str() else {
                continue; // background has no source
            };
            let Some(def) = cfg.layer.iter().find(|l| l.name == source_layer) else {
                panic!(
                    "style layer `{}` references unknown source-layer `{source_layer}`",
                    layer["id"]
                );
            };
            // Filter properties must be advertised attrs of that layer.
            if let Some(filter) = layer.get("filter").and_then(|f| f.as_array()) {
                if let Some(prop) = filter.get(1).and_then(|p| p.as_str()) {
                    assert!(
                        def.attrs.iter().any(|a| a.name == prop),
                        "style layer `{}` filters on `{prop}` which `{source_layer}` does not ship",
                        layer["id"]
                    );
                }
            }
        }
    }

    /// The tile URL template must carry the {BASE_URL} placeholder (resolved
    /// at serve time) and the z/x/y slots the clients interpolate.
    #[test]
    fn style_tiles_template_has_placeholders() {
        let style: serde_json::Value = serde_json::from_str(EMBEDDED_STYLE).unwrap();
        let tiles = style["sources"]["n50"]["tiles"][0].as_str().unwrap();
        for needle in ["{BASE_URL}", "{z}", "{x}", "{y}"] {
            assert!(tiles.contains(needle), "tiles template missing {needle}: {tiles}");
        }
    }
}
