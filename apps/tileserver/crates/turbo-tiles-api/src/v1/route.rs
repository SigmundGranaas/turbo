//! `/v1/route` + `/v1/debug/graph/*` — routing primitive.

use std::time::Instant;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::wgs84_to_utm33n;
use turbo_tiles_graph::{GraphStats, Profile, RouteResult};

use crate::error::ApiError;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct RouteReq {
    pub from: [f64; 2],
    pub to: [f64; 2],
    #[serde(default = "default_profile")]
    pub profile: Profile,
    #[serde(default = "default_snap_radius_m")]
    pub snap_radius_m: f32,
}
fn default_profile() -> Profile {
    Profile::Foot
}
fn default_snap_radius_m() -> f32 {
    1_000.0
}

#[derive(Debug, Serialize)]
pub struct RouteResp {
    pub from_node: u32,
    pub to_node: u32,
    pub length_m: f32,
    pub cost: f32,
    pub geometry: Vec<[f64; 2]>, // EPSG:25833 polyline
    pub edges: Vec<u32>,
    pub took_us: u64,
}

pub async fn route(
    State(state): State<ApiState>,
    Json(req): Json<RouteReq>,
) -> Result<Json<RouteResp>, ApiError> {
    let g = state.graph.as_ref().ok_or(ApiError::PrimitiveUnavailable("graph"))?;
    let from_p = wgs84_to_utm33n(req.from[0], req.from[1]);
    let to_p = wgs84_to_utm33n(req.to[0], req.to[1]);
    let from = g
        .snap(from_p.x, from_p.y, req.snap_radius_m)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let to = g
        .snap(to_p.x, to_p.y, req.snap_radius_m)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let start = Instant::now();
    let res: Option<RouteResult> = g
        .route(from, to, req.profile)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let took_us = start.elapsed().as_micros() as u64;
    let res = res.ok_or(ApiError::BadRequest("no route".into()))?;
    let geometry: Vec<[f64; 2]> = res
        .nodes
        .iter()
        .filter_map(|&nid| g.node(nid))
        .map(|p| [p.x as f64, p.y as f64])
        .collect();
    Ok(Json(RouteResp {
        from_node: from,
        to_node: to,
        length_m: res.length_m,
        cost: res.cost,
        geometry,
        edges: res.edges,
        took_us,
    }))
}

pub async fn stats(
    State(state): State<ApiState>,
) -> Result<Json<GraphStats>, ApiError> {
    let g = state.graph.as_ref().ok_or(ApiError::PrimitiveUnavailable("graph"))?;
    Ok(Json(g.stats()))
}

#[derive(Debug, Serialize)]
pub struct DensityResp {
    /// WGS84 [lon, lat] for each sampled node.
    pub points: Vec<[f64; 2]>,
    /// Source node count + stride used.
    pub source_count: u32,
    pub returned_count: u32,
}

/// Returns a stride-sampled set of graph node positions for the
/// admin "show me trail density" overlay. Default sample size is
/// 5000 — enough to convey shape, small enough to draw cheaply.
pub async fn density(
    State(state): State<ApiState>,
) -> Result<Json<DensityResp>, ApiError> {
    let g = state.graph.as_ref().ok_or(ApiError::PrimitiveUnavailable("graph"))?;
    let nodes = g.sample_nodes(5_000);
    let points: Vec<[f64; 2]> = nodes
        .into_iter()
        .map(|p| {
            let (lon, lat) =
                turbo_tiles_pathfind::utm33n_to_wgs84(p.x as f64, p.y as f64);
            [lon, lat]
        })
        .collect();
    let returned_count = points.len() as u32;
    Ok(Json(DensityResp {
        points,
        source_count: g.stats().meta.node_count,
        returned_count,
    }))
}
