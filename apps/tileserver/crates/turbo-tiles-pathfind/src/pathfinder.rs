//! `Pathfinder` — the Stage 6 composition layer.
//!
//! Holds a routing graph and an ordered stack of `CostLayer`s.
//! Solves a from→to lon/lat query via three strategies:
//!
//! 1. **On-graph**: both endpoints snap to the graph within
//!    `snap_radius_m` → Dijkstra over the CSR graph, with layer-
//!    derived edge multipliers (premade-track preference, etc).
//! 2. **Hybrid**: at least one endpoint is too far from the graph
//!    to snap directly but a graph node sits within
//!    `bridge_radius_m`. Build a small off-trail mesh from each
//!    off-graph endpoint to its bridge node, route on the graph
//!    between bridges, stitch the three segments.
//! 3. **Off-trail**: neither endpoint has a usable bridge. Single
//!    Theta\* mesh spanning [from, to].
//!
//! Strategy 1 is tried first, then 2, then 3. The strategy
//! actually used is reported in `Path::strategy` so callers can
//! reason about confidence.
//!
//! Layers are evaluated at mesh-build time (per-cell cost samples
//! and refusal polygons) and at edge-relaxation time (per-edge
//! multipliers fed into Dijkstra). Per-request `layer_weights`
//! lets callers dial individual layers up/down without changes
//! to the binary.

use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;
use turbo_tiles_elev::{wgs84_to_utm33n, Dem, PointXY};
use turbo_tiles_graph::{Graph, NodePos, Profile, RouteResult};
use turbo_tiles_mask::Mask;

use crate::cost::{compose_cell, compose_edge, compose_mesh_edge, CostLayer};
use crate::core::off_trail::{theta_star, theta_star_with_edge_cost, Mesh, MeshNodeId, Point2};
use crate::core::off_trail_mesh::{
    build_local_mesh, CostSample, ExitNode, MeshBbox, MeshBuildInput, RefusedPolygon,
};
use crate::layers::{
    AvalancheTerrainLayer, DirectionalSlopeLayer, GraphSlopeLayer, MarkingLayer, MaskRefusalLayer,
    PreferredEdgeLayer, SlopeLayer, TotalGainLayer, TrailProximityLayer,
};

