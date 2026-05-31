//! Geometry hit-test primitives. Pure math — no rendering, no platform.
//!
//! Everything operates in a single coordinate space; the *caller* is
//! responsible for transforming screen pixels into whichever space the
//! features live in (typically tile-local) and the tolerance into the same
//! space.

use crate::vector::Geometry;

/// Closest squared distance from `p` to the segment `(a, b)`.
/// Squared so callers can compare without taking a `sqrt`.
pub fn sq_distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        // Degenerate segment — distance to the single point.
        let ddx = p.0 - a.0;
        let ddy = p.1 - a.1;
        return ddx * ddx + ddy * ddy;
    }
    // Project `p` onto the segment, clamping the parameter to [0, 1].
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0);
    let closest = (a.0 + t * dx, a.1 + t * dy);
    let ddx = p.0 - closest.0;
    let ddy = p.1 - closest.1;
    ddx * ddx + ddy * ddy
}

/// Closest squared distance from `p` to a polyline.
pub fn sq_distance_to_polyline(p: (f64, f64), line: &[(f64, f64)]) -> f64 {
    if line.len() < 2 {
        return f64::INFINITY;
    }
    let mut best = f64::INFINITY;
    for w in line.windows(2) {
        let d = sq_distance_to_segment(p, w[0], w[1]);
        if d < best {
            best = d;
        }
    }
    best
}

