//! Unified single-solve router — the default foot router.
//!
//! ONE A* over a combined node graph:
//!   - off-trail MESH cells over the from→to corridor (16-direction), and
//!   - the TRAIL network (graph edges) over a generous region, and
//!   - zero-cost TRANSITIONS between a trail node and the mesh cell it
//!     sits in (so a hiker can join/leave a trail anywhere a graph node
//!     exists).
//!
//! Every edge is priced in the SAME walk-seconds field: mesh edges via
//! Tobler slope × the per-cell contributor overlay, trail edges via the
//! contributor stack on `EdgeKind::Graph`. Because it is one search, the
//! "follow this trail vs cut across here" decision is made per-edge.
//!
//! ## Shared seams
//! Per the routing-engine unification plan, this module shares the
//! CANONICAL seams with the FMM solver instead of duplicating them:
//! the grid ([`turbo_tiles_fmm::GridShape`] via [`corridor_shape`])
//! and the per-cell cost field ([`crate::cost_field::LazyCostField`]),
//! plus the cost model (the `CostContributor` stack) and the raw data
//! (graph + DEM). The SOLVERS stay independent — this A* and the FMM
//! eikonal solve share no search code — but a cost/grid definition
//! exists exactly once.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use turbo_tiles_elev::{Dem, PointXY};
use turbo_tiles_fmm::GridShape;
use turbo_tiles_graph::{Graph, Profile};

use crate::contributor::{compose_edge_walk_seconds, CostContributor, EdgeContext, EdgeKind};
use crate::native_contributors::OffTrailRoughnessContributor;

// Steep-terrain shaping for off-trail mesh edges (own copy — see module
// docs on independence; these are physical constants, not shared code).
const CLIFF_DEG: f32 = 60.0;
const STEEP_PENALTY_K: f32 = 10.0;

/// Pace floor (s/m) for the admissible A* heuristic: the cheapest any
/// metre can cost (a well-maintained trail — flat base pace 1/1.4 minus
/// the ≈−30% marking+preferred discounts ⇒ ≈0.50). At/above the true min
/// the search becomes a (mildly) weighted A*: it focuses hard toward the
/// goal instead of flooding the wide corridor, trading a few % of
/// optimality for a large speedup on long routes. 0.85 ≈ flat-trail pace,
/// so trail routes stay essentially optimal while off-trail flooding is
/// cut sharply.
const HEURISTIC_MIN_PACE: f64 = 0.85;

/// Cap on trail edges spliced into the unified graph — high enough that a
/// realistic hiking route's trail region isn't stride-sampled (which would
/// fragment connectivity); the sparse trail graph stays cheap at this size.
const MAX_TRAIL_EDGES: usize = 250_000;

/// Walk-seconds charged each time the route gets ON or OFF a trail —
/// "trail stickiness". Without it, dense join points let the route hop off
/// a path to shave a few metres off a minor wiggle (real hikers don't —
/// you stay on the path for the footing). This friction means the route
/// only joins/leaves a trail when it's worth a real amount (a long stretch
/// to follow, or a substantial cut to make), not for trivial savings — so
/// it follows trails sensibly without being forced onto every wiggle.
const TRANSITION_PENALTY_S: f32 = 30.0;

/// Mesh-corridor extent caps (m). The off-trail search must stay bounded
/// (O(d) area, not O(d²)); the trail network spans wider (see `trail_pad`).
const PAD_CAP_M: f64 = 3000.0;
const HALF_WIDTH_CAP_M: f64 = 3000.0;

/// 16-direction move set — 8 compass + 8 knight (2:1) moves. The knight
/// moves add ~26.6°/63.4° headings so off-trail legs travel at natural
/// angles instead of staircasing.
const NEIGH: [(i32, i32); 16] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
    (2, 1),
    (2, -1),
    (-2, 1),
    (-2, -1),
    (1, 2),
    (1, -2),
    (-1, 2),
    (-1, -2),
];

/// Tobler hiking pace (s/m) from gradient magnitude (tan of slope).
/// Own copy — a physical formula, not shared solver code.
#[inline]
fn tobler_pace(grad_mag: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (grad_mag.abs() + 0.05)).exp();
    if v < 1e-4 {
        1.0e6
    } else {
        1.0 / v
    }
}

