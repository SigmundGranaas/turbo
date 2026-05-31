//! `/v1/route/plan` + `/v1/route/presets` — the curated, app-facing
//! routing API.
//!
//! Stable, versioned contract intended for the Flutter app (via the
//! gateway at `/api/route/*`). Deliberately narrow and **decoupled from
//! the internal `Path` / debug surface** in `pathfind.rs`: it exposes only
//! what a client needs (distance, time, ascent, surface mix, geometry,
//! per-leg summary) so the solver internals stay free to change. Admin /
//! debug endpoints (recording, layer weights, cost overrides) live in
//! `pathfind.rs` and are NOT part of this contract.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::{wgs84_to_utm33n, PointXY};
use turbo_tiles_graph::Profile;
use turbo_tiles_pathfind::Prefs;

use super::pathfind::{apply_preset, map_pathfind_err};
use crate::error::ApiError;
use crate::state::ApiState;

/// `POST /v1/route/plan` request.
#[derive(Debug, Deserialize)]
pub struct RoutePlanReq {
    /// Ordered waypoints `[lon, lat]`, at least 2 (start, vias, end).
    pub points: Vec<[f64; 2]>,
    /// Trip-style preset (see `GET /v1/route/presets`). Defaults to
    /// "balanced" when omitted.
    #[serde(default)]
    pub preset: Option<String>,
    /// Travel profile: "foot" (default), "bicycle", or "ski".
    #[serde(default)]
    pub profile: Option<String>,
}

/// GeoJSON `LineString` of the route in `[lon, lat]` order.
#[derive(Debug, Serialize)]
pub struct GeoLineString {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub coordinates: Vec<[f64; 2]>,
}

/// One inter-waypoint leg (indices into the request's `points`).
#[derive(Debug, Serialize)]
pub struct RouteLeg {
    pub from_index: u32,
    pub to_index: u32,
    pub distance_m: f64,
}

/// `POST /v1/route/plan` response — the stable app contract.
#[derive(Debug, Serialize)]
pub struct RoutePlanResp {
    pub distance_m: f64,
    /// Estimated travel time (Naismith-style: flat pace + ascent), seconds.
    pub duration_s: f64,
    /// Total positive ascent along the route (metres); 0 if no DEM coverage.
    pub ascent_m: f64,
    /// Percent of the route on marked trails.
    pub on_trail_pct: f32,
    /// Metres by surface — keys: `trail`, `road`, `ski_track`, `off_trail`,
    /// `unknown`. Only present surfaces are included.
    pub surfaces: BTreeMap<String, f64>,
    pub geometry: GeoLineString,
    pub legs: Vec<RouteLeg>,
}

/// `POST /v1/route/plan`
pub async fn plan(
    State(state): State<ApiState>,
    Json(req): Json<RoutePlanReq>,
) -> Result<Json<RoutePlanResp>, ApiError> {
    let pf = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?
        .clone();
    if req.points.len() < 2 {
        return Err(ApiError::BadRequest("`points` needs at least 2 entries".into()));
    }

    let mut prefs = Prefs::default();
    prefs.profile = match req.profile.as_deref() {
        Some("bicycle") => Profile::Bicycle,
        Some("ski") => Profile::Ski,
        _ => Profile::Foot,
    };
    let profile = prefs.profile;
    // Default to the "balanced" preset (the app's default) when omitted.
    let preset = req.preset.clone().or_else(|| Some("balanced".to_string()));
    apply_preset(&state, &preset, &mut prefs).map_err(ApiError::BadRequest)?;

    let points = req.points.clone();
    let path = pf
        .solve_route(&points, prefs)
        .map_err(|e| map_pathfind_err(&state, e))?;

    // Map internal surface keys to app-friendly names.
    let mut surfaces: BTreeMap<String, f64> = BTreeMap::new();
    for (k, v) in &path.fkb_breakdown {
        let name = match k.as_str() {
            "sti" => "trail",
            "vei" => "road",
            "skiloype" => "ski_track",
            "off_trail" => "off_trail",
            other => other,
        };
        *surfaces.entry(name.to_string()).or_insert(0.0) += v;
    }

    let ascent_m = ascent_along(&state, &path.geometry);
    let duration_s = naismith_seconds(path.length_m, ascent_m, profile);
    let legs = path
        .waypoint_legs
        .iter()
        .map(|l| RouteLeg {
            from_index: l.from_point_idx,
            to_index: l.to_point_idx,
            distance_m: l.length_m,
        })
        .collect();

    Ok(Json(RoutePlanResp {
        distance_m: path.length_m,
        duration_s,
        ascent_m,
        on_trail_pct: path.on_trail_pct,
        surfaces,
        geometry: GeoLineString { kind: "LineString", coordinates: path.geometry },
        legs,
    }))
}

/// Total positive ascent (m) along the geometry, sampled from the DEM.
/// 0 when no DEM is loaded or the route falls outside coverage.
fn ascent_along(state: &ApiState, geom: &[[f64; 2]]) -> f64 {
    let Some(dem) = state.dem.as_ref() else { return 0.0 };
    let mut prev: Option<f32> = None;
    let mut gain = 0.0_f64;
    for c in geom {
        let u = wgs84_to_utm33n(c[0], c[1]);
        let z = dem.sample(PointXY { x: u.x, y: u.y }).ok().flatten();
        if let (Some(a), Some(b)) = (prev, z) {
            if b > a {
                gain += (b - a) as f64;
            }
        }
        if z.is_some() {
            prev = z;
        }
    }
    gain
}

/// Naismith-style time estimate (seconds): flat pace + ascent penalty.
/// Honest + solver-independent (the solver `cost` carries preference
/// penalties that aren't real time).
fn naismith_seconds(length_m: f64, ascent_m: f64, profile: Profile) -> f64 {
    let (flat_kmh, ascent_m_per_h) = match profile {
        Profile::Bicycle => (14.0, 400.0),
        Profile::Ski => (8.0, 500.0),
        Profile::Foot => (4.5, 600.0),
    };
    let flat_s = length_m / 1000.0 / flat_kmh * 3600.0;
    let climb_s = ascent_m / ascent_m_per_h * 3600.0;
    flat_s + climb_s
}
