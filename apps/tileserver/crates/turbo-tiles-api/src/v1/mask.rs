//! `/v1/mask/*` — refusal-mask primitive HTTP surface.

use std::time::Instant;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::wgs84_to_utm33n;
use turbo_tiles_mask::{MaskCoverage, RefusalKind};

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
    pub refused: bool,
    pub kind: RefusalKind,
    pub took_us: u64,
}

pub async fn sample(
    State(state): State<ApiState>,
    Json(req): Json<SampleReq>,
) -> Result<Json<SampleResp>, ApiError> {
    let mask = state.mask.as_ref().ok_or(ApiError::PrimitiveUnavailable("mask"))?;
    let p = wgs84_to_utm33n(req.lon, req.lat);
    let start = Instant::now();
    // "Outside the mask extent" means the artifact has no opinion
    // about this point — semantically equivalent to "no refusal".
    // Reporting that as 400 forces every caller to special-case
    // their queries; mapping it to RefusalKind::None instead lets
    // clients treat the mask as a sparse veto layer.
    let kind = match mask.refused(p.x, p.y) {
        Ok(k) => k,
        Err(turbo_tiles_mask::MaskError::OutOfCoverage) => RefusalKind::None,
        Err(e) => return Err(ApiError::Internal(e.to_string())),
    };
    Ok(Json(SampleResp {
        lon: req.lon,
        lat: req.lat,
        refused: kind.refused(),
        kind,
        took_us: start.elapsed().as_micros() as u64,
    }))
}

pub async fn coverage(
    State(state): State<ApiState>,
) -> Result<Json<MaskCoverage>, ApiError> {
    let mask = state.mask.as_ref().ok_or(ApiError::PrimitiveUnavailable("mask"))?;
    Ok(Json(mask.coverage()))
}
