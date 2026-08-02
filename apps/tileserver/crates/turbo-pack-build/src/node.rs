//! Endpoint noding — the one real algorithm that lives in the database
//! today.
//!
//! The server pipeline calls:
//!
//! ```sql
//! SELECT pgr_createTopology('paths.edge', 1.0, 'geom', 'id',
//!                           'source_node', 'target_node')
//! ```
//!
//! It is worth being precise about what that does, because the name
//! suggests more than it performs. `pgr_createTopology` **does not split
//! edges where they cross**. It looks only at each edge's first and last
//! vertex, groups endpoints that fall within the tolerance, and writes a
//! node id onto each end. Two paths that intersect in the middle without
//! sharing an endpoint stay unconnected — in the data and here alike.
//!
//! So this is not planar noding, and implementing planar noding here
//! would be a behaviour change dressed as a port: it would connect
//! junctions the server's graph leaves separate, and every route over
//! them would differ.
//!
//! # Grid hash, not a tree
//!
//! Endpoints are bucketed into cells of one tolerance. Any two points
//! within the tolerance are then in the same cell or in one of the eight
//! neighbours, so a match needs nine bucket probes rather than a spatial
//! index. Linear in endpoints, no allocation per query, and small enough
//! to run on a phone.
//!
//! Grouping is transitive via union-find — if a is within tolerance of
//! b, and b of c, all three become one node even when a and c are
//! further apart than the tolerance. That is a real property, not an
//! accident: it matches what a tolerance-based clusterer does, and the
//! alternative (first-match-wins) makes node identity depend on input
//! order.

use std::collections::HashMap;

/// Matches the `1.0` passed to `pgr_createTopology`. Changing it changes
/// which paths connect, which changes routes.
pub const TOLERANCE_M: f64 = 1.0;

/// An endpoint awaiting a node id.
#[derive(Debug, Clone, Copy)]
struct Endpoint {
    x: f64,
    y: f64,
}

/// Disjoint-set over endpoint indices.
struct Union {
    parent: Vec<u32>,
}

impl Union {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n as u32).collect(),
        }
    }
    fn find(&mut self, mut i: u32) -> u32 {
        while self.parent[i as usize] != i {
            // Path halving: keeps find near-constant without recursion,
            // which matters because a long chain of near-collinear
            // endpoints is exactly what a trail network produces.
            self.parent[i as usize] = self.parent[self.parent[i as usize] as usize];
            i = self.parent[i as usize];
        }
        i
    }
    fn union(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            // Lower index wins, so node numbering depends on input order
            // only through that order — not through hash iteration.
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.parent[hi as usize] = lo;
        }
    }
}

/// The result of noding: one node id per endpoint, plus node positions.
#[derive(Debug, Clone)]
pub struct Topology {
    /// `node_of[i]` is the node id of endpoint `i`.
    pub node_of: Vec<u32>,
    /// Node positions, indexed by node id.
    pub nodes: Vec<(f64, f64)>,
}

