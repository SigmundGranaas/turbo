//! `/v1/slope/*` — slope + aspect derived from the DEM via Horn (1981)
//! central differences. Shares the same `Dem` artifact as the
//! elevation primitive; no separate boot or artifact required.

use std::time::Instant;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::{wgs84_to_utm33n, PointXY, SlopeAspect};

use crate::error::ApiError;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct SampleReq {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Debug, Serialize)]
pub struct SampleResp {
    pub lon: f64,
    pub lat: f64,
    pub slope_deg: Option<f32>,
    pub aspect_deg: Option<f32>,
    pub took_us: u64,
}

pub async fn sample(
    State(state): State<ApiState>,
    Json(req): Json<SampleReq>,
) -> Result<Json<SampleResp>, ApiError> {
    let dem = state
        .dem
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("dem"))?;
    let p = wgs84_to_utm33n(req.lon, req.lat);
    let start = Instant::now();
    let sa = dem.slope_aspect(p).map_err(|e| match e {
        turbo_tiles_elev::DemError::OutOfCoverage { .. } => ApiError::BadRequest(e.to_string()),
        other => ApiError::Internal(other.to_string()),
    })?;
    let took_us = start.elapsed().as_micros() as u64;
    Ok(Json(SampleResp {
        lon: req.lon,
        lat: req.lat,
        slope_deg: sa.map(|s| s.slope_deg),
        aspect_deg: sa.map(|s| s.aspect_deg),
        took_us,
    }))
}

#[derive(Debug, Deserialize)]
pub struct AlongReq {
    /// WGS84 vertices of the line.
    pub line: Vec<[f64; 2]>,
    /// Step length in metres along the projected line. Default 25 m.
    /// Bounded so a million-step request can't tie up the server.
    #[serde(default)]
    pub step_m: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AlongStep {
    pub distance_m: f64,
    pub slope_deg: Option<f32>,
    pub aspect_deg: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct AlongResp {
    pub steps: Vec<AlongStep>,
    pub took_us: u64,
}

pub async fn along(
    State(state): State<ApiState>,
    Json(req): Json<AlongReq>,
) -> Result<Json<AlongResp>, ApiError> {
    let dem = state
        .dem
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("dem"))?;
    if req.line.len() < 2 {
        return Err(ApiError::BadRequest(
            "line needs at least 2 vertices".into(),
        ));
    }
    let step_m = req.step_m.unwrap_or(25.0).max(1.0);
    let projected: Vec<PointXY> = req
        .line
        .iter()
        .map(|p| wgs84_to_utm33n(p[0], p[1]))
        .collect();
    let mut seg_lens = vec![0.0];
    let mut total = 0.0;
    for w in projected.windows(2) {
        let dx = w[1].x - w[0].x;
        let dy = w[1].y - w[0].y;
        total += (dx * dx + dy * dy).sqrt();
        seg_lens.push(total);
    }
    let nsteps = ((total / step_m).ceil() as usize + 1).clamp(2, 8192);
    let mut steps = Vec::with_capacity(nsteps);
    let start = Instant::now();
    for i in 0..nsteps {
        let t = i as f64 / (nsteps - 1) as f64;
        let d = t * total;
        let mut seg_idx = 0;
        for j in 1..seg_lens.len() {
            if d <= seg_lens[j] + 1e-6 {
                seg_idx = j - 1;
                break;
            }
            seg_idx = j - 1;
        }
        let a = projected[seg_idx];
        let b = projected[seg_idx + 1];
        let seg_len = seg_lens[seg_idx + 1] - seg_lens[seg_idx];
        let local_t = if seg_len > 0.0 {
            (d - seg_lens[seg_idx]) / seg_len
        } else {
            0.0
        };
        let p = PointXY {
            x: a.x + (b.x - a.x) * local_t,
            y: a.y + (b.y - a.y) * local_t,
        };
        let sa: Option<SlopeAspect> = match dem.slope_aspect(p) {
            Ok(s) => s,
            Err(turbo_tiles_elev::DemError::OutOfCoverage { .. }) => None,
            Err(e) => return Err(ApiError::Internal(e.to_string())),
        };
        steps.push(AlongStep {
            distance_m: d,
            slope_deg: sa.map(|s| s.slope_deg),
            aspect_deg: sa.map(|s| s.aspect_deg),
        });
    }
    Ok(Json(AlongResp {
        steps,
        took_us: start.elapsed().as_micros() as u64,
    }))
}
