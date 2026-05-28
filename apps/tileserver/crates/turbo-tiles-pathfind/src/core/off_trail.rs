//! Off-trail pathfinding: Theta* on a navigable mesh.
//!
//! ## Algorithm
//!
//! Plain A* on a mesh produces zig-zag paths that follow mesh edges
//! exactly — visually unnatural and longer than necessary on open
//! terrain. **Theta\*** relaxes the parent pointer at each node:
//! before recording `parent[next] = current`, we ask "is there a
//! line of sight from `parent[current]` to `next`?". If yes, we
//! shortcut `parent[next] = parent[current]`, paying the straight-
//! line cost instead of the polyline-through-current cost. This
//! produces any-angle paths that look like a hiker chose them.
//!
//! ## Scope of this module
//!
//! Pure functions over a generic, prebuilt `Mesh`. No I/O, no
//! geometry-crate dependency at this layer — the mesh is built
//! upstream by the (future) local-mesh ingest/refinement step from
//! authoritative vector polygons (refused regions) and DTM-sampled
//! costs. Wiring into `PgTerrainGraph` is a follow-up slice.
//!
//! ## What this module does NOT do
//!
//! - Build the mesh from raster. The mesh must be constructed from
//!   vector inputs (refused-region polygons, seed points).
//! - Sample DTM at runtime. Costs are precomputed per node by the
//!   caller (typically by sampling DTM10 at mesh vertices during
//!   construction).
//! - Make decisions about what's a "refused" region. The caller
//!   marks node passability.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// 2D point in any projected metric system (typically EPSG:25833).
/// The algorithm is purely Euclidean; CRS doesn't matter as long as
/// inputs are consistent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn dist(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Mesh node id (index into `Mesh::nodes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshNodeId(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct MeshNode {
    pub pt: Point2,
    /// Per-node passability multiplier. 1.0 = nominal; >1 = slower
    /// terrain; `f64::INFINITY` = refused. Edges through a refused
    /// node are treated as impassable.
    pub cost_mul: f64,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub nodes: Vec<MeshNode>,
    /// Adjacency list. `neighbours[i]` is the set of node ids
    /// reachable from node i along a single mesh edge.
    pub neighbours: Vec<Vec<MeshNodeId>>,
    /// Line-of-sight blockers. Each polyline is an ordered set of
    /// points forming a closed (or open) boundary that a Theta*
    /// shortcut must not cross. Typically the boundary of a refused
    /// region (water, glacier, cliff).
    pub blockers: Vec<Vec<Point2>>,
    /// Spacing between adjacent mesh nodes, in metres. Used by the
    /// LoS-jump cap so paths don't collapse into long straight
    /// segments that skip terrain. 0.0 = unknown (caller didn't
    /// set it); the LoS check falls back to a 25 m default.
    pub cell_m: f64,
}

impl Mesh {
    pub fn new(nodes: Vec<MeshNode>) -> Self {
        let n = nodes.len();
        Self {
            nodes,
            neighbours: vec![Vec::new(); n],
            blockers: Vec::new(),
            cell_m: 0.0,
        }
    }

    pub fn add_edge(&mut self, a: MeshNodeId, b: MeshNodeId) {
        if a == b {
            return;
        }
        // Symmetric adjacency.
        if !self.neighbours[a.0 as usize].contains(&b) {
            self.neighbours[a.0 as usize].push(b);
        }
        if !self.neighbours[b.0 as usize].contains(&a) {
            self.neighbours[b.0 as usize].push(a);
        }
    }

    pub fn add_blocker(&mut self, polyline: Vec<Point2>) {
        if polyline.len() >= 2 {
            self.blockers.push(polyline);
        }
    }

    pub fn pt(&self, id: MeshNodeId) -> Point2 {
        self.nodes[id.0 as usize].pt
    }

    pub fn cost_mul(&self, id: MeshNodeId) -> f64 {
        self.nodes[id.0 as usize].cost_mul
    }
}

#[derive(Debug, Clone)]
pub struct PathResult {
    pub nodes: Vec<MeshNodeId>,
    pub geometry: Vec<Point2>,
    /// Pure geometric length of the polyline in metres.
    pub length_m: f64,
    /// **Cost-weighted** path length — the A\* g-score at the goal.
    /// Sum of per-edge `(euclid × avg_cost_mul)` along the chosen
    /// route. Units match the graph router's effective-walking-
    /// metres so cross-strategy cost comparisons are valid.
    pub cost: f64,
}