/// True iff `p` lies inside the polygon defined by `ring`. Uses the even-odd
/// ray-casting rule. Edge points are considered *inside* (we lean inclusive,
/// since the user wants their click to register).
///
/// `ring` is expected to be a single closed ring (first and last point can
/// be either equal or unequal; the algorithm closes implicitly).
pub fn point_in_polygon(p: (f64, f64), ring: &[(f64, f64)]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        // Half-open ray test: only count edges where the ray (going +x from p)
        // crosses *strictly* between the endpoints in y.
        let intersect =
            ((yi > p.1) != (yj > p.1)) && (p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Is `point` inside (polygon) or within `tol_sq` of (line / point) `geom`?
/// All coordinates are in the same frame as `geom`'s vertices — typically
/// tile-local space when called from `VectorMap::hit_test`.
///
/// Polygon: first ring is the outer; later rings are holes. A click inside
/// the outer but inside a hole is treated as *outside*.
pub fn geometry_hit(geom: &Geometry, point: (f64, f64), tol_sq: f64) -> bool {
    match geom {
        Geometry::Polygon(rings) => {
            if rings.is_empty() {
                return false;
            }
            let outer: Vec<(f64, f64)> =
                rings[0].iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
            if !point_in_polygon(point, &outer) {
                return false;
            }
            for hole in &rings[1..] {
                let h: Vec<(f64, f64)> = hole.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
                if point_in_polygon(point, &h) {
                    return false;
                }
            }
            true
        }
        Geometry::LineString(lines) => {
            for line in lines {
                let l: Vec<(f64, f64)> = line.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
                if sq_distance_to_polyline(point, &l) <= tol_sq {
                    return true;
                }
            }
            false
        }
        Geometry::Point(points) => {
            for &p in points {
                let dx = point.0 - p.0 as f64;
                let dy = point.1 - p.1 as f64;
                if dx * dx + dy * dy <= tol_sq {
                    return true;
                }
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: developers calling these from a host's input handler
    //! need them to (a) be correct on simple shapes, (b) round-trip cleanly
    //! on edges and corners, (c) handle degenerate inputs without panicking.
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn sq_distance_to_segment_zero_on_segment() {
        // Midpoint of (0,0)→(10,0) is (5,0). Distance must be 0.
        assert!(sq_distance_to_segment((5.0, 0.0), (0.0, 0.0), (10.0, 0.0)) < 1e-12);
        // Endpoint also.
        assert!(sq_distance_to_segment((10.0, 0.0), (0.0, 0.0), (10.0, 0.0)) < 1e-12);
    }

    #[test]
    fn sq_distance_to_segment_perpendicular() {
        // Point at (5, 3) — perpendicular distance from (0,0)→(10,0) is 3.
        let d = sq_distance_to_segment((5.0, 3.0), (0.0, 0.0), (10.0, 0.0));
        assert!(approx(d, 9.0, 1e-12), "expected 9, got {d}");
    }

    #[test]
    fn sq_distance_to_segment_past_endpoint_clamps_to_endpoint() {
        // (15, 0) is 5 past the (10, 0) endpoint — distance is 5 (sq = 25).
        let d = sq_distance_to_segment((15.0, 0.0), (0.0, 0.0), (10.0, 0.0));
        assert!(approx(d, 25.0, 1e-12));
    }

    #[test]
    fn sq_distance_to_segment_degenerate_segment_is_point_distance() {
        // Zero-length segment — both endpoints coincide.
        let d = sq_distance_to_segment((3.0, 4.0), (0.0, 0.0), (0.0, 0.0));
        assert!(approx(d, 25.0, 1e-12));
    }

    #[test]
    fn sq_distance_to_polyline_picks_the_closest_segment() {
        // Two segments — L shape. Closest point to (5, 5) is on the second
        // segment at (10, 5), distance 5 (sq = 25).
        let line = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        let d = sq_distance_to_polyline((5.0, 5.0), &line);
        assert!(approx(d, 25.0, 1e-12), "expected 25, got {d}");
    }

    #[test]
    fn point_in_polygon_inside_a_simple_square() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(point_in_polygon((5.0, 5.0), &sq));
        assert!(!point_in_polygon((-1.0, 5.0), &sq));
        assert!(!point_in_polygon((11.0, 5.0), &sq));
        assert!(!point_in_polygon((5.0, -1.0), &sq));
        assert!(!point_in_polygon((5.0, 11.0), &sq));
    }

    #[test]
    fn point_in_polygon_handles_concave_shapes() {
        // L-shape: a 10×10 square with a 5×5 bite taken out of the top-right.
        //
        //  (0,0) -- (10,0)
        //    |        |
        //    |        |  ← y = 5: only x in [0, 10]
        //    |  +-----+  (10, 5)
        //    |  |       (5, 5)
        //    |  |
        //    +--+  (5, 10) - (0, 10)
        let l = [
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 5.0),
            (5.0, 5.0),
            (5.0, 10.0),
            (0.0, 10.0),
        ];
        // Below the bite — inside.
        assert!(point_in_polygon((3.0, 3.0), &l), "(3,3) should be inside");
        // Bottom-right corner area — inside.
        assert!(point_in_polygon((7.0, 3.0), &l), "(7,3) should be inside");
        // In the bite — outside.
        assert!(!point_in_polygon((7.0, 7.0), &l), "(7,7) is in the bite");
        // Top-left area — inside.
        assert!(point_in_polygon((3.0, 7.0), &l), "(3,7) should be inside");
    }

    #[test]
    fn point_in_polygon_too_few_points_is_always_outside() {
        assert!(!point_in_polygon((0.0, 0.0), &[]));
        assert!(!point_in_polygon((0.0, 0.0), &[(0.0, 0.0)]));
        assert!(!point_in_polygon((0.0, 0.0), &[(0.0, 0.0), (1.0, 0.0)]));
    }

    #[test]
    fn geometry_hit_polygon_outer_includes_hole_excludes() {
        // Outer ring 10×10 square at origin; one hole 5×5 in the middle.
        let geom = Geometry::Polygon(vec![
            vec![(0, 0), (10, 0), (10, 10), (0, 10), (0, 0)],
            vec![(3, 3), (7, 3), (7, 7), (3, 7), (3, 3)],
        ]);
        assert!(geometry_hit(&geom, (1.0, 1.0), 0.0), "outside hole → hit");
        assert!(!geometry_hit(&geom, (5.0, 5.0), 0.0), "in hole → miss");
        assert!(!geometry_hit(&geom, (-1.0, -1.0), 0.0), "outside → miss");
    }

    #[test]
    fn geometry_hit_linestring_within_tolerance() {
        // A horizontal segment from (0,0) to (10,0). Tolerance 2 units.
        let geom = Geometry::LineString(vec![vec![(0, 0), (10, 0)]]);
        let tol_sq = 2.0_f64 * 2.0;
        assert!(geometry_hit(&geom, (5.0, 1.5), tol_sq), "within tol → hit");
        assert!(
            !geometry_hit(&geom, (5.0, 5.0), tol_sq),
            "beyond tol → miss"
        );
    }

    #[test]
    fn geometry_hit_point_uses_radius_tolerance() {
        let geom = Geometry::Point(vec![(100, 100)]);
        let tol_sq = 5.0_f64 * 5.0;
        assert!(geometry_hit(&geom, (102.0, 103.0), tol_sq));
        assert!(!geometry_hit(&geom, (110.0, 110.0), tol_sq));
    }
}
