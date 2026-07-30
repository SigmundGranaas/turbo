//! **The solver seam.** Route-finding algorithms as swappable
//! implementations, selected per request.
//!
//! See `docs/architecture/2026-07-routing-engine-module-design.md` §5.
//!
//! Before D1 the two routers were private methods on `Pathfinder`,
//! reaching directly into its fields and dispatched by an `if` on one
//! preference flag. Adding a third meant editing that `if`; swapping
//! one for an experiment meant editing the engine. That is the coupling
//! this module removes.
//!
//! # What a solver is allowed to see
//!
//! [`SolveContext`] is a *borrowed view* of the engine's inputs — the
//! heightfield, the network, the cost stack, the resolved tuning. It
//! deliberately is not `&Pathfinder`: handing the whole engine to a
//! solver would let one reach `leg_cache`, `solve_route`, or another
//! solver, and the trait would document a boundary the code does not
//! have. A solver receives exactly what it needs to price and search.
//!
//! # Selection is data, not control flow
//!
//! [`SolverSet`] holds an ordered list and picks the first whose
//! [`Solver::accepts`] returns true. Registration order is the priority
//! order, which makes "try the experimental solver first, fall back to
//! the default" a composition decision rather than an engine edit.

use std::sync::Arc;

use turbo_route_model::{Heightfield, Point};
use turbo_tiles_graph::Graph;

use crate::config::CostConfig;
use crate::contributor::CostContributor;
use crate::core::off_trail_mesh::Point2;
use crate::pathfinder::{
    cumulative_distances_planar, LegKind, OffTrailSegment, Path, PathLeg, PathStrategy,
    PathfindError, Prefs,
};

/// Everything a solver may read. A borrowed view, constructed per
/// request — solvers hold no state and outlive no solve.
pub struct SolveContext<'a> {
    /// The terrain field. `None` in degraded mode (no DEM artifact);
    /// solvers that require it must say so via [`Solver::accepts`]
    /// rather than failing mid-solve.
    pub terrain: Option<&'a Arc<dyn Heightfield>>,
    /// The traversable network. `None` when no graph is loaded.
    pub network: Option<&'a Arc<Graph>>,
    /// The cost stack, in registration order.
    pub contributors: &'a [Arc<dyn CostContributor>],
    /// Resolved tuning for this request (boot config + any per-request
    /// patch, already merged by the caller).
    pub cost_config: &'a CostConfig,
}

/// One route request, in the engine's planar frame.
pub struct SolveRequest<'a> {
    pub from: Point,
    pub to: Point,
    pub prefs: &'a Prefs,
}

/// A route-finding algorithm.
pub trait Solver: Send + Sync {
    /// Stable identifier, for tracing and for naming one in config.
    fn name(&self) -> &'static str;

    /// Can this solver answer this request, given these inputs?
    ///
    /// Checked *before* solving, so a solver that needs terrain can
    /// decline in degraded mode instead of failing halfway through and
    /// leaving the caller to guess whether the route or the setup was
    /// at fault.
    fn accepts(&self, ctx: &SolveContext<'_>, req: &SolveRequest<'_>) -> bool;

    fn solve(&self, ctx: &SolveContext<'_>, req: &SolveRequest<'_>) -> Result<Path, PathfindError>;
}

/// The unified single-solve router: ONE A\* over the off-trail mesh and
/// the trail network in one walk-seconds field, so a route follows
/// trails only while they are worth it and cuts across otherwise.
///
/// Replaced an on-graph / hybrid / off-trail candidate race whose
/// incomparable strategies and forced single-trail "bridge" produced
/// the detours this engine exists to avoid.
pub struct UnifiedAStar;

impl Solver for UnifiedAStar {
    fn name(&self) -> &'static str {
        "unified_astar"
    }
    fn accepts(&self, _ctx: &SolveContext<'_>, req: &SolveRequest<'_>) -> bool {
        !req.prefs.force_off_trail
    }
    fn solve(&self, ctx: &SolveContext<'_>, req: &SolveRequest<'_>) -> Result<Path, PathfindError> {
        crate::solver_trace::begin_phase("solve_unified");
        crate::tracer::phase("solve_unified", || {
            solve_unified(ctx, req.from, req.to, req.prefs)
        })
    }
}