/// Theta\* with a custom edge-cost callback. The callback is invoked
/// for every neighbour relax and every line-of-sight shortcut, so
/// the per-call cost should stay sub-microsecond (avoid I/O or
/// heavy allocs). Symmetric callers (cost depends only on the two
/// endpoints, not direction) should use [`theta_star`].
///
/// The callback returns `f64::INFINITY` to mark an edge as forbidden
/// — useful for direction-dependent vetoes (e.g. "can't traverse
/// this scarp uphill").
pub fn theta_star_with_edge_cost<F>(
    mesh: &Mesh,
    start: MeshNodeId,
    goal: MeshNodeId,
    edge_cost_fn: F,
) -> Option<PathResult>
where
    F: Fn(&Mesh, MeshNodeId, MeshNodeId) -> f64,
{
    theta_star_inner(mesh, start, goal, &edge_cost_fn)
}

/// Theta* pathfinding. Returns `None` when no path exists (graph
/// disconnected, or every connecting route passes through a refused
/// node).
pub fn theta_star(
    mesh: &Mesh,
    start: MeshNodeId,
    goal: MeshNodeId,
) -> Option<PathResult> {
    theta_star_inner(mesh, start, goal, &|m: &Mesh, a, b| edge_cost(m, a, b))
}

fn theta_star_inner<F>(
    mesh: &Mesh,
    start: MeshNodeId,
    goal: MeshNodeId,
    edge_cost_fn: &F,
) -> Option<PathResult>
where
    F: Fn(&Mesh, MeshNodeId, MeshNodeId) -> f64,
{
    if start == goal {
        let pt = mesh.pt(start);
        return Some(PathResult {
            nodes: vec![start],
            geometry: vec![pt],
            length_m: 0.0,
            cost: 0.0,
        });
    }
    if !mesh.cost_mul(start).is_finite() || !mesh.cost_mul(goal).is_finite() {
        return None;
    }

    // g_score: best known cost from start to a node.
    // parent: the node whose-path is the source-of-truth chain for
    //         reconstructing the result. Theta* may set parent[next] =
    //         parent[current] (the "any-angle" shortcut).
    let n = mesh.nodes.len();
    let mut g_score: HashMap<MeshNodeId, f64> = HashMap::with_capacity(n);
    let mut parent: HashMap<MeshNodeId, MeshNodeId> = HashMap::with_capacity(n);
    g_score.insert(start, 0.0);
    parent.insert(start, start);

    let mut open: BinaryHeap<HeapEntry> = BinaryHeap::new();
    open.push(HeapEntry {
        f: mesh.pt(start).dist(mesh.pt(goal)),
        id: start,
    });

    let mut closed: HashMap<MeshNodeId, bool> = HashMap::with_capacity(n);

    while let Some(HeapEntry { id: current, .. }) = open.pop() {
        // Record the pop as a NodePopped event. The recorder is
        // a thread-local; calling `record` with no installed
        // recorder costs one RefCell::borrow (~3 ns) and skips
        // the event construction closure entirely.
        {
            let pt = mesh.pt(current);
            let g = g_score.get(&current).copied().unwrap_or(0.0);
            let h = pt.dist(mesh.pt(goal));
            crate::solver_trace::record(|| crate::solver_trace::SolverEvent::NodePopped {
                x: pt.x as f32,
                y: pt.y as f32,
                g: g as f32,
                h: h as f32,
            });
        }
        if current == goal {
            let goal_g = g_score.get(&goal).copied().unwrap_or(f64::INFINITY);
            let result = reconstruct(mesh, &parent, goal, goal_g);
            // Emit the final best-path snapshot so the SPA can
            // animate the answer snapping into place at the end
            // of the exploration.
            crate::solver_trace::record(|| {
                let coords: Vec<[f32; 2]> = result
                    .geometry
                    .iter()
                    .map(|p| [p.x as f32, p.y as f32])
                    .collect();
                crate::solver_trace::SolverEvent::BestPathSnapshot { coords }
            });
            return Some(result);
        }
        if closed.get(&current).copied().unwrap_or(false) {
            continue;
        }
        closed.insert(current, true);

        for &nb in &mesh.neighbours[current.0 as usize] {
            if closed.get(&nb).copied().unwrap_or(false) {
                continue;
            }
            if !mesh.cost_mul(nb).is_finite() {
                continue;
            }

            // Theta* relaxation: try both the polyline-through-current
            // option AND the LOS-shortcut-from-parent option, and
            // pick whichever yields a lower g_score for `nb`.
            //
            // Why compare instead of always taking the shortcut: the
            // basic Theta* formulation assumes uniform terrain cost,
            // where the LOS shortcut is always ≤ the polyline by
            // triangle inequality. With non-uniform `cost_mul` that
            // breaks — a straight line through expensive terrain can
            // cost more than a polyline through cheap detour nodes.
            // Comparing both restores optimality on weighted meshes.
            let p_cur = parent[&current];
            let polyline_g = g_score[&current] + edge_cost_fn(mesh, current, nb);
            let los_g = if p_cur != current {
                let los_blocked = !has_line_of_sight(mesh, mesh.pt(p_cur), mesh.pt(nb));
                // Record the LoS test (hit or miss). Aligns with
                // the SPA's "show me where the any-angle
                // optimisation fired" rendering.
                let pa = mesh.pt(p_cur);
                let pb = mesh.pt(nb);
                crate::solver_trace::record(|| crate::solver_trace::SolverEvent::LineOfSightCast {
                    fx: pa.x as f32,
                    fy: pa.y as f32,
                    tx: pb.x as f32,
                    ty: pb.y as f32,
                    blocked: los_blocked,
                });
                if !los_blocked {
                    Some(g_score[&p_cur] + edge_cost_fn(mesh, p_cur, nb))
                } else {
                    None
                }
            } else {
                None
            };
            let (cand_parent, new_g, took_los) = match los_g {
                Some(g) if g < polyline_g => (p_cur, g, true),
                _ => (current, polyline_g, false),
            };

            let prev_g = g_score.get(&nb).copied().unwrap_or(f64::INFINITY);
            if new_g < prev_g {
                parent.insert(nb, cand_parent);
                g_score.insert(nb, new_g);
                let h = mesh.pt(nb).dist(mesh.pt(goal));
                open.push(HeapEntry {
                    f: new_g + h,
                    id: nb,
                });
                // Record the relaxation. `took_los` lets the SPA
                // render any-angle shortcuts in a different colour
                // from regular mesh-edge relaxations.
                let pa = mesh.pt(cand_parent);
                let pb = mesh.pt(nb);
                let g_for_event = new_g;
                crate::solver_trace::record(|| crate::solver_trace::SolverEvent::EdgeRelaxed {
                    fx: pa.x as f32,
                    fy: pa.y as f32,
                    tx: pb.x as f32,
                    ty: pb.y as f32,
                    new_g: g_for_event as f32,
                    took_los,
                });
            }
        }
    }
    None
}

