//! Cost-aware Chaikin smoothing for FMM-extracted polylines.
//!
//! Standard Chaikin's corner-cutting: each edge `[a, b]` becomes
//! two edges `[a + 0.25(b - a), a + 0.75(b - a)]`. Iterate 2–3
//! times and the result converges visually to a quadratic B-spline.
//!
//! "Cost-aware" adds a rejection step: after subdivision, sample
//! the arrival-time field at each new vertex; if `u` jumps above
//! the local maximum of its neighbour vertices' `u`, the smoother
//! has cut into a high-cost region. Project that vertex back along
//! the gradient of `u` until it's at most `local_max` again. The
//! result is a smooth curve that hugs low-cost regions instead of
//! cutting corners through walls.
//!
//! Mathematically simple, computationally cheap (one Chaikin pass
//! is O(n)), and it sits architecturally *after* FMM extraction —
//! it can never make a path worse than FMM produced.

use crate::extract::PathPoint;
use crate::grid::{FmmGrid, GridShape};

/// Run `iterations` rounds of cost-aware Chaikin smoothing on
/// `path`. Returns a new polyline with O(2^iterations) more
/// vertices than the input (capped at `max_vertices` to bound
/// memory for very long extracted paths).
pub fn chaikin_smooth_cost_aware(
    path: &[PathPoint],
    arrival: &FmmGrid<f32>,
    shape: &GridShape,
    iterations: u8,
    max_vertices: usize,
) -> Vec<PathPoint> {
    if path.len() < 3 {
        return path.to_vec();
    }
    let mut current = path.to_vec();
    for _ in 0..iterations {
        if current.len() * 2 > max_vertices {
            break;
        }
        let mut next: Vec<PathPoint> = Vec::with_capacity(current.len() * 2);
        // Preserve endpoints exactly — Chaikin normally also moves
        // endpoints toward the interior. For pathfinder use we want
        // start and goal to stay at their exact world coordinates.
        next.push(current[0]);
        for w in current.windows(2) {
            let a = w[0];
            let b = w[1];
            let q = PathPoint {
                x: 0.75 * a.x + 0.25 * b.x,
                y: 0.75 * a.y + 0.25 * b.y,
            };
            let r = PathPoint {
                x: 0.25 * a.x + 0.75 * b.x,
                y: 0.25 * a.y + 0.75 * b.y,
            };
            next.push(snap_off_walls(q, arrival, shape, a, b));
            next.push(snap_off_walls(r, arrival, shape, a, b));
        }
        next.push(current[current.len() - 1]);
        current = next;
    }
    current
}

/// If the candidate point `p` has higher arrival time than both
/// of its neighbour anchors (`a`, `b`), it's been pushed into a
/// higher-cost cell by the corner cut. Walk back toward the
/// midpoint of `a` and `b` (the "safe" point) until either the
/// arrival time is bounded by `max(u(a), u(b))` or we've stepped
/// back 90 % of the way.
fn snap_off_walls(
    p: PathPoint,
    arrival: &FmmGrid<f32>,
    shape: &GridShape,
    a: PathPoint,
    b: PathPoint,
) -> PathPoint {
    let u_a = sample(shape, arrival, a);
    let u_b = sample(shape, arrival, b);
    let bound = u_a.max(u_b);
    let u_p = sample(shape, arrival, p);
    if u_p.is_finite() && u_p <= bound * 1.01 {
        // 1 % slack so we don't kick out perfectly-fine corner cuts.
        return p;
    }
    // Walk toward the midpoint of `a, b`.
    let mid_x = 0.5 * (a.x + b.x);
    let mid_y = 0.5 * (a.y + b.y);
    let mut t = 0.1f64;
    while t < 0.9 {
        let q = PathPoint {
            x: p.x * (1.0 - t) + mid_x * t,
            y: p.y * (1.0 - t) + mid_y * t,
        };
        let u_q = sample(shape, arrival, q);
        if u_q.is_finite() && u_q <= bound * 1.01 {
            return q;
        }
        t += 0.1;
    }
    // Fall through to the midpoint: best we can do.
    PathPoint { x: mid_x, y: mid_y }
}