/// Pure cross-country: the Fast Marching Method over a corridor cost
/// field, optionally lifted to (x, y, heading) so the geodesic can
/// switchback up steep ground.
///
/// Selected by `force_off_trail` — the mimicry harness and the SPA's
/// off-trail toggle. Requires terrain, and says so rather than
/// discovering it mid-solve.
pub struct FmmGradeLimited;

impl Solver for FmmGradeLimited {
    fn name(&self) -> &'static str {
        "fmm_grade_limited"
    }
    fn accepts(&self, ctx: &SolveContext<'_>, req: &SolveRequest<'_>) -> bool {
        req.prefs.force_off_trail && ctx.terrain.is_some()
    }
    fn solve(&self, ctx: &SolveContext<'_>, req: &SolveRequest<'_>) -> Result<Path, PathfindError> {
        crate::solver_trace::begin_phase("solve_off_trail");
        crate::tracer::phase("solve_off_trail", || {
            solve_off_trail(ctx, req.from, req.to, req.prefs)
        })
    }
}

/// An ordered set of solvers. First acceptor wins.
pub struct SolverSet {
    solvers: Vec<Arc<dyn Solver>>,
}

impl SolverSet {
    pub fn new(solvers: Vec<Arc<dyn Solver>>) -> Self {
        Self { solvers }
    }

    /// The production set. Order is priority: the off-trail solver is
    /// first because its `accepts` is the narrower predicate, so the
    /// unified router reads as the default rather than as a fallback
    /// that happens to be listed second.
    pub fn production() -> Self {
        Self::new(vec![Arc::new(FmmGradeLimited), Arc::new(UnifiedAStar)])
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.solvers.iter().map(|s| s.name()).collect()
    }

    /// The first solver that accepts this request, or `None`.
    ///
    /// `None` is a real outcome, not an internal error: `force_off_trail`
    /// with no DEM loaded has no solver that can honestly answer, and
    /// saying so beats returning a straight line through terrain nobody
    /// measured.
    pub fn select(
        &self,
        ctx: &SolveContext<'_>,
        req: &SolveRequest<'_>,
    ) -> Option<&Arc<dyn Solver>> {
        self.solvers.iter().find(|s| s.accepts(ctx, req))
    }
}

impl Default for SolverSet {
    fn default() -> Self {
        Self::production()
    }
}