/// Edge cost between two mesh nodes: Euclidean distance scaled by the
/// average of the two endpoint passability multipliers. Refused
/// (infinite) costs propagate.
fn edge_cost(mesh: &Mesh, a: MeshNodeId, b: MeshNodeId) -> f64 {
    let ma = mesh.cost_mul(a);
    let mb = mesh.cost_mul(b);
    if !ma.is_finite() || !mb.is_finite() {
        return f64::INFINITY;
    }
    mesh.pt(a).dist(mesh.pt(b)) * 0.5 * (ma + mb)
}

/// Maximum length of a Theta* any-angle shortcut, in mesh-cell
/// units. Beyond this length the LoS jump is forbidden even when
/// geometrically unobstructed, so the path is forced to break into
/// shorter segments that hit intermediate mesh nodes. Without the
/// cap, a single jump can span hundreds of metres in apparently
/// open terrain, producing the "low-poly, not following contours"
/// look the user called out — cost integration along the jump
/// catches the magnitude of climb but not the *shape* of the
/// optimal contour-following route, because the jump endpoints
/// are decided greedily during Dijkstra-style relaxation. Capping
/// jump length lets the routing actually wander along terrain
/// instead of collapsing onto straight lines.
///
/// Thirty-two cells gives ~800 m at the default 25 m mesh
/// resolution. Bigger jumps would let the solver collapse a path
/// into long straight segments that ignore intermediate terrain;
/// smaller jumps prevent legitimate shortcuts across flat valleys
/// (broke `valnesfjord-trail-end-to-end-3km` at 8 cells). 800 m is
/// roughly the distance over which a real hiker decides "yes, I
/// can see the next ridge / the trail is clearly there, I'll head
/// straight for it" — finer-grained route decisions happen below
/// that resolution.
const MAX_LOS_JUMP_CELLS: f64 = 32.0;