/// Build the corridor grid for a route: a rectangle oriented to the
/// from→to line, padded enough to detour around local obstacles but
/// capped so the cell count grows O(d), not O(d²). Returns the
/// CANONICAL [`GridShape`] (the same cell↔world mapping the FMM solver
/// uses) — `None` if the endpoints are closer than half a cell.
fn corridor_shape(from: PointXY, to: PointXY, cell_m: f64) -> Option<GridShape> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let d = (dx * dx + dy * dy).sqrt();
    if d < cell_m * 0.5 {
        return None;
    }
    let pad = (4.0 * cell_m).max(0.30 * d).min(PAD_CAP_M);
    let half_width = 800.0_f64.max(0.20 * d).min(HALF_WIDTH_CAP_M);
    let u = (dx / d, dy / d);
    let v = (-u.1, u.0);
    let along = d * 0.5 + pad;
    let cross = half_width + pad;
    let cx = (from.x + to.x) * 0.5;
    let cy = (from.y + to.y) * 0.5;
    let corners = [
        (
            cx + along * u.0 + cross * v.0,
            cy + along * u.1 + cross * v.1,
        ),
        (
            cx + along * u.0 - cross * v.0,
            cy + along * u.1 - cross * v.1,
        ),
        (
            cx - along * u.0 + cross * v.0,
            cy - along * u.1 + cross * v.1,
        ),
        (
            cx - along * u.0 - cross * v.0,
            cy - along * u.1 - cross * v.1,
        ),
    ];
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for (x, y) in corners {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let origin_x = (min_x / cell_m).floor() * cell_m;
    let origin_y = (min_y / cell_m).floor() * cell_m;
    let nx = ((max_x - origin_x) / cell_m).ceil() as u32 + 1;
    let ny = ((max_y - origin_y) / cell_m).ceil() as u32 + 1;
    Some(GridShape::new_2d(nx, ny, origin_x, origin_y, cell_m))
}

/// Result of a unified solve: geometry in EPSG:25833 + a per-segment
/// on-trail flag (for leg colouring / surface breakdown) + walk-seconds cost.
pub(crate) struct UnifiedRoute {
    pub geometry_utm: Vec<(f64, f64)>,
    /// `seg_on_trail[k]` = true if segment `geometry[k]→geometry[k+1]` is
    /// a trail (graph) segment, false if off-trail mesh.
    pub seg_on_trail: Vec<bool>,
    /// Per-segment surface code: the graph edge's `fkb_type` (0 unknown,
    /// 1 sti, 2 vei/road, 3 skiloype) for trail segments, or 255 for
    /// off-trail mesh. Lets the caller break down metres by surface
    /// (trail vs road vs off-trail) honestly.
    pub seg_fkb: Vec<u8>,
    pub cost_s: f64,
}

#[derive(Copy, Clone)]
struct HeapItem {
    f: f32,
    node: u32,
}
impl PartialEq for HeapItem {
    fn eq(&self, o: &Self) -> bool {
        self.f == o.f
    }
}
impl Eq for HeapItem {}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for HeapItem {
    // min-heap: smallest f pops first.
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}

