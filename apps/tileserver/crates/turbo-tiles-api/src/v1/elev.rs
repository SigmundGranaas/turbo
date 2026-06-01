//! `/v1/elev/*` — elevation primitive HTTP surface.
//!
//! All inputs are WGS84 (lon, lat) for client convenience; conversion
//! to EPSG:25833 happens here at the boundary.
//!
//! Debug endpoints under `/v1/debug/elev/*` are exposed unconditionally
//! today; Stage 7 gates them behind `TURBO_ENABLE_DEBUG=1` for prod.

use std::time::Instant;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::{wgs84_to_utm33n, PointXY};

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
    pub x_25833: f64,
    pub y_25833: f64,
    /// `None` when the point falls on a nodata neighbourhood.
    pub elev_m: Option<f32>,
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
    let z = dem.sample(p).map_err(|e| match e {
        turbo_tiles_elev::DemError::OutOfCoverage { .. } => ApiError::BadRequest(e.to_string()),
        other => ApiError::Internal(other.to_string()),
    })?;
    let took_us = start.elapsed().as_micros() as u64;
    Ok(Json(SampleResp {
        lon: req.lon,
        lat: req.lat,
        x_25833: p.x,
        y_25833: p.y,
        elev_m: z,
        took_us,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ProfileReq {
    /// WGS84 (lon, lat) vertices of the line.
    pub line: Vec<[f64; 2]>,
    /// Number of samples to take along the line (inclusive). Defaults
    /// to the vertex count when `samples` is missing.
    #[serde(default)]
    pub samples: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ProfileResp {
    pub elev_m: Vec<Option<f32>>,
    pub distances_m: Vec<f64>,
    pub took_us: u64,
}

pub async fn profile(
    State(state): State<ApiState>,
    Json(req): Json<ProfileReq>,
) -> Result<Json<ProfileResp>, ApiError> {
    let dem = state
        .dem
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("dem"))?;
    if req.line.len() < 2 {
        return Err(ApiError::BadRequest(
            "line needs at least 2 vertices".into(),
        ));
    }
    // Convert vertices to EPSG:25833 first; sampling is uniformly
    // spaced along the polyline in projected coordinates so the
    // metric "distance_m" stays honest.
    let projected: Vec<PointXY> = req
        .line
        .iter()
        .map(|p| wgs84_to_utm33n(p[0], p[1]))
        .collect();
    let mut seg_lens = Vec::with_capacity(projected.len());
    let mut total = 0.0;
    seg_lens.push(0.0);
    for w in projected.windows(2) {
        let dx = w[1].x - w[0].x;
        let dy = w[1].y - w[0].y;
        total += (dx * dx + dy * dy).sqrt();
        seg_lens.push(total);
    }
    let samples = req.samples.unwrap_or(projected.len() as u32).clamp(2, 4096) as usize;
    let mut pts = Vec::with_capacity(samples);
    let mut distances = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = if samples == 1 {
            0.0
        } else {
            i as f64 / (samples - 1) as f64
        };
        let d = t * total;
        // Find segment by linear scan — n_vertices is small (<10k
        // realistically; clamp above bounds the worst case).
        let mut seg_idx = 0;
        for (j, &cum) in seg_lens.iter().enumerate().skip(1) {
            if d <= cum + 1e-6 {
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
        pts.push(PointXY {
            x: a.x + (b.x - a.x) * local_t,
            y: a.y + (b.y - a.y) * local_t,
        });
        distances.push(d);
    }
    let start = Instant::now();
    let elev = dem
        .profile(&pts)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let took_us = start.elapsed().as_micros() as u64;
    Ok(Json(ProfileResp {
        elev_m: elev,
        distances_m: distances,
        took_us,
    }))
}

pub async fn coverage(
    State(state): State<ApiState>,
) -> Result<Json<turbo_tiles_elev::DemCoverage>, ApiError> {
    let dem = state
        .dem
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("dem"))?;
    Ok(Json(dem.coverage()))
}

#[derive(Debug, Serialize)]
pub struct BenchResp {
    pub sample_p50_us: u64,
    pub sample_p99_us: u64,
    pub sample_mean_us: f64,
    pub sample_count: u32,
    pub profile_p50_us: u64,
    pub profile_p99_us: u64,
    pub profile_mean_us: f64,
    pub profile_count: u32,
    pub profile_points: u32,
}

/// In-process micro-benchmark. Picks random points inside the loaded
/// DEM extent and times sample()/profile(). Useful as a quick smoke
/// from the admin UI; the criterion benches are the real gate.
pub async fn bench(State(state): State<ApiState>) -> Result<Json<BenchResp>, ApiError> {
    let dem = state
        .dem
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("dem"))?;
    let cov = dem.coverage();
    // Deterministic stream of pseudo-randoms — splitmix is the
    // smallest decent generator that doesn't need a crate.
    let mut s = 0xDEAD_BEEF_CAFE_BABE_u64;
    let mut rng_f = || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z = z ^ (z >> 31);
        (z as f64) / (u64::MAX as f64)
    };
    let dx = cov.max_x - cov.min_x;
    let dy = cov.max_y - cov.min_y;
    let sample_count = 10_000;
    let mut sample_times = Vec::with_capacity(sample_count as usize);
    for _ in 0..sample_count {
        let p = PointXY {
            x: cov.min_x + dx * rng_f(),
            y: cov.min_y + dy * rng_f(),
        };
        let t0 = Instant::now();
        let _ = dem.sample(p);
        sample_times.push(t0.elapsed().as_micros() as u64);
    }
    sample_times.sort_unstable();
    let s_p50 = sample_times[sample_times.len() / 2];
    let s_p99 = sample_times[(sample_times.len() as f64 * 0.99) as usize];
    let s_mean = sample_times.iter().sum::<u64>() as f64 / sample_times.len() as f64;

    let profile_count = 1_000u32;
    let profile_points = 100u32;
    let mut profile_times = Vec::with_capacity(profile_count as usize);
    for _ in 0..profile_count {
        let x0 = cov.min_x + dx * rng_f();
        let y0 = cov.min_y + dy * rng_f();
        let pts: Vec<PointXY> = (0..profile_points)
            .map(|i| PointXY {
                x: x0 + i as f64 * 5.0,
                y: y0 + i as f64 * 5.0,
            })
            .collect();
        let t0 = Instant::now();
        let _ = dem.profile(&pts);
        profile_times.push(t0.elapsed().as_micros() as u64);
    }
    profile_times.sort_unstable();
    let p_p50 = profile_times[profile_times.len() / 2];
    let p_p99 = profile_times[(profile_times.len() as f64 * 0.99) as usize];
    let p_mean = profile_times.iter().sum::<u64>() as f64 / profile_times.len() as f64;

    Ok(Json(BenchResp {
        sample_p50_us: s_p50,
        sample_p99_us: s_p99,
        sample_mean_us: s_mean,
        sample_count,
        profile_p50_us: p_p50,
        profile_p99_us: p_p99,
        profile_mean_us: p_mean,
        profile_count,
        profile_points,
    }))
}