/// FMM dispatch for off-trail. Sizes a corridor, bakes the
/// cost field (Tobler + per-cell vetoes from the contributor
/// stack), runs the eikonal solve, extracts and smooths the
/// path. Returns the same `OffTrailSegment` shape as the
/// Theta\* path so the caller doesn't care which solver
/// produced the route. Errors when DEM isn't loaded, when the
/// corridor is degenerate, or when the goal is unreachable.
fn build_off_trail_segment_fmm(
    ctx: &SolveContext<'_>,
    from: Point,
    to: Point,
    prefs: &Prefs,
) -> Result<OffTrailSegment, PathfindError> {
    let dem = ctx
        .terrain
        .as_ref()
        .ok_or_else(|| PathfindError::Internal("FMM mode requires DEM artifact loaded".into()))?;
    // `ctx.cost_config` is ALREADY resolved — boot config merged with
    // any per-request patch, once, by the caller. Resolving it again
    // here (as this code did when it was a `Pathfinder` method) would
    // apply the patch twice. The design's rule is that the engine
    // receives numbers, not patches; the solvers are downstream of that.
    let effective_cfg = ctx.cost_config;
    let off_trail_base = prefs
        .off_trail_base
        .unwrap_or_else(|| effective_cfg.off_trail_base.for_profile(prefs.profile));
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
    let contributors = ctx.contributors;
    let out =
        crate::fmm_adapter::solve_fmm_path(inputs, Arc::clone(dem), contributors, prefs.profile)
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
fn solve_unified(
    ctx: &SolveContext<'_>,
    from_xy: Point,
    to_xy: Point,
    prefs: &Prefs,
) -> Result<Path, PathfindError> {
    let Some(graph) = ctx.network.as_ref() else {
        return Err(PathfindError::NoRoute);
    };
    let Some(dem) = ctx.terrain.as_ref() else {
        return Err(PathfindError::NoRoute);
    };
    // Already resolved by the caller — see the note in the FMM solver.
    let effective_cfg = ctx.cost_config;
    let off_trail_factor = prefs
        .off_trail_base
        .unwrap_or_else(|| effective_cfg.off_trail_base.for_profile(prefs.profile))
        as f32;
    let base_pace = crate::contributor::BASE_PACE_S_PER_M as f32;
    // Per-surface pace (road avoidance) is applied live here using the
    // EFFECTIVE config (boot + per-request/preset patch), so it isn't
    // baked at boot and presets can tune it. Graph edges only; mesh
    // edges return 1.0.
    let mut contributors = ctx.contributors.to_vec();
    // Rebuild the cheap, purely config-driven graph contributors from the
    // EFFECTIVE (boot + per-request/preset) config, so overrides for
    // slope_graph / total_gain actually bite on the unified solve (they
    // were previously baked at boot and silently ignored here). Build
    // fresh by name — no downcast needed. (TrailProximity holds an RTree
    // and isn't rebuilt per request; off_trail_base/surface_pace cover the
    // trail-vs-everything preference.)
    for c in contributors.iter_mut() {
        match c.name() {
            "graph_slope" => {
                *c = std::sync::Arc::new(crate::native_contributors::GraphSlopeContributor {
                    quadratic_scale_deg: effective_cfg.slope_graph.quadratic_scale_deg,
                    refuse_above_deg: effective_cfg.slope_graph.refuse_above_deg,
                })
            }
            "total_gain" => {
                *c = std::sync::Arc::new(crate::native_contributors::TotalGainContributor {
                    gain_amplifier: effective_cfg.total_gain.amplifier,
                })
            }
            _ => {}
        }
    }
    contributors.push(std::sync::Arc::new(
        crate::native_contributors::SurfacePaceContributor::from_config(
            &effective_cfg.surface_pace,
        ),
    ));
    // Off-trail mesh steepness + climb-aversion knobs (previously hard-
    // coded in the mesh). `max_grade_deg` sets where the soft steep
    // penalty starts; `gain_k` (k·(amplifier−1)) adds Naismith climb cost
    // per gain-metre so "less height difference" shapes off-trail too.
    let mesh_max_grade_deg = effective_cfg.grade_limited.max_grade_deg;
    let mesh_gain_k = if (effective_cfg.total_gain.amplifier - 1.0).abs() < 1e-6 {
        0.0
    } else {
        let k = match prefs.profile {
            turbo_tiles_graph::Profile::Foot => 8.0_f32,
            turbo_tiles_graph::Profile::Bicycle => 20.0,
            turbo_tiles_graph::Profile::Ski => 6.0,
        };
        k * (effective_cfg.total_gain.amplifier - 1.0)
    };
    // Adaptive mesh cell: fine (10 m) for short routes where off-trail
    // detail matters, much coarser for long routes where the path is
    // mostly on trails and off-trail is a minor connector. The per-cell
    // overlay evaluation (DEM + contributor stack) times the number of
    // visited cells dominates long-route solve time, so the cell area
    // must grow with distance to keep it bounded — a ~26 km leg at 30 m
    // was ~130 s; at ~70 m it's a handful of seconds.
    let dist_m = ((to_xy.x - from_xy.x).powi(2) + (to_xy.y - from_xy.y).powi(2)).sqrt();
    let cell_m = (dist_m / 180.0).clamp(10.0, 70.0);

    // Project the avoided polylines (if any) onto the trail edges they
    // run along. The penalty lands on the GRAPH (Dijkstra) leg only —
    // the off-trail mesh keeps its ordinary high cost, so the router
    // can't shadow-walk parallel off-trail to escape the corridor.
    let avoid_edges = if prefs.avoid.is_empty() {
        std::collections::HashSet::new()
    } else {
        let radius = prefs
            .avoid_radius_m
            .unwrap_or(effective_cfg.avoid.radius_m)
            .max(0.0);
        let polylines_planar: Vec<Vec<(f64, f64)>> = prefs
            .avoid
            .iter()
            .map(|pl| {
                pl.iter()
                    .map(|c| {
                        let u = *c;
                        (u.x, u.y)
                    })
                    .collect()
            })
            .collect();
        crate::avoid::project_avoided_edges(graph, &polylines_planar, radius)
    };
    let avoid_multiplier = effective_cfg.avoid.edge_multiplier as f32;

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
        mesh_max_grade_deg,
        mesh_gain_k,
        &avoid_edges,
        avoid_multiplier,
    )
    .ok_or(PathfindError::NoRoute)?;

    let geometry: Vec<Point> = route
        .geometry_planar
        .iter()
        .map(|&(x, y)| Point { x, y })
        .collect();
    // Build legs from contiguous on-trail / off-trail runs.
    let seg_len = |k: usize| -> f64 {
        let a = route.geometry_planar[k];
        let b = route.geometry_planar[k + 1];
        ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
    };
    // Cumulative distance along the route, planar metres.
    let mut distances_m: Vec<f64> = Vec::with_capacity(route.geometry_planar.len());
    let mut acc = 0.0f64;
    distances_m.push(0.0);
    for k in 0..route.geometry_planar.len().saturating_sub(1) {
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
        let push = |kind: bool,
                    start: usize,
                    end: usize,
                    len: f64,
                    legs: &mut Vec<PathLeg>,
                    on_m: &mut f64,
                    off_m: &mut f64| {
            legs.push(PathLeg {
                kind: if kind {
                    LegKind::Graph
                } else {
                    LegKind::OffTrailPrefix
                },
                start_idx: start as u32,
                end_idx: end as u32,
                length_m: len,
            });
            if kind {
                *on_m += len
            } else {
                *off_m += len
            }
        };
        for k in 0..route.seg_on_trail.len() {
            if route.seg_on_trail[k] != run_kind {
                push(
                    run_kind, run_start, k, run_len, &mut legs, &mut on_m, &mut off_m,
                );
                run_start = k;
                run_kind = route.seg_on_trail[k];
                run_len = 0.0;
            }
            run_len += seg_len(k);
        }
        push(
            run_kind,
            run_start,
            route.seg_on_trail.len(),
            run_len,
            &mut legs,
            &mut on_m,
            &mut off_m,
        );
    }
    // Per-surface breakdown from the route's per-segment fkb codes, so
    // the response distinguishes trail (sti) from road (vei) from
    // off-trail — the distinction the unified solver previously hid by
    // bucketing every graph edge as "sti".
    let mut fkb_breakdown: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    for k in 0..route.seg_fkb.len() {
        let name = match route.seg_fkb[k] {
            1 => "sti",
            2 => "vei",
            3 => "skiloype",
            255 => "off_trail",
            _ => "unknown",
        };
        *fkb_breakdown.entry(name.to_string()).or_insert(0.0) += seg_len(k);
    }
    fkb_breakdown.retain(|_, v| *v > 0.0);
    let _ = off_m;
    let on_trail_pct = if length_m > 0.0 {
        (on_m / length_m * 100.0) as f32
    } else {
        0.0
    };

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
        waypoint_legs: Vec::new(),
    })
}

