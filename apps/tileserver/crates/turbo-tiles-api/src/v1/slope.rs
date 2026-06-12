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
        for (j, &len) in seg_lens.iter().enumerate().skip(1) {
            seg_idx = j - 1;
            if d <= len + 1e-6 {
                break;
            }
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

/// `GET /v1/slope/tiles/{z}/{x}/{y}.png` — the slope-angle ("bratthet")
/// overlay tile: avalanche bands (27/30/35/40/45°) coloured Varsom-style,
/// transparent below 27° and outside DEM coverage. Self-hosted replacement
/// for the NVE steepness overlay. Cached hard (the DEM changes only when
/// the artifact is rebuilt); the edge worker's allowlist covers the path.
pub async fn tile(
    State(state): State<ApiState>,
    axum::extract::Path((z, x, y_ext)): axum::extract::Path<(u8, u32, String)>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::{header, HeaderValue, StatusCode};

    let y_str = y_ext
        .strip_suffix(".png")
        .ok_or_else(|| ApiError::BadRequest("slope tile path must end in .png".into()))?;
    let y: u32 = y_str
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("invalid tile y `{y_str}`")))?;
    let coord = turbo_tiles_core::tile::TileCoord::new(z, x, y)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let dem = state
        .dem
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("dem"))?
        .clone();

    // CPU-bound (256² DEM samples + gradients) — off the runtime threads,
    // same pattern as the Terrain-RGB endpoint.
    let png = tokio::task::spawn_blocking(move || {
        let env = turbo_tiles_raster::tile_envelope_3857(coord);
        match turbo_tiles_raster::render_slope_tile(&dem, env, 256) {
            Some(Ok(png)) => Ok(Some(png)),
            Some(Err(e)) => Err(e),
            None => Ok(None), // entirely outside coverage
        }
    })
    .await
    .map_err(|e| ApiError::Internal(format!("join: {e}")))?
    .map_err(ApiError::Internal)?;

    // Out-of-coverage: a 1×1 transparent PNG so clients tile seamlessly.
    let bytes = match png {
        Some(b) => b,
        None => tiny_skia_blank(),
    };

    let mut resp = axum::response::Response::builder()
        .status(StatusCode::OK)
        .body(axum::body::Body::from(bytes))
        .unwrap();
    let h = resp.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400, stale-while-revalidate=604800"),
    );
    Ok(resp)
}

/// A 1×1 fully-transparent PNG, encoded once.
fn tiny_skia_blank() -> Vec<u8> {
    static BLANK: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    BLANK
        .get_or_init(|| {
            // Hand-assembled minimal transparent PNG (89 bytes) — avoids a
            // tiny-skia dependency in the api crate for one constant.
            const B: &[u8] = &[
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
                0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
                0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78,
                0x9C, 0x62, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
                0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
            ];
            B.to_vec()
        })
        .clone()
}
