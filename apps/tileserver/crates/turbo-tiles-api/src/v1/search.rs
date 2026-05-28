//! `/v1/search/*` — anchor search primitive (Stage 5).

use std::time::Instant;

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::wgs84_to_utm33n;
use turbo_tiles_search::{AnchorHit, AnchorKind, IndexStats};

use crate::error::ApiError;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct NearestReq {
    pub lon: f64,
    pub lat: f64,
    #[serde(default)]
    pub kind: Option<AnchorKind>,
    #[serde(default = "default_n")]
    pub n: usize,
}
fn default_n() -> usize {
    10
}

#[derive(Debug, Serialize)]
pub struct NearestResp {
    pub anchors: Vec<AnchorHit>,
    pub took_us: u64,
}

pub async fn nearest(
    State(state): State<ApiState>,
    Json(req): Json<NearestReq>,
) -> Result<Json<NearestResp>, ApiError> {
    let idx = state
        .search
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("search"))?;
    let p = wgs84_to_utm33n(req.lon, req.lat);
    let start = Instant::now();
    let anchors = idx.nearest(p.x as f32, p.y as f32, req.kind, req.n.clamp(1, 200));
    Ok(Json(NearestResp {
        anchors,
        took_us: start.elapsed().as_micros() as u64,
    }))
}

#[derive(Debug, Deserialize)]
pub struct NameReq {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_limit() -> usize {
    20
}

#[derive(Debug, Serialize)]
pub struct NameResp {
    pub anchors: Vec<AnchorHit>,
    pub took_us: u64,
}

pub async fn name(
    State(state): State<ApiState>,
    Query(req): Query<NameReq>,
) -> Result<Json<NameResp>, ApiError> {
    let idx = state
        .search
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("search"))?;
    let start = Instant::now();
    let anchors = idx.search_name(&req.q, req.limit.clamp(1, 200));
    Ok(Json(NameResp {
        anchors,
        took_us: start.elapsed().as_micros() as u64,
    }))
}

pub async fn coverage(
    State(state): State<ApiState>,
) -> Result<Json<IndexStats>, ApiError> {
    let idx = state
        .search
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("search"))?;
    Ok(Json(idx.stats()))
}
