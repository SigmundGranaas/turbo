//! `/v1/pathfind` + `/v1/debug/pathfind/*` — composing primitive.
//!
//! The Pathfinder lives on `ApiState` so custom `CostLayer`s
//! registered at boot persist across requests. The endpoint itself
//! is a thin shim over `Pathfinder::solve`.

use std::time::Instant;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use turbo_tiles_pathfind::{Inspect, InspectPoint, Path, PathfindError, Prefs};

use crate::crash_dump::{run_or_dump, CaughtPanic};
use crate::error::ApiError;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct PathfindReq {
    /// Legacy 2-point shape. Still accepted; equivalent to
    /// `points: [from, to]`.
    #[serde(default)]
    pub from: Option<[f64; 2]>,
    #[serde(default)]
    pub to: Option<[f64; 2]>,
    /// Ordered list of >= 2 waypoints `[lon, lat]` (start, vias, end).
    /// Takes precedence over `from`/`to` when present. The route visits
    /// each point in order.
    #[serde(default)]
    pub points: Option<Vec<[f64; 2]>>,
    /// Named trip preset ("balanced", "avoid_roads", "direct",
    /// "easy_grade", "trail_purist"). Resolved server-side to a cost
    /// patch; an explicit `prefs.cost_config_override` overlays on top.
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub prefs: Option<Prefs>,
}

