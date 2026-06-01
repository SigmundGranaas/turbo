//! `/v1/debug/data/*` — viewport-bbox data inspection.
//!
//! The SPA's Plot screen renders these as toggleable overlays so a
//! curator can see *every* primitive that influences pathfinding:
//! water/glacier/wetland/forest cells, the trail/road/ski-track
//! graph segments by surface type, anchors by kind, and DEM tile
//! coverage. Each endpoint takes a bbox (the SPA passes the visible
//! map viewport) and returns simple GeoJSON-style payloads.

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::wgs84_to_utm33n;
use turbo_tiles_pathfind::utm33n_to_wgs84;

use crate::error::ApiError;
use crate::state::ApiState;

/// Common bbox arguments: WGS84 west/south/east/north. We convert to
/// UTM33N internally so the artifact lookups are linear-cost.
#[derive(Debug, Deserialize)]
pub struct BboxQuery {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Optional class filter (e.g. `kind=summit` for anchors,
    /// `fkb_type=sti` for edges, etc.). The endpoint ignores it
    /// when it doesn't apply.
    #[serde(default)]
    pub filter: Option<String>,
}
fn default_limit() -> usize {
    5000
}

#[derive(Debug, Serialize)]
pub struct MaskCellsResp {
    /// Each entry: [lon, lat, value]. Value = 1 (water/present),
    /// 2 (glacier/secondary), 3 (reserved). Caller renders as
    /// coloured squares.
    pub cells: Vec<[f64; 3]>,
    pub resolution_m: f32,
    pub returned: u32,
    pub bbox_clipped: bool,
}

/// Convert the bbox to UTM33N corners. Use the corners of the WGS84
/// rectangle as input — accept the small distortion (~tens of metres
/// across a single viewport) since the artifact lookups are happy
/// with any consistent bbox.
fn bbox_to_utm(q: &BboxQuery) -> (f64, f64, f64, f64) {
    let sw = wgs84_to_utm33n(q.west, q.south);
    let ne = wgs84_to_utm33n(q.east, q.north);
    let nw = wgs84_to_utm33n(q.west, q.north);
    let se = wgs84_to_utm33n(q.east, q.south);
    // UTM is not axis-aligned with lat/lon; take the conservative
    // axis-aligned envelope of the four corners.
    let min_x = sw.x.min(nw.x).min(se.x).min(ne.x);
    let max_x = sw.x.max(nw.x).max(se.x).max(ne.x);
    let min_y = sw.y.min(nw.y).min(se.y).min(ne.y);
    let max_y = sw.y.max(nw.y).max(se.y).max(ne.y);
    (min_x, min_y, max_x, max_y)
}

fn mask_cells(
    state: &ApiState,
    artifact_field: fn(&ApiState) -> Option<&std::sync::Arc<turbo_tiles_mask::Mask>>,
    err_name: &'static str,
    q: &BboxQuery,
) -> Result<Json<MaskCellsResp>, ApiError> {
    let mask = artifact_field(state).ok_or(ApiError::PrimitiveUnavailable(err_name))?;
    let (min_x, min_y, max_x, max_y) = bbox_to_utm(q);
    let raw = mask.cells_in_bbox(min_x, min_y, max_x, max_y, q.limit);
    let was_capped = raw.len() == q.limit;
    let cells: Vec<[f64; 3]> = raw
        .into_iter()
        .map(|(x, y, v)| {
            let (lon, lat) = utm33n_to_wgs84(x, y);
            [lon, lat, v as f64]
        })
        .collect();
    let returned = cells.len() as u32;
    Ok(Json(MaskCellsResp {
        cells,
        resolution_m: mask.meta().resolution_m,
        returned,
        bbox_clipped: was_capped,
    }))
}

pub async fn mask_water(
    State(state): State<ApiState>,
    Query(q): Query<BboxQuery>,
) -> Result<Json<MaskCellsResp>, ApiError> {
    mask_cells(&state, |s| s.mask.as_ref(), "mask", &q)
}

pub async fn landcover_wetland(
    State(state): State<ApiState>,
    Query(q): Query<BboxQuery>,
) -> Result<Json<MaskCellsResp>, ApiError> {
    mask_cells(&state, |s| s.landcover.get("wetland"), "wetland", &q)
}

pub async fn landcover_forest(
    State(state): State<ApiState>,
    Query(q): Query<BboxQuery>,
) -> Result<Json<MaskCellsResp>, ApiError> {
    mask_cells(&state, |s| s.landcover.get("forest"), "forest", &q)
}

#[derive(Debug, Serialize)]
pub struct EdgesResp {
    /// Each edge: full polyline as a list of [lon, lat] pairs + the
    /// fkb_type code as the last entry's `kind`. Older clients that
    /// expected `[from_lon, from_lat, to_lon, to_lat, kind]` can use
    /// the first/last of `coords` — but the SPA overlay reads the
    /// whole polyline so winding trails render correctly.
    pub edges: Vec<EdgePolyline>,
    pub returned: u32,
    pub capped: bool,
    /// True when the underlying graph has its sibling `graph_geom`
    /// artifact attached. When false, every edge in `edges` is a
    /// 2-point straight segment between endpoint nodes — the
    /// overlay is then effectively the old node-secant behaviour
    /// and the SPA should warn.
    pub has_polylines: bool,
}

#[derive(Debug, Serialize)]
pub struct EdgePolyline {
    pub coords: Vec<[f64; 2]>,
    pub kind: u8,
}

pub async fn edges(
    State(state): State<ApiState>,
    Query(q): Query<BboxQuery>,
) -> Result<Json<EdgesResp>, ApiError> {
    let g = state
        .graph
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("graph"))?;
    let (min_x, min_y, max_x, max_y) = bbox_to_utm(&q);
    // Filter by fkb_type name from `?filter=sti|vei|skiloype|all`.
    // Default = all surfaces.
    let codes: Option<Vec<u8>> = match q.filter.as_deref() {
        Some("sti") => Some(vec![1]),
        Some("vei") => Some(vec![2]),
        Some("skiloype") => Some(vec![3]),
        Some("all") | None | Some("") => None,
        Some(_) => Some(vec![]),
    };
    let polys = g.edge_polylines_in_bbox(min_x, min_y, max_x, max_y, codes.as_deref(), q.limit);
    let was_capped = polys.len() == q.limit;
    let edges: Vec<EdgePolyline> = polys
        .into_iter()
        .map(|(poly, kind)| EdgePolyline {
            coords: poly
                .into_iter()
                .map(|p| {
                    let (lon, lat) = utm33n_to_wgs84(p.x as f64, p.y as f64);
                    [lon, lat]
                })
                .collect(),
            kind,
        })
        .collect();
    let returned = edges.len() as u32;
    Ok(Json(EdgesResp {
        edges,
        returned,
        capped: was_capped,
        has_polylines: g.has_geom(),
    }))
}

