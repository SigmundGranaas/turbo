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
use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use turbo_tiles_elev::{wgs84_to_utm33n, PointXY};
use turbo_tiles_graph::Profile;
use turbo_tiles_pathfind::{utm33n_to_wgs84, Path, Prefs, Recorder, SolverEvent};

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
        return Err(ApiError::BadRequest(
            "`points` needs at least 2 entries".into(),
        ));
    }

    let profile = match req.profile.as_deref() {
        Some("bicycle") => Profile::Bicycle,
        Some("ski") => Profile::Ski,
        _ => Profile::Foot,
    };
    let mut prefs = Prefs {
        profile,
        ..Default::default()
    };
    // Default to the "balanced" preset (the app's default) when omitted.
    let preset = req.preset.clone().or_else(|| Some("balanced".to_string()));
    apply_preset(&state, &preset, &mut prefs).map_err(ApiError::BadRequest)?;

    let points = req.points.clone();
    let path = pf
        .solve_route(&points, prefs)
        .map_err(|e| map_pathfind_err(&state, e))?;

    Ok(Json(build_resp(&state, &path, profile)))
}

/// `POST /v1/route/plan/stream` — same contract as `plan`, but streams the
/// solve over Server-Sent Events so clients can show a live preview:
///
/// - `event: progress` — `{ "coordinates": [[lon,lat], …] }`, the
///   best-path-so-far. Replaces itself each time (the latest wins).
/// - `event: result`   — the full [`RoutePlanResp`] on success.
/// - `event: error`    — `{ "error": "…" }` if the solve fails.
///
/// Deliberately narrow: it exposes only the best-path snapshots, not the
/// internal solver-event firehose that `/v1/pathfind/stream` carries.
pub async fn plan_stream(
    State(state): State<ApiState>,
    Json(req): Json<RoutePlanReq>,
) -> impl IntoResponse {
    let Some(pf) = state.pathfinder.as_ref().map(|p| p.clone()) else {
        return sse_error("routing is currently unavailable".to_string());
    };
    if req.points.len() < 2 {
        return sse_error("`points` needs at least 2 entries".to_string());
    }

    let profile = match req.profile.as_deref() {
        Some("bicycle") => Profile::Bicycle,
        Some("ski") => Profile::Ski,
        _ => Profile::Foot,
    };
    let mut prefs = Prefs {
        profile,
        ..Default::default()
    };
    let preset = req.preset.clone().or_else(|| Some("balanced".to_string()));
    if let Err(msg) = apply_preset(&state, &preset, &mut prefs) {
        return sse_error(msg);
    }
    // Our streaming recorder is the live channel; don't let `solve` install
    // a second in-memory recorder that would shadow it (see pathfind.rs).
    prefs.record = false;
    let record_cap = prefs.record_cap;

    let points = req.points.clone();
    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<SolverEvent>(2048);
    let (terminal_tx, mut terminal_rx) = tokio::sync::mpsc::channel::<Terminal>(2);

    tokio::task::spawn_blocking(move || {
        let recorder = Arc::new(Recorder::new_streaming(record_cap, event_tx.clone()));
        let result = turbo_tiles_pathfind::solver_trace::with_installed(recorder, || {
            pf.solve_route(&points, prefs)
        });
        let terminal = match result {
            Ok(path) => Terminal::Done(Box::new(path)),
            Err(e) => Terminal::Error(e.to_string()),
        };
        let _ = terminal_tx.blocking_send(terminal);
    });

    // Forward only best-path snapshots as `progress` events, projected to
    // WGS84 [lon,lat]. Everything else (node pops, edge relaxations) is
    // dropped — clients only want the evolving line.
    let progress =
        tokio_stream::wrappers::ReceiverStream::new(event_rx).filter_map(|ev| async move {
            let SolverEvent::BestPathSnapshot { coords } = ev else {
                return None;
            };
            let projected: Vec<[f64; 2]> = coords
                .iter()
                .map(|c| {
                    let (lon, lat) = utm33n_to_wgs84(c[0] as f64, c[1] as f64);
                    [lon, lat]
                })
                .collect();
            let body = serde_json::json!({ "coordinates": projected }).to_string();
            Some(Ok::<Event, Infallible>(
                Event::default().event("progress").data(body),
            ))
        });

    let state_for_tail = state.clone();
    let tail = async_stream::stream! {
        if let Some(t) = terminal_rx.recv().await {
            let ev = match t {
                Terminal::Done(path) => {
                    let resp = build_resp(&state_for_tail, &path, profile);
                    let body = serde_json::to_string(&resp).unwrap_or_default();
                    Event::default().event("result").data(body)
                }
                Terminal::Error(msg) => {
                    let body = serde_json::json!({ "error": msg }).to_string();
                    Event::default().event("error").data(body)
                }
            };
            yield Ok::<Event, Infallible>(ev);
        }
    };

    Sse::new(progress.chain(tail))
        .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)))
        .into_response()
}

/// Terminal frame from the blocking solve thread.
enum Terminal {
    Done(Box<Path>),
    Error(String),
}

/// A one-frame SSE stream carrying a single `error` event, for the
/// short-circuit cases (bad request, primitive unavailable).
fn sse_error(msg: String) -> axum::response::Response {
    let body = serde_json::json!({ "error": msg }).to_string();
    let s = futures::stream::once(async move {
        Ok::<Event, Infallible>(Event::default().event("error").data(body))
    });
    Sse::new(s).keep_alive(KeepAlive::default()).into_response()
}

/// Build the stable response DTO from a solved [`Path`]. Shared by the
/// blocking (`plan`) and streaming (`plan_stream`) handlers.
fn build_resp(state: &ApiState, path: &Path, profile: Profile) -> RoutePlanResp {
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

    let ascent_m = ascent_along(state, &path.geometry);
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

    RoutePlanResp {
        distance_m: path.length_m,
        duration_s,
        ascent_m,
        on_trail_pct: path.on_trail_pct,
        surfaces,
        geometry: GeoLineString {
            kind: "LineString",
            coordinates: path.geometry.clone(),
        },
        legs,
    }
}

/// Total positive ascent (m) along the geometry, sampled from the DEM.
/// 0 when no DEM is loaded or the route falls outside coverage.
fn ascent_along(state: &ApiState, geom: &[[f64; 2]]) -> f64 {
    let Some(dem) = state.dem.as_ref() else {
        return 0.0;
    };
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