/// Resolve `preset` into `prefs.cost_config_override`: the preset patch
/// is the base, and any explicit override the client also sent overlays
/// on top (fine-tune wins). Unknown preset → 400 with the valid names.
pub(crate) fn apply_preset(
    state: &ApiState,
    preset: &Option<String>,
    prefs: &mut Prefs,
) -> Result<(), String> {
    let Some(name) = preset else { return Ok(()) };
    let Some(p) = state.presets.get(name) else {
        let names: Vec<&str> = state
            .presets
            .presets
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        return Err(format!(
            "unknown preset '{name}'; valid: {}",
            names.join(", ")
        ));
    };
    let base = p.patch.clone();
    prefs.cost_config_override = Some(match prefs.cost_config_override.take() {
        Some(explicit) => explicit.over(&base),
        None => base,
    });
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct PresetInfo {
    pub name: String,
    pub label: String,
    pub description: String,
}

/// `GET /v1/route/presets` — the trip styles for the SPA dropdown.
pub async fn presets(State(state): State<ApiState>) -> Json<Vec<PresetInfo>> {
    Json(
        state
            .presets
            .presets
            .iter()
            .map(|p| PresetInfo {
                name: p.name.clone(),
                label: p.label.clone(),
                description: p.description.clone(),
            })
            .collect(),
    )
}

impl PathfindReq {
    /// Normalize the request to an ordered point list. Accepts either
    /// `points` (>= 2) or both `from` and `to`; rejects neither/too-few.
    fn resolve_points(&self) -> Result<Vec<[f64; 2]>, ApiError> {
        match &self.points {
            Some(pts) if pts.len() >= 2 => Ok(pts.clone()),
            Some(_) => Err(ApiError::BadRequest(
                "`points` needs at least 2 entries".into(),
            )),
            None => match (self.from, self.to) {
                (Some(f), Some(t)) => Ok(vec![f, t]),
                _ => Err(ApiError::BadRequest(
                    "provide either `points` (>= 2) or both `from` and `to`".into(),
                )),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PathfindResp {
    pub path: Path,
    pub took_us: u64,
    /// Layers that contributed to this request — useful for the
    /// admin UI to display "with marking=0.0 disabled" etc.
    pub layers: Vec<&'static str>,
}

pub async fn pathfind(
    State(state): State<ApiState>,
    Json(req): Json<PathfindReq>,
) -> Result<Json<PathfindResp>, ApiError> {
    let pf = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?
        .clone();
    let mut prefs = req.prefs.clone().unwrap_or_default();
    apply_preset(&state, &req.preset, &mut prefs).map_err(ApiError::BadRequest)?;
    let points = req.resolve_points()?;
    let start = Instant::now();
    // Wrap the synchronous solve in `catch_unwind` so a Rust panic
    // (out-of-bounds, unwrap-on-None, …) becomes an HTTP 500 with a
    // dump_id instead of crashing the whole server. The dump file
    // captures the request body verbatim — curl-replayable into a
    // debug binary. See `crash_dump.rs`.
    let req_json = serde_json::json!({
        "points": points,
        "prefs": serde_json::to_value(PrefsEcho::from(&prefs)).unwrap_or_default(),
    });
    let points_for_solve = points.clone();
    let solve_result = run_or_dump("/v1/pathfind", req_json, move || {
        pf.solve_route(&points_for_solve, prefs)
    });
    let path = match solve_result {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => return Err(map_pathfind_err(&state, e)),
        Err(panic) => return Err(panic_to_api_error(panic)),
    };
    let pf2 = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?;
    Ok(Json(PathfindResp {
        path,
        took_us: start.elapsed().as_micros() as u64,
        layers: pf2.layer_names(),
    }))
}

fn panic_to_api_error(panic: CaughtPanic) -> ApiError {
    ApiError::Internal(format!(
        "internal panic captured (dump_id={}, msg=\"{}\")",
        panic.dump_id, panic.message
    ))
}

/// `POST /v1/pathfind/record` — same shape as `/v1/pathfind`, but
/// always sets `prefs.record = true` regardless of what the client
/// sent. Convenience endpoint for the SPA's algorithm-replay panel
/// so the curator doesn't have to remember the flag. The recording
/// adds a few hundred KB to the response and ~10% CPU during the
/// solve; tracker payload is gated behind this dedicated route so
/// hot interactive `/v1/pathfind` calls stay lean.
pub async fn pathfind_record(
    State(state): State<ApiState>,
    Json(mut req): Json<PathfindReq>,
) -> Result<Json<PathfindResp>, ApiError> {
    let mut prefs = req.prefs.take().unwrap_or_default();
    prefs.record = true;
    // Also turn on the per-layer trace so the SPA's debug panel
    // and replay panel light up together — same UX expectation.
    prefs.debug = true;
    req.prefs = Some(prefs);
    pathfind(State(state), Json(req)).await
}

/// Subset of `Prefs` we serialize back into the crash dump. The
/// full `Prefs` doesn't implement `Serialize` (its `Profile` field
/// is Deserialize-only), so we extract the human-relevant fields
/// by hand.
#[derive(Serialize)]
struct PrefsEcho {
    snap_radius_m: f32,
    bridge_radius_m: f32,
    mesh_cell_m: f64,
    max_off_trail_km: f64,
    allow_off_trail: bool,
    refusal_snap_m: f64,
    debug: bool,
    profile: &'static str,
}

impl PrefsEcho {
    fn from(p: &Prefs) -> Self {
        use turbo_tiles_graph::Profile::*;
        Self {
            snap_radius_m: p.snap_radius_m,
            bridge_radius_m: p.bridge_radius_m,
            mesh_cell_m: p.mesh_cell_m,
            max_off_trail_km: p.max_off_trail_km,
            allow_off_trail: p.allow_off_trail,
            refusal_snap_m: p.refusal_snap_m,
            debug: p.debug,
            profile: match p.profile {
                Foot => "foot",
                Bicycle => "bicycle",
                Ski => "ski",
            },
        }
    }
}

/// `GET /v1/debug/cost-config` — returns the active cost
/// calibration (boot config + the embedded fallback if any
/// resolution step failed). Useful to confirm a config file edit
/// actually took effect after a restart.
pub async fn cost_config(
    State(state): State<ApiState>,
) -> Result<Json<turbo_tiles_pathfind::CostConfig>, ApiError> {
    let pf = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?;
    Ok(Json(pf.cost_config.clone()))
}

#[derive(Debug, Deserialize)]
pub struct CostBreakdownReq {
    pub from: [f64; 2],
    pub to: [f64; 2],
    #[serde(default)]
    pub profile: Option<turbo_tiles_graph::Profile>,
}

#[derive(Debug, Serialize)]
pub struct CostBreakdownResp {
    pub cost: turbo_tiles_pathfind::EdgeWalkCost,
    pub took_us: u64,
    /// The unit every contribution is expressed in. Documented
    /// alongside so curators inspecting raw JSON know what they're
    /// reading without crawling the source.
    pub unit: &'static str,
    pub base_pace_s_per_m: f64,
}

/// `POST /v1/debug/cost-breakdown` — given a candidate edge
/// (from, to, profile), return the walk-seconds each registered
/// cost contributor would add for that edge. Decouples "what
/// the solver is doing" from "the eyeballed multipliers we used
/// to debug it" — every contribution is in real physical time
/// units, additive, comparable across contributors.
pub async fn cost_breakdown(
    State(state): State<ApiState>,
    Json(req): Json<CostBreakdownReq>,
) -> Result<Json<CostBreakdownResp>, ApiError> {
    let pf = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?;
    let profile = req.profile.unwrap_or(turbo_tiles_graph::Profile::Foot);
    let start = Instant::now();
    let cost = pf.cost_breakdown(req.from, req.to, profile);
    Ok(Json(CostBreakdownResp {
        cost,
        took_us: start.elapsed().as_micros() as u64,
        unit: "walk_seconds",
        base_pace_s_per_m: turbo_tiles_pathfind::BASE_PACE_S_PER_M,
    }))
}

/// `POST /v1/pathfind/stream` — Server-Sent Events endpoint that
/// streams the solver's exploration *as it computes* rather than
/// after. The SPA's "Live mode" toggle uses this to render the
/// frontier expansion live, especially valuable for the 5+ second
/// Marka-style off-trail solves where waiting for the full
/// `/pathfind/record` response feels frozen.
///
/// Wire shape:
/// - Each event is one JSON-serialised `SolverEvent` (same type
///   as the record+replay path) carried by a single SSE `event:
///   solver` frame.
/// - Once the solver finishes a `done` event carries the final
///   `Path` payload so the client can render the answer.
/// - On error, a `error` event carries a short reason string.
/// - A 15-second keep-alive ping prevents proxy buffers from
///   stalling long solves.
///
/// Backpressure: the recorder uses `try_send` so a slow client
/// drops frames rather than blocking the solver. The final `done`
/// event is sent through `blocking_send` (it's the one event we
/// MUST deliver) so the client always sees the answer.
pub async fn pathfind_stream(
    State(state): State<ApiState>,
    Json(req): Json<PathfindReq>,
) -> impl IntoResponse {
    use futures::stream::StreamExt;

    let pf = match state.pathfinder.as_ref() {
        Some(pf) => pf.clone(),
        None => {
            // No pathfinder loaded — short-circuit with a single
            // error event followed by EOF.
            let s = futures::stream::once(async move {
                Ok::<_, std::convert::Infallible>(
                    Event::default()
                        .event("error")
                        .data(r#"{"message":"pathfind primitive not loaded"}"#),
                )
            });
            return Sse::new(s).keep_alive(KeepAlive::default()).into_response();
        }
    };

    // Resolve the waypoint list before consuming prefs. On a bad
    // request, emit a single error frame + EOF (same shape as the
    // no-pathfinder case) since this handler returns a stream, not a
    // Result.
    let points = match req.resolve_points() {
        Ok(p) => p,
        Err(e) => {
            let body = serde_json::json!({ "message": e.to_string() }).to_string();
            let s = futures::stream::once(async move {
                Ok::<_, std::convert::Infallible>(Event::default().event("error").data(body))
            });
            return Sse::new(s).keep_alive(KeepAlive::default()).into_response();
        }
    };
    let mut prefs = req.prefs.unwrap_or_default();
    if let Err(msg) = apply_preset(&state, &req.preset, &mut prefs) {
        let body = serde_json::json!({ "message": msg }).to_string();
        let s = futures::stream::once(async move {
            Ok::<_, std::convert::Infallible>(Event::default().event("error").data(body))
        });
        return Sse::new(s).keep_alive(KeepAlive::default()).into_response();
    }
    // Streaming endpoint installs its OWN recorder externally
    // (the one that fans into the SSE channel). Setting
    // `prefs.record = false` prevents `Pathfinder::solve` from
    // installing a second in-memory recorder that would shadow
    // ours on the thread-local. The live consumer doesn't need
    // the in-memory snapshot anyway; it's already seeing every
    // event over the wire.
    prefs.record = false;

    // Bounded channel sized for ~50ms of solver throughput at
    // ~10k events/s. `try_send` drops frames when the SPA falls
    // behind; the solver keeps going at full speed.
    let (event_tx, event_rx) =
        tokio::sync::mpsc::channel::<turbo_tiles_pathfind::SolverEvent>(2048);
    // Out-of-band done/error channel for the terminal frame —
    // separate so it can NEVER lose its single message even when
    // the event channel is full.
    let (terminal_tx, mut terminal_rx) = tokio::sync::mpsc::channel::<TerminalFrame>(2);

    // Run the synchronous solver on a blocking thread. The
    // streaming recorder fans every record() call into event_tx
    // via try_send. When solve returns we ship the terminal frame
    // through terminal_tx.
    tokio::task::spawn_blocking(move || {
        let recorder = std::sync::Arc::new(turbo_tiles_pathfind::Recorder::new_streaming(
            prefs.record_cap,
            event_tx.clone(),
        ));
        let result = turbo_tiles_pathfind::solver_trace::with_installed(recorder, || {
            pf.solve_route(&points, prefs)
        });
        let terminal = match result {
            Ok(path) => TerminalFrame::Done(Box::new(path)),
            Err(e) => TerminalFrame::Error(e.to_string()),
        };
        // Block until the consumer reads the terminal frame, but
        // bounded — if the client has hung up, we just give up.
        let _ = terminal_tx.blocking_send(terminal);
    });

    // Bridge the two channels into a single SSE stream. Each
    // solver event becomes one `event: solver` SSE frame; the
    // final terminal frame becomes `event: done` or `event: error`.
    let event_stream = tokio_stream::wrappers::ReceiverStream::new(event_rx).map(|ev| {
        let projected = project_solver_event(ev);
        let body = serde_json::to_string(&projected).unwrap_or_else(|_| "{}".to_string());
        Ok::<Event, std::convert::Infallible>(Event::default().event("solver").data(body))
    });
    // After event_stream ends (sender dropped), pull the terminal
    // frame and emit it as the last SSE event.
    let tail = async_stream::stream! {
        if let Some(t) = terminal_rx.recv().await {
            let ev = match t {
                TerminalFrame::Done(path) => {
                    let body = serde_json::to_string(&*path).unwrap_or_default();
                    Event::default().event("done").data(body)
                }
                TerminalFrame::Error(msg) => {
                    let body = serde_json::json!({ "message": msg }).to_string();
                    Event::default().event("error").data(body)
                }
            };
            yield Ok::<Event, std::convert::Infallible>(ev);
        }
    };
    let combined = event_stream.chain(tail);
    Sse::new(combined)
        .keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)))
        .into_response()
}

enum TerminalFrame {
    Done(Box<turbo_tiles_pathfind::Path>),
    Error(String),
}

/// Project one SolverEvent's coordinates from UTM33N to WGS84.
/// The recorder stores everything in UTM at record time to keep
/// the hot loop cheap; we project once per event on the way out.
fn project_solver_event(
    ev: turbo_tiles_pathfind::SolverEvent,
) -> turbo_tiles_pathfind::SolverEvent {
    use turbo_tiles_pathfind::{utm33n_to_wgs84, SolverEvent};
    let proj = |x: f32, y: f32| -> [f32; 2] {
        let (lon, lat) = utm33n_to_wgs84(x as f64, y as f64);
        [lon as f32, lat as f32]
    };
    match ev {
        SolverEvent::NodePopped { x, y, g, h } => {
            let p = proj(x, y);
            SolverEvent::NodePopped {
                x: p[0],
                y: p[1],
                g,
                h,
            }
        }
        SolverEvent::EdgeRelaxed {
            fx,
            fy,
            tx,
            ty,
            new_g,
            took_los,
        } => {
            let a = proj(fx, fy);
            let b = proj(tx, ty);
            SolverEvent::EdgeRelaxed {
                fx: a[0],
                fy: a[1],
                tx: b[0],
                ty: b[1],
                new_g,
                took_los,
            }
        }
        SolverEvent::LineOfSightCast {
            fx,
            fy,
            tx,
            ty,
            blocked,
        } => {
            let a = proj(fx, fy);
            let b = proj(tx, ty);
            SolverEvent::LineOfSightCast {
                fx: a[0],
                fy: a[1],
                tx: b[0],
                ty: b[1],
                blocked,
            }
        }
        SolverEvent::BestPathSnapshot { coords } => {
            let projected = coords
                .into_iter()
                .map(|c| {
                    let p = proj(c[0], c[1]);
                    [p[0], p[1]]
                })
                .collect();
            SolverEvent::BestPathSnapshot { coords: projected }
        }
        other => other,
    }
}

/// `POST /v1/debug/induce-panic` — dev-mode endpoint that panics
/// inside the same `run_or_dump` wrapper the real handler uses.
/// Verifies the safety net is actually wired end-to-end: the
/// server stays up, a dump file lands on disk, and the client gets
/// a 500 with the dump_id. Gated by TURBO_DEV_AUTH; production
/// builds shouldn't expose it.
pub async fn induce_panic(
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if std::env::var("TURBO_DEV_AUTH").as_deref() != Ok("1") {
        return Err(ApiError::BadRequest(
            "induce-panic is dev-only; set TURBO_DEV_AUTH=1".into(),
        ));
    }
    let req_for_dump = req.clone();
    let result = run_or_dump::<_, ()>("/v1/debug/induce-panic", req_for_dump, move || {
        let msg = req
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("induced panic for crash-safety-net verification");
        panic!("{msg}");
    });
    match result {
        Ok(()) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(panic) => Err(panic_to_api_error(panic)),
    }
}

/// `GET /v1/debug/recent-crashes?limit=N` — list the most recent
/// captured panic dumps with their request body and panic message.
/// Bounded; defaults to 20. Gated behind the same TURBO_DEV_AUTH
/// trust the dev-login endpoint sits behind.
#[derive(Debug, Deserialize)]
pub struct RecentCrashesQuery {
    #[serde(default = "default_recent_limit")]
    pub limit: usize,
}
fn default_recent_limit() -> usize {
    20
}

pub async fn recent_crashes(
    axum::extract::Query(q): axum::extract::Query<RecentCrashesQuery>,
) -> Json<serde_json::Value> {
    let dumps = crate::crash_dump::list_recent_crashes(q.limit);
    Json(serde_json::json!({
        "dumps": dumps,
        "dir": crate::crash_dump::crash_dir().display().to_string(),
    }))
}

#[derive(Debug, Serialize)]
pub struct LayersResp {
    pub layers: Vec<&'static str>,
}

#[derive(Debug, Deserialize)]
pub struct InspectReq {
    pub from: [f64; 2],
    pub to: [f64; 2],
    #[serde(default)]
    pub prefs: Option<Prefs>,
}

#[derive(Debug, Serialize)]
pub struct InspectResp {
    pub inspect: Inspect,
    pub took_us: u64,
}

#[derive(Debug, Deserialize)]
pub struct CellInspectReq {
    pub lon: f64,
    pub lat: f64,
    #[serde(default)]
    pub profile: Option<turbo_tiles_graph::Profile>,
}

#[derive(Debug, Serialize)]
pub struct CellInspectResp {
    pub point: InspectPoint,
    pub took_us: u64,
}

/// Cell-level "why is this red?" inspector. Takes one (lon, lat) and
/// returns every layer's verdict at that point, including the raw
/// multiplier, the refusal reason if any, and whether the layer
/// claims coverage there. Drives the SPA's click-to-inspect UI so
/// the curator can see exactly which layer is responsible for a
/// red/expensive/refused cell.
pub async fn cell_inspect(
    State(state): State<ApiState>,
    Json(req): Json<CellInspectReq>,
) -> Result<Json<CellInspectResp>, ApiError> {
    let pf = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?;
    let profile = req.profile.unwrap_or(turbo_tiles_graph::Profile::Foot);
    let start = Instant::now();
    let point = pf.inspect_point(req.lon, req.lat, profile);
    Ok(Json(CellInspectResp {
        point,
        took_us: start.elapsed().as_micros() as u64,
    }))
}

pub async fn inspect(
    State(state): State<ApiState>,
    Json(req): Json<InspectReq>,
) -> Result<Json<InspectResp>, ApiError> {
    let pf = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?;
    let prefs = req.prefs.unwrap_or_default();
    let start = Instant::now();
    let inspect = pf.inspect(req.from, req.to, &prefs);
    Ok(Json(InspectResp {
        inspect,
        took_us: start.elapsed().as_micros() as u64,
    }))
}

pub async fn layers(State(state): State<ApiState>) -> Result<Json<LayersResp>, ApiError> {
    let pf = state
        .pathfinder
        .as_ref()
        .ok_or(ApiError::PrimitiveUnavailable("pathfind"))?;
    Ok(Json(LayersResp {
        layers: pf.layer_names(),
    }))
}

pub(crate) fn map_pathfind_err(state: &ApiState, e: PathfindError) -> ApiError {
    use PathfindError::*;
    match e {
        DegenerateInputs { .. } | BboxTooLarge { .. } => ApiError::BadRequest(e.to_string()),
        NoRoute => ApiError::BadRequest("no route".into()),
        NoCoverage {
            from_covered,
            to_covered,
            from_has_graph_anchor,
            to_has_graph_anchor,
        } => {
            // Collect the bbox of whichever primitives are loaded so
            // the SPA can show the user where coverage *does* exist.
            // Sent as WGS84 [west, south, east, north] for direct
            // consumption by MapLibre's fitBounds.
            let mut hints: Vec<serde_json::Value> = Vec::new();
            if let Some(dem) = state.dem.as_ref() {
                let c = dem.coverage();
                hints.push(serde_json::json!({
                    "kind": "dem",
                    "bbox_25833": [c.min_x, c.min_y, c.max_x, c.max_y],
                    "bbox_wgs84": utm_bbox_to_wgs84(c.min_x, c.min_y, c.max_x, c.max_y),
                    "cells_x": c.cells_x,
                    "cells_y": c.cells_y,
                }));
            }
            if let Some(g) = state.graph.as_ref() {
                let s = g.stats();
                let min_x = s.min_x as f64;
                let min_y = s.min_y as f64;
                let max_x = s.max_x as f64;
                let max_y = s.max_y as f64;
                hints.push(serde_json::json!({
                    "kind": "graph",
                    "bbox_25833": [min_x, min_y, max_x, max_y],
                    "bbox_wgs84": utm_bbox_to_wgs84(min_x, min_y, max_x, max_y),
                    "nodes": s.meta.node_count,
                    "edges": s.meta.edge_count,
                }));
            }
            if let Some(m) = state.mask.as_ref() {
                let c = m.coverage();
                hints.push(serde_json::json!({
                    "kind": "mask",
                    "bbox_25833": [c.meta.min_x, c.meta.min_y, c.meta.max_x, c.meta.max_y],
                    "bbox_wgs84": utm_bbox_to_wgs84(c.meta.min_x, c.meta.min_y, c.meta.max_x, c.meta.max_y),
                    "water_cells": c.cells_water,
                    "glacier_cells": c.cells_glacier,
                }));
            }
            ApiError::NoCoverage {
                message: "no terrain data at these coordinates".into(),
                details: serde_json::json!({
                    "from_in_coverage": from_covered,
                    "to_in_coverage": to_covered,
                    "from_has_graph_anchor": from_has_graph_anchor,
                    "to_has_graph_anchor": to_has_graph_anchor,
                    "available_coverage": hints,
                }),
            }
        }
        EndpointRefused { which, layer } => ApiError::NoCoverage {
            message: format!(
                "{which} endpoint is in a refused region (layer: {layer}). Click on a different spot — e.g. a trail, summit, or open ground.",
            ),
            details: serde_json::json!({
                "kind": "endpoint_refused",
                "which": which,
                "layer": layer,
            }),
        },
        Graph(g) => ApiError::Internal(g.to_string()),
        Dem(d) => ApiError::Internal(d.to_string()),
        Internal(msg) => ApiError::Internal(msg),
        SegmentFailed {
            leg_index,
            from,
            to,
            source,
        } => {
            // Attribute the failure to the exact stop so the SPA can
            // highlight it. The user-facing message identifies the leg;
            // `details` carry the index + endpoints + the underlying
            // reason for the UI to render inline.
            ApiError::NoCoverage {
                message: format!(
                    "no route for leg {} (stop {} → stop {}): {source}",
                    leg_index,
                    leg_index + 1,
                    leg_index + 2,
                ),
                details: serde_json::json!({
                    "kind": "segment_failed",
                    "leg_index": leg_index,
                    "from": from,
                    "to": to,
                    "reason": source.to_string(),
                }),
            }
        }
    }
}

/// Approximate UTM33N → WGS84 bbox conversion just for the response
/// hint — the SPA only needs ~100 m accuracy to flyTo. Full inverse
/// projection lives in `turbo_tiles_pathfind::pathfinder::utm33n_to_wgs84`.
fn utm_bbox_to_wgs84(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> [f64; 4] {
    let (w, s) = turbo_tiles_pathfind::utm33n_to_wgs84(min_x, min_y);
    let (e, n) = turbo_tiles_pathfind::utm33n_to_wgs84(max_x, max_y);
    [w, s, e, n]
}
