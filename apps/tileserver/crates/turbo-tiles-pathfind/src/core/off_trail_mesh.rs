//! Pure local-mesh builder. Constructs a navigable `Mesh` over a
//! bounded area for short-range off-trail pathfinding (start point →
//! nearest graph node).
//!
//! ## Construction strategy
//!
//! - **Grid**: regular square grid covering the bbox at `cell_m`
//!   spacing. Each cell centre is a `MeshNode`. 8-connected
//!   adjacency by default. Simple, deterministic, and good enough
//!   for 5–10 km bbox queries; CDT-based meshes are a follow-up.
//! - **Cost from samples**: caller supplies `Vec<CostSample>` —
//!   typically the centroid of each landcover polygon within the
//!   bbox. For each grid node, the nearest sample's `cost_mul`
//!   wins. Caller may also pre-assign cost per cell directly via
//!   `with_cell_cost`.
//! - **Refused regions**: caller supplies `Vec<RefusedPolygon>`.
//!   Grid nodes whose centre lies inside any refused polygon are
//!   marked `cost_mul = INFINITY`. The polygon boundaries are
//!   added as blocker polylines so Theta\*'s LOS shortcut rejects
//!   any line that crosses them.
//! - **Exit anchors**: caller supplies `Vec<ExitNode>` — graph nodes
//!   (paths.node ids) that lie inside or on the bbox boundary.
//!   Each gets a dedicated `MeshNode` at its exact position and
//!   8-connected edges to the surrounding grid cells. Cheap
//!   `cost_mul = 0.5` so Theta\* prefers reaching a graph node.
//!
//! ## What this module does NOT do
//!
//! - I/O. Caller does sampling and polygon lookup.
//! - DTM sampling. Costs are inputs; the impure caller (`local_mesh`
//!   module) is the one that hits Postgres.
//! - Heuristic feature derivation. Refused regions come from
//!   authoritative vector polygons (water, glacier) and are passed
//!   in as polygons.

use std::collections::HashMap;

use super::off_trail::{Mesh, MeshNode, MeshNodeId, Point2};

/// Bbox in the SAME projected CRS as the mesh points (typically
/// EPSG:25833 metres). The pure builder doesn't know about lon/lat
/// — that's the caller's transform.
#[derive(Debug, Clone, Copy)]
pub struct MeshBbox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl MeshBbox {
    pub fn is_valid(&self) -> bool {
        self.max_x > self.min_x && self.max_y > self.min_y
    }