#[derive(Debug, Serialize)]
pub struct AnchorsResp {
    pub anchors: Vec<AnchorPoint>,
    pub returned: u32,
    pub capped: bool,
}

#[derive(Debug, Serialize)]
pub struct AnchorPoint {
    pub id: u64,
    pub lon: f64,
    pub lat: f64,
    pub kind: turbo_tiles_search::AnchorKind,
    pub name: Option<String>,
    pub elev_m: f32,
}

pub async fn anchors(
    State(state): State<ApiState>,
    Query(q): Query<BboxQuery>,
) -> Result<Json<AnchorsResp>, ApiError> {
    let idx = state
        .search
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("search"))?;
    let (min_x, min_y, max_x, max_y) = bbox_to_utm(&q);
    let kind_filter = match q.filter.as_deref() {
        Some("all") | None | Some("") => None,
        Some(s) => match turbo_tiles_search::AnchorKind::from_text(s) {
            turbo_tiles_search::AnchorKind::Unknown => None,
            k => Some(k),
        },
    };
    let hits = idx.anchors_in_bbox(min_x, min_y, max_x, max_y, kind_filter, q.limit);
    let was_capped = hits.len() == q.limit;
    let anchors: Vec<AnchorPoint> = hits
        .into_iter()
        .map(|h| {
            let (lon, lat) = utm33n_to_wgs84(h.x as f64, h.y as f64);
            AnchorPoint {
                id: h.id,
                lon,
                lat,
                kind: h.kind,
                name: h.name,
                elev_m: h.elev_m,
            }
        })
        .collect();
    let returned = anchors.len() as u32;
    Ok(Json(AnchorsResp {
        anchors,
        returned,
        capped: was_capped,
    }))
}
