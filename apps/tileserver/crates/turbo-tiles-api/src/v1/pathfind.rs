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
use crate::v1::frame;
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


/// Wire shape of a solved route.
///
/// The engine's `Path` was serialised straight to the client, which
/// made an internal type the HTTP contract. C4 turned the engine
/// planar, and without this DTO the change would have silently swapped
/// `geometry` from lon/lat degrees to metres — the JSON keeps its shape
/// and its types, so nothing would have complained until a map drew the
/// route somewhere off the coast of Africa.
///
/// Every field is listed explicitly rather than flattened. That is the
/// point: the external contract should be readable in one place and
/// should not change because an engine-internal struct grew a field.
#[derive(Debug, Serialize)]
pub struct PathView {
    pub strategy: turbo_tiles_pathfind::PathStrategy,
    /// WGS84 `[lon, lat]`, projected here from the engine's planar frame.
    pub geometry: Vec<[f64; 2]>,
    pub distances_m: Vec<f64>,
    pub length_m: f64,
    pub cost: f64,
    pub on_trail_pct: f32,
    pub fkb_breakdown: std::collections::BTreeMap<String, f64>,
    pub legs: Vec<turbo_tiles_pathfind::PathLeg>,
    pub waypoint_legs: Vec<turbo_tiles_pathfind::WaypointLeg>,
    pub refused_by: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<turbo_tiles_pathfind::TraceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording: Option<turbo_tiles_pathfind::SolverRecording>,
}