/// Solve a route. Returns `None` when start/goal are degenerate or the
/// goal is unreachable (caller maps that to an honest no-route error).
pub(crate) fn solve_unified(
    graph: &Graph,
    dem: &Arc<Dem>,
    contributors: &[Arc<dyn CostContributor>],
    profile: Profile,
    from: PointXY,
    to: PointXY,
    cell_m: f64,
    base_pace_s_per_m: f32,
    off_trail_factor: f32,
    // Off-trail steepness knob: the grade (deg) above which the soft steep
    // penalty kicks in. Lower = avoid steep ground harder (gentler routes);
    // higher = tolerate steep (direct). Default 27.
    mesh_max_grade_deg: f32,
    // Off-trail climb aversion: extra walk-seconds per metre of positive
    // elevation gain (Naismith-style). 0 = none (default). Higher = "less
    // height difference" — prefer flatter detours.
    mesh_gain_k: f32,
    // "Avoid" set: graph edge ids whose per-metre cost is multiplied by
    // `avoid_multiplier`. Edge-based, so an avoided trail becomes
    // expensive while the mesh beside it stays at ordinary off-trail
    // cost — the route detours onto a divergent trail, never shadow-walks
    // parallel off-trail. Empty = no avoidance.
    avoid_edges: &std::collections::HashSet<u32>,
    // Strong-but-finite multiplier for avoided edges (soft: a start/end on
    // an avoided edge still routes). 1.0 = no effect.
    avoid_multiplier: f32,
) -> Option<UnifiedRoute> {
    let corr = corridor_shape(from, to, cell_m)?;
    let nx = corr.nx as usize;
    let ny = corr.ny as usize;
    let nm = nx * ny;
    let start_cell = corr.world_to_cell(from.x, from.y)?;
    let goal_cell = corr.world_to_cell(to.x, to.y)?;
    let start = (start_cell.1 as usize) * nx + (start_cell.0 as usize);
    let goal = (goal_cell.1 as usize) * nx + (goal_cell.0 as usize);

    // Per-cell off-trail pace/veto overlay. Off-trail roughness rides in as
    // a multiplicative `pace_factor` contributor (per-request/profile), so
    // the factor is applied uniformly by the cost stack and never touches
    // graph (trail) edges.
    let mut mesh_contribs: Vec<Arc<dyn CostContributor>> = contributors.to_vec();
    mesh_contribs.push(Arc::new(OffTrailRoughnessContributor::new(
        off_trail_factor,
    )));
    let overlay = crate::cost_field::LazyCostField::new(
        corr,
        dem.clone(),
        base_pace_s_per_m,
        profile,
        &mesh_contribs,
    );

    // ---- Splice the trail network over a GENEROUS region ----
    // Trails are NOT clipped to the mesh corridor: a long route swinging
    // wide around a lake needs the road network that goes around, which
    // lies far outside the ±3 km corridor. The trail graph is sparse, so
    // including it over a region that scales with route length is cheap and
    // lets the single A* route the long haul on roads. Trail nodes inside
    // the corridor get a 0-cost transition to their cell; nodes outside it
    // are pure long-haul network.
    let d = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
    // Trail region: wide enough to route the long haul on roads around big
    // obstacles, but a much tighter cap than before — a 15–25 km pad pulled
    // in tens of thousands of trail edges and dominated long-route solve
    // time. ~6 km is plenty to go around a lake/fjord neck.
    let trail_pad = (0.25 * d).clamp(2000.0, 6000.0);
    let eids = graph.edge_ids_in_bbox(
        from.x.min(to.x) - trail_pad,
        from.y.min(to.y) - trail_pad,
        from.x.max(to.x) + trail_pad,
        from.y.max(to.y) + trail_pad,
        MAX_TRAIL_EDGES,
    );

    // ---- Build trail nodes from edge POLYLINE VERTICES, not just the two
    // junctions, so the mesh can join/leave a trail anywhere it's near (a
    // hiker uses a path mid-line, not only at its endpoints). For each edge
    // we keep its two junctions (deduped across edges, for the long-haul
    // network) PLUS every interior polyline vertex that falls inside the
    // mesh corridor as an "anchor" join point. Consecutive split points are
    // linked with cost ∝ their polyline sub-length (total edge cost
    // preserved) and carry the vertex range `(eid, k_from, k_to)` so
    // extraction splices the exact trail shape.
    //
    // Far edges (entirely outside the corridor) get no interior anchors —
    // just junction→junction — so long routes don't bloat: anchors exist
    // only where the mesh can actually transition onto them.
    let mut trail_pos: Vec<(f64, f64)> = Vec::new();
    // (to_global_node, cost_s, edge_id, k_from, k_to)
    let mut trail_adj: Vec<Vec<(u32, f32, u32, u32, u32)>> = Vec::new();
    let mut junction: HashMap<u32, u32> = HashMap::new(); // graph node id → trail node
    for &eid in &eids {
        let Some(er) = graph.edge(eid) else { continue };
        if er.length_m as f64 <= 0.0 {
            continue;
        }
        // SYNTHETIC coords (origin): price from baked metadata (length +
        // slope + marking + preferred), NOT by sampling the terrain, so a
        // trail bridging a river isn't water-vetoed (which would fragment
        // the network at every crossing).
        let ctx = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: er.length_m as f64,
            ty: 0.0,
            length_m: er.length_m as f64,
            profile,
            kind: EdgeKind::Graph(er),
            elev_probe: None,
        };
        let cost = compose_edge_walk_seconds(contributors, &ctx);
        if !cost.total_walk_seconds.is_finite() {
            continue; // profile forbids this edge
        }
        let poly = graph.edge_polyline(eid);
        if poly.len() < 2 {
            continue;
        }
        let seg: Vec<f64> = (0..poly.len() - 1)
            .map(|k| ((poly[k + 1].x - poly[k].x) as f64).hypot((poly[k + 1].y - poly[k].y) as f64))
            .collect();
        let total: f64 = seg.iter().sum();
        if total <= 0.0 {
            continue;
        }
        let mut per_m = cost.total_walk_seconds / total;
        // Edge-based avoidance: this trail edge runs along avoided
        // geometry, so make traversing it expensive (but finite). The
        // penalty rides on every sub-segment via `per_m`, so a partial
        // splice pays proportionally.
        if avoid_multiplier > 1.0 && avoid_edges.contains(&eid) {
            per_m *= avoid_multiplier as f64;
        }

        // Split indices: both junctions + interior vertices inside the corridor.
        let last = poly.len() - 1;
        let mut splits = vec![0usize];
        for (k, p) in poly.iter().enumerate().take(last).skip(1) {
            if corr.world_to_cell(p.x as f64, p.y as f64).is_some() {
                splits.push(k);
            }
        }
        splits.push(last);

        // Resolve each split index to a trail node id (junctions deduped).
        let mut nodes: Vec<u32> = Vec::with_capacity(splits.len());
        for &k in &splits {
            let pos = (poly[k].x as f64, poly[k].y as f64);
            let nid = if k == 0 || k == last {
                let gid = if k == 0 { er.from_id } else { er.to_id };
                *junction.entry(gid).or_insert_with(|| {
                    let n = trail_pos.len() as u32;
                    trail_pos.push(pos);
                    trail_adj.push(Vec::new());
                    n
                })
            } else {
                let n = trail_pos.len() as u32;
                trail_pos.push(pos);
                trail_adj.push(Vec::new());
                n
            };
            nodes.push(nid);
        }

        // Link consecutive split points (bidirectional), carrying the
        // polyline vertex range for exact-geometry extraction.
        for i in 0..splits.len() - 1 {
            let (ka, kb) = (splits[i], splits[i + 1]);
            let (na, nb) = (nodes[i], nodes[i + 1]);
            let sub_len: f64 = seg[ka..kb].iter().sum();
            let c = (sub_len * per_m) as f32;
            trail_adj[na as usize].push((nm as u32 + nb, c, eid, ka as u32, kb as u32));
            trail_adj[nb as usize].push((nm as u32 + na, c, eid, kb as u32, ka as u32));
        }
    }
    let nt = trail_pos.len();

    // Cell → trail nodes sitting in it (transition endpoints).
    let mut cell_trails: HashMap<usize, Vec<u32>> = HashMap::new();
    for (t, &(x, y)) in trail_pos.iter().enumerate() {
        if let Some((ci, cj)) = corr.world_to_cell(x, y) {
            cell_trails
                .entry((cj as usize) * nx + (ci as usize))
                .or_default()
                .push(t as u32);
        }
    }

    let n_total = nm + nt;
    if std::env::var("UNIFIED_DEBUG").is_ok() {
        let adj: usize = trail_adj.iter().map(|v| v.len()).sum();
        let start_has = cell_trails.get(&start).map(|v| v.len()).unwrap_or(0);
        let goal_has = cell_trails.get(&goal).map(|v| v.len()).unwrap_or(0);
        eprintln!(
            "UNIFIED: corr {nx}x{ny} cell_m={:.0}; eids={} trail_nodes={nt} adj_edges={adj} \
             cells_with_trail={} start_cell_trails={start_has} goal_cell_trails={goal_has}",
            corr.cell_m,
            eids.len(),
            cell_trails.len()
        );
    }
    let goal_pos = corr.cell_centre(goal_cell.0, goal_cell.1);
    let pos_of = |node: usize| -> (f64, f64) {
        if node < nm {
            corr.cell_centre((node % nx) as u32, (node / nx) as u32)
        } else {
            trail_pos[node - nm]
        }
    };
    let heuristic = |node: usize| -> f32 {
        let (x, y) = pos_of(node);
        (((x - goal_pos.0).powi(2) + (y - goal_pos.1).powi(2)).sqrt() * HEURISTIC_MIN_PACE) as f32
    };

    // Off-trail mesh-edge cost (walk-seconds): Tobler slope × per-cell
    // overlay × steep-penalty. `None` = impassable (refused cell or cliff).
    let mesh_step = |i: u32, j: u32, ni: u32, nj: u32| -> Option<f32> {
        if overlay.refused(ni, nj) {
            return None;
        }
        let (di, dj) = (ni as i32 - i as i32, nj as i32 - j as i32);
        let span = di.abs().max(dj.abs());
        if span > 1 {
            // Knight (2:1) move spans an intermediate cell — reject if the
            // chord clips a refused cell (no jumping over a lake sliver).
            let steps = span * 2;
            for s in 1..steps {
                let t = s as f32 / steps as f32;
                let ci = (i as f32 + di as f32 * t).round() as i32;
                let cj = (j as f32 + dj as f32 * t).round() as i32;
                if ci >= 0
                    && cj >= 0
                    && (ci as u32) < corr.nx
                    && (cj as u32) < corr.ny
                    && overlay.refused(ci as u32, cj as u32)
                {
                    return None;
                }
            }
        }
        let (cx0, cy0) = corr.cell_centre(i, j);
        let (cx1, cy1) = corr.cell_centre(ni, nj);
        let step_m = (((cx1 - cx0).powi(2) + (cy1 - cy0).powi(2)).sqrt()) as f32;
        let mul = overlay.pace_mul(ni, nj);
        let z0 = overlay.elevation(i, j);
        let z1 = overlay.elevation(ni, nj);
        let cost = match (z0, z1) {
            (Some(a), Some(b)) => {
                let grad = ((b - a) / step_m).abs();
                let grade_deg = grad.atan().to_degrees();
                if grade_deg > CLIFF_DEG {
                    return None;
                }
                let steep = if grade_deg > mesh_max_grade_deg {
                    let over = (grade_deg - mesh_max_grade_deg) / mesh_max_grade_deg.max(1.0);
                    1.0 + STEEP_PENALTY_K * over * over
                } else {
                    1.0
                };
                // Naismith-style climb aversion: extra seconds per metre
                // of positive gain (0 by default → no change).
                let gain = if mesh_gain_k > 0.0 && b > a {
                    mesh_gain_k * (b - a)
                } else {
                    0.0
                };
                step_m * tobler_pace(grad) * mul * steep + gain
            }
            // Missing elevation: passable but discouraged (flat × 3).
            _ => step_m * base_pace_s_per_m * 3.0 * mul,
        };
        Some(cost)
    };

    // ---- A* over mesh ∪ trail ----
    let mut g: Vec<f32> = vec![f32::INFINITY; n_total];
    let mut prev: Vec<u32> = vec![u32::MAX; n_total];
    // Per node: the trail sub-segment used to reach it — (edge_id, k_from,
    // k_to) into that edge's polyline. `(u32::MAX, _, _)` = mesh/transition.
    let mut prev_seg: Vec<(u32, u32, u32)> = vec![(u32::MAX, 0, 0); n_total];
    let mut heap: BinaryHeap<HeapItem> = BinaryHeap::new();
    g[start] = 0.0;
    heap.push(HeapItem {
        f: heuristic(start),
        node: start as u32,
    });

    // Live progress: stream the best-path-so-far reaching toward the
    // goal as A* advances (same mechanism the FMM off-trail solver
    // uses), so the SPA's blue preview grows out from the start. The
    // path reconstruction lives INSIDE the `record` closure, so it
    // costs nothing unless a recorder is installed (record=true / SSE).
    // (The enclosing `solve_unified` phase is opened by the caller.)
    let emit_stride = (n_total as u64 / 120).max(32);
    let mut best_h = f32::INFINITY;
    let mut pops: u64 = 0;
    let mut last_emit: u64 = 0;

    let mut reached = false;
    while let Some(HeapItem { node, .. }) = heap.pop() {
        let node = node as usize;
        if node == goal {
            reached = true;
            break;
        }
        // Snapshot the route to the closest-to-goal node seen so far,
        // throttled so a large corridor emits ~120 frames, not millions.
        pops += 1;
        let h = heuristic(node);
        if h < best_h {
            best_h = h;
            if pops - last_emit >= emit_stride {
                last_emit = pops;
                crate::solver_trace::record(|| {
                    let mut coords: Vec<[f32; 2]> = Vec::new();
                    let mut v = node;
                    loop {
                        let (x, y) = pos_of(v);
                        coords.push([x as f32, y as f32]);
                        if v == start {
                            break;
                        }
                        let p = prev[v];
                        if p == u32::MAX {
                            break;
                        }
                        v = p as usize;
                    }
                    coords.reverse();
                    crate::solver_trace::SolverEvent::BestPathSnapshot { coords }
                });
            }
        }
        let gu = g[node];
        let relax = |v: usize,
                     w: f32,
                     seg: (u32, u32, u32),
                     g: &mut [f32],
                     prev: &mut [u32],
                     prev_seg: &mut [(u32, u32, u32)],
                     heap: &mut BinaryHeap<HeapItem>| {
            if !w.is_finite() {
                return;
            }
            let nd = gu + w;
            if nd < g[v] {
                g[v] = nd;
                prev[v] = node as u32;
                prev_seg[v] = seg;
                heap.push(HeapItem {
                    f: nd + heuristic(v),
                    node: v as u32,
                });
            }
        };
        const NONE_SEG: (u32, u32, u32) = (u32::MAX, 0, 0);
        if node < nm {
            let (i, j) = ((node % nx) as u32, (node / nx) as u32);
            for (di, dj) in NEIGH {
                let ni = i as i32 + di;
                let nj = j as i32 + dj;
                if ni < 0 || nj < 0 || ni >= nx as i32 || nj >= ny as i32 {
                    continue;
                }
                if let Some(w) = mesh_step(i, j, ni as u32, nj as u32) {
                    relax(
                        (nj as usize) * nx + (ni as usize),
                        w,
                        NONE_SEG,
                        &mut g,
                        &mut prev,
                        &mut prev_seg,
                        &mut heap,
                    );
                }
            }
            if let Some(ts) = cell_trails.get(&node) {
                for &t in ts {
                    // Getting ON a trail costs the stickiness penalty.
                    relax(
                        nm + t as usize,
                        TRANSITION_PENALTY_S,
                        NONE_SEG,
                        &mut g,
                        &mut prev,
                        &mut prev_seg,
                        &mut heap,
                    );
                }
            }
        } else {
            let t = node - nm;
            for &(to_global, w, eid, kf, kt) in &trail_adj[t] {
                relax(
                    to_global as usize,
                    w,
                    (eid, kf, kt),
                    &mut g,
                    &mut prev,
                    &mut prev_seg,
                    &mut heap,
                );
            }
            let (x, y) = trail_pos[t];
            if let Some((ci, cj)) = corr.world_to_cell(x, y) {
                // Getting OFF a trail costs the stickiness penalty.
                relax(
                    (cj as usize) * nx + (ci as usize),
                    TRANSITION_PENALTY_S,
                    NONE_SEG,
                    &mut g,
                    &mut prev,
                    &mut prev_seg,
                    &mut heap,
                );
            }
        }
    }

    if !reached || !g[goal].is_finite() {
        return None;
    }

    // ---- Backtrack, splicing the exact trail sub-polylines ----
    let mut chain: Vec<usize> = vec![goal];
    let mut cur = goal;
    while cur != start {
        let p = prev[cur];
        if p == u32::MAX {
            return None;
        }
        chain.push(p as usize);
        cur = p as usize;
    }
    chain.reverse();

    let mut geom: Vec<(f64, f64)> = vec![pos_of(chain[0])];
    let mut on_trail: Vec<bool> = Vec::new();
    // Per-segment surface code (255 = off-trail mesh, else the edge's
    // fkb_type) so the caller can split metres into trail/road/off-trail.
    let mut on_fkb: Vec<u8> = Vec::new();
    for w in chain.windows(2) {
        let v = w[1];
        let (eid, kf, kt) = prev_seg[v];
        if eid != u32::MAX {
            // Trail sub-segment: splice poly[kf..kt] (oriented), skipping
            // the first vertex (already the current tail).
            let fkb = graph.edge(eid).map(|e| e.fkb_type).unwrap_or(0);
            let poly = graph.edge_polyline(eid);
            if kf <= kt {
                for p in &poly[(kf as usize + 1)..=(kt as usize)] {
                    geom.push((p.x as f64, p.y as f64));
                    on_trail.push(true);
                    on_fkb.push(fkb);
                }
            } else {
                let mut k = kf as usize;
                while k > kt as usize {
                    k -= 1;
                    geom.push((poly[k].x as f64, poly[k].y as f64));
                    on_trail.push(true);
                    on_fkb.push(fkb);
                }
            }
        } else {
            geom.push(pos_of(v));
            on_trail.push(false);
            on_fkb.push(255);
        }
    }

    // Smooth off-trail runs (trail runs stay exact).
    let (geometry_utm, seg_on_trail, seg_fkb) =
        smooth_off_trail(&geom, &on_trail, &on_fkb, &corr, &overlay);

    // Final snapshot: the exact answer, so the live preview snaps into
    // place when the solve completes.
    crate::solver_trace::record(|| crate::solver_trace::SolverEvent::BestPathSnapshot {
        coords: geometry_utm
            .iter()
            .map(|&(x, y)| [x as f32, y as f32])
            .collect(),
    });

    // refused_by is empty by construction: `mesh_step` never steps onto a
    // refused cell and trails bridge water legitimately.
    Some(UnifiedRoute {
        geometry_utm,
        seg_on_trail,
        seg_fkb,
        cost_s: g[goal] as f64,
    })
}

