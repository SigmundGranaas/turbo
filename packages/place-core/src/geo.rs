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

#[cfg(test)]
mod tests {
    use super::*;

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