impl From<Path> for PathView {
    fn from(p: Path) -> Self {
        Self {
            strategy: p.strategy,
            geometry: frame::lonlat_all(&p.geometry),
            distances_m: p.distances_m,
            length_m: p.length_m,
            cost: p.cost,
            on_trail_pct: p.on_trail_pct,
            fkb_breakdown: p.fkb_breakdown,
            legs: p.legs,
            waypoint_legs: p.waypoint_legs,
            refused_by: p.refused_by,
            debug: p.debug,
            recording: p.recording.map(project_recording),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PathfindResp {
    pub path: PathView,
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
    let points_for_solve = frame::planar_all(&points);
    // Concurrency-gated + off the async worker: the solve holds MBs of
    // corridor/trail scratch for seconds, so it runs on the blocking
    // pool under a routing permit (excess requests queue, not allocate).
    let permit = state.acquire_routing_permit().await;
    let solve_result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        run_or_dump("/v1/pathfind", req_json, move || {
            pf.solve_route(&points_for_solve, prefs)
        })
    })
    .await
    .map_err(|e| ApiError::Internal(format!("solve task failed: {e}")))?;
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
        path: path.into(),
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
    let cost = pf.cost_breakdown(frame::planar(req.from), frame::planar(req.to), profile);
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
    // through terminal_tx. The routing permit rides into the
    // closure so the global solve-concurrency cap covers SSE solves
    // too (the stream starts only once a permit is free).
    let permit = state.acquire_routing_permit().await;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let recorder = std::sync::Arc::new(turbo_tiles_pathfind::Recorder::new_streaming(
            prefs.record_cap,
            event_tx.clone(),
        ));
        let result = turbo_tiles_pathfind::solver_trace::with_installed(recorder, || {
            pf.solve_route(&frame::planar_all(&points), prefs)
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

/// Project one `SolverEvent` from the engine's planar frame to WGS84.
///
/// The recorder stores planar metres at record time to keep the hot
/// loop cheap, and the engine has no CRS to convert with (C4), so this
/// is where it happens — for both the live SSE stream and the recorded
/// `Path::recording` (see [`project_recording`]).
/// Project every event in a completed recording.
///
/// C4 moved this out of the engine, where `serialise_recording` used to
/// do it. Missing this is the kind of change that type-checks and then
/// silently ships planar metres to a client expecting lon/lat, so it is
/// pinned by `recording_is_projected_to_wgs84` below.
pub(crate) fn project_recording(
    mut rec: turbo_tiles_pathfind::SolverRecording,
) -> turbo_tiles_pathfind::SolverRecording {
    for phase in &mut rec.phases {
        phase.events = std::mem::take(&mut phase.events)
            .into_iter()
            .map(project_solver_event)
            .collect();
    }
    rec
}

fn project_solver_event(
    ev: turbo_tiles_pathfind::SolverEvent,
) -> turbo_tiles_pathfind::SolverEvent {
    use turbo_geo_frame::utm33n_to_wgs84;
    use turbo_tiles_pathfind::SolverEvent;
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

/// Wire shape of an inspect result.
///
/// The engine's `Inspect` used to be serialised straight to the client,
/// which made an internal type the HTTP contract — so C4 turning the
/// engine planar would have silently changed the JSON's units. These
/// DTOs project at the boundary and pin the field names the admin SPA
/// actually reads (`apps/admin/src/api/v1.ts`).
#[derive(Debug, Serialize)]
pub struct InspectView {
    pub mesh_cell_m: f64,
    pub cells: Vec<InspectCellView>,
    pub refused_polygons: Vec<Vec<[f64; 2]>>,
    pub refused_by: Vec<String>,
    pub nearest_graph_node_from: Option<[f64; 2]>,
    pub nearest_graph_node_to: Option<[f64; 2]>,
}

#[derive(Debug, Serialize)]
pub struct InspectCellView {
    pub lon: f64,
    pub lat: f64,
    pub cost_mul: f32,
}

impl From<Inspect> for InspectView {
    fn from(i: Inspect) -> Self {
        Self {
            mesh_cell_m: i.mesh_cell_m,
            cells: i
                .cells
                .into_iter()
                .map(|c| {
                    let [lon, lat] = frame::lonlat(c.at);
                    InspectCellView {
                        lon,
                        lat,
                        cost_mul: c.cost_mul,
                    }
                })
                .collect(),
            refused_polygons: frame::lonlat_rings(&i.refused_polygons),
            refused_by: i.refused_by,
            nearest_graph_node_from: i.nearest_graph_node_from.map(frame::lonlat),
            nearest_graph_node_to: i.nearest_graph_node_to.map(frame::lonlat),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct InspectResp {
    pub inspect: InspectView,
    pub took_us: u64,
}

#[derive(Debug, Deserialize)]
pub struct CellInspectReq {
    pub lon: f64,
    pub lat: f64,
    #[serde(default)]
    pub profile: Option<turbo_tiles_graph::Profile>,
}

/// Wire shape of a cell inspection. `x_25833` / `y_25833` are named
/// here, at the boundary that actually knows the projection — the
/// engine no longer does (C4). The SPA reads both.
#[derive(Debug, Serialize)]
pub struct InspectPointView {
    pub lon: f64,
    pub lat: f64,
    pub x_25833: f64,
    pub y_25833: f64,
    pub composed_multiplier: f32,
    pub refused_by: Option<String>,
    pub layers: Vec<turbo_tiles_pathfind::InspectLayer>,
}

impl From<InspectPoint> for InspectPointView {
    fn from(p: InspectPoint) -> Self {
        let [lon, lat] = frame::lonlat(p.at);
        Self {
            lon,
            lat,
            x_25833: p.at.x,
            y_25833: p.at.y,
            composed_multiplier: p.composed_multiplier,
            refused_by: p.refused_by,
            layers: p.layers,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CellInspectResp {
    pub point: InspectPointView,
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
    let point = pf.inspect_point(frame::planar([req.lon, req.lat]), profile);
    Ok(Json(CellInspectResp {
        point: point.into(),
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
    let inspect = pf.inspect(frame::planar(req.from), frame::planar(req.to), &prefs);
    Ok(Json(InspectResp {
        inspect: inspect.into(),
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
    let (w, s) = turbo_geo_frame::utm33n_to_wgs84(min_x, min_y);
    let (e, n) = turbo_geo_frame::utm33n_to_wgs84(max_x, max_y);
    [w, s, e, n]
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use turbo_tiles_pathfind::{PhaseFrame, SolverEvent, SolverRecording};

    /// C4 guard on the projector itself.
    ///
    /// The engine used to project the recording; that moved here when
    /// the engine became planar-only. Both the type and the field name
    /// survive the move unchanged, so a projection that silently ships
    /// planar metres to a client plotting lon/lat would type-check.
    ///
    /// Scope, stated honestly: this pins what `project_recording` does,
    /// not that the handler calls it — verifying the wiring needs a
    /// full `ApiState` and a live solve. The unit check is the cheap
    /// half; the expensive half is the corpus gate.
    #[test]
    fn recording_is_projected_out_of_the_planar_frame() {
        // A Sjunkhatten-area planar point, in the corpus region.
        let (px, py) = (525_000.0f32, 7_450_000.0f32);
        let rec = SolverRecording {
            phases: vec![PhaseFrame {
                name: "test".into(),
                started_at_us: 0,
                events: vec![SolverEvent::NodePopped {
                    x: px,
                    y: py,
                    g: 1.0,
                    h: 2.0,
                }],
            }],
            decimated: false,
            events_observed: 1,
            events_retained: 1,
        };

        let out = project_recording(rec);
        let SolverEvent::NodePopped { x, y, .. } = out.phases[0].events[0] else {
            panic!("event kind must survive projection");
        };
        assert!(
            (5.0..30.0).contains(&x) && (55.0..75.0).contains(&y),
            "recording coordinates must leave as lon/lat degrees, not planar \
             metres; got ({x}, {y})"
        );
    }
}