/// Corner-cutting Chaikin on a fixed-endpoint polyline.
fn chaikin(pts: &[(f64, f64)], iters: u32) -> Vec<(f64, f64)> {
    let mut cur = pts.to_vec();
    for _ in 0..iters {
        if cur.len() < 3 {
            break;
        }
        let mut next = vec![cur[0]];
        for w in cur.windows(2) {
            let (a, b) = (w[0], w[1]);
            next.push((0.75 * a.0 + 0.25 * b.0, 0.75 * a.1 + 0.25 * b.1));
            next.push((0.25 * a.0 + 0.75 * b.0, 0.25 * a.1 + 0.75 * b.1));
        }
        next.push(*cur.last().unwrap());
        cur = next;
    }
    cur
}

/// Smooth contiguous off-trail runs; trail runs pass through verbatim.
/// Reverts any run whose smoothed line would clip a refused cell.
fn smooth_off_trail(
    geom: &[(f64, f64)],
    on_trail: &[bool],
    seg_fkb: &[u8],
    corr: &GridShape,
    overlay: &crate::cost_field::LazyCostField,
) -> (Vec<(f64, f64)>, Vec<bool>, Vec<u8>) {
    if geom.len() < 3 {
        return (geom.to_vec(), on_trail.to_vec(), seg_fkb.to_vec());
    }
    let clear = |pts: &[(f64, f64)]| -> bool {
        for w in pts.windows(2) {
            let dd = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
            let steps = ((dd / (0.4 * corr.cell_m)).ceil() as i32).max(1);
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let x = w[0].0 + (w[1].0 - w[0].0) * t;
                let y = w[0].1 + (w[1].1 - w[0].1) * t;
                if let Some((ci, cj)) = corr.world_to_cell(x, y) {
                    if overlay.refused(ci, cj) {
                        return false;
                    }
                }
            }
        }
        true
    };
    let mut out_geom: Vec<(f64, f64)> = vec![geom[0]];
    let mut out_on: Vec<bool> = Vec::new();
    let mut out_fkb: Vec<u8> = Vec::new();
    let n = on_trail.len();
    let mut k = 0usize;
    while k < n {
        let kind = on_trail[k];
        let run_start = k;
        while k < n && on_trail[k] == kind {
            k += 1;
        }
        let verts: Vec<(f64, f64)> = geom[run_start..=k].to_vec();
        if kind {
            // Trail run is exact: one output segment per input segment, so
            // the per-segment fkb codes carry through unchanged.
            for (i, p) in verts.iter().enumerate().skip(1) {
                out_geom.push(*p);
                out_on.push(true);
                out_fkb.push(seg_fkb[run_start + i - 1]);
            }
        } else {
            // Off-trail run is smoothed (segment count changes); every
            // output segment is off-trail (255).
            let sm = chaikin(&verts, 2);
            let pts = if clear(&sm) { sm } else { verts };
            for p in pts.iter().skip(1) {
                out_geom.push(*p);
                out_on.push(false);
                out_fkb.push(255);
            }
        }
    }
    (out_geom, out_on, out_fkb)
}
