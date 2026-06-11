//! Pure-function geometry kernel.
//!
//! Every vector cost layer (water crossings, stream crossings,
//! wetland traversal, cliff avoidance) reduces to the same handful
//! of geometric questions:
//!
//!   * "How many metres of segment AB lie inside this polygon?"
//!     → [`segment_polygon_intersection_length`]
//!   * "Does AB cross this polyline, and where?"
//!     → [`segment_linestring_crossings`]
//!   * "Is this point inside the polygon?"
//!     → [`point_in_polygon`]
//!   * "Is any of segment AB inside this AABB?"
//!     → [`segment_intersects_aabb`]
//!
//! Keeping these as pure functions means they're testable without
//! any I/O, swappable for alternate implementations, and shared
//! across the off-trail solver, the graph route reconstruction, and
//! debug overlays without ceremony.
//!
//! Coordinates are always EPSG:25833 metres throughout the system,
//! so we don't carry units in the types — every input and output
//! is a planar metre.

use serde::{Deserialize, Serialize};

/// 2D planar point in EPSG:25833 metres.
///
/// `Pod`-able (8 bytes, no padding) so it casts straight from
/// mmap'd byte slices via `bytemuck::cast_slice` — same shape used
/// by [`turbo-tiles-graph`] for graph nodes and per-edge polyline
/// vertices. Holding a shared definition here means the vector
/// store can hand a `&[Point]` directly to the geometry kernel
/// without copying.
#[repr(C)]
#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable,
)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned bounding box (EPSG:25833 metres).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Aabb {
    /// Tight AABB of a polyline or polygon ring. Empty input
    /// degenerates to a zero-extent box at the origin — callers
    /// that care should check before invoking.
    pub fn of(points: &[Point]) -> Self {
        if points.is_empty() {
            return Aabb {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            };
        }
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for p in points {
            if p.x < min_x {
                min_x = p.x;
            }
            if p.y < min_y {
                min_y = p.y;
            }
            if p.x > max_x {
                max_x = p.x;
            }
            if p.y > max_y {
                max_y = p.y;
            }
        }
        Aabb {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    pub fn expand(&self, m: f32) -> Aabb {
        Aabb {
            min_x: self.min_x - m,
            min_y: self.min_y - m,
            max_x: self.max_x + m,
            max_y: self.max_y + m,
        }
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        !(other.max_x < self.min_x
            || other.min_x > self.max_x
            || other.max_y < self.min_y
            || other.min_y > self.max_y)
    }

    pub fn contains_point(&self, p: Point) -> bool {
        p.x >= self.min_x && p.x <= self.max_x && p.y >= self.min_y && p.y <= self.max_y
    }
}

/// Liang-Barsky segment-vs-AABB clip. Returns the clipped sub-segment
/// or `None` if the segment lies entirely outside.
///
/// Used as the cheap first cull before any polygon-edge work, and as
/// a building block in `segment_intersects_aabb`.
pub fn clip_segment_to_aabb(a: Point, b: Point, bbox: Aabb) -> Option<(Point, Point)> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let mut t_min = 0.0_f32;
    let mut t_max = 1.0_f32;
    let p = [-dx, dx, -dy, dy];
    let q = [
        a.x - bbox.min_x,
        bbox.max_x - a.x,
        a.y - bbox.min_y,
        bbox.max_y - a.y,
    ];
    for i in 0..4 {
        if p[i].abs() < 1e-9 {
            if q[i] < 0.0 {
                return None;
            }
        } else {
            let t = q[i] / p[i];
            if p[i] < 0.0 {
                if t > t_max {
                    return None;
                }
                if t > t_min {
                    t_min = t;
                }
            } else {
                if t < t_min {
                    return None;
                }
                if t < t_max {
                    t_max = t;
                }
            }
        }
    }
    let clipped_a = Point::new(a.x + t_min * dx, a.y + t_min * dy);
    let clipped_b = Point::new(a.x + t_max * dx, a.y + t_max * dy);
    Some((clipped_a, clipped_b))
}

pub fn segment_intersects_aabb(a: Point, b: Point, bbox: Aabb) -> bool {
    clip_segment_to_aabb(a, b, bbox).is_some()
}

/// Squared distance from `p` to the closed segment `[a, b]`.
/// Using the squared form lets callers avoid `sqrt` when only
/// comparing distances.
pub fn point_to_segment_distance_sq(p: Point, a: Point, b: Point) -> f32 {
    let ax = a.x;
    let ay = a.y;
    let bx = b.x;
    let by = b.y;
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        let qx = p.x - ax;
        let qy = p.y - ay;
        return qx * qx + qy * qy;
    }
    let t = ((p.x - ax) * dx + (p.y - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let qx = p.x - (ax + t * dx);
    let qy = p.y - (ay + t * dy);
    qx * qx + qy * qy
}

/// Standard ray-cast point-in-polygon for a single ring. The ring
/// is expected to be closed (first == last); if it's not, the cast
/// still works because the segment from `ring[n-1]` to `ring[0]`
/// is included implicitly via the modular index.
///
/// Treats edge cases consistently: a point exactly on an edge is
/// classified as inside, which matches the "every meter that's in
/// water costs" semantics we want for integral layers.
pub fn point_in_polygon(p: Point, ring: &[Point]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    // Promote to f64 for the ray-cast arithmetic. Polygon rings
    // stored at UTM33N magnitudes (vertex coords ~7M m) have only
    // ~0.5 m of f32 precision, which is enough to misclassify
    // crossings when the cross-product is computed with values that
    // size — we saw this in production with lake `innsjo` polygons
    // reporting 0 metres of intersection on segments that visibly
    // crossed them. The f64 promotion costs a handful of extra
    // instructions per polygon edge and removes the misclassification.
    let px = p.x as f64;
    let py = p.y as f64;
    let n = ring.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pix = ring[i].x as f64;
        let piy = ring[i].y as f64;
        let pjx = ring[j].x as f64;
        let pjy = ring[j].y as f64;
        // Even-odd ray test eastward. Only edges that actually
        // straddle `py` contribute. The denominator `pjy - piy` is
        // signed; clamping it (the original `.max(MIN_POSITIVE)`
        // approach) flips the sign for downhill edges and either
        // misses or duplicates crossings.
        if (piy > py) != (pjy > py) {
            let denom = pjy - piy;
            let x_at = (pjx - pix) * (py - piy) / denom + pix;
            if px < x_at {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Intersection point of two line segments, parameterised on the
/// first segment. Returns `Some((t, point))` where `t` is the
/// fraction along `[a1, a2]` if the segments meet, or `None` if
/// they don't.
///
/// Treats colinear-overlapping as non-intersecting — the integral
/// helpers handle "lies along the boundary" by sampling the
/// midpoint of the candidate sub-segment, which gives a stable
/// inside/outside classification without special-casing.
pub fn segment_segment_intersection(
    a1: Point,
    a2: Point,
    b1: Point,
    b2: Point,
) -> Option<(f32, Point)> {
    let r_x = a2.x - a1.x;
    let r_y = a2.y - a1.y;
    let s_x = b2.x - b1.x;
    let s_y = b2.y - b1.y;
    let denom = r_x * s_y - r_y * s_x;
    if denom.abs() < 1e-9 {
        // Parallel or colinear — caller treats this as "no crossing".
        return None;
    }
    let t = ((b1.x - a1.x) * s_y - (b1.y - a1.y) * s_x) / denom;
    let u = ((b1.x - a1.x) * r_y - (b1.y - a1.y) * r_x) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((t, Point::new(a1.x + t * r_x, a1.y + t * r_y)))
    } else {
        None
    }
}

/// Total length in metres of segment `[a, b]` that lies inside the
/// polygon ring. This is the workhorse for every polygon-integral
/// cost layer.
///
/// Algorithm: collect all parametric intersections `t` between AB
/// and the polygon edges, sort them, and walk consecutive intervals.
/// For each interval, classify "inside" by testing the midpoint
/// against `point_in_polygon`. Sum the inside-interval lengths.
///
/// O(n) in the polygon edge count, no allocation beyond a small
/// `Vec<f32>` of intersection parameters that the compiler can
/// usually keep on-stack via `SmallVec` (we use a heap-Vec here
/// for portability — typical polygons have ~5–30 edges and the
/// allocation is negligible at this rate).
///
/// Returns 0.0 for degenerate inputs (empty ring or zero-length AB).
pub fn segment_polygon_intersection_length(a: Point, b: Point, ring: &[Point]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let seg_len = (dx * dx + dy * dy).sqrt();
    if seg_len < 1e-9 {
        return 0.0;
    }
    let n = ring.len();
    // Parametric intersections along AB, including endpoints. We
    // compute t in f64 so coordinates at UTM33N magnitudes (millions
    // of metres) don't lose precision: an f32 cross-product of two
    // vectors at 7M only resolves to ~m-scale, which is enough to
    // miss a 2 km lake-spanning crossing. Promotion to f64 costs
    // nothing meaningful (a few extra MAD instructions per edge).
    let ax = a.x as f64;
    let ay = a.y as f64;
    let bx = b.x as f64;
    let by = b.y as f64;
    let rx = bx - ax;
    let ry = by - ay;
    let mut params: Vec<f64> = Vec::with_capacity(8);
    params.push(0.0);
    for i in 0..n {
        let j = (i + 1) % n;
        let px = ring[i].x as f64;
        let py = ring[i].y as f64;
        let qx = ring[j].x as f64;
        let qy = ring[j].y as f64;
        let sx = qx - px;
        let sy = qy - py;
        let denom = rx * sy - ry * sx;
        if denom.abs() < 1e-12 {
            continue;
        }
        let t = ((px - ax) * sy - (py - ay) * sx) / denom;
        let u = ((px - ax) * ry - (py - ay) * rx) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            params.push(t);
        }
    }
    params.push(1.0);
    params.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let mut inside_len: f64 = 0.0;
    for w in params.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if (t1 - t0) < 1e-9 {
            continue;
        }
        let mid_t = 0.5 * (t0 + t1);
        let mid = Point::new((ax + mid_t * rx) as f32, (ay + mid_t * ry) as f32);
        if point_in_polygon_f64(mid, ring, ax, ay, rx, ry, mid_t) {
            inside_len += (t1 - t0) * seg_len;
        }
    }
    inside_len
}

/// Even-odd ray-cast point-in-polygon promoted to f64 so polygon
/// rings stored at UTM33N magnitudes (vertex coords ~7M m) don't
/// lose enough precision in the ray-test arithmetic to miscount
/// crossings. Caller passes the segment's f64 anchor + direction
/// to compute the test point in full precision (the `mid: Point`
/// argument is kept for compatibility, but the f64 reconstruction
/// is what's actually used).
fn point_in_polygon_f64(
    _mid_f32: Point,
    ring: &[Point],
    ax: f64,
    ay: f64,
    rx: f64,
    ry: f64,
    t: f64,
) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let px = ax + t * rx;
    let py = ay + t * ry;
    let n = ring.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pix = ring[i].x as f64;
        let piy = ring[i].y as f64;
        let pjx = ring[j].x as f64;
        let pjy = ring[j].y as f64;
        // Ray-cast east. `(piy > py) != (pjy > py)` ensures the
        // edge actually straddles `py`, which already implies the
        // denominator below is non-zero in the same direction as
        // the slope — no clamping is needed (and clamping with
        // `.max(MIN_POSITIVE)` would flip the sign of a downhill
        // denominator, producing the wrong intersection x and
        // either missing or duplicating crossings).
        if (piy > py) != (pjy > py) {
            let denom = pjy - piy;
            let x_at = (pjx - pix) * (py - piy) / denom + pix;
            if px < x_at {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// A single crossing point between two segments, parameterised on
/// the first segment so callers can sort by `t_on_query` and
/// process them in encounter order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    pub t_on_query: f32,
    pub point: Point,
    /// Index of the linestring segment that was crossed
    /// (the segment from `line[seg_index]` to `line[seg_index+1]`).
    pub seg_index: u32,
}

/// All crossings between segment `[a, b]` and a polyline `line`.
/// Empty result means the segment doesn't touch the line.
///
/// Used by stream-crossing and fence-crossing layers: count the
/// returned crossings, look up each crossed segment's attributes
/// (river width, fence height, …) and sum a per-crossing cost.
pub fn segment_linestring_crossings(a: Point, b: Point, line: &[Point]) -> Vec<Crossing> {
    if line.len() < 2 {
        return Vec::new();
    }
    let mut out: Vec<Crossing> = Vec::new();
    for i in 0..line.len() - 1 {
        if let Some((t, p)) = segment_segment_intersection(a, b, line[i], line[i + 1]) {
            out.push(Crossing {
                t_on_query: t,
                point: p,
                seg_index: i as u32,
            });
        }
    }
    out.sort_by(|x, y| {
        x.t_on_query
            .partial_cmp(&y.t_on_query)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Euclidean length of a polyline in metres.
pub fn polyline_length(line: &[Point]) -> f64 {
    let mut acc = 0.0;
    for w in line.windows(2) {
        let dx = (w[1].x - w[0].x) as f64;
        let dy = (w[1].y - w[0].y) as f64;
        acc += (dx * dx + dy * dy).sqrt();
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_square() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ]
    }

    #[test]
    fn point_in_polygon_basic() {
        let sq = unit_square();
        assert!(point_in_polygon(Point::new(5.0, 5.0), &sq));
        assert!(!point_in_polygon(Point::new(-1.0, 5.0), &sq));
        assert!(!point_in_polygon(Point::new(11.0, 5.0), &sq));
    }

    #[test]
    fn point_in_polygon_downward_edge_does_not_misclassify() {
        // Regression test: the previous implementation clamped the
        // edge denominator with `.max(MIN_POSITIVE)`, which made
        // downhill (pjy < piy) edges produce wrong intersection x
        // and either miss or double-count crossings. A diamond is
        // the minimal repro: ray from a point clearly outside the
        // diamond must cross exactly two of the four edges, with
        // two of them being downhill.
        let diamond = vec![
            Point::new(0.0, 1.0),
            Point::new(1.0, 0.0),
            Point::new(0.0, -1.0),
            Point::new(-1.0, 0.0),
        ];
        // Inside
        assert!(point_in_polygon(Point::new(0.0, 0.0), &diamond));
        // Outside on the same y as the centre, both sides
        assert!(!point_in_polygon(Point::new(-2.0, 0.0), &diamond));
        assert!(!point_in_polygon(Point::new(2.0, 0.0), &diamond));
    }

    #[test]
    fn point_in_polygon_at_utm33n_magnitude() {
        // Regression test: ring coordinates at UTM33N northing
        // values (~7.4M m) lose enough f32 precision that the
        // ray-test arithmetic miscounted crossings. Promotion to
        // f64 inside the test fixes it. Polygon = 100 m square
        // around (482000, 7466000); a point clearly inside and
        // a point clearly outside should classify correctly.
        let sq = vec![
            Point::new(482000.0, 7466000.0),
            Point::new(482100.0, 7466000.0),
            Point::new(482100.0, 7466100.0),
            Point::new(482000.0, 7466100.0),
        ];
        assert!(point_in_polygon(Point::new(482050.0, 7466050.0), &sq));
        assert!(!point_in_polygon(Point::new(481000.0, 7466050.0), &sq));
        assert!(!point_in_polygon(Point::new(483000.0, 7466050.0), &sq));

        // And a segment that crosses the polygon should report the
        // full 100 m of intersection — earlier this returned 0.
        let len = segment_polygon_intersection_length(
            Point::new(481000.0, 7466050.0),
            Point::new(483000.0, 7466050.0),
            &sq,
        );
        assert!((len - 100.0).abs() < 1.0, "expected ~100 m, got {len}");
    }

    #[test]
    fn segment_polygon_intersection_full_inside() {
        let sq = unit_square();
        let len =
            segment_polygon_intersection_length(Point::new(2.0, 5.0), Point::new(8.0, 5.0), &sq);
        assert!((len - 6.0).abs() < 1e-3, "got {len}");
    }

    #[test]
    fn segment_polygon_intersection_half_inside() {
        // Starts outside at (-5, 5), enters at (0, 5), exits at (10, 5).
        let sq = unit_square();
        let len =
            segment_polygon_intersection_length(Point::new(-5.0, 5.0), Point::new(15.0, 5.0), &sq);
        // Inside region is x in [0, 10] → 10m.
        assert!((len - 10.0).abs() < 1e-3, "got {len}");
    }

    #[test]
    fn segment_polygon_intersection_fully_outside() {
        let sq = unit_square();
        let len = segment_polygon_intersection_length(
            Point::new(100.0, 100.0),
            Point::new(120.0, 100.0),
            &sq,
        );
        assert_eq!(len, 0.0);
    }

    #[test]
    fn segment_polygon_intersection_crosses_corner() {
        // Diagonal through (-5,-5) → (15,15). Inside portion is the
        // segment from (0,0) to (10,10), length 10*sqrt(2) ≈ 14.142.
        let sq = unit_square();
        let len = segment_polygon_intersection_length(
            Point::new(-5.0, -5.0),
            Point::new(15.0, 15.0),
            &sq,
        );
        let expected = (2.0_f64).sqrt() * 10.0;
        assert!(
            (len - expected).abs() < 1e-2,
            "got {len} expected {expected}"
        );
    }

    #[test]
    fn linestring_crossings_counts_two() {
        // A zig-zag polyline at y=5 with two segments meeting an
        // AB crossing twice.
        let line = vec![
            Point::new(2.0, 4.0),
            Point::new(4.0, 6.0),
            Point::new(6.0, 4.0),
            Point::new(8.0, 6.0),
        ];
        let crossings =
            segment_linestring_crossings(Point::new(0.0, 5.0), Point::new(10.0, 5.0), &line);
        assert_eq!(crossings.len(), 3, "got {crossings:?}");
        // Sorted by t — crossings should be in left-to-right order.
        assert!(crossings[0].t_on_query < crossings[1].t_on_query);
        assert!(crossings[1].t_on_query < crossings[2].t_on_query);
    }

    #[test]
    fn clip_segment_to_aabb_works() {
        let bbox = Aabb {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        };
        let (a, b) =
            clip_segment_to_aabb(Point::new(-5.0, 5.0), Point::new(15.0, 5.0), bbox).unwrap();
        assert!((a.x - 0.0).abs() < 1e-3);
        assert!((b.x - 10.0).abs() < 1e-3);
    }

    #[test]
    fn clip_segment_outside_returns_none() {
        let bbox = Aabb {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 10.0,
        };
        assert!(
            clip_segment_to_aabb(Point::new(100.0, 100.0), Point::new(120.0, 100.0), bbox)
                .is_none()
        );
    }

    #[test]
    fn aabb_of_handles_empty() {
        let bbox = Aabb::of(&[]);
        assert_eq!(bbox.min_x, 0.0);
    }

    #[test]
    fn point_to_segment_distance_axis_aligned() {
        let d_sq = point_to_segment_distance_sq(
            Point::new(5.0, 3.0),
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
        );
        assert!((d_sq - 9.0).abs() < 1e-3);
    }

    #[test]
    fn polyline_length_matches_sum() {
        let line = vec![
            Point::new(0.0, 0.0),
            Point::new(3.0, 0.0),
            Point::new(3.0, 4.0),
        ];
        let l = polyline_length(&line);
        assert!((l - 7.0).abs() < 1e-6);
    }
}

// ---------------------------------------------------------------------------
// RingIndex — y-banded edge index for big polygon rings
// ---------------------------------------------------------------------------

/// Y-banded edge index over one polygon ring, for accelerating
/// [`segment_polygon_intersection_length`] on big rings (Norway lake
/// rings reach tens of thousands of vertices, and the off-trail cost
/// field evaluates a tiny cell-spanning segment against them once per
/// mesh cell — O(ring) per cell without an index).
///
/// Both halves of the kernel admit a CONSERVATIVE y-band prefilter
/// that leaves results bit-identical to the brute-force scan:
///
///   - intersection collection: a ring edge can only intersect the
///     query segment if their y-ranges overlap. Visiting an edge from
///     more than one band pushes a duplicate `t`, which produces a
///     zero-length span that the interval walk already skips.
///   - point-in-polygon parity: the ray-cast predicate starts with
///     `(piy > py) != (pjy > py)`, so only edges whose y-range
///     contains `py` can toggle parity — and a single band's edge
///     list contains each edge at most once, preserving parity.
///
/// Build is O(n); memory ~one u32 per edge-band entry (≈ n entries).
pub struct RingIndex {
    y_min: f64,
    inv_band_h: f64,
    nbands: u32,
    /// `bands[b]` = indices `k` of edges `ring[k] → ring[(k+1)%n]`
    /// whose y-range intersects band `b`.
    bands: Vec<Vec<u32>>,
}

impl RingIndex {
    /// Below this ring size the brute scan is cheaper than indexing.
    pub const MIN_RING_LEN: usize = 512;

    pub fn build(ring: &[Point]) -> Self {
        let n = ring.len();
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for p in ring {
            y_min = y_min.min(p.y as f64);
            y_max = y_max.max(p.y as f64);
        }
        if !y_min.is_finite() {
            y_min = 0.0;
            y_max = 1.0;
        }
        let nbands = (n / 32).clamp(8, 512) as u32;
        let span = (y_max - y_min).max(1e-9);
        let inv_band_h = nbands as f64 / span;
        let mut bands = vec![Vec::new(); nbands as usize];
        for k in 0..n {
            let j = (k + 1) % n;
            let ya = ring[k].y as f64;
            let yb = ring[j].y as f64;
            let (lo, hi) = if ya <= yb { (ya, yb) } else { (yb, ya) };
            let b0 = band_index(lo, y_min, inv_band_h, nbands);
            let b1 = band_index(hi, y_min, inv_band_h, nbands);
            for band in bands.iter_mut().take(b1 + 1).skip(b0) {
                band.push(k as u32);
            }
        }
        Self {
            y_min,
            inv_band_h,
            nbands,
            bands,
        }
    }

    #[inline]
    fn band_of(&self, y: f64) -> usize {
        band_index(y, self.y_min, self.inv_band_h, self.nbands)
    }
}

#[inline]
fn band_index(y: f64, y_min: f64, inv_band_h: f64, nbands: u32) -> usize {
    let b = ((y - y_min) * inv_band_h) as i64;
    b.clamp(0, nbands as i64 - 1) as usize
}

/// [`segment_polygon_intersection_length`] accelerated by a
/// [`RingIndex`]. Bit-identical results (see the index docs for why
/// the band prefilter is conservative for both kernel halves).
pub fn segment_polygon_intersection_length_indexed(
    a: Point,
    b: Point,
    ring: &[Point],
    idx: &RingIndex,
) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let seg_len = (dx * dx + dy * dy).sqrt();
    if seg_len < 1e-9 {
        return 0.0;
    }
    let n = ring.len();
    let ax = a.x as f64;
    let ay = a.y as f64;
    let bx = b.x as f64;
    let by = b.y as f64;
    let rx = bx - ax;
    let ry = by - ay;
    let (qy_lo, qy_hi) = if ay <= by { (ay, by) } else { (by, ay) };
    let b0 = idx.band_of(qy_lo);
    let b1 = idx.band_of(qy_hi);
    let mut params: Vec<f64> = Vec::with_capacity(8);
    params.push(0.0);
    for band in idx.bands.iter().take(b1 + 1).skip(b0) {
        for &k in band {
            let i = k as usize;
            let j = (i + 1) % n;
            let px = ring[i].x as f64;
            let py = ring[i].y as f64;
            let qx = ring[j].x as f64;
            let qy = ring[j].y as f64;
            let sx = qx - px;
            let sy = qy - py;
            let denom = rx * sy - ry * sx;
            if denom.abs() < 1e-12 {
                continue;
            }
            let t = ((px - ax) * sy - (py - ay) * sx) / denom;
            let u = ((px - ax) * ry - (py - ay) * rx) / denom;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                params.push(t);
            }
        }
    }
    params.push(1.0);
    params.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let mut inside_len: f64 = 0.0;
    for w in params.windows(2) {
        let (t0, t1) = (w[0], w[1]);
        if (t1 - t0) < 1e-9 {
            continue;
        }
        let mid_t = 0.5 * (t0 + t1);
        if point_in_polygon_banded(ring, idx, ax, ay, rx, ry, mid_t) {
            inside_len += (t1 - t0) * seg_len;
        }
    }
    inside_len
}

/// Band-filtered twin of `point_in_polygon_f64`: identical per-edge
/// arithmetic and tie rules over the single band containing the test
/// point's `y` (the only edges whose straddle test can pass).
fn point_in_polygon_banded(
    ring: &[Point],
    idx: &RingIndex,
    ax: f64,
    ay: f64,
    rx: f64,
    ry: f64,
    t: f64,
) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let px = ax + t * rx;
    let py = ay + t * ry;
    let n = ring.len();
    let mut inside = false;
    for &k in &idx.bands[idx.band_of(py)] {
        let jdx = k as usize;
        let inext = (jdx + 1) % n;
        let pix = ring[inext].x as f64;
        let piy = ring[inext].y as f64;
        let pjx = ring[jdx].x as f64;
        let pjy = ring[jdx].y as f64;
        if (piy > py) != (pjy > py) {
            let denom = pjy - piy;
            let x_at = (pjx - pix) * (py - piy) / denom + pix;
            if px < x_at {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod ring_index_tests {
    use super::*;

    /// Tiny deterministic LCG so the property test needs no rand dep.
    struct Lcg(u64);
    impl Lcg {
        fn next_f(&mut self) -> f32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as f32) / (u32::MAX >> 1) as f32
        }
    }

    /// Star-shaped pseudo-random polygon around a centre — guaranteed
    /// simple (non-self-intersecting) so PIP semantics are sane.
    fn star_polygon(rng: &mut Lcg, cx: f32, cy: f32, nv: usize) -> Vec<Point> {
        (0..nv)
            .map(|i| {
                let ang = (i as f32 / nv as f32) * std::f32::consts::TAU;
                let r = 200.0 + 800.0 * rng.next_f();
                Point::new(cx + r * ang.cos(), cy + r * ang.sin())
            })
            .collect()
    }

    #[test]
    fn indexed_matches_brute_bit_exactly() {
        let mut rng = Lcg(0x5eed);
        // UTM33N-magnitude coordinates to exercise the precision path.
        let (cx, cy) = (262_000.0f32, 6_649_000.0f32);
        for nv in [16usize, 100, 700, 2500] {
            let ring = star_polygon(&mut rng, cx, cy, nv);
            let idx = RingIndex::build(&ring);
            for _ in 0..200 {
                let a = Point::new(
                    cx + 2400.0 * (rng.next_f() - 0.5),
                    cy + 2400.0 * (rng.next_f() - 0.5),
                );
                // Mix of horizontal (the cost-field case), short and
                // long segments.
                let b = match (rng.next_f() * 3.0) as u32 {
                    0 => Point::new(a.x + 25.0, a.y),
                    1 => Point::new(
                        a.x + 60.0 * (rng.next_f() - 0.5),
                        a.y + 60.0 * (rng.next_f() - 0.5),
                    ),
                    _ => Point::new(
                        cx + 2400.0 * (rng.next_f() - 0.5),
                        cy + 2400.0 * (rng.next_f() - 0.5),
                    ),
                };
                let brute = segment_polygon_intersection_length(a, b, &ring);
                let fast = segment_polygon_intersection_length_indexed(a, b, &ring, &idx);
                assert_eq!(
                    brute.to_bits(),
                    fast.to_bits(),
                    "mismatch nv={nv} a=({},{}) b=({},{}) brute={brute} fast={fast}",
                    a.x,
                    a.y,
                    b.x,
                    b.y
                );
            }
        }
    }

    #[test]
    fn degenerate_rings_are_safe() {
        let idx = RingIndex::build(&[]);
        assert_eq!(
            segment_polygon_intersection_length_indexed(
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                &[],
                &idx
            ),
            0.0
        );
        // All-collinear (zero y-span) ring.
        let flat: Vec<Point> = (0..8).map(|i| Point::new(i as f32, 5.0)).collect();
        let idx = RingIndex::build(&flat);
        let a = Point::new(-1.0, 5.0);
        let b = Point::new(9.0, 5.0);
        let brute = segment_polygon_intersection_length(a, b, &flat);
        let fast = segment_polygon_intersection_length_indexed(a, b, &flat, &idx);
        assert_eq!(brute.to_bits(), fast.to_bits());
    }
}