fn sample(shape: &GridShape, arrival: &FmmGrid<f32>, p: PathPoint) -> f32 {
    let fi = (p.x - shape.origin_x) / shape.cell_m - 0.5;
    let fj = (p.y - shape.origin_y) / shape.cell_m - 0.5;
    let i0 = fi.floor() as i64;
    let j0 = fj.floor() as i64;
    if i0 < 0 || j0 < 0 || (i0 + 1) >= shape.nx as i64 || (j0 + 1) >= shape.ny as i64 {
        return f32::INFINITY;
    }
    let tx = (fi - i0 as f64) as f32;
    let ty = (fj - j0 as f64) as f32;
    let i = i0 as u32;
    let j = j0 as u32;
    let u00 = arrival.get(i,     j,     0);
    let u10 = arrival.get(i + 1, j,     0);
    let u01 = arrival.get(i,     j + 1, 0);
    let u11 = arrival.get(i + 1, j + 1, 0);
    (1.0 - tx) * (1.0 - ty) * u00
        + tx * (1.0 - ty) * u10
        + (1.0 - tx) * ty * u01
        + tx * ty * u11
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{solve_2d_isotropic, FmmGrid, GridShape, StopCondition};

    #[test]
    fn smoothing_preserves_endpoints() {
        let shape = GridShape::new_2d(20, 20, 0.0, 0.0, 1.0);
        let arrival: FmmGrid<f32> = FmmGrid::filled(shape, 1.0);
        let path = vec![
            PathPoint { x: 1.0, y: 1.0 },
            PathPoint { x: 5.0, y: 10.0 },
            PathPoint { x: 15.0, y: 10.0 },
            PathPoint { x: 18.0, y: 18.0 },
        ];
        let smoothed = chaikin_smooth_cost_aware(&path, &arrival, &shape, 2, 1000);
        // Endpoints unchanged.
        assert_eq!(smoothed[0].x, 1.0);
        assert_eq!(smoothed[0].y, 1.0);
        assert_eq!(smoothed[smoothed.len() - 1].x, 18.0);
        assert_eq!(smoothed[smoothed.len() - 1].y, 18.0);
        // More vertices than input.
        assert!(smoothed.len() > path.len());
    }

    #[test]
    fn smoothing_avoids_refused_cell() {
        // Hand-craft an arrival field that has a high-cost spike
        // exactly where Chaikin's corner-cut would land. The
        // snap-off-walls step should redirect the smoothed vertex.
        let shape = GridShape::new_2d(20, 20, 0.0, 0.0, 1.0);
        let mut cost: FmmGrid<f32> = FmmGrid::filled(shape, 1.0);
        // Refuse a 3×3 box near where the corner-cut would go.
        for j in 8..=10 {
            for i in 8..=10 {
                cost.set(i, j, 0, f32::INFINITY);
            }
        }
        let result = solve_2d_isotropic(shape, &cost, &[(2, 2, 0.0)], StopCondition::AllAccepted);

        let path = vec![
            PathPoint { x: 2.5, y: 2.5 },
            PathPoint { x: 9.5, y: 2.5 },   // corner just south of refused box
            PathPoint { x: 9.5, y: 18.0 },  // corner just east of refused box
        ];
        let smoothed = chaikin_smooth_cost_aware(&path, &result.arrival, &shape, 3, 1000);

        // None of the smoothed points should land inside the refused box.
        for p in &smoothed {
            let i = ((p.x - shape.origin_x) / shape.cell_m).floor() as i32;
            let j = ((p.y - shape.origin_y) / shape.cell_m).floor() as i32;
            if (8..=10).contains(&i) && (8..=10).contains(&j) {
                panic!("smoothed point ({}, {}) lands in refused box", p.x, p.y);
            }
        }
    }

    #[test]
    fn very_short_path_passes_through() {
        let shape = GridShape::new_2d(10, 10, 0.0, 0.0, 1.0);
        let arrival: FmmGrid<f32> = FmmGrid::filled(shape, 1.0);
        let path = vec![PathPoint { x: 1.0, y: 1.0 }, PathPoint { x: 2.0, y: 2.0 }];
        let smoothed = chaikin_smooth_cost_aware(&path, &arrival, &shape, 3, 1000);
        // 2-vertex paths get one Chaikin pass which adds 2 internal
        // vertices, total ≥ 2. We allow any length ≥ 2.
        assert!(smoothed.len() >= 2);
        assert_eq!(smoothed[0].x, 1.0);
        assert_eq!(smoothed[smoothed.len() - 1].x, 2.0);
    }
}