    /// Number of cell centres along x and y at the given cell size.
    /// Capped to keep mesh size sane — a 10 km × 10 km bbox at 50 m
    /// cells already yields 40 000 nodes.
    pub fn grid_dims(&self, cell_m: f64) -> (u32, u32) {
        if cell_m <= 0.0 || !self.is_valid() {
            return (0, 0);
        }
        let nx = (((self.max_x - self.min_x) / cell_m).ceil() as i64)
            .max(2)
            .min(400);
        let ny = (((self.max_y - self.min_y) / cell_m).ceil() as i64)
            .max(2)
            .min(400);
        (nx as u32, ny as u32)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CostSample {
    pub at: Point2,
    pub cost_mul: f64,
}

/// Refused-region polygon. The outer ring is required; we don't
/// model holes — water bodies with islands are rare at the scale
/// of off-trail queries and the false-positive (extra refusal) is
/// safer than a false-negative.
#[derive(Debug, Clone)]
pub struct RefusedPolygon {
    pub ring: Vec<Point2>,
}

/// A graph node within or adjacent to the bbox that off-trail paths
/// can "exit" to. Each exit becomes a mesh node directly; the caller
/// receives a mapping `exit_id → MeshNodeId` so they can recover the
/// reached graph node after pathfinding.
#[derive(Debug, Clone)]
pub struct ExitNode {
    pub graph_node_id: i64,
    pub at: Point2,
}

#[derive(Debug, Clone, Default)]
pub struct MeshBuildInput {
    pub bbox: MeshBbox,
    pub cell_m: f64,
    pub samples: Vec<CostSample>,
    pub refused: Vec<RefusedPolygon>,
    pub exits: Vec<ExitNode>,
    /// Caller's start point (typically the user's snapped lon/lat).
    /// Becomes its own mesh node so the start position is exactly
    /// representable, not just snapped to the nearest grid cell.
    pub start: Option<Point2>,
}

impl Default for MeshBbox {
    fn default() -> Self {
        Self {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuiltMesh {
    pub mesh: Mesh,
    /// MeshNodeId for the start position (if `MeshBuildInput::start`
    /// was supplied). Caller uses this as the Theta\* origin.
    pub start_node: Option<MeshNodeId>,
    /// graph_node_id → MeshNodeId for every supplied exit.
    pub exits: HashMap<i64, MeshNodeId>,
    /// Grid dims for the regular cells, in case the caller wants
    /// to visualise the mesh.
    pub grid_dims: (u32, u32),
}

/// Build the local mesh. Pure: same inputs → same outputs.
pub fn build_local_mesh(input: MeshBuildInput) -> BuiltMesh {
    let (nx, ny) = input.bbox.grid_dims(input.cell_m);
    if nx == 0 || ny == 0 {
        return BuiltMesh {
            mesh: Mesh::new(Vec::new()),
            start_node: None,
            exits: HashMap::new(),
            grid_dims: (0, 0),
        };
    }

    let cell_x = (input.bbox.max_x - input.bbox.min_x) / nx as f64;
    let cell_y = (input.bbox.max_y - input.bbox.min_y) / ny as f64;

    // 1. Grid nodes. `grid_id[(c, r)] = MeshNodeId` lookup.
    let mut nodes: Vec<MeshNode> = Vec::with_capacity((nx * ny) as usize + input.exits.len() + 1);
    let mut grid_id: HashMap<(u32, u32), MeshNodeId> = HashMap::new();
    for r in 0..ny {
        for c in 0..nx {
            let pt = Point2 {
                x: input.bbox.min_x + (c as f64 + 0.5) * cell_x,
                y: input.bbox.min_y + (r as f64 + 0.5) * cell_y,
            };
            // Cost from nearest sample, defaulting to 1.0 (nominal).
            let cost_mul = if input.samples.is_empty() {
                1.0
            } else {
                nearest_sample_cost(&input.samples, pt)
            };
            // Refused if centre lies inside any refused polygon.
            let mul = if point_in_any_refused(&input.refused, pt) {
                f64::INFINITY
            } else {
                cost_mul
            };
            let id = MeshNodeId(nodes.len() as u32);
            nodes.push(MeshNode { pt, cost_mul: mul });
            grid_id.insert((c, r), id);
        }
    }

    // 2. Exit nodes. Inserted as their own mesh nodes at exact
    // positions so the path geometry hits the graph node precisely.
    let mut exits_map: HashMap<i64, MeshNodeId> = HashMap::with_capacity(input.exits.len());
    let mut exit_ids: Vec<MeshNodeId> = Vec::with_capacity(input.exits.len());
    for e in &input.exits {
        let id = MeshNodeId(nodes.len() as u32);
        nodes.push(MeshNode {
            pt: e.at,
            cost_mul: 0.5, // mild bonus — Theta* prefers reaching a graph node
        });
        exits_map.insert(e.graph_node_id, id);
        exit_ids.push(id);
    }

    // 3. Start node — same idea: exact position rather than snapped
    // to the nearest grid cell.
    let start_node = input.start.map(|pt| {
        let id = MeshNodeId(nodes.len() as u32);
        nodes.push(MeshNode { pt, cost_mul: 1.0 });
        id
    });

    let mut mesh = Mesh::new(nodes);
    // Record cell size so Theta*'s LoS cap knows the local mesh
    // resolution. Without this the cap falls back to a 25 m
    // default, which is wrong for queries that override
    // `mesh_cell_m`.
    mesh.cell_m = input.cell_m;

    // 4. 8-connected grid adjacency.
    for r in 0..ny {
        for c in 0..nx {
            let a = grid_id[&(c, r)];
            for (dc, dr) in [(1, 0), (0, 1), (1, 1), (1u32.wrapping_sub(2), 1)] {
                let nc = c.wrapping_add(dc);
                let nr = r.wrapping_add(dr);
                if nc < nx && nr < ny {
                    let b = grid_id[&(nc, nr)];
                    mesh.add_edge(a, b);
                }
            }
        }
    }

    // 5. Connect exits to nearby grid cells (cells within ~sqrt(2)*
    // cell_size of the exit position). For each exit, link to the
    // grid cell(s) whose centre lies within `link_r`.
    let link_r = (cell_x.max(cell_y)) * 1.5;
    for (i, e) in input.exits.iter().enumerate() {
        let exit_id = exit_ids[i];
        for r in 0..ny {
            for c in 0..nx {
                let g = grid_id[&(c, r)];
                let d = mesh.pt(g).dist(e.at);
                if d <= link_r {
                    mesh.add_edge(exit_id, g);
                }
            }
        }
    }

    // 6. Connect the start node similarly.
    if let (Some(sid), Some(spt)) = (start_node, input.start) {
        for r in 0..ny {
            for c in 0..nx {
                let g = grid_id[&(c, r)];
                let d = mesh.pt(g).dist(spt);
                if d <= link_r {
                    mesh.add_edge(sid, g);
                }
            }
        }
    }

    // 7. Refused-polygon rings as blockers. Theta\*'s LOS shortcuts
    // must not slice across them.
    for poly in &input.refused {
        if poly.ring.len() >= 2 {
            // Close the ring if not already closed.
            let mut closed = poly.ring.clone();
            if closed.first() != closed.last() {
                if let Some(&first) = closed.first() {
                    closed.push(first);
                }
            }
            mesh.add_blocker(closed);
        }
    }

    BuiltMesh {
        mesh,
        start_node,
        exits: exits_map,
        grid_dims: (nx, ny),
    }
}

fn nearest_sample_cost(samples: &[CostSample], pt: Point2) -> f64 {
    let mut best_d2 = f64::INFINITY;
    let mut best_cost = 1.0;
    for s in samples {
        let dx = s.at.x - pt.x;
        let dy = s.at.y - pt.y;
        let d2 = dx * dx + dy * dy;
        if d2 < best_d2 {
            best_d2 = d2;
            best_cost = s.cost_mul;
        }
    }
    best_cost
}

/// Even-odd rule point-in-polygon. Treats the ring as a closed
/// polygon; on-boundary classification is implementation-dependent
/// and we accept that — boundary cells get nudged into refused or
/// not depending on rounding, and the LOS blocker catches any line
/// that would cross.
fn point_in_polygon(ring: &[Point2], pt: Point2) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = ring[i];
        let pj = ring[j];
        let intersects = (pi.y > pt.y) != (pj.y > pt.y)
            && (pt.x < (pj.x - pi.x) * (pt.y - pi.y) / (pj.y - pi.y + f64::EPSILON) + pi.x);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn point_in_any_refused(polys: &[RefusedPolygon], pt: Point2) -> bool {
    polys.iter().any(|p| point_in_polygon(&p.ring, pt))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point2 {
        Point2 { x, y }
    }

    fn square_ring(cx: f64, cy: f64, half: f64) -> Vec<Point2> {
        vec![
            pt(cx - half, cy - half),
            pt(cx + half, cy - half),
            pt(cx + half, cy + half),
            pt(cx - half, cy + half),
        ]
    }

    #[test]
    fn grid_dims_clamp_to_sensible_range() {
        // A tiny bbox at a large cell size must still yield at least
        // 2x2 — Theta* needs more than one cell to do anything
        // useful. A huge bbox at a tiny cell size must clamp at the
        // 400-cap so we don't allocate a million nodes.
        let small = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        };
        assert_eq!(small.grid_dims(100.0), (2, 2));

        let huge = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1_000_000.0,
            max_y: 1_000_000.0,
        };
        assert_eq!(huge.grid_dims(1.0), (400, 400));
    }

    #[test]
    fn invalid_bbox_returns_empty_mesh() {
        // Inverted bbox or zero cell size → empty mesh, not panic.
        let bbox = MeshBbox {
            min_x: 10.0,
            min_y: 10.0,
            max_x: 0.0,
            max_y: 0.0,
        };
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 50.0,
            ..Default::default()
        });
        assert!(built.mesh.nodes.is_empty());
        assert!(built.start_node.is_none());
    }

    #[test]
    fn grid_nodes_have_default_cost_when_no_samples() {
        // Empty samples → every grid node gets cost_mul = 1.0.
        let bbox = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 200.0,
            max_y: 200.0,
        };
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 100.0,
            ..Default::default()
        });
        for n in &built.mesh.nodes {
            assert!((n.cost_mul - 1.0).abs() < 1e-9, "cost={}", n.cost_mul);
        }
    }

    #[test]
    fn nearest_sample_assigns_cost_correctly() {
        // Two samples: one cheap-left, one expensive-right. Cells
        // on the left half should get the cheap cost; cells on the
        // right half, the expensive one.
        let bbox = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 400.0,
            max_y: 200.0,
        };
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 100.0,
            samples: vec![
                CostSample {
                    at: pt(50.0, 100.0),
                    cost_mul: 1.0,
                },
                CostSample {
                    at: pt(350.0, 100.0),
                    cost_mul: 5.0,
                },
            ],
            ..Default::default()
        });
        // Find a left cell and a right cell, compare costs.
        let left = built.mesh.nodes.iter().find(|n| n.pt.x < 200.0).unwrap();
        let right = built.mesh.nodes.iter().find(|n| n.pt.x > 200.0).unwrap();
        assert!(left.cost_mul < right.cost_mul);
    }

    #[test]
    fn refused_polygon_marks_interior_cells_infinite() {
        // A small refused square in the middle must mark the cells
        // inside it as refused. Cells outside stay finite.
        let bbox = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 500.0,
            max_y: 500.0,
        };
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 100.0,
            refused: vec![RefusedPolygon {
                ring: square_ring(250.0, 250.0, 80.0),
            }],
            ..Default::default()
        });
        let inside = built
            .mesh
            .nodes
            .iter()
            .find(|n| (n.pt.x - 250.0).abs() < 50.0 && (n.pt.y - 250.0).abs() < 50.0);
        if let Some(n) = inside {
            assert!(n.cost_mul.is_infinite(), "centre cell should be refused");
        }
        // At least one outside cell stays finite.
        assert!(built.mesh.nodes.iter().any(|n| n.cost_mul.is_finite()));
    }

    #[test]
    fn refused_polygon_boundary_added_as_blocker() {
        // The polygon ring (closed) becomes a blocker so Theta*
        // shortcuts can't slice across it.
        let bbox = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 500.0,
            max_y: 500.0,
        };
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 100.0,
            refused: vec![RefusedPolygon {
                ring: square_ring(250.0, 250.0, 80.0),
            }],
            ..Default::default()
        });
        assert_eq!(built.mesh.blockers.len(), 1);
        // Closed ring: first == last.
        let b = &built.mesh.blockers[0];
        assert_eq!(b.first(), b.last());
        assert!(b.len() >= 5);
    }

    #[test]
    fn exit_nodes_become_mesh_nodes_with_cheap_cost() {
        // An exit at a known graph node id must appear in the exit
        // map and have a cost_mul lower than 1.0 (so Theta* prefers
        // reaching it over grid cells).
        let bbox = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 500.0,
            max_y: 500.0,
        };
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 100.0,
            exits: vec![ExitNode {
                graph_node_id: 42,
                at: pt(250.0, 250.0),
            }],
            ..Default::default()
        });
        let mid = built.exits.get(&42).copied().unwrap();
        assert!(built.mesh.cost_mul(mid) < 1.0);
        // Exit is connected to at least one neighbour.
        assert!(!built.mesh.neighbours[mid.0 as usize].is_empty());
    }

    #[test]
    fn start_node_is_added_at_exact_position() {
        // The start node should appear at exactly the supplied
        // coordinates, not snapped to the nearest grid centre. This
        // matters for path geometry — the user clicked HERE, the
        // path must start from HERE.
        let bbox = MeshBbox {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 500.0,
            max_y: 500.0,
        };
        let s = pt(137.5, 213.0);
        let built = build_local_mesh(MeshBuildInput {
            bbox,
            cell_m: 100.0,
            start: Some(s),
            ..Default::default()
        });
        let sid = built.start_node.unwrap();
        let sp = built.mesh.pt(sid);
        assert!((sp.x - s.x).abs() < 1e-9);
        assert!((sp.y - s.y).abs() < 1e-9);
        // Start node must be connected to nearby grid cells.
        assert!(!built.mesh.neighbours[sid.0 as usize].is_empty());
    }

    #[test]
    fn point_in_polygon_simple_square() {
        let r = square_ring(0.0, 0.0, 10.0);
        assert!(point_in_polygon(&r, pt(0.0, 0.0)));
        assert!(point_in_polygon(&r, pt(5.0, 5.0)));
        assert!(!point_in_polygon(&r, pt(20.0, 0.0)));
        assert!(!point_in_polygon(&r, pt(0.0, -20.0)));
    }

    #[test]
    fn point_in_polygon_degenerate_returns_false() {
        // < 3 vertices isn't a polygon — defensive return rather
        // than panic on a malformed input.
        assert!(!point_in_polygon(
            &[pt(0.0, 0.0), pt(1.0, 0.0)],
            pt(0.5, 0.0)
        ));
    }
}