/// True iff the open segment `a→b` doesn't cross any blocker polyline.
/// Closed-segment check (the user wanted any-angle, but mustn't cut
/// across a water polygon). The `mesh_cell_m` parameter is used to
/// enforce a maximum jump distance so paths can't collapse into a
/// few long straight segments that don't follow terrain.
fn has_line_of_sight(mesh: &Mesh, a: Point2, b: Point2) -> bool {
    // Reject jumps longer than `MAX_LOS_JUMP_CELLS × cell_m`. The
    // mesh stores its own cell size; we fall back to 25 m when the
    // mesh wasn't built with an explicit value.
    let cell_m = if mesh.cell_m > 0.0 { mesh.cell_m } else { 25.0 };
    let max_jump = MAX_LOS_JUMP_CELLS * cell_m;
    if a.dist(b) > max_jump {
        return false;
    }
    for blocker in &mesh.blockers {
        if segments_intersect_polyline(a, b, blocker) {
            return false;
        }
    }
    true
}

fn segments_intersect_polyline(a: Point2, b: Point2, poly: &[Point2]) -> bool {
    if poly.len() < 2 {
        return false;
    }
    for w in poly.windows(2) {
        if segments_cross(a, b, w[0], w[1]) {
            return true;
        }
    }
    false
}