fn solve_off_trail(
    ctx: &SolveContext<'_>,
    from_xy: Point,
    to_xy: Point,
    prefs: &Prefs,
) -> Result<Path, PathfindError> {
    let segment = build_off_trail_segment(ctx, from_xy, to_xy, prefs)?;
    let geometry: Vec<Point> = segment
        .geometry
        .iter()
        .map(|p| Point { x: p.x, y: p.y })
        .collect();
    let (distances_m, length_m) = cumulative_distances_planar(&segment.geometry);
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
        waypoint_legs: Vec::new(),
    })
}

/// Build one off-trail mesh between two points (start and goal
/// planar metres), run Theta\*, return the resulting polyline
/// plus cost + observed-refusal layer names.
fn build_off_trail_segment(
    ctx: &SolveContext<'_>,
    from: Point,
    to: Point,
    prefs: &Prefs,
) -> Result<OffTrailSegment, PathfindError> {
    // Off-trail routing is FMM-only. The legacy Theta* mesh fallback was
    // removed: it produced blocky line-of-sight routes and, worse, masked
    // a genuinely unreachable goal (corridor severed by water/cliff/no
    // coverage) with a plausible-looking straight line. On failure the
    // FMM path now returns an honest error (NoRoute) instead of garbage.
    build_off_trail_segment_fmm(ctx, from, to, prefs)
}