#[derive(Debug, Error)]
pub enum PathfindError {
    #[error("from/to too close: {dist_m:.1} m")]
    DegenerateInputs { dist_m: f64 },
    #[error("from/to bbox too large for off-trail mesh: {extent_km:.1} km")]
    BboxTooLarge { extent_km: f64 },
    #[error("no route found")]
    NoRoute,
    /// Endpoint(s) lie outside every loaded primitive's coverage and
    /// no graph anchor is reachable within `bridge_radius_m`. The
    /// API surface uses this to return 422 with a "where coverage
    /// does exist" hint instead of inventing a path.
    #[error("no terrain data at these coordinates")]
    NoCoverage {
        from_covered: bool,
        to_covered: bool,
        from_has_graph_anchor: bool,
        to_has_graph_anchor: bool,
    },
    /// One of the endpoints is itself in a refused region — typically
    /// the user clicked in a lake. Carries the layer name so the SPA
    /// can show "destination is on water" instead of "no route".
    #[error("{which} endpoint refused by layer '{layer}'")]
    EndpointRefused {
        which: &'static str, // "from" or "to"
        layer: String,
    },
    #[error("graph: {0}")]
    Graph(#[from] turbo_tiles_graph::GraphError),
    #[error("dem: {0}")]
    Dem(#[from] turbo_tiles_elev::DemError),
    /// Catch-all for solver-internal errors that aren't a clean
    /// "no route" but a misconfiguration or missing artifact. The
    /// FMM adapter uses this when the DEM is missing or the
    /// corridor extraction fails.
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathStrategy {
    OnGraph,
    OffTrail,
    Hybrid,
}

/// One leg of a hybrid path, useful for UIs that want to colour
/// graph segments differently from off-trail prefixes/suffixes.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegKind {
    OffTrailPrefix,
    Graph,
    OffTrailSuffix,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PathLeg {
    pub kind: LegKind,
    /// Index range into `Path::geometry` for this leg's vertices
    /// (start inclusive, end inclusive).
    pub start_idx: u32,
    pub end_idx: u32,
    pub length_m: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Path {
    pub strategy: PathStrategy,
    pub geometry: Vec<[f64; 2]>,
    pub distances_m: Vec<f64>,
    pub length_m: f64,
    pub cost: f64,
    /// Legacy "% on a graph edge". Computed against the graph leg's
    /// length over the total path length — confusing for callers
    /// because it counts roads as "trail". Prefer
    /// [`fkb_breakdown`] for a per-surface answer.
    pub on_trail_pct: f32,
    /// Metres of route by edge surface type. Keys are stable strings
    /// (`sti`, `vei`, `traktorvei`, `skogsvei`, `skiloype`,
    /// `off_trail`, `unknown`) matching the `fkb_type` encoded on
    /// the routing graph plus a synthetic `off_trail` bucket for
    /// metres spent on mesh segments rather than graph edges.
    /// Empty for trivial paths.
    pub fkb_breakdown: std::collections::BTreeMap<String, f64>,
    /// Per-leg breakdown for hybrid paths. Single-strategy paths
    /// emit a one-element list.
    pub legs: Vec<PathLeg>,
    /// Names of layers that *forbade* one or more cells along the
    /// computed corridor (best-effort; populated only for off-trail
    /// or hybrid strategies). Empty for `on_graph`.
    pub refused_by: Vec<String>,
    /// Per-request trace, present only when `Prefs::debug = true`.
    /// Includes per-layer call counts + timing, phase timing, and
    /// mesh stats.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<crate::tracer::TraceSnapshot>,
    /// Per-event solver recording, present only when
    /// `Prefs::record = true`. Drives the SPA's algorithm-replay
    /// animation: explored set, frontier, line-of-sight rays,
    /// emerging best path. See `crate::solver_trace`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording: Option<crate::solver_trace::SolverRecording>,
}

/// Bridge from Dijkstra's low-level event stream into the
/// solver-trace recorder. Translates `DijkstraEvent` (defined in
/// the graph crate to avoid a pathfind dependency) into
/// `solver_trace::SolverEvent` and pushes to the thread-local
/// recorder. Coordinates stay in UTM here; `serialise_recording`
/// projects them all to WGS84 once at the end.
fn graph_observer(ev: turbo_tiles_graph::DijkstraEvent) {
    use turbo_tiles_graph::DijkstraEvent;
    match ev {
        DijkstraEvent::NodePopped { x, y, g } => {
            crate::solver_trace::record(|| crate::solver_trace::SolverEvent::NodePopped {
                x,
                y,
                g,
                h: 0.0,
            });
        }
        DijkstraEvent::EdgeRelaxed { fx, fy, tx, ty, new_g } => {
            crate::solver_trace::record(|| crate::solver_trace::SolverEvent::EdgeRelaxed {
                fx,
                fy,
                tx,
                ty,
                new_g,
                took_los: false,
            });
        }
    }
}

/// Convert the recorder's UTM33N coordinates to WGS84 for the
/// SPA. Coordinates are stored as `f32` metres at record time
/// because the hot Theta* loop is dense and a per-event projection
/// would hurt; the projection runs once here on the snapshot.
fn serialise_recording(
    mut rec: crate::solver_trace::SolverRecording,
) -> crate::solver_trace::SolverRecording {
    use crate::solver_trace::SolverEvent;
    let project = |x: f32, y: f32| -> [f32; 2] {
        let (lon, lat) = utm33n_to_wgs84(x as f64, y as f64);
        [lon as f32, lat as f32]
    };
    for phase in &mut rec.phases {
        for ev in &mut phase.events {
            match ev {
                SolverEvent::NodePopped { x, y, .. } => {
                    let p = project(*x, *y);
                    *x = p[0];
                    *y = p[1];
                }
                SolverEvent::EdgeRelaxed { fx, fy, tx, ty, .. } => {
                    let pa = project(*fx, *fy);
                    let pb = project(*tx, *ty);
                    *fx = pa[0];
                    *fy = pa[1];
                    *tx = pb[0];
                    *ty = pb[1];
                }
                SolverEvent::LineOfSightCast { fx, fy, tx, ty, .. } => {
                    let pa = project(*fx, *fy);
                    let pb = project(*tx, *ty);
                    *fx = pa[0];
                    *fy = pa[1];
                    *tx = pb[0];
                    *ty = pb[1];
                }
                SolverEvent::BestPathSnapshot { coords } => {
                    for c in coords.iter_mut() {
                        let p = project(c[0], c[1]);
                        *c = p;
                    }
                }
                SolverEvent::MeshBuilt { .. } => {}
            }
        }
    }
    rec
}

/// Map the `fkb_type` u8 code (as baked into `EdgeRecord` by
/// `graph_builder::encode_fkb_type`) to a stable string key for the
/// breakdown. Keep in lockstep with the encoder — `sti`, `vei`,
/// `skiloype` are the only codes the current builder emits.
fn fkb_code_to_str(code: u8) -> &'static str {
    match code {
        1 => "sti",
        2 => "vei",
        3 => "skiloype",
        _ => "unknown",
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Prefs {
    /// Max metres from query endpoint to snap to the graph for an
    /// on-graph route. Below this radius we trust the snap.
    #[serde(default = "default_snap_radius_m")]
    pub snap_radius_m: f32,
    /// Max metres from query endpoint to find a hybrid bridge node
    /// when direct snap fails. Above this, the endpoint is treated
    /// as fully off-graph.
    #[serde(default = "default_bridge_radius_m")]
    pub bridge_radius_m: f32,
    /// Profile for the graph Dijkstra leg.
    #[serde(default = "default_profile")]
    pub profile: Profile,
    /// Off-trail mesh cell size in metres.
    #[serde(default = "default_mesh_cell_m")]
    pub mesh_cell_m: f64,
    /// Maximum end-to-end bbox extent for off-trail mesh queries (km).
    #[serde(default = "default_max_off_trail_km")]
    pub max_off_trail_km: f64,
    /// If false and on-graph fails, return NoRoute. Used by clients
    /// that explicitly want to refuse off-trail suggestions.
    #[serde(default = "default_allow_off_trail")]
    pub allow_off_trail: bool,
    /// Skip the graph leg entirely and route via the off-trail
    /// mesh only. Used by the trail-mimicry harness to evaluate
    /// what the mesh would do if graph snaps weren't available.
    /// `snap_radius_m=0` / `bridge_radius_m=0` is *not* sufficient
    /// for this — endpoints sitting exactly on a graph node still
    /// snap at zero distance. This flag is the explicit override.
    #[serde(default)]
    pub force_off_trail: bool,
    /// Extra metres added around the [from, to] bbox when building
    /// the off-trail mesh. Drives how far the solver may detour
    /// around obstacles. Default scales with route distance — see
    /// `effective_mesh_pad_m`.
    ///
    /// `None` means "auto"; explicit values override the auto rule.
    #[serde(default)]
    pub mesh_pad_m: Option<f64>,
    /// When an endpoint lands in a refused cell (typical of a click
    /// just inside a < 100 m water sliver), snap outward to the
    /// nearest passable cell within this radius before failing.
    /// 0 = strict refusal (old behaviour).
    #[serde(default = "default_refusal_snap_m")]
    pub refusal_snap_m: f64,
    /// Per-layer weight overrides keyed by layer name. Layers not
    /// listed default to 1.0; 0.0 disables. See `cost::compose_cell`
    /// for how the weight applies to the layer's deviation from 1.0.
    #[serde(default)]
    pub layer_weights: HashMap<String, f32>,
    /// Capture a per-layer + per-phase trace and return it in the
    /// response's `debug` field. Off by default — instrumentation
    /// adds a few percent overhead even when most layer calls are
    /// cheap, and the response payload grows.
    #[serde(default)]
    pub debug: bool,
    /// Record per-event solver exploration (every node pop, every
    /// edge relaxation, every line-of-sight cast, the final best
    /// path snapshot). The recording lands in `Path::recording`
    /// for the SPA to animate. Off by default — adds ~10% CPU
    /// during the solve and several hundred KB to the response.
    #[serde(default)]
    pub record: bool,
    /// Cap on the number of recorded events. Beyond this, the
    /// recorder decimates (keeps 1 in N) so a 5 km Marka solve
    /// doesn't blow up the response. Defaults to 200_000.
    #[serde(default = "default_record_cap")]
    pub record_cap: u64,
    /// Sparse per-request cost-config patch. Each field is
    /// optional; unset values inherit from the Pathfinder's boot
    /// config. Use this to A/B the same query with different knob
    /// values without restarting the server.
    #[serde(default)]
    pub cost_config_override: Option<crate::config::CostConfigPatch>,
    /// How to compose layer costs inside the solver. Default keeps
    /// the legacy multiplicative behavior the scenario corpus is
    /// calibrated against. `WalkSeconds` flips both the graph
    /// Dijkstra and the off-trail Theta* to additive composition
    /// via [`crate::contributor::compose_edge_walk_seconds`] —
    /// each layer contributes seconds (positive or negative) and
    /// edge cost equals base traversal time plus the sum of
    /// contributions. The Pathfinder's
    /// [`crate::Pathfinder::contributors_for_breakdown`] is reused
    /// so native physical contributors (slope/marking/water) are
    /// used where available, with [`crate::contributor::
    /// LegacyLayerAdapter`] filling in the rest.
    #[serde(default)]
    pub cost_mode: CostMode,
    /// Off-trail base multiplier applied to every mesh edge inside
    /// the off-trail solver. Higher = mesh routes look more
    /// expensive vs trail/road alternatives. `None` (default) reads
    /// the per-profile value from the cost config (foot=1.7,
    /// bicycle=2.5, ski=1.0 with the embedded defaults). Override
    /// per request when the curator wants a stronger or weaker
    /// preference for trails for one specific query.
    ///
    /// Calibration notes:
    /// - 1.0: off-trail walking costs the same as trail walking.
    ///   The shortest geometric path always wins. Causes Q1-style
    ///   straight-line cuts through forest when a curved trail
    ///   exists.
    /// - 1.5: matches the empirical (Tobler) ratio for open ground
    ///   vs flat trail. Fixes Q1 but Q3-style Marka case still
    ///   prefers mesh by ~8%.
    /// - 1.7 (default): Q3 flips to trail-mostly hybrid; Q4-style
    ///   "trail goes the long way" still uses mesh because the
    ///   trail alternative is 2× longer.
    /// - 2.0+: aggressive trail preference. Use for queries where
    ///   the operator strongly trusts the trail network.
    #[serde(default)]
    pub off_trail_base: Option<f64>,
}

fn default_snap_radius_m() -> f32 {
    200.0
}
fn default_bridge_radius_m() -> f32 {
    3_000.0
}
fn default_profile() -> Profile {
    Profile::Foot
}
fn default_mesh_cell_m() -> f64 {
    // 25 m gives ~16× finer detail than the old 100 m, enough for
    // the path to bend along contours and around small obstacles.
    // For a 5 km query the mesh is ~40 k cells (was ~10 k); solve
    // time goes from ~50 ms to ~200–500 ms — acceptable for an
    // interactive Plot UX.
    25.0
}
fn default_max_off_trail_km() -> f64 {
    10.0
}
fn default_allow_off_trail() -> bool {
    true
}
fn default_record_cap() -> u64 {
    // 200 K events is enough for a 5 km Marka query at full
    // fidelity; beyond that the adaptive decimation in
    // `solver_trace::Recorder` keeps the recording bounded.
    200_000
}
fn default_refusal_snap_m() -> f64 {
    150.0
}

/// Compute the bbox padding to use for an off-trail mesh build,
/// given a straight-line distance and an optional user override.
///
/// Without enough padding the solver can't detour around large
/// refused regions — the corridor degenerates to a thin strip
/// between the two clicks. Default rule: pad ≥ 4 mesh cells *and*
/// ≥ 30% of the straight-line distance. So a 5 km query gets
/// ~1.5 km of padding on each side, enough to detour around a
/// 1 km-wide lake.
pub(crate) fn effective_mesh_pad_m(override_m: Option<f64>, cell_m: f64, dist_m: f64) -> f64 {
    if let Some(v) = override_m {
        return v.max(cell_m);
    }
    (cell_m * 4.0).max(dist_m * 0.30)
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            snap_radius_m: default_snap_radius_m(),
            bridge_radius_m: default_bridge_radius_m(),
            profile: default_profile(),
            mesh_cell_m: default_mesh_cell_m(),
            max_off_trail_km: default_max_off_trail_km(),
            allow_off_trail: default_allow_off_trail(),
            force_off_trail: false,
            mesh_pad_m: None,
            refusal_snap_m: default_refusal_snap_m(),
            layer_weights: HashMap::new(),
            debug: false,
            record: false,
            record_cap: default_record_cap(),
            off_trail_base: None,
            cost_config_override: None,
            cost_mode: CostMode::default(),
        }
    }
}

/// Cost-composition mode for the solver loops. Toggled per request
/// via [`Prefs::cost_mode`]. See `Prefs::cost_mode` doc for the
/// behavioural contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostMode {
    /// Multiplicative composition via `compose_cell` / `compose_edge`
    /// / `compose_mesh_edge`. Pre-Stage-2 default; kept for A/B
    /// comparisons and as an escape valve while the remaining
    /// legacy layers are ported to native CostContributor impls.
    Multiplicative,
    /// Additive composition via `compose_edge_walk_seconds`. Native
    /// physical contributors (slope/marking/water) replace their
    /// legacy multiplicative equivalents; remaining layers fall
    /// back to `LegacyLayerAdapter`. Edge cost is base walk-seconds
    /// plus the sum of contributions, in real physical units.
    /// Kept as the Theta\*-based escape valve when the FMM corridor
    /// can't be sized (degenerate from→to, no DEM coverage).
    WalkSeconds,
    /// Fast Marching Method off-trail solver. Replaces the Theta\*
    /// search with an eikonal-PDE solve on a regular 10 m grid
    /// over a corridor around the from-to centerline; the path is
    /// then extracted by gradient descent on the arrival-time
    /// field and Chaikin-smoothed.
    ///
    /// Anisotropic Tobler-Finsler metric (phase 5) — Selling-reduced
    /// per-cell norm forms drive an AGSI stencil that produces
    /// contour-following geodesics on slopes. Production default
    /// from phase 8 onward.
    #[default]
    FastMarching,
}

pub struct Pathfinder {
    pub graph: Option<Arc<Graph>>,
    /// Legacy multiplicative cost layers. Kept for the build-time
    /// mesh refusal sampler (`compose_cell`), the inspect endpoint
    /// (per-layer point breakdown), and the `Multiplicative` cost
    /// mode escape valve. The production solver path runs on
    /// [`Self::native_contributors`] instead.
    pub layers: Vec<Arc<dyn CostLayer>>,
    /// Native cost contributors in walk-seconds (Stage 2 unified
    /// unit). Populated alongside `layers` at boot — every legacy
    /// layer pushed via [`Self::push_with_native`] also registers
    /// its native equivalent here, so when the solver runs in
    /// `WalkSeconds` mode (the production default) it composes
    /// physical-unit contributions without ever touching
    /// `LegacyLayerAdapter`.
    pub native_contributors: Vec<Arc<dyn crate::contributor::CostContributor>>,
    /// Boot-time cost config. The off-trail solver reads
    /// `off_trail_base` from here when no per-request override is
    /// set; future layers will read their own knobs the same way.
    /// Stored by value (cheap clone — a few f64 per knob group)
    /// so per-request overrides can produce an effective config
    /// without disturbing the boot config.
    pub cost_config: crate::config::CostConfig,
    /// Primitive handles kept around for the breakdown / inspect
    /// paths and for boot wiring that needs to defer water from
    /// the raster mask to the vector water layer at request time.
    pub dem: Option<Arc<Dem>>,
    pub mask: Option<Arc<Mask>>,
}

impl Pathfinder {
    pub fn new(graph: Option<Arc<Graph>>, layers: Vec<Arc<dyn CostLayer>>) -> Self {
        Self {
            graph,
            layers,
            native_contributors: Vec::new(),
            cost_config: crate::config::CostConfig::from_embedded()
                .expect("embedded cost-config defaults must parse"),
            dem: None,
            mask: None,
        }
    }

    /// Convenience constructor that assembles the default layer
    /// stack from whichever primitive artifacts are loaded:
    ///   slope (if DEM present), mask_refusal (if mask present),
    ///   trail_proximity (if graph present), preferred_edge, marking.
    /// Equivalent to writing the boot wiring by hand.
    pub fn with_defaults(
        dem: Option<Arc<Dem>>,
        mask: Option<Arc<Mask>>,
        graph: Option<Arc<Graph>>,
    ) -> Self {
        Self::with_defaults_and_config(
            dem,
            mask,
            graph,
            crate::config::CostConfig::from_embedded()
                .expect("embedded cost-config defaults must parse"),
        )
    }

    /// Same as [`with_defaults`] but takes an explicit
    /// [`CostConfig`]. Boot wiring in `tileserver-bin` calls this
    /// directly with the config resolved from disk / env / embedded
    /// defaults so each layer reads its physical knobs from one
    /// place instead of the scattered hardcoded values that
    /// produced the multi-knob calibration drift documented in
    /// this codebase's session notes.
    pub fn with_defaults_and_config(
        dem: Option<Arc<Dem>>,
        mask: Option<Arc<Mask>>,
        graph: Option<Arc<Graph>>,
        cost_config: crate::config::CostConfig,
    ) -> Self {
        use crate::native_contributors::{
            AvalancheTerrainContributor, ContourCrossingContributor, DemCoveragePenaltyContributor,
            GraphSlopeContributor, MarkingBonusContributor, MaskRefusalContributor,
            NaismithGainContributor, PreferredEdgeContributor, ToblerSlopeContributor,
            TotalGainContributor, TrailProximityContributor,
        };
        let dem_for_breakdown = dem.clone();
        let mask_for_breakdown = mask.clone();
        let mut layers: Vec<Arc<dyn CostLayer>> = Vec::new();
        let mut natives: Vec<Arc<dyn crate::contributor::CostContributor>> = Vec::new();
        if let Some(d) = dem.as_ref() {
            // Three DEM-driven layers; SlopeLayer reads its scale
            // + refusal threshold from cost_config.slope_cell.
            layers.push(Arc::new(SlopeLayer::with_knobs(
                d.clone(),
                cost_config.slope_cell.quadratic_scale_deg,
                cost_config.slope_cell.refuse_above_deg,
            )));
            natives.push(Arc::new(ToblerSlopeContributor::new(
                d.clone(),
                cost_config.slope_cell.refuse_above_deg,
            )));
            // Naismith's gain term — mesh edges pay extra time per
            // metre of vertical gain. Without this, Tobler alone
            // under-penalises moderate climbs (10–15°), so a long
            // LoS jump straight up a mountainside out-priced any
            // switchback alternative.
            natives.push(Arc::new(NaismithGainContributor::new(d.clone())));
            // DEM-coverage penalty: when a long LoS jump crosses
            // tiles the DEM lacks, Tobler + Naismith silently
            // report zero contribution because they don't know
            // the slope. This contributor charges per metre of
            // nodata so the solver detours around known DEM gaps
            // when an alternative exists. Norway has ~6k absent
            // tiles concentrated in alpine areas (Jotunheimen,
            // Sognefjell), so this matters.
            natives.push(Arc::new(DemCoveragePenaltyContributor::new(d.clone())));
            // Cross-contour penalty — charges edges that cut
            // across slopes rather than following them. This is
            // what stops Theta* paths from looking "low-poly":
            // two equidistant + equi-net-gain edges (one along a
            // contour, one across a ridge) now have very different
            // costs. Without this the cost model couldn't tell
            // them apart, and the solver picked whichever was
            // geometrically shorter — which is usually the
            // cross-contour cheat.
            natives.push(Arc::new(ContourCrossingContributor::new(d.clone())));
            // DirectionalSlope subsumed by the integrated Tobler
            // contributor above — it samples N+1 points along each
            // edge and uses signed slope per sub-segment, so the
            // separate midpoint-only directional layer would only
            // double-count slope cost. Keep the legacy layer in
            // `layers` for the inspect endpoint (per-layer debug
            // view) but drop it from the native cost stack.
            layers.push(Arc::new(DirectionalSlopeLayer::new(d.clone())));
            layers.push(Arc::new(AvalancheTerrainLayer::new(d.clone())));
            natives.push(Arc::new(AvalancheTerrainContributor::new(d.clone())));
        }
        if let Some(m) = mask.as_ref() {
            layers.push(Arc::new(MaskRefusalLayer::new(m.clone())));
            natives.push(Arc::new(MaskRefusalContributor::new(m.clone())));
        }
        if let Some(g) = graph.as_ref() {
            layers.push(Arc::new(TrailProximityLayer::new(
                g.as_ref(),
                cost_config.trail_proximity.influence_radius_m as f64,
                cost_config.trail_proximity.bonus_at_zero,
            )));
            natives.push(Arc::new(TrailProximityContributor::new(
                g.as_ref(),
                cost_config.trail_proximity.influence_radius_m as f64,
                cost_config.trail_proximity.bonus_at_zero,
            )));
        }
        layers.push(Arc::new(PreferredEdgeLayer::default()));
        natives.push(Arc::new(PreferredEdgeContributor::default()));
        layers.push(Arc::new(MarkingLayer::default()));
        natives.push(Arc::new(MarkingBonusContributor::default()));
        layers.push(Arc::new(GraphSlopeLayer {
            quadratic_scale_deg: cost_config.slope_graph.quadratic_scale_deg,
            refuse_above_deg: cost_config.slope_graph.refuse_above_deg,
        }));
        natives.push(Arc::new(GraphSlopeContributor {
            quadratic_scale_deg: cost_config.slope_graph.quadratic_scale_deg,
            refuse_above_deg: cost_config.slope_graph.refuse_above_deg,
        }));
        layers.push(Arc::new(TotalGainLayer {
            gain_amplifier: cost_config.total_gain.amplifier,
        }));
        natives.push(Arc::new(TotalGainContributor {
            gain_amplifier: cost_config.total_gain.amplifier,
        }));
        Self {
            graph,
            layers,
            native_contributors: natives,
            cost_config,
            dem: dem_for_breakdown,
            mask: mask_for_breakdown,
        }
    }

    /// Push a paired (legacy, native) cost source. The legacy
    /// representation keeps the build-time mesh-refusal sampler and
    /// the inspect endpoint working; the native is what the solver
    /// uses for cost composition in `WalkSeconds` mode. Boot wiring
    /// for vector + landcover layers uses this so both halves stay
    /// in lockstep without each call site having to remember the
    /// two pushes.
    pub fn push_with_native(
        &mut self,
        legacy: Arc<dyn CostLayer>,
        native: Arc<dyn crate::contributor::CostContributor>,
    ) {
        self.layers.push(legacy);
        self.native_contributors.push(native);
    }

    /// Append a custom layer at runtime. Layers added later compose
    /// after the built-ins.
    pub fn push_layer(&mut self, layer: Arc<dyn CostLayer>) {
        self.layers.push(layer);
    }

    /// Switch the rasterised `mask_refusal` layer (if present) into
    /// "defer water to vector" mode. Used when a vector `water`
    /// integral layer supersedes the 25 m water bitmap — without
    /// this call, both layers fire and the bitmap reintroduces the
    /// "5 m tarn = 100 m halo" pathology we're trying to remove.
    /// Glaciers + other refusal kinds in the raster mask remain
    /// active.
    ///
    /// Takes the [`Mask`] from the caller (the original `Arc<Mask>`
    /// is also held by `api_state.mask`) so we don't need to
    /// downcast `Arc<dyn CostLayer>` to recover it.
    pub fn defer_mask_water_to_vector(&mut self, mask: Arc<Mask>) {
        for i in 0..self.layers.len() {
            if self.layers[i].name() == "mask_refusal" {
                self.layers[i] = Arc::new(
                    crate::layers::MaskRefusalLayer::new(mask.clone()).deferring_water(),
                );
                break;
            }
        }
        // Keep the native side in lockstep so the solver's walk-
        // seconds path sees the same "water now belongs to the
        // vector layer" decision.
        for i in 0..self.native_contributors.len() {
            if self.native_contributors[i].name() == "mask_refusal" {
                self.native_contributors[i] = Arc::new(
                    crate::native_contributors::MaskRefusalContributor::new(mask.clone())
                        .deferring_water(),
                );
                return;
            }
        }
    }

    /// List enabled layer names — useful for the admin UI.
    pub fn layer_names(&self) -> Vec<&'static str> {
        self.layers.iter().map(|l| l.name()).collect()
    }

    /// True if at least one registered layer claims authoritative
    /// data at this EPSG:25833 point. Used by the no-coverage
    /// pre-check in `solve()`.
    pub fn point_covered(&self, x: f64, y: f64) -> bool {
        self.layers.iter().any(|l| l.covers(x, y))
    }

    fn has_graph_anchor(&self, x: f64, y: f64, radius_m: f32) -> bool {
        self.graph
            .as_ref()
            .map(|g| g.snap(x, y, radius_m).is_ok())
            .unwrap_or(false)
    }

    /// Try to nudge each endpoint outward to the nearest passable
    /// cell within `prefs.refusal_snap_m`. The click UX otherwise
    /// fails on mask cells smaller than the user's precision —
    /// e.g. a 30 m water sliver under a mountain summit pin. If no
    /// passable cell exists within the radius, the original point
    /// is returned unchanged and the strict refusal check kicks in.
    ///
    /// Search pattern: 16 directions × N rings stepping outward by
    /// half a mesh cell. Cheap (≤256 point checks) and bounded so
    /// it can't lock the solver into a spiral.
    fn snap_endpoints_out_of_refusal(
        &self,
        from: PointXY,
        to: PointXY,
        prefs: &Prefs,
    ) -> (PointXY, PointXY) {
        let from = self.snap_one_out_of_refusal(from, prefs);
        let to = self.snap_one_out_of_refusal(to, prefs);
        (from, to)
    }

    fn snap_one_out_of_refusal(&self, p: PointXY, prefs: &Prefs) -> PointXY {
        if prefs.refusal_snap_m <= 0.0 {
            return p;
        }
        if !self.point_is_refused(p.x, p.y, prefs) {
            return p;
        }
        let step = (prefs.mesh_cell_m * 0.5).max(10.0);
        let max_r = prefs.refusal_snap_m;
        let mut r = step;
        while r <= max_r {
            // 16 evenly-spaced directions on this ring.
            for i in 0..16 {
                let theta = (i as f64) * std::f64::consts::TAU / 16.0;
                let cand = PointXY {
                    x: p.x + r * theta.cos(),
                    y: p.y + r * theta.sin(),
                };
                if !self.point_is_refused(cand.x, cand.y, prefs) {
                    return cand;
                }
            }
            r += step;
        }
        p
    }

    fn point_is_refused(&self, x: f64, y: f64, prefs: &Prefs) -> bool {
        for layer in &self.layers {
            if layer.cell_cost(x, y, prefs.profile).refused.is_some() {
                return true;
            }
        }
        false
    }

    /// Return `Some((which, layer_name))` if either endpoint is in a
    /// refused region. Only triggers when the endpoint is *off the
    /// graph* — if the user snapped to a real graph node, the graph
    /// route can step around the refusal without us needing to veto
    /// the request.
    fn endpoint_refused(
        &self,
        from_xy: PointXY,
        to_xy: PointXY,
        prefs: &Prefs,
    ) -> Option<(&'static str, String)> {
        let layers = self.layers.as_slice();
        let check = |x: f64, y: f64| -> Option<String> {
            for layer in layers {
                let c = layer.cell_cost(x, y, prefs.profile);
                if c.refused.is_some() {
                    return Some(layer.name().to_string());
                }
            }
            None
        };
        // Skip the refusal check at an endpoint that already snaps —
        // the graph leg will route around the refused region.
        if !self.has_graph_anchor(from_xy.x, from_xy.y, prefs.snap_radius_m) {
            if let Some(layer) = check(from_xy.x, from_xy.y) {
                return Some(("from", layer));
            }
        }
        if !self.has_graph_anchor(to_xy.x, to_xy.y, prefs.snap_radius_m) {
            if let Some(layer) = check(to_xy.x, to_xy.y) {
                return Some(("to", layer));
            }
        }
        None
    }

    /// Per-edge cost breakdown in walk-seconds (the new unified
    /// unit). Treats the (from, to) input as a single mesh-style
    /// edge — answers "if the solver was deciding whether to
    /// traverse this edge, how much would each contributor add to
    /// the cost?" Returns base traversal time + a per-contributor
    /// list + grand total. Used by `/v1/debug/cost-breakdown` to
    /// give the curator a physical-unit explanation of routing
    /// decisions, replacing the multiplicative-hairball debugging
    /// that's been required so far.
    ///
    /// The breakdown uses [`LegacyLayerAdapter`] to translate the
    /// existing `CostLayer` impls into walk-seconds. Once layers
    /// migrate to the new [`crate::contributor::CostContributor`]
    /// trait this will report their native walk-seconds directly.
    pub fn cost_breakdown(
        &self,
        from_lonlat: [f64; 2],
        to_lonlat: [f64; 2],
        profile: Profile,
    ) -> crate::contributor::EdgeWalkCost {
        let from = wgs84_to_utm33n(from_lonlat[0], from_lonlat[1]);
        let to = wgs84_to_utm33n(to_lonlat[0], to_lonlat[1]);
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let length_m = (dx * dx + dy * dy).sqrt();
        let ctx = crate::contributor::EdgeContext {
            fx: from.x,
            fy: from.y,
            tx: to.x,
            ty: to.y,
            length_m,
            profile,
            kind: crate::contributor::EdgeKind::Mesh,
        };
        let contributors = self.contributors_for_breakdown();
        crate::contributor::compose_edge_walk_seconds(&contributors, &ctx)
    }

    /// Contributor list used by the solver + breakdown. Returns
    /// `native_contributors` if any are registered (the production
    /// path: every layer pushed via `push_with_native` or
    /// `with_defaults_and_config` has a paired native impl, so the
    /// list is complete). When `native_contributors` is empty
    /// (downstream consumers of `Pathfinder::new` that haven't
    /// migrated) we fall back to wrapping every legacy layer in
    /// [`crate::contributor::LegacyLayerAdapter`] as a last resort
    /// so the breakdown still has data to show.
    pub fn contributors_for_breakdown(
        &self,
    ) -> Vec<Arc<dyn crate::contributor::CostContributor>> {
        if !self.native_contributors.is_empty() {
            return self.native_contributors.clone();
        }
        self.layers
            .iter()
            .map(|l| {
                Arc::new(crate::contributor::LegacyLayerAdapter::new(l.clone()))
                    as Arc<dyn crate::contributor::CostContributor>
            })
            .collect()
    }

    /// Per-point debug: for one (lon, lat), ask every layer what it
    /// thinks. Powers the SPA's click-a-cell-to-inspect UX — the
    /// user wants to understand *why* a cell is red, not just see
    /// that it is.
    pub fn inspect_point(
        &self,
        lon: f64,
        lat: f64,
        profile: Profile,
    ) -> InspectPoint {
        let p = wgs84_to_utm33n(lon, lat);
        let mut layers = Vec::with_capacity(self.layers.len());
        let mut composed = 1.0f32;
        let mut refused_by: Option<String> = None;
        for layer in &self.layers {
            let c = layer.cell_cost(p.x, p.y, profile);
            let layer_refused = c.refused.map(|r| r.to_string());
            if refused_by.is_none() {
                if let Some(ref r) = layer_refused {
                    refused_by = Some(format!("{}:{}", layer.name(), r));
                }
            }
            layers.push(InspectLayer {
                name: layer.name().to_string(),
                multiplier: c.multiplier,
                refused: layer_refused,
                covers: layer.covers(p.x, p.y),
            });
            if c.refused.is_none() {
                composed *= c.multiplier;
            }
        }
        InspectPoint {
            lon,
            lat,
            x_25833: p.x,
            y_25833: p.y,
            composed_multiplier: composed,
            refused_by,
            layers,
        }
    }

    /// Inspect call used by `/v1/debug/pathfind/inspect`. Builds the
    /// same off-trail mesh the solver would, but returns the cells
    /// (cost samples + refused polygons) instead of running Theta\*.
    /// Lets the admin UI visualise *why* the solver did what it did.
    pub fn inspect(
        &self,
        from_lonlat: [f64; 2],
        to_lonlat: [f64; 2],
        prefs: &Prefs,
    ) -> Inspect {
        let from_xy = wgs84_to_utm33n(from_lonlat[0], from_lonlat[1]);
        let to_xy = wgs84_to_utm33n(to_lonlat[0], to_lonlat[1]);
        // Inspect with the same effective padding the solver would
        // use so the overlay matches what the solver actually sees.
        let dx = to_xy.x - from_xy.x;
        let dy = to_xy.y - from_xy.y;
        let dist_m = (dx * dx + dy * dy).sqrt();
        let pad_m = effective_mesh_pad_m(prefs.mesh_pad_m, prefs.mesh_cell_m, dist_m);
        let bbox = MeshBbox {
            min_x: from_xy.x.min(to_xy.x) - pad_m,
            max_x: from_xy.x.max(to_xy.x) + pad_m,
            min_y: from_xy.y.min(to_xy.y) - pad_m,
            max_y: from_xy.y.max(to_xy.y) + pad_m,
        };
        let (samples, refused, refused_by) = self.mesh_inputs_for_bbox(
            bbox,
            prefs.mesh_cell_m,
            prefs.profile,
            &prefs.layer_weights,
        );
        let cells: Vec<InspectCell> = samples
            .into_iter()
            .map(|s| {
                let (lon, lat) = utm33n_to_wgs84(s.at.x, s.at.y);
                InspectCell {
                    lon,
                    lat,
                    cost_mul: s.cost_mul as f32,
                }
            })
            .collect();
        let refused_polys: Vec<Vec<[f64; 2]>> = refused
            .into_iter()
            .map(|rp| {
                rp.ring
                    .into_iter()
                    .map(|p| {
                        let (lon, lat) = utm33n_to_wgs84(p.x, p.y);
                        [lon, lat]
                    })
                    .collect()
            })
            .collect();
        let snap_from = self
            .graph
            .as_ref()
            .and_then(|g| g.snap(from_xy.x, from_xy.y, prefs.bridge_radius_m).ok())
            .and_then(|n| self.graph.as_ref().and_then(|g| g.node(n)))
            .map(|n| {
                let (lon, lat) = utm33n_to_wgs84(n.x as f64, n.y as f64);
                [lon, lat]
            });
        let snap_to = self
            .graph
            .as_ref()
            .and_then(|g| g.snap(to_xy.x, to_xy.y, prefs.bridge_radius_m).ok())
            .and_then(|n| self.graph.as_ref().and_then(|g| g.node(n)))
            .map(|n| {
                let (lon, lat) = utm33n_to_wgs84(n.x as f64, n.y as f64);
                [lon, lat]
            });
        Inspect {
            mesh_cell_m: prefs.mesh_cell_m,
            cells,
            refused_polygons: refused_polys,
            refused_by,
            nearest_graph_node_from: snap_from,
            nearest_graph_node_to: snap_to,
        }
    }

    pub fn solve(
        &self,
        from_lonlat: [f64; 2],
        to_lonlat: [f64; 2],
        prefs: Prefs,
    ) -> Result<Path, PathfindError> {
        // Recorder installed first so solver events from inside
        // tracer phases pick it up. Both thread-locals are
        // restored on drop in LIFO order.
        let recorder = prefs
            .record
            .then(|| Arc::new(crate::solver_trace::Recorder::new(prefs.record_cap)));
        let tracer = prefs
            .debug
            .then(|| Arc::new(crate::tracer::Tracer::new()));

        let run = |inner_prefs: Prefs| -> Result<Path, PathfindError> {
            self.solve_inner(from_lonlat, to_lonlat, inner_prefs)
        };
        let with_tracer = |inner_prefs: Prefs| -> Result<Path, PathfindError> {
            match tracer.clone() {
                Some(t) => crate::tracer::with_installed(t, || run(inner_prefs)),
                None => run(inner_prefs),
            }
        };
        let result = match recorder.clone() {
            Some(r) => crate::solver_trace::with_installed(r, || with_tracer(prefs.clone())),
            None => with_tracer(prefs.clone()),
        };

        result.map(|mut p| {
            if let Some(t) = tracer.as_ref() {
                p.debug = Some(t.snapshot());
            }
            if let Some(r) = recorder.as_ref() {
                p.recording = Some(serialise_recording(r.snapshot()));
            }
            p
        })
    }

    fn solve_inner(
        &self,
        from_lonlat: [f64; 2],
        to_lonlat: [f64; 2],
        prefs: Prefs,
    ) -> Result<Path, PathfindError> {
        let from_xy = wgs84_to_utm33n(from_lonlat[0], from_lonlat[1]);
        let to_xy = wgs84_to_utm33n(to_lonlat[0], to_lonlat[1]);
        let dx = to_xy.x - from_xy.x;
        let dy = to_xy.y - from_xy.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < prefs.mesh_cell_m {
            return Err(PathfindError::DegenerateInputs { dist_m: dist });
        }

        // Coverage pre-check: refuse early when there is genuinely
        // nothing for the solver to work with. Without this guard,
        // the off-trail solver builds a uniform-cost mesh between
        // the two points and Theta* returns a straight line —
        // semantically a lie. Honest failure beats silent nonsense.
        let from_covered = self.point_covered(from_xy.x, from_xy.y);
        let to_covered = self.point_covered(to_xy.x, to_xy.y);
        let from_graph =
            self.has_graph_anchor(from_xy.x, from_xy.y, prefs.bridge_radius_m);
        let to_graph =
            self.has_graph_anchor(to_xy.x, to_xy.y, prefs.bridge_radius_m);
        if !from_covered && !to_covered && !from_graph && !to_graph {
            return Err(PathfindError::NoCoverage {
                from_covered,
                to_covered,
                from_has_graph_anchor: from_graph,
                to_has_graph_anchor: to_graph,
            });
        }

        // Endpoint refusal: if an endpoint is itself in a refused
        // region (e.g. the user clicked in a lake), Theta\* later
        // returns the opaque NoRoute. First try to snap the click
        // outward to the nearest passable cell within
        // `refusal_snap_m` — accommodates the typical case of a
        // click that lands a few metres inside a water sliver.
        let (from_xy, to_xy) = self.snap_endpoints_out_of_refusal(from_xy, to_xy, &prefs);
        if let Some((which, layer)) = self.endpoint_refused(from_xy, to_xy, &prefs) {
            return Err(PathfindError::EndpointRefused { which, layer });
        }

        // Compute every strategy that's viable, then pick the
        // cheapest by *cost* (cost-weighted effective walking
        // metres — common units across all three strategies). The
        // alternative (precedence-based: take the first viable)
        // produces nonsense like a 62 km road detour when a 6 km
        // cross-country path is the obviously better answer.
        //
        // All strategies are computed in series. None is skipped —
        // when on-graph snaps perfectly but routes through 60 km of
        // road network, off-trail's 6 km mesh path wins on cost
        // and the user gets the right answer.
        let mut candidates: Vec<Path> = Vec::new();
        let mut last_err: Option<PathfindError> = None;

        // `force_off_trail` skips both graph strategies. Used by
        // the mimicry harness and the SPA's "force off-trail" toggle
        // to evaluate the mesh in isolation. Endpoints that sit
        // exactly on graph nodes would otherwise snap at zero
        // distance even with snap_radius_m=0, defeating the test.
        if !prefs.force_off_trail {
            crate::solver_trace::begin_phase("try_on_graph");
            match crate::tracer::phase("try_on_graph", || self.try_on_graph(from_xy, to_xy, &prefs)) {
                Ok(Some(p)) => candidates.push(p),
                Ok(None) => {}
                Err(e) => last_err = Some(e),
            }
            crate::solver_trace::begin_phase("try_hybrid");
            match crate::tracer::phase("try_hybrid", || self.try_hybrid(from_xy, to_xy, &prefs)) {
                Ok(Some(p)) => candidates.push(p),
                Ok(None) => {}
                Err(e) => last_err = Some(e),
            }
        }
        if prefs.allow_off_trail {
            let extent_km = dist / 1000.0;
            if extent_km <= prefs.max_off_trail_km {
                crate::solver_trace::begin_phase("solve_off_trail");
                match crate::tracer::phase("solve_off_trail", || {
                    self.solve_off_trail(from_xy, to_xy, &prefs)
                }) {
                    Ok(p) => candidates.push(p),
                    Err(e) => last_err = Some(e),
                }
            }
        }

        candidates
            .into_iter()
            .min_by(|a, b| {
                a.cost
                    .partial_cmp(&b.cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or(last_err.unwrap_or(PathfindError::NoRoute))
    }

    fn try_on_graph(
        &self,
        from_xy: PointXY,
        to_xy: PointXY,
        prefs: &Prefs,
    ) -> Result<Option<Path>, PathfindError> {
        let Some(graph) = self.graph.as_ref() else {
            return Ok(None);
        };
        let Ok(from_node) = graph.snap(from_xy.x, from_xy.y, prefs.snap_radius_m) else {
            return Ok(None);
        };
        let Ok(to_node) = graph.snap(to_xy.x, to_xy.y, prefs.snap_radius_m) else {
            return Ok(None);
        };
        let edge_mul = self.edge_mul_closure(prefs);
        // Observe Dijkstra exploration into the solver-trace recorder
        // when one's installed. The closure is no-op-cheap when no
        // recorder is active (`solver_trace::record` short-circuits
        // on the thread-local check).
        let Some(rr) = graph.route_with_observer(
            from_node,
            to_node,
            prefs.profile,
            edge_mul,
            graph_observer,
        )? else {
            return Ok(None);
        };
        Ok(Some(route_result_to_path(graph, &rr, PathStrategy::OnGraph)))
    }

    fn try_hybrid(
        &self,
        from_xy: PointXY,
        to_xy: PointXY,
        prefs: &Prefs,
    ) -> Result<Option<Path>, PathfindError> {
        let Some(graph) = self.graph.as_ref() else {
            return Ok(None);
        };
        let from_snap = graph.snap(from_xy.x, from_xy.y, prefs.snap_radius_m).ok();
        let to_snap = graph.snap(to_xy.x, to_xy.y, prefs.snap_radius_m).ok();
        // Look further to find a bridge node when direct snap failed.
        let from_bridge = from_snap.or_else(|| {
            graph
                .snap(from_xy.x, from_xy.y, prefs.bridge_radius_m)
                .ok()
        });
        let to_bridge = to_snap.or_else(|| {
            graph.snap(to_xy.x, to_xy.y, prefs.bridge_radius_m).ok()
        });
        let (Some(from_node), Some(to_node)) = (from_bridge, to_bridge) else {
            return Ok(None);
        };

        // If both endpoints already snap, on-graph would have caught
        // this — but try_on_graph runs first. We only arrive here
        // when at least one side needs bridging.
        let from_node_pos = graph.node(from_node).ok_or(PathfindError::NoRoute)?;
        let to_node_pos = graph.node(to_node).ok_or(PathfindError::NoRoute)?;

        // Off-trail prefix from `from_xy` to `from_node_pos`, only
        // when the user's actual start lies off the graph.
        let prefix = if from_snap.is_some() {
            None
        } else {
            Some(self.build_off_trail_segment(
                from_xy,
                PointXY {
                    x: from_node_pos.x as f64,
                    y: from_node_pos.y as f64,
                },
                prefs,
            )?)
        };
        let suffix = if to_snap.is_some() {
            None
        } else {
            Some(self.build_off_trail_segment(
                PointXY {
                    x: to_node_pos.x as f64,
                    y: to_node_pos.y as f64,
                },
                to_xy,
                prefs,
            )?)
        };

        // Graph middle. Use the same per-edge multiplier as the
        // pure-graph strategy so layer weights stay consistent.
        let edge_mul = self.edge_mul_closure(prefs);
        let middle = match graph.route_with_observer(
            from_node,
            to_node,
            prefs.profile,
            edge_mul,
            graph_observer,
        )? {
            Some(r) => r,
            None => return Ok(None),
        };
        // Concatenate the per-edge polylines so the hybrid graph
        // leg looks like the actual trail, not straight-segments
        // between junction nodes. Drop the duplicate first vertex
        // on every non-initial leg. Aggregate per-fkb_type metres
        // alongside so the stitched Path can report which surface
        // the middle leg traversed.
        let mut middle_geom_utm: Vec<(f32, f32)> = Vec::new();
        let mut middle_fkb: std::collections::BTreeMap<String, f64> =
            std::collections::BTreeMap::new();
        for (i, &eid) in middle.edges.iter().enumerate() {
            let poly = graph.edge_polyline(eid);
            if poly.is_empty() {
                continue;
            }
            if let Some(er) = graph.edge(eid) {
                *middle_fkb
                    .entry(fkb_code_to_str(er.fkb_type).to_string())
                    .or_insert(0.0) += er.length_m as f64;
            }
            let skip = if i == 0 { 0 } else { 1 };
            for p in poly.iter().skip(skip) {
                middle_geom_utm.push((p.x, p.y));
            }
        }
        if middle_geom_utm.is_empty() {
            middle_geom_utm = middle
                .nodes
                .iter()
                .filter_map(|nid| graph.node(*nid).map(|p| (p.x, p.y)))
                .collect();
        }

        Ok(Some(stitch_hybrid(
            &prefix,
            &middle_geom_utm,
            &middle_fkb,
            &suffix,
            middle.cost as f64,
            middle.length_m as f64,
        )))
    }

    /// FMM dispatch for off-trail. Sizes a corridor, bakes the
    /// cost field (Tobler + per-cell vetoes from the contributor
    /// stack), runs the eikonal solve, extracts and smooths the
    /// path. Returns the same `OffTrailSegment` shape as the
    /// Theta\* path so the caller doesn't care which solver
    /// produced the route. Errors when DEM isn't loaded, when the
    /// corridor is degenerate, or when the goal is unreachable.
    fn try_build_off_trail_segment_fmm(
        &self,
        from: PointXY,
        to: PointXY,
        prefs: &Prefs,
    ) -> Result<OffTrailSegment, PathfindError> {
        let dem = self.dem.as_ref().ok_or_else(|| {
            PathfindError::Internal("FMM mode requires DEM artifact loaded".into())
        })?;
        let off_trail_base = prefs.off_trail_base.unwrap_or_else(|| {
            self.cost_config
                .off_trail_base
                .for_profile(prefs.profile)
        });
        let inputs = crate::fmm_adapter::FmmSolveInputs {
            from,
            to,
            cell_m: 10.0,
            base_pace_s_per_m: self.cost_config.base.pace_s_per_m as f32,
            refuse_above_deg: self.cost_config.slope_cell.refuse_above_deg,
            off_trail_factor: off_trail_base as f32,
            use_anisotropic: true,
        };
        let contributors = self.native_contributors.clone();
        let out = crate::fmm_adapter::solve_fmm_path(
            inputs, dem.clone(), &contributors, prefs.profile,
        )
        .map_err(|e| PathfindError::Internal(format!("FMM adapter: {e}")))?;
        tracing::debug!(
            cells_accepted = out.cells_accepted,
            vetoed_cells = out.vetoed_cells,
            solve_ms = out.solve_ms,
            "FMM off-trail solve ok"
        );
        // Convert smoothed PathPoints into Pathfinder's Point2.
        let geometry: Vec<Point2> = out
            .polyline
            .iter()
            .map(|p| Point2 { x: p.x, y: p.y })
            .collect();
        let length_m = geometry
            .windows(2)
            .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
            .sum();
        Ok(OffTrailSegment {
            geometry,
            length_m,
            cost: out.cost_seconds,
            refused_by: out.refused_by,
        })
    }

    fn solve_off_trail(
        &self,
        from_xy: PointXY,
        to_xy: PointXY,
        prefs: &Prefs,
    ) -> Result<Path, PathfindError> {
        let segment = self.build_off_trail_segment(from_xy, to_xy, prefs)?;
        let geometry: Vec<[f64; 2]> = segment
            .geometry
            .iter()
            .map(|p| {
                let (lon, lat) = utm33n_to_wgs84(p.x, p.y);
                [lon, lat]
            })
            .collect();
        let (distances_m, length_m) = cumulative_distances_utm(&segment.geometry);
        let leg_len = length_m;
        let mut fkb_breakdown: std::collections::BTreeMap<String, f64> =
            std::collections::BTreeMap::new();
        if length_m > 0.0 {
            fkb_breakdown.insert("off_trail".to_string(), length_m);
        }
        Ok(Path {
            strategy: PathStrategy::OffTrail,
            legs: vec![PathLeg {
                kind: LegKind::OffTrailPrefix,
                start_idx: 0,
                end_idx: geometry.len().saturating_sub(1) as u32,
                length_m: leg_len,
            }],
            geometry,
            distances_m,
            length_m,
            // Cost-weighted, comparable to graph router output.
            cost: segment.cost,
            on_trail_pct: 0.0,
            fkb_breakdown,
            refused_by: segment.refused_by,
            debug: None,
            recording: None,
        })
    }

    /// Build one off-trail mesh between two points (start and goal
    /// in EPSG:25833 m), run Theta\*, return the resulting polyline
    /// in UTM coordinates plus cost + observed-refusal layer names.
    fn build_off_trail_segment(
        &self,
        from: PointXY,
        to: PointXY,
        prefs: &Prefs,
    ) -> Result<OffTrailSegment, PathfindError> {
        // FastMarching dispatch — phase 6 wiring. The FMM adapter
        // sizes its own corridor, bakes the cost field from the
        // same CostContributor stack used by the Theta\* path,
        // runs the eikonal solve, and returns a Chaikin-smoothed
        // polyline. Falls back to Theta\* if the FMM solve errors
        // (missing DEM, unreachable goal); operator can also force
        // a specific cost_mode per request.
        if prefs.cost_mode == CostMode::FastMarching {
            match self.try_build_off_trail_segment_fmm(from, to, prefs) {
                Ok(seg) => return Ok(seg),
                Err(e) => {
                    tracing::warn!(error = %e, "FMM off-trail solve failed; falling back to Theta*");
                    // Fall through to Theta\* code below.
                }
            }
        }
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let dist_m = (dx * dx + dy * dy).sqrt();
        let pad_m = effective_mesh_pad_m(prefs.mesh_pad_m, prefs.mesh_cell_m, dist_m);
        let bbox = MeshBbox {
            min_x: from.x.min(to.x) - pad_m,
            max_x: from.x.max(to.x) + pad_m,
            min_y: from.y.min(to.y) - pad_m,
            max_y: from.y.max(to.y) + pad_m,
        };
        crate::solver_trace::begin_phase("mesh_inputs");
        let (samples, refused, refused_by) = crate::tracer::phase("mesh_inputs", || {
            self.mesh_inputs_for_bbox(bbox, prefs.mesh_cell_m, prefs.profile, &prefs.layer_weights)
        });
        // Mesh-size stats: how many cells did the bbox produce vs.
        // how many were refused outright by some layer. A high
        // refusal ratio explains "why didn't theta_star find a
        // direct path?" at a glance.
        crate::tracer::with(|t| {
            if let Some(t) = t {
                t.set_mesh_stats(samples.len() as u32 + refused.len() as u32, refused.len() as u32);
            }
        });
        // Emit the one-shot MeshBuilt event so the SPA can render
        // a "X cells, Y refused" overlay at the start of the
        // theta_star animation.
        let total_cells = (samples.len() + refused.len()) as u32;
        let refused_cells_count = refused.len() as u32;
        crate::solver_trace::record(|| crate::solver_trace::SolverEvent::MeshBuilt {
            cells: total_cells,
            refused_cells: refused_cells_count,
        });
        let mesh_input = MeshBuildInput {
            bbox,
            cell_m: prefs.mesh_cell_m,
            samples,
            refused,
            exits: vec![ExitNode {
                graph_node_id: 0,
                at: Point2 { x: to.x, y: to.y },
            }],
            start: Some(Point2 { x: from.x, y: from.y }),
        };
        crate::solver_trace::begin_phase("build_local_mesh");
        let built = crate::tracer::phase("build_local_mesh", || build_local_mesh(mesh_input));
        let Some(start_node) = built.start_node else {
            return Err(PathfindError::NoRoute);
        };
        let goal_node = match built.exits.values().next() {
            Some(id) => *id,
            None => return Err(PathfindError::NoRoute),
        };
        // Direction-aware edge cost: combine the symmetric
        // cell-average with per-layer `edge_cost_modifier` so
        // climbing vs traversing a slope costs differently.
        //
        // `off_trail_base` is the price of walking through trackless
        // terrain vs walking on a maintained trail of equal length.
        // Without it the cost model treats them as equal per metre
        // and the cost-based selector picks straight-line mesh
        // shortcuts over real trails whenever the trail is more
        // than a few percent longer.
        //
        // Resolve off_trail_base in priority order:
        //   1. Per-request `Prefs::off_trail_base` (explicit knob)
        //   2. Per-request `Prefs::cost_config_override` patch
        //   3. Boot-time `Pathfinder::cost_config`
        // The single hardcoded fallback chain that used to live
        // here (foot=1.7 / bicycle=2.5 / ski=1.0) is gone — those
        // values now live in `tools/cost-config.toml`.
        let effective_cfg = if let Some(patch) = prefs.cost_config_override.as_ref() {
            self.cost_config.with_patch(patch)
        } else {
            self.cost_config.clone()
        };
        let off_trail_base: f64 = prefs
            .off_trail_base
            .unwrap_or_else(|| effective_cfg.off_trail_base.for_profile(prefs.profile));
        let layers = self.layers.clone();
        let profile = prefs.profile;
        let weights = prefs.layer_weights.clone();
        let weight_fn = move |name: &str| weights.get(name).copied().unwrap_or(1.0);
        // Snapshot for the WalkSeconds branch — needs the native
        // contributor list (which depends on the Pathfinder's DEM /
        // mask handles) and an owned Vec since the closure outlives
        // the &self borrow.
        let contributors_for_mesh: Vec<Arc<dyn crate::contributor::CostContributor>> =
            if prefs.cost_mode == CostMode::WalkSeconds {
                self.contributors_for_breakdown()
            } else {
                Vec::new()
            };
        let cost_mode = prefs.cost_mode;
        let edge_cost_fn = move |m: &Mesh, a: MeshNodeId, b: MeshNodeId| -> f64 {
            let pt_a = m.pt(a);
            let pt_b = m.pt(b);
            let ma = m.cost_mul(a);
            let mb = m.cost_mul(b);
            if !ma.is_finite() || !mb.is_finite() {
                return f64::INFINITY;
            }
            let euclid = pt_a.dist(pt_b);
            match cost_mode {
                // `FastMarching` is dispatched up-stream in
                // `build_off_trail_segment` and never reaches the
                // Theta\* edge_cost_fn; if it does fall through (FMM
                // errored and we degraded gracefully), use the same
                // additive walk-seconds composition as the canonical
                // WalkSeconds escape valve.
                CostMode::FastMarching => {
                    let cell_avg = 0.5 * (ma + mb);
                    let dir_mult = compose_mesh_edge(
                        &layers, &weight_fn, pt_a.x, pt_a.y, pt_b.x, pt_b.y, profile,
                    ) as f64;
                    euclid * cell_avg * dir_mult * off_trail_base
                }
                CostMode::Multiplicative => {
                    let cell_avg = 0.5 * (ma + mb);
                    let dir_mult = compose_mesh_edge(
                        &layers, &weight_fn, pt_a.x, pt_a.y, pt_b.x, pt_b.y, profile,
                    ) as f64;
                    euclid * cell_avg * dir_mult * off_trail_base
                }
                CostMode::WalkSeconds => {
                    // Base traversal + Σ contributions in walk-seconds,
                    // then apply off_trail_base as a flat multiplier on
                    // the total mesh-edge cost. The cell_mul veto via
                    // `is_finite` above is preserved so refused samples
                    // still block.
                    let ctx = crate::contributor::EdgeContext {
                        fx: pt_a.x,
                        fy: pt_a.y,
                        tx: pt_b.x,
                        ty: pt_b.y,
                        length_m: euclid,
                        profile,
                        kind: crate::contributor::EdgeKind::Mesh,
                    };
                    let cost = crate::contributor::compose_edge_walk_seconds(
                        &contributors_for_mesh,
                        &ctx,
                    );
                    if !cost.total_walk_seconds.is_finite() {
                        return f64::INFINITY;
                    }
                    cost.total_walk_seconds * off_trail_base
                }
            }
        };
        crate::solver_trace::begin_phase("theta_star");
        let path = crate::tracer::phase("theta_star", || {
            theta_star_with_edge_cost(&built.mesh, start_node, goal_node, edge_cost_fn)
        })
        .ok_or(PathfindError::NoRoute)?;
        // Theta* produces sparse polylines — line-of-sight jumps
        // can span hundreds of metres between vertices. That's
        // correct for routing but renders as a visibly straight
        // line in the SPA. Interpolate at `mesh_cell_m` intervals
        // so the user sees a polyline matching the mesh resolution.
        // The length + cost are unchanged because all inserted
        // points lie on the original segments.
        let densified = densify_polyline(&path.geometry, prefs.mesh_cell_m);
        Ok(OffTrailSegment {
            geometry: densified,
            length_m: path.length_m,
            // Cost-weighted: comparable to graph-router cost units.
            cost: path.cost,
            refused_by,
        })
    }

    /// Walk a regular grid over `bbox` and ask every layer for a
    /// cell cost. Cells with multiplier=INF or any refusal land in
    /// `refused`; everyone else lands in `samples`. The mesh
    /// builder picks nearest-sample-per-cell, so emitting one
    /// sample per cell at the cell centre is the cleanest path.
    fn mesh_inputs_for_bbox(
        &self,
        bbox: MeshBbox,
        cell_m: f64,
        profile: Profile,
        weights: &HashMap<String, f32>,
    ) -> (Vec<CostSample>, Vec<RefusedPolygon>, Vec<String>) {
        let weight_fn = |name: &str| -> f32 {
            weights.get(name).copied().unwrap_or(1.0)
        };
        let (nx, ny) = bbox.grid_dims(cell_m);
        let mut samples = Vec::with_capacity((nx * ny) as usize);
        let mut refused = Vec::new();
        let mut refused_layers = std::collections::HashSet::new();
        if nx == 0 || ny == 0 {
            return (samples, refused, Vec::new());
        }
        let cx_w = (bbox.max_x - bbox.min_x) / nx as f64;
        let cy_w = (bbox.max_y - bbox.min_y) / ny as f64;
        let half_x = cx_w * 0.5;
        let half_y = cy_w * 0.5;
        for r in 0..ny {
            for c in 0..nx {
                let x = bbox.min_x + (c as f64 + 0.5) * cx_w;
                let y = bbox.min_y + (r as f64 + 0.5) * cy_w;
                let composed = compose_cell(&self.layers, &weight_fn, x, y, profile);
                if let Some(reason) = composed.refused {
                    refused_layers.insert(reason.to_string());
                    refused.push(RefusedPolygon {
                        ring: vec![
                            Point2 { x: x - half_x, y: y - half_y },
                            Point2 { x: x + half_x, y: y - half_y },
                            Point2 { x: x + half_x, y: y + half_y },
                            Point2 { x: x - half_x, y: y + half_y },
                            Point2 { x: x - half_x, y: y - half_y },
                        ],
                    });
                } else {
                    samples.push(CostSample {
                        at: Point2 { x, y },
                        cost_mul: composed.multiplier as f64,
                    });
                }
            }
        }
        let mut refused_by: Vec<String> = refused_layers.into_iter().collect();
        refused_by.sort();
        (samples, refused, refused_by)
    }

    fn edge_mul_closure<'a>(
        &'a self,
        prefs: &'a Prefs,
    ) -> Box<dyn Fn(&turbo_tiles_graph::EdgeRecord) -> f32 + Send + Sync + 'a> {
        let weights = prefs.layer_weights.clone();
        let weight_fn = move |name: &str| -> f32 {
            weights.get(name).copied().unwrap_or(1.0)
        };
        // FastMarching only affects the off-trail leg; the graph
        // leg keeps using walk-seconds composition so on-graph and
        // FMM costs remain comparable for the candidate-min
        // selection. Treat them identically here.
        let effective_mode = match prefs.cost_mode {
            CostMode::FastMarching => CostMode::WalkSeconds,
            other => other,
        };
        match effective_mode {
            CostMode::Multiplicative => {
                Box::new(move |edge| compose_edge(&self.layers, &weight_fn, edge, prefs.profile))
            }
            CostMode::FastMarching => unreachable!("normalised above"),
            CostMode::WalkSeconds => {
                // Convert the additive walk-seconds composition back
                // into a multiplier the Graph's `route_with` can
                // scale `baked` by, so the Dijkstra entry point
                // doesn't have to change shape. baked ≈ length ×
                // profile_pace_s_per_m (Naismith-adjusted); we want
                // the resulting `baked × mul` to equal
                // `total_walk_seconds`. baked is in profile-cost
                // units, so we divide by the flat-trail baseline
                // (length × BASE_PACE_S_PER_M) to keep the scale
                // consistent with mesh edges.
                let contributors = self.contributors_for_breakdown();
                let profile = prefs.profile;
                Box::new(move |edge| {
                    let length_m = edge.length_m as f64;
                    if length_m <= 0.0 {
                        return 1.0;
                    }
                    let ctx = crate::contributor::EdgeContext {
                        fx: 0.0,
                        fy: 0.0,
                        tx: length_m,
                        ty: 0.0,
                        length_m,
                        profile,
                        kind: crate::contributor::EdgeKind::Graph(edge),
                    };
                    let cost = crate::contributor::compose_edge_walk_seconds(&contributors, &ctx);
                    if !cost.total_walk_seconds.is_finite() {
                        return f32::INFINITY;
                    }
                    let base = length_m * crate::contributor::BASE_PACE_S_PER_M;
                    if base <= 0.0 {
                        return 1.0;
                    }
                    (cost.total_walk_seconds / base) as f32
                })
            }
        }
    }
}

/// Per-layer contribution at a single (lon, lat) point.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InspectLayer {
    pub name: String,
    pub multiplier: f32,
    pub refused: Option<String>,
    pub covers: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InspectPoint {
    pub lon: f64,
    pub lat: f64,
    pub x_25833: f64,
    pub y_25833: f64,
    pub composed_multiplier: f32,
    pub refused_by: Option<String>,
    pub layers: Vec<InspectLayer>,
}

/// One mesh cell as the inspect endpoint sees it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InspectCell {
    pub lon: f64,
    pub lat: f64,
    pub cost_mul: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Inspect {
    pub mesh_cell_m: f64,
    pub cells: Vec<InspectCell>,
    /// Outer rings of refused-region polygons in WGS84 lon/lat.
    pub refused_polygons: Vec<Vec<[f64; 2]>>,
    /// Layers that vetoed at least one cell.
    pub refused_by: Vec<String>,
    /// Closest graph node within `bridge_radius_m` of `from`, in
    /// WGS84 lon/lat — useful for understanding why a hybrid path
    /// landed where it did.
    pub nearest_graph_node_from: Option<[f64; 2]>,
    pub nearest_graph_node_to: Option<[f64; 2]>,
}

struct OffTrailSegment {
    geometry: Vec<Point2>,
    /// Geometric length of the segment.
    length_m: f64,
    /// Cost-weighted A* score, same units as graph router output.
    cost: f64,
    refused_by: Vec<String>,
}

fn route_result_to_path(graph: &Graph, rr: &RouteResult, strategy: PathStrategy) -> Path {
    // Build the route polyline by concatenating each edge's
    // polyline rather than jumping from node to node. With the
    // `norway.graph_geom` sibling artifact loaded, each edge's
    // polyline reflects the original LineString from `paths.edge.
    // geom`; without it, `edge_polyline` falls back to a 2-point
    // straight segment between endpoints.
    let mut geom_utm: Vec<Point2> = Vec::new();
    // Per-fkb_type metres aggregator. Reads each edge's length_m
    // from the EdgeRecord (correct even when the polyline density
    // varies). Buckets by stable string key — see `fkb_code_to_str`.
    let mut fkb_breakdown: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    for (i, &eid) in rr.edges.iter().enumerate() {
        let poly = graph.edge_polyline(eid);
        if poly.is_empty() {
            continue;
        }
        if let Some(er) = graph.edge(eid) {
            *fkb_breakdown
                .entry(fkb_code_to_str(er.fkb_type).to_string())
                .or_insert(0.0) += er.length_m as f64;
        }
        // Drop the first vertex on every non-initial leg to avoid
        // duplicating the junction node (it's the last vertex of
        // the previous edge).
        let skip = if i == 0 { 0 } else { 1 };
        for p in poly.iter().skip(skip) {
            geom_utm.push(Point2 {
                x: p.x as f64,
                y: p.y as f64,
            });
        }
    }
    // Fall back to node-positions if the route had no edges (which
    // would be a single-node trivial path).
    if geom_utm.is_empty() {
        for &nid in &rr.nodes {
            if let Some(p) = graph.node(nid) {
                geom_utm.push(Point2 {
                    x: p.x as f64,
                    y: p.y as f64,
                });
            }
        }
    }
    let geometry: Vec<[f64; 2]> = geom_utm
        .iter()
        .map(|p| {
            let (lon, lat) = utm33n_to_wgs84(p.x, p.y);
            [lon, lat]
        })
        .collect();
    let (distances_m, length_m) = cumulative_distances_utm(&geom_utm);
    let legs = if geometry.is_empty() {
        Vec::new()
    } else {
        vec![PathLeg {
            kind: LegKind::Graph,
            start_idx: 0,
            end_idx: (geometry.len() - 1) as u32,
            length_m,
        }]
    };
    Path {
        strategy,
        geometry,
        distances_m,
        length_m,
        cost: rr.cost as f64,
        on_trail_pct: 100.0,
        fkb_breakdown,
        legs,
        refused_by: Vec::new(),
        debug: None,
        recording: None,
    }
}

/// Interpolate intermediate vertices along a polyline so each
/// consecutive pair is at most `step_m` apart. Endpoints and every
/// original vertex are preserved; inserted points lie on the
/// segment between two originals.
///
/// Theta*'s output is intentionally sparse — line-of-sight jumps
/// can run for hundreds of metres without producing intermediate
/// vertices. That's the right answer for routing but renders as a
/// visibly straight line in MapLibre. Densifying just for the
/// returned polyline gives the curator a visualisation that matches
/// the mesh resolution without changing routing cost or length.
fn densify_polyline(line: &[Point2], step_m: f64) -> Vec<Point2> {
    if line.len() < 2 || step_m <= 0.0 {
        return line.to_vec();
    }
    let mut out: Vec<Point2> = Vec::with_capacity(line.len() * 4);
    out.push(line[0]);
    for w in line.windows(2) {
        let dx = w[1].x - w[0].x;
        let dy = w[1].y - w[0].y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= step_m {
            out.push(w[1]);
            continue;
        }
        let n = (len / step_m).ceil() as usize;
        for k in 1..n {
            let t = k as f64 / n as f64;
            out.push(Point2 {
                x: w[0].x + t * dx,
                y: w[0].y + t * dy,
            });
        }
        out.push(w[1]);
    }
    out
}

fn cumulative_distances_utm(pts: &[Point2]) -> (Vec<f64>, f64) {
    let mut distances = Vec::with_capacity(pts.len());
    let mut total = 0.0;
    if !pts.is_empty() {
        distances.push(0.0);
        for w in pts.windows(2) {
            let dx = w[1].x - w[0].x;
            let dy = w[1].y - w[0].y;
            total += (dx * dx + dy * dy).sqrt();
            distances.push(total);
        }
    }
    (distances, total)
}

fn stitch_hybrid(
    prefix: &Option<OffTrailSegment>,
    middle_utm: &[(f32, f32)],
    middle_fkb: &std::collections::BTreeMap<String, f64>,
    suffix: &Option<OffTrailSegment>,
    middle_cost: f64,
    middle_length: f64,
) -> Path {
    // Each segment carries cost (cost-weighted, comparable to the
    // graph router) and length_m (pure geometric). Total cost is the
    // sum across legs; total length is the sum of geometric lengths.

    // Compose the full UTM polyline. The bridge node sits at the
    // junction; drop the duplicate vertex when we splice.
    let mut all_utm: Vec<Point2> = Vec::new();
    let mut legs: Vec<PathLeg> = Vec::new();
    let mut refused_by_set = std::collections::HashSet::<String>::new();
    let mut prefix_cost = 0.0;
    let mut suffix_cost = 0.0;
    let mut middle_len_actual = middle_length;
    let _ = middle_len_actual; // referenced below for clarity

    if let Some(pref) = prefix {
        for r in &pref.refused_by {
            refused_by_set.insert(r.clone());
        }
        prefix_cost = pref.cost;
        let start_idx = all_utm.len() as u32;
        all_utm.extend(pref.geometry.iter().copied());
        let end_idx = all_utm.len().saturating_sub(1) as u32;
        let (_, len) = cumulative_distances_utm(&pref.geometry);
        legs.push(PathLeg {
            kind: LegKind::OffTrailPrefix,
            start_idx,
            end_idx,
            length_m: len,
        });
    }
    let mid_pts: Vec<Point2> = middle_utm
        .iter()
        .map(|&(x, y)| Point2 { x: x as f64, y: y as f64 })
        .collect();
    if !mid_pts.is_empty() {
        let start_idx = if all_utm
            .last()
            .map(|last| points_match(last, &mid_pts[0]))
            .unwrap_or(false)
        {
            // Skip the duplicate vertex; report the index of the
            // already-present last point as the leg's start.
            (all_utm.len() - 1) as u32
        } else {
            all_utm.len() as u32
        };
        let skip = (start_idx as usize) < all_utm.len();
        let mid_iter = if skip { &mid_pts[1..] } else { &mid_pts[..] };
        all_utm.extend(mid_iter.iter().copied());
        let end_idx = all_utm.len().saturating_sub(1) as u32;
        let (_, mid_len) = cumulative_distances_utm(&mid_pts);
        middle_len_actual = mid_len;
        legs.push(PathLeg {
            kind: LegKind::Graph,
            start_idx,
            end_idx,
            length_m: mid_len,
        });
    }
    if let Some(suf) = suffix {
        for r in &suf.refused_by {
            refused_by_set.insert(r.clone());
        }
        suffix_cost = suf.cost;
        let start_idx = if all_utm
            .last()
            .map(|last| !suf.geometry.is_empty() && points_match(last, &suf.geometry[0]))
            .unwrap_or(false)
        {
            (all_utm.len() - 1) as u32
        } else {
            all_utm.len() as u32
        };
        let skip = (start_idx as usize) < all_utm.len();
        let suf_iter = if skip {
            &suf.geometry[1..]
        } else {
            &suf.geometry[..]
        };
        all_utm.extend(suf_iter.iter().copied());
        let end_idx = all_utm.len().saturating_sub(1) as u32;
        let (_, len) = cumulative_distances_utm(&suf.geometry);
        legs.push(PathLeg {
            kind: LegKind::OffTrailSuffix,
            start_idx,
            end_idx,
            length_m: len,
        });
    }

    let geometry: Vec<[f64; 2]> = all_utm
        .iter()
        .map(|p| {
            let (lon, lat) = utm33n_to_wgs84(p.x, p.y);
            [lon, lat]
        })
        .collect();
    let (distances_m, total_len) = cumulative_distances_utm(&all_utm);
    let on_trail_pct = if total_len > 0.0 {
        (middle_len_actual / total_len * 100.0) as f32
    } else {
        0.0
    };
    let mut refused_by: Vec<String> = refused_by_set.into_iter().collect();
    refused_by.sort();
    // Merge the middle leg's per-fkb metres with the off_trail
    // prefix/suffix lengths under the `off_trail` bucket.
    let mut fkb_breakdown: std::collections::BTreeMap<String, f64> = middle_fkb.clone();
    let off_trail_m: f64 =
        prefix.as_ref().map(|p| p.length_m).unwrap_or(0.0)
            + suffix.as_ref().map(|s| s.length_m).unwrap_or(0.0);
    if off_trail_m > 0.0 {
        *fkb_breakdown.entry("off_trail".to_string()).or_insert(0.0) += off_trail_m;
    }
    Path {
        strategy: PathStrategy::Hybrid,
        geometry,
        distances_m,
        length_m: total_len,
        cost: prefix_cost + middle_cost + suffix_cost,
        on_trail_pct,
        fkb_breakdown,
        legs,
        refused_by,
        debug: None,
        recording: None,
    }
}

fn points_match(a: &Point2, b: &Point2) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

/// Approximate UTM33N → WGS84 — the inverse of [`wgs84_to_utm33n`].
pub fn utm33n_to_wgs84(x: f64, y: f64) -> (f64, f64) {
    const A: f64 = 6_378_137.0;
    const F: f64 = 1.0 / 298.257_223_563;
    let e2 = F * (2.0 - F);
    let ep2 = e2 / (1.0 - e2);
    let k0 = 0.9996;
    let lon0 = 15.0_f64.to_radians();
    let false_e = 500_000.0;
    let m = y / k0;

    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
    let mu =
        m / (A * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0));
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1.powi(2) / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin();
    let sin_phi1 = phi1.sin();
    let cos_phi1 = phi1.cos();
    let tan_phi1 = phi1.tan();
    let c1 = ep2 * cos_phi1 * cos_phi1;
    let t1 = tan_phi1 * tan_phi1;
    let n1 = A / (1.0 - e2 * sin_phi1 * sin_phi1).sqrt();
    let r1 = A * (1.0 - e2) / (1.0 - e2 * sin_phi1 * sin_phi1).powf(1.5);
    let d = (x - false_e) / (n1 * k0);
    let phi = phi1
        - (n1 * tan_phi1 / r1)
            * (d * d / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d.powi(4)
                    / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2
                    - 3.0 * c1 * c1)
                    * d.powi(6)
                    / 720.0);
    let lambda = lon0
        + (d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1)
                * d.powi(5)
                / 120.0)
            / cos_phi1;
    (lambda.to_degrees(), phi.to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utm33n_to_wgs84_round_trips_oslo() {
        let lon0 = 10.7522;
        let lat0 = 59.9139;
        let p = wgs84_to_utm33n(lon0, lat0);
        let (lon, lat) = utm33n_to_wgs84(p.x, p.y);
        assert!((lon - lon0).abs() < 1e-3);
        assert!((lat - lat0).abs() < 1e-3);
    }
}