/// Assign node ids to the endpoints of `lines`.
///
/// Endpoints are taken two per line, in order, so line `i` owns
/// endpoints `2i` (start) and `2i + 1` (end).
pub fn node_endpoints(lines: &[(f64, f64, f64, f64)], tolerance_m: f64) -> Topology {
    let mut points: Vec<Endpoint> = Vec::with_capacity(lines.len() * 2);
    for &(x0, y0, x1, y1) in lines {
        points.push(Endpoint { x: x0, y: y0 });
        points.push(Endpoint { x: x1, y: y1 });
    }

    let cell = tolerance_m.max(f64::MIN_POSITIVE);
    let key = |p: &Endpoint| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);

    let mut buckets: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
    for (i, p) in points.iter().enumerate() {
        buckets.entry(key(p)).or_default().push(i as u32);
    }

    let mut uf = Union::new(points.len());
    let tol2 = tolerance_m * tolerance_m;
    for (i, p) in points.iter().enumerate() {
        let (cx, cy) = key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(others) = buckets.get(&(cx + dx, cy + dy)) else {
                    continue;
                };
                for &j in others {
                    if (j as usize) <= i {
                        continue;
                    }
                    let q = &points[j as usize];
                    let d2 = (p.x - q.x).powi(2) + (p.y - q.y).powi(2);
                    if d2 <= tol2 {
                        uf.union(i as u32, j);
                    }
                }
            }
        }
    }

    // Number nodes by first appearance, so ids are a function of input
    // order alone. Hash iteration order must not reach the artifact —
    // two builds of the same region would otherwise produce graphs that
    // are equivalent but not comparable.
    let mut id_of_root: HashMap<u32, u32> = HashMap::new();
    let mut nodes: Vec<(f64, f64)> = Vec::new();
    let mut sums: Vec<(f64, f64, u32)> = Vec::new();
    let mut node_of = vec![0u32; points.len()];
    for i in 0..points.len() {
        let root = uf.find(i as u32);
        let id = *id_of_root.entry(root).or_insert_with(|| {
            nodes.push((0.0, 0.0));
            sums.push((0.0, 0.0, 0));
            (nodes.len() - 1) as u32
        });
        node_of[i] = id;
        let s = &mut sums[id as usize];
        s.0 += points[i].x;
        s.1 += points[i].y;
        s.2 += 1;
    }
    // A node's position is the centroid of the endpoints that merged
    // into it, not whichever one happened to be first.
    for (i, &(sx, sy, n)) in sums.iter().enumerate() {
        nodes[i] = (sx / n as f64, sy / n as f64);
    }

    Topology { node_of, nodes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_lines_meeting_at_a_point_share_a_node() {
        let t = node_endpoints(
            &[(0.0, 0.0, 10.0, 0.0), (10.0, 0.0, 20.0, 5.0)],
            TOLERANCE_M,
        );
        assert_eq!(t.node_of[1], t.node_of[2], "the shared endpoint split");
        assert_eq!(t.nodes.len(), 3);
    }

    #[test]
    fn endpoints_within_the_tolerance_merge() {
        let t = node_endpoints(
            &[(0.0, 0.0, 10.0, 0.0), (10.4, 0.3, 20.0, 0.0)],
            TOLERANCE_M,
        );
        assert_eq!(t.node_of[1], t.node_of[2], "0.5 m apart should merge");
    }

    #[test]
    fn endpoints_beyond_the_tolerance_stay_apart() {
        let t = node_endpoints(
            &[(0.0, 0.0, 10.0, 0.0), (11.5, 0.0, 20.0, 0.0)],
            TOLERANCE_M,
        );
        assert_ne!(t.node_of[1], t.node_of[2], "1.5 m apart must not merge");
        assert_eq!(t.nodes.len(), 4);
    }

    /// The behaviour that must NOT be added. `pgr_createTopology` looks
    /// only at endpoints, so an X crossing with no shared endpoint is
    /// four separate nodes. Splitting here would connect junctions the
    /// server's graph leaves separate and silently reroute everything.
    #[test]
    fn a_mid_segment_crossing_does_not_create_a_node() {
        let t = node_endpoints(
            &[(0.0, 0.0, 10.0, 10.0), (0.0, 10.0, 10.0, 0.0)],
            TOLERANCE_M,
        );
        assert_eq!(t.nodes.len(), 4, "an X crossing must stay unconnected");
    }

    /// Chains merge transitively, which is what makes node identity
    /// independent of which endpoint was visited first.
    #[test]
    fn a_chain_of_near_endpoints_becomes_one_node() {
        let t = node_endpoints(
            &[
                (0.0, 0.0, 100.0, 0.0),
                (100.8, 0.0, 200.0, 0.0),
                (101.6, 0.0, 300.0, 0.0),
            ],
            TOLERANCE_M,
        );
        assert_eq!(t.node_of[1], t.node_of[2]);
        assert_eq!(t.node_of[2], t.node_of[4]);
        assert_eq!(t.nodes.len(), 4, "chain should collapse to one shared node");
    }

    /// Grid bucketing must not miss a pair that straddles a cell
    /// boundary — the classic off-by-one in a hashed-grid neighbour
    /// search, and it would silently disconnect trails.
    #[test]
    fn a_pair_straddling_a_bucket_boundary_still_merges() {
        // Exactly on a cell edge for tolerance 1.0.
        let t = node_endpoints(
            &[(0.0, 0.0, 999.9, 0.0), (1000.1, 0.0, 2000.0, 0.0)],
            TOLERANCE_M,
        );
        assert_eq!(
            t.node_of[1], t.node_of[2],
            "straddling pair failed to merge"
        );
    }

    #[test]
    fn node_positions_are_the_centroid_of_what_merged() {
        let t = node_endpoints(
            &[(0.0, 0.0, 10.0, 0.0), (10.6, 0.0, 20.0, 0.0)],
            TOLERANCE_M,
        );
        let id = t.node_of[1] as usize;
        assert!((t.nodes[id].0 - 10.3).abs() < 1e-9, "got {}", t.nodes[id].0);
    }

    /// Ids must come from input order, never from hash iteration, or two
    /// builds of the same region produce incomparable graphs.
    #[test]
    fn node_ids_are_deterministic_across_runs() {
        let lines: Vec<(f64, f64, f64, f64)> = (0..200)
            .map(|i| {
                let f = i as f64;
                (f * 10.0, 0.0, f * 10.0 + 10.0, 0.0)
            })
            .collect();
        let a = node_endpoints(&lines, TOLERANCE_M);
        let b = node_endpoints(&lines, TOLERANCE_M);
        assert_eq!(a.node_of, b.node_of);
        assert_eq!(a.nodes, b.nodes);
        assert_eq!(a.node_of[0], 0, "the first endpoint must be node 0");
    }
}
