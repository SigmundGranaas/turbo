//! Distance helper. The core takes pre-resolved `distance_m` on each candidate,
//! but exposes this so callers that only have coordinates (and no server-side
//! `meterFraPunkt`) can resolve it the same way everywhere.

/// Great-circle distance in metres (Haversine, R = 6 371 000 m — matching the
/// clients' `latlong2` default).
pub fn haversine_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    const R: f64 = 6_371_000.0;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dlng / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

/// Point-in-ring by ray casting. `ring` is a closed (or open) sequence of
/// `(lng, lat)` vertices. Used by the embedded engine for polygon containment
/// (parks / kommuner) after an R*Tree bounding-box prefilter — no GEOS.
pub fn point_in_ring(lng: f64, lat: f64, ring: &[(f64, f64)]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if ((yi > lat) != (yj > lat)) && (lng < (xj - xi) * (lat - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Point-in-polygon: inside the outer ring (`rings[0]`) and outside every hole
/// (`rings[1..]`).
pub fn point_in_polygon(lng: f64, lat: f64, rings: &[Vec<(f64, f64)>]) -> bool {
    match rings.split_first() {
        None => false,
        Some((outer, holes)) => {
            point_in_ring(lng, lat, outer) && !holes.iter().any(|h| point_in_ring(lng, lat, h))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A unit square (lng/lat) 0..1.
    fn square() -> Vec<(f64, f64)> {
        vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
    }

    #[test]
    fn point_inside_and_outside_a_ring() {
        assert!(point_in_ring(0.5, 0.5, &square()));
        assert!(!point_in_ring(1.5, 0.5, &square()));
        assert!(!point_in_ring(0.5, 1.5, &square()));
    }

    #[test]
    fn polygon_with_a_hole_excludes_the_hole() {
        let outer = square();
        // Hole 0.4..0.6.
        let hole = vec![(0.4, 0.4), (0.6, 0.4), (0.6, 0.6), (0.4, 0.6)];
        let rings = vec![outer, hole];
        assert!(point_in_polygon(0.2, 0.2, &rings)); // in outer, outside hole
        assert!(!point_in_polygon(0.5, 0.5, &rings)); // in the hole
        assert!(!point_in_polygon(2.0, 2.0, &rings)); // outside everything
    }

    #[test]
    fn one_degree_of_latitude_is_about_111km() {
        let d = haversine_m(0.0, 0.0, 1.0, 0.0);
        assert!((d - 111_194.9).abs() < 1.0, "got {d}");
    }

    #[test]
    fn zero_distance() {
        assert_eq!(haversine_m(61.6, 8.3, 61.6, 8.3), 0.0);
    }
}