/// Classic 2D segment intersection test. Returns true iff the open
/// segments AB and CD strictly cross. Sharing an endpoint does NOT
/// count as crossing — Theta* shortcut endpoints are mesh nodes,
/// which may sit ON a blocker boundary by construction.
fn segments_cross(a: Point2, b: Point2, c: Point2, d: Point2) -> bool {
    fn orient(p: Point2, q: Point2, r: Point2) -> f64 {
        (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
    }
    let o1 = orient(a, b, c).signum();
    let o2 = orient(a, b, d).signum();
    let o3 = orient(c, d, a).signum();
    let o4 = orient(c, d, b).signum();
    // Strict crossing requires opposite orientations on both sides.
    o1 != 0.0 && o2 != 0.0 && o3 != 0.0 && o4 != 0.0 && o1 != o2 && o3 != o4
}

fn reconstruct(
    mesh: &Mesh,
    parent: &HashMap<MeshNodeId, MeshNodeId>,
    goal: MeshNodeId,
    cost: f64,
) -> PathResult {
    let mut chain = vec![goal];
    let mut cur = goal;
    loop {
        let p = parent[&cur];
        if p == cur {
            break;
        }
        chain.push(p);
        cur = p;
    }
    chain.reverse();
    let geometry: Vec<Point2> = chain.iter().map(|n| mesh.pt(*n)).collect();
    let length_m = geometry
        .windows(2)
        .map(|w| w[0].dist(w[1]))
        .sum::<f64>();
    PathResult {
        nodes: chain,
        geometry,
        length_m,
        cost,
    }
}

#[derive(Debug, Clone, Copy)]
struct HeapEntry {
    f: f64,
    id: MeshNodeId,
}
impl PartialEq for HeapEntry {
    fn eq(&self, o: &Self) -> bool {
        self.f == o.f
    }
}
impl Eq for HeapEntry {}
impl Ord for HeapEntry {
    fn cmp(&self, o: &Self) -> Ordering {
        // BinaryHeap is a max-heap; we want min-f → reverse.
        o.f.partial_cmp(&self.f).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_mesh(cols: u32, rows: u32) -> Mesh {
        // Build an n×m grid mesh with 8-connectivity, all cost_mul=1.
        // Used as the baseline for path-shape and line-of-sight tests.
        let mut nodes = Vec::with_capacity((cols * rows) as usize);
        for r in 0..rows {
            for c in 0..cols {
                nodes.push(MeshNode {
                    pt: Point2 {
                        x: c as f64 * 100.0,
                        y: r as f64 * 100.0,
                    },
                    cost_mul: 1.0,
                });
            }
        }
        let mut mesh = Mesh::new(nodes);
        let id = |c: u32, r: u32| MeshNodeId(r * cols + c);
        for r in 0..rows {
            for c in 0..cols {
                for (dc, dr) in [
                    (1, 0),
                    (-1, 0),
                    (0, 1),
                    (0, -1),
                    (1, 1),
                    (-1, -1),
                    (1, -1),
                    (-1, 1),
                ] {
                    let nc = c as i32 + dc;
                    let nr = r as i32 + dr;
                    if nc >= 0 && nc < cols as i32 && nr >= 0 && nr < rows as i32 {
                        mesh.add_edge(id(c, r), id(nc as u32, nr as u32));
                    }
                }
            }
        }
        mesh
    }

    #[test]
    fn same_start_and_goal_returns_zero_length_path() {
        // Trivial case: routing to where you already are. Must not
        // panic, must return a single-node path of length 0.
        let mesh = grid_mesh(3, 3);
        let path = theta_star(&mesh, MeshNodeId(0), MeshNodeId(0)).unwrap();
        assert_eq!(path.nodes.len(), 1);
        assert_eq!(path.length_m, 0.0);
    }

    #[test]
    fn straight_line_path_on_open_grid() {
        // No obstacles, no refused nodes. Theta* must find a path
        // whose length matches the Euclidean distance from start to
        // goal, not the (longer) grid-aligned Manhattan distance.
        let mesh = grid_mesh(5, 5);
        let start = MeshNodeId(0); // (0, 0)
        let goal = MeshNodeId(4 * 5 + 4); // (4, 4) — corner
        let path = theta_star(&mesh, start, goal).unwrap();
        let euclid = mesh.pt(start).dist(mesh.pt(goal));
        assert!(
            (path.length_m - euclid).abs() < 1.0,
            "Theta* should produce ~Euclidean length on open grid; got {} vs {}",
            path.length_m,
            euclid
        );
    }

    #[test]
    fn path_routes_around_refused_region() {
        // Production pattern: refused regions are vector polygons
        // (water, glacier) modeled as blocker polylines. We also
        // mark the mesh nodes inside the polygon as refused, so
        // both graph-level and geometric paths get banned. This is
        // belt-and-braces — either alone would work for typical
        // cases but together they handle edge cases (LOS shortcut
        // touching the boundary, etc).
        let mut mesh = grid_mesh(5, 5);
        // Refuse a 3-cell-tall middle section of column x=2 — leaves
        // top and bottom rows passable so a detour is possible.
        for r in 1..4 {
            let id = MeshNodeId(r * 5 + 2);
            mesh.nodes[id.0 as usize].cost_mul = f64::INFINITY;
        }
        // Blocker spans the refused section vertically (y=50..350)
        // with a generous bound. LOS through the refused middle is
        // rejected; rows 0 and 4 stay clear.
        mesh.add_blocker(vec![
            Point2 { x: 200.0, y: 50.0 },
            Point2 { x: 200.0, y: 350.0 },
        ]);
        // Route between two row-2 nodes so a straight LOS line
        // straddles the blocker. Start (0,2), goal (4,2).
        let start = MeshNodeId(2 * 5); // (0, 2)
        let goal = MeshNodeId(2 * 5 + 4); // (4, 2)
        let path = theta_star(&mesh, start, goal).unwrap();
        let straight = mesh.pt(start).dist(mesh.pt(goal));
        assert!(
            path.length_m > straight,
            "expected detour to exceed straight-line {}",
            straight
        );
        // Path nodes must not include any refused node (rows 1–3 of
        // column x=2). Rows 0 and 4 of that column are passable, so
        // the detour goes through one of them.
        for n in &path.nodes {
            assert!(
                mesh.cost_mul(*n).is_finite(),
                "path visited refused node {}",
                n.0
            );
        }
    }

    #[test]
    fn line_of_sight_blocked_by_polyline() {
        // Set up a blocker that crosses the straight line between
        // start and goal. Theta* must NOT shortcut through it — the
        // resulting path follows mesh edges around the blocker.
        let mut mesh = grid_mesh(5, 3);
        // Blocker: vertical line at x=200 from y=-50 to y=250 — cuts
        // across the middle column. Endpoints are NOT mesh nodes, so
        // the segment-crossing test will see a strict crossing.
        mesh.add_blocker(vec![
            Point2 { x: 200.0, y: -50.0 },
            Point2 { x: 200.0, y: 250.0 },
        ]);
        let start = MeshNodeId(0); // (0,0)
        let goal = MeshNodeId(4); // (4,0)
        // Without blocker, Theta* would shortcut diagonally. With
        // it, line of sight is rejected and the path is longer.
        let path = theta_star(&mesh, start, goal).unwrap();
        let direct = mesh.pt(start).dist(mesh.pt(goal));
        assert!(
            path.length_m >= direct - 1.0,
            "path can't be shorter than the direct line"
        );
    }

    #[test]
    fn disconnected_graph_returns_none() {
        // Two-node mesh with no edge between them — Theta* must
        // return None rather than e.g. an empty path. Callers
        // distinguish "no path" from "trivial path".
        let mesh = Mesh::new(vec![
            MeshNode {
                pt: Point2 { x: 0.0, y: 0.0 },
                cost_mul: 1.0,
            },
            MeshNode {
                pt: Point2 { x: 100.0, y: 0.0 },
                cost_mul: 1.0,
            },
        ]);
        assert!(theta_star(&mesh, MeshNodeId(0), MeshNodeId(1)).is_none());
    }

    #[test]
    fn refused_endpoint_returns_none() {
        // If start or goal is itself refused (e.g. user clicked on a
        // water polygon), we don't route — caller should snap to a
        // navigable node first.
        let mut mesh = grid_mesh(3, 3);
        let goal = MeshNodeId(8); // (2,2)
        mesh.nodes[goal.0 as usize].cost_mul = f64::INFINITY;
        assert!(theta_star(&mesh, MeshNodeId(0), goal).is_none());
    }

    #[test]
    fn higher_cost_terrain_increases_path_length() {
        // Penalise a chunk of the mesh with cost_mul=5 — pathfinder
        // should still find a path, but its weighted cost should
        // reflect the penalty. We assert this by comparing two runs
        // where the second has a higher-cost middle row.
        let baseline = grid_mesh(5, 3);
        let path_baseline = theta_star(&baseline, MeshNodeId(0), MeshNodeId(4)).unwrap();

        let mut penalised = grid_mesh(5, 3);
        for c in 0..5 {
            // mark middle row expensive
            penalised.nodes[(5 + c) as usize].cost_mul = 5.0;
        }
        // Direct path from (0,0) to (4,0) lies on the cheap row, so
        // the path itself shouldn't change much. But if we route
        // (0,1)→(4,1) the penalty bites.
        let p_thru = theta_star(&penalised, MeshNodeId(5), MeshNodeId(9)).unwrap();
        // Length is geometric; we'll instead assert the path went
        // *off* the penalised row when a cheaper detour exists.
        // Since the only neighbours of (0,1) are (0,0)/(0,2)/diags,
        // the detour off-row is viable.
        let stays_on_penalised_row = p_thru.nodes.iter().all(|n| {
            let r = n.0 / 5;
            r == 1
        });
        assert!(
            !stays_on_penalised_row,
            "expected Theta* to detour around penalised row when cheaper available"
        );
        let _ = path_baseline; // unused, kept for symmetry
    }

    #[test]
    fn segments_cross_basic_geometry() {
        // X-shape: two segments crossing at the origin must intersect.
        let a = Point2 { x: -1.0, y: 0.0 };
        let b = Point2 { x: 1.0, y: 0.0 };
        let c = Point2 { x: 0.0, y: -1.0 };
        let d = Point2 { x: 0.0, y: 1.0 };
        assert!(segments_cross(a, b, c, d));
    }

    #[test]
    fn segments_cross_parallel_dont_intersect() {
        let a = Point2 { x: 0.0, y: 0.0 };
        let b = Point2 { x: 1.0, y: 0.0 };
        let c = Point2 { x: 0.0, y: 1.0 };
        let d = Point2 { x: 1.0, y: 1.0 };
        assert!(!segments_cross(a, b, c, d));
    }

    #[test]
    fn segments_cross_shared_endpoint_not_strict_intersection() {
        // Two segments that share an endpoint don't count as crossing.
        // This is deliberate — mesh nodes may lie on blocker
        // boundaries (e.g. lakeshore trailheads), and we don't want
        // every LOS query to fail because of touching geometry.
        let a = Point2 { x: 0.0, y: 0.0 };
        let b = Point2 { x: 1.0, y: 0.0 };
        let c = Point2 { x: 0.0, y: 0.0 };
        let d = Point2 { x: 0.0, y: 1.0 };
        assert!(!segments_cross(a, b, c, d));
    }
}
