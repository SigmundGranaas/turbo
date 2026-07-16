//! Per-request "avoid" edge projection.
//!
//! Turns a set of avoided polylines (freehand routes, other on-map
//! tracks, or an outbound leg fed back for round-trip self-avoidance)
//! into a set of GRAPH EDGE ids for the unified solver to penalise on
//! its trail (Dijkstra) leg.
//!
//! The projection is deliberately EDGE-based, never spatial: we do NOT
//! add cost to off-trail mesh cells. If we buffered cells within the
//! radius instead, the router would escape a corridor by walking
//! `radius + ε` off-trail *parallel* to it forever ("shadow-walking").
//! By penalising the trail edges themselves and leaving the high
//! off-trail base cost untouched, the cheapest escape becomes a
//! genuinely divergent marked trail — exactly the product intent.

use std::collections::HashSet;

use turbo_tiles_graph::Graph;

/// Project avoided polylines (EPSG:25833 metres) onto graph edges.
///
/// An edge is "avoided" when at least half of its sampled length lies
/// within `radius_m` of some avoided polyline segment. The half-length
/// rule is what keeps a trail that merely SHARES A JUNCTION with an
/// avoided path from being flagged: only edges that RUN ALONG the
/// avoided geometry qualify, not ones that touch it at an endpoint and
/// immediately diverge.
///
/// `radius_m` is the edge-projection distance (how far an avoided
/// geometry reaches onto nearby edges), not a no-go tube width — the
/// penalty lands on the edge, and the mesh alongside it stays at its
/// ordinary (high) off-trail cost.
pub(crate) fn project_avoided_edges(
    graph: &Graph,
    polylines: &[Vec<(f64, f64)>],
    radius_m: f64,
) -> HashSet<u32> {
    let mut out = HashSet::new();
    if polylines.is_empty() || radius_m <= 0.0 {
        return out;
    }

    // Flatten avoided segments and accumulate the overall bbox.
    let mut segs: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for pl in polylines {
        for p in pl {
            min_x = min_x.min(p.0);
            min_y = min_y.min(p.1);
            max_x = max_x.max(p.0);
            max_y = max_y.max(p.1);
        }
        for w in pl.windows(2) {
            segs.push((w[0], w[1]));
        }
        // A single-vertex "polyline" still anchors a degenerate segment
        // so a lone avoided point projects onto the edges through it.
        if pl.len() == 1 {
            segs.push((pl[0], pl[0]));
        }
    }
    if segs.is_empty() {
        return out;
    }

    let r2 = radius_m * radius_m;
    // Edges whose endpoints fall in the avoid bbox (expanded by radius).
    // A large avoided set is still bounded by the caller's corridor, so
    // the cap here is generous.
    let eids = graph.edge_ids_in_bbox(
        min_x - radius_m,
        min_y - radius_m,
        max_x + radius_m,
        max_y + radius_m,
        1_000_000,
    );
    for eid in eids {
        let poly = graph.edge_polyline(eid);
        if poly.len() < 2 {
            continue;
        }
        // Sample the edge densely (≈ every `radius` metres) and measure
        // how much of its length sits inside the avoid buffer.
        let mut within = 0.0f64;
        let mut total = 0.0f64;
        for w in poly.windows(2) {
            let (ax, ay) = (w[0].x as f64, w[0].y as f64);
            let (bx, by) = (w[1].x as f64, w[1].y as f64);
            let seg_len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
            if seg_len <= 0.0 {
                continue;
            }
            let steps = ((seg_len / radius_m).ceil() as usize).max(1);
            let w_m = seg_len / steps as f64;
            for s in 0..steps {
                // Sub-segment midpoint stands for `w_m` metres of edge.
                let t = (s as f64 + 0.5) / steps as f64;
                let px = ax + (bx - ax) * t;
                let py = ay + (by - ay) * t;
                total += w_m;
                if min_dist2_point_to_segs(px, py, &segs) <= r2 {
                    within += w_m;
                }
            }
        }
        if total > 0.0 && within / total >= 0.5 {
            out.insert(eid);
        }
    }
    out
}

/// Minimum squared distance from a point to any of the segments.
fn min_dist2_point_to_segs(px: f64, py: f64, segs: &[((f64, f64), (f64, f64))]) -> f64 {
    let mut best = f64::INFINITY;
    for &(a, b) in segs {
        let d = point_seg_dist2(px, py, a, b);
        if d < best {
            best = d;
        }
    }
    best
}

/// Squared distance from point `(px, py)` to segment `a→b`.
fn point_seg_dist2(px: f64, py: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    let (ax, ay) = a;
    let (bx, by) = b;
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    if len2 <= 0.0 {
        // Degenerate segment: distance to the point.
        let ex = px - ax;
        let ey = py - ay;
        return ex * ex + ey * ey;
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
    let cx = ax + t * dx;
    let cy = ay + t * dy;
    let ex = px - cx;
    let ey = py - cy;
    ex * ex + ey * ey
}
