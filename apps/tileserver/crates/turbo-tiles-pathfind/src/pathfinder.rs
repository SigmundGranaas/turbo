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
use turbo_tiles_graph::{Graph, Profile};
use turbo_tiles_mask::Mask;

use crate::cost::{compose_cell, CostLayer};
use crate::core::off_trail::Point2;
use crate::core::off_trail_mesh::{CostSample, MeshBbox, RefusedPolygon};
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
            // Mesh slope hard-veto fires only at the true-cliff
            // threshold; 45–60° is continuous high cost (Tobler), not a
            // wall, so corridors aren't severed by the 10 m DEM's steep
            // cells.
            natives.push(Arc::new(ToblerSlopeContributor::new(
                d.clone(),
                cost_config.slope_cell.cliff_refuse_deg,
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
            natives.push(Arc::new(
                MaskRefusalContributor::new(m.clone())
                    .with_water(cost_config.water.cost_s_per_m, cost_config.water.shore_band_m),
            ));
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

        // Two INDEPENDENT routers, dispatched by intent:
        //
        //  - `force_off_trail` → the off-trail FMM solver (pure
        //    cross-country; switchbacks on steep ground). Used by the
        //    mimicry harness and the "force off-trail" toggle.
        //
        //  - otherwise → the UNIFIED single-solve router (the default):
        //    ONE A* over the off-trail mesh + the trail network in one
        //    walk-seconds field, so it follows trails only while they're
        //    worth it and cuts across otherwise. This replaced the old
        //    on-graph / hybrid / off-trail candidate race, whose
        //    incomparable strategies and forced single-trail "bridge"
        //    produced the detours.
        //
        // The two share NOTHING but the cost model (`CostContributor`s)
        // and the raw data (graph + DEM) — see `unified` / `fmm_adapter`.
        let _ = dist;
        if prefs.force_off_trail {
            crate::solver_trace::begin_phase("solve_off_trail");
            return crate::tracer::phase("solve_off_trail", || {
                self.solve_off_trail(from_xy, to_xy, &prefs)
            });
        }
        crate::solver_trace::begin_phase("solve_unified");
        crate::tracer::phase("solve_unified", || self.solve_unified_path(from_xy, to_xy, &prefs))
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
        // Resolve the effective cost config: boot config + per-request
        // `cost_config_override` patch. Without this the off-trail FMM
        // path silently ignored per-request overrides (they only reached
        // the Theta\* escape valve), so SPA calibration and knob sweeps
        // had no effect on the production geodesic.
        let effective_cfg = match prefs.cost_config_override.as_ref() {
            Some(patch) => self.cost_config.with_patch(patch),
            None => self.cost_config.clone(),
        };
        let off_trail_base = prefs.off_trail_base.unwrap_or_else(|| {
            effective_cfg.off_trail_base.for_profile(prefs.profile)
        });
        // Naismith vertical-gain weight folded directionally into the
        // FMM along-fall-line pace (effective flat-metres per gain-metre,
        // matching on-graph pricing). DEFAULT 0 (amplifier = 1.0): the
        // terrain corpus showed the full k=8 foot term marginally
        // REGRESSED every axis (composite 91.7→91.0) — once the edge-
        // racetrack solver bug was fixed, Tobler alone already prices
        // slope well. Gated on the runtime `total_gain.amplifier` knob so
        // it's a one-request experiment (override) rather than a recompile;
        // `gain_factor_k = k·(amplifier − 1)`.
        let gain_k = if (effective_cfg.total_gain.amplifier - 1.0).abs() < 1e-6 {
            0.0
        } else {
            let k = match prefs.profile {
                turbo_tiles_graph::Profile::Foot => 8.0_f32,
                turbo_tiles_graph::Profile::Bicycle => 20.0,
                turbo_tiles_graph::Profile::Ski => 6.0,
            };
            k * (effective_cfg.total_gain.amplifier - 1.0)
        };
        // Adaptive cell size: 10 m preserves switchback fidelity on short
        // routes; 20 m quarters the cell count (and the lifted state space)
        // on long routes where the path is mostly long traverses, not tight
        // switchbacks. The breakpoint is the straight-line distance.
        let dist_m = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
        let cell_m = if dist_m <= 3000.0 { 10.0 } else { 20.0 };
        let inputs = crate::fmm_adapter::FmmSolveInputs {
            from,
            to,
            cell_m,
            base_pace_s_per_m: effective_cfg.base.pace_s_per_m as f32,
            // FMM metric refuses only true cliffs; 45–60° is continuous
            // high-cost Tobler (see slope_cell.cliff_refuse_deg) so the
            // corridor stays connected instead of being walled off.
            refuse_above_deg: effective_cfg.slope_cell.cliff_refuse_deg,
            off_trail_factor: off_trail_base as f32,
            use_anisotropic: true,
            gain_factor_k: gain_k,
            // Grade-limited (x,y,heading) solver: switchbacks up steep
            // ground. Opt-in via cost-config `[grade_limited]`.
            use_grade_limited: effective_cfg.grade_limited.enabled,
            max_grade_deg: effective_cfg.grade_limited.max_grade_deg,
            turn_penalty_s: effective_cfg.grade_limited.turn_penalty_s,
        };
        let contributors = self.native_contributors.clone();
        let out = crate::fmm_adapter::solve_fmm_path(
            inputs, dem.clone(), &contributors, prefs.profile,
        )
        .map_err(|e| {
            use crate::fmm_adapter::FmmAdapterError;
            match e {
                // Goal genuinely unreachable through the terrain (corridor
                // severed by water/glacier/cliff, or no DEM coverage). This
                // is an honest "no route", NOT an internal error — and there
                // is no Theta* fallback to paper over it with a garbage line.
                FmmAdapterError::GoalUnreachable
                | FmmAdapterError::StartOutsideGrid
                | FmmAdapterError::GoalOutsideGrid => PathfindError::NoRoute,
                other => PathfindError::Internal(format!("off-trail solver: {other}")),
            }
        })?;
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

    /// Build a [`Path`] from the unified single-solve router. Trail runs
    /// are `Graph` legs (blue), off-trail runs `OffTrailPrefix` (vermillion).
    fn solve_unified_path(
        &self,
        from_xy: PointXY,
        to_xy: PointXY,
        prefs: &Prefs,
    ) -> Result<Path, PathfindError> {
        let Some(graph) = self.graph.as_ref() else {
            return Err(PathfindError::NoRoute);
        };
        let Some(dem) = self.dem.as_ref() else {
            return Err(PathfindError::NoRoute);
        };
        let effective_cfg = match prefs.cost_config_override.as_ref() {
            Some(patch) => self.cost_config.with_patch(patch),
            None => self.cost_config.clone(),
        };
        let off_trail_factor = prefs
            .off_trail_base
            .unwrap_or_else(|| effective_cfg.off_trail_base.for_profile(prefs.profile))
            as f32;
        let base_pace = crate::contributor::BASE_PACE_S_PER_M as f32;
        let contributors = self.contributors_for_breakdown();
        // Adaptive mesh cell: fine (10 m) for short routes where off-trail
        // detail matters, coarser (up to 30 m) for long routes where the
        // path is mostly on trails and off-trail is a minor connector — so
        // the mesh cell count (and solve time) stays bounded as length grows.
        let dist_m = ((to_xy.x - from_xy.x).powi(2) + (to_xy.y - from_xy.y).powi(2)).sqrt();
        let cell_m = (dist_m / 250.0).clamp(10.0, 30.0);
        let route = crate::unified::solve_unified(
            graph,
            dem,
            &contributors,
            prefs.profile,
            from_xy,
            to_xy,
            cell_m,
            base_pace,
            off_trail_factor,
        )
        .ok_or(PathfindError::NoRoute)?;

        let geometry: Vec<[f64; 2]> = route
            .geometry_utm
            .iter()
            .map(|&(x, y)| {
                let (lon, lat) = utm33n_to_wgs84(x, y);
                [lon, lat]
            })
            .collect();
        // Build legs from contiguous on-trail / off-trail runs.
        let seg_len = |k: usize| -> f64 {
            let a = route.geometry_utm[k];
            let b = route.geometry_utm[k + 1];
            ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
        };
        // Cumulative distance along the route (EPSG:25833 metres).
        let mut distances_m: Vec<f64> = Vec::with_capacity(route.geometry_utm.len());
        let mut acc = 0.0f64;
        distances_m.push(0.0);
        for k in 0..route.geometry_utm.len().saturating_sub(1) {
            acc += seg_len(k);
            distances_m.push(acc);
        }
        let length_m = acc;
        let mut legs: Vec<PathLeg> = Vec::new();
        let mut on_m = 0.0f64;
        let mut off_m = 0.0f64;
        if !route.seg_on_trail.is_empty() {
            let mut run_start = 0usize;
            let mut run_kind = route.seg_on_trail[0];
            let mut run_len = 0.0f64;
            let mut push = |kind: bool, start: usize, end: usize, len: f64,
                            legs: &mut Vec<PathLeg>, on_m: &mut f64, off_m: &mut f64| {
                legs.push(PathLeg {
                    kind: if kind { LegKind::Graph } else { LegKind::OffTrailPrefix },
                    start_idx: start as u32,
                    end_idx: end as u32,
                    length_m: len,
                });
                if kind { *on_m += len } else { *off_m += len }
            };
            for k in 0..route.seg_on_trail.len() {
                if route.seg_on_trail[k] != run_kind {
                    push(run_kind, run_start, k, run_len, &mut legs, &mut on_m, &mut off_m);
                    run_start = k;
                    run_kind = route.seg_on_trail[k];
                    run_len = 0.0;
                }
                run_len += seg_len(k);
            }
            push(run_kind, run_start, route.seg_on_trail.len(), run_len, &mut legs, &mut on_m, &mut off_m);
        }
        let mut fkb_breakdown: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
        if on_m > 0.0 {
            fkb_breakdown.insert("sti".to_string(), on_m);
        }
        if off_m > 0.0 {
            fkb_breakdown.insert("off_trail".to_string(), off_m);
        }
        let on_trail_pct = if length_m > 0.0 { (on_m / length_m * 100.0) as f32 } else { 0.0 };

        Ok(Path {
            strategy: PathStrategy::Hybrid,
            legs,
            geometry,
            distances_m,
            length_m,
            cost: route.cost_s,
            on_trail_pct,
            fkb_breakdown,
            refused_by: Vec::new(),
            debug: None,
            recording: None,
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
        // Off-trail routing is FMM-only. The legacy Theta* mesh fallback was
        // removed: it produced blocky line-of-sight routes and, worse, masked
        // a genuinely unreachable goal (corridor severed by water/cliff/no
        // coverage) with a plausible-looking straight line. On failure the
        // FMM path now returns an honest error (NoRoute) instead of garbage.
        self.try_build_off_trail_segment_fmm(from, to, prefs)
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
