//! Geographic primitives and the Web-Mercator projection used by the
//! renderer. World coordinates are normalised to `[0, 1] x [0, 1]` with
//! `(0, 0)` at the north-west corner, matching the XYZ tile convention.

/// The latitude at which Web-Mercator is conventionally clamped.
/// `atan(sinh(pi)).to_degrees()` — the latitude where `y` would otherwise
/// diverge to infinity.
pub const MAX_LATITUDE_DEG: f64 = 85.051_128_779_806_59;

/// A geographic point in degrees. `lat` north-positive, `lng` east-positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

/// A point in renderer world space — Web-Mercator, normalised to `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

impl LatLng {
    pub const fn new(lat: f64, lng: f64) -> Self {
        Self { lat, lng }
    }

    /// Project to normalised Web-Mercator world coordinates. Latitude is
    /// clamped to [`MAX_LATITUDE_DEG`] to keep the result finite.
    pub fn to_world(self) -> WorldPoint {
        let lat = self.lat.clamp(-MAX_LATITUDE_DEG, MAX_LATITUDE_DEG);
        let x = (self.lng + 180.0) / 360.0;
        let y = 0.5 - lat.to_radians().tan().asinh() / (2.0 * std::f64::consts::PI);
        WorldPoint { x, y }
    }
}

impl WorldPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Inverse of [`LatLng::to_world`].
    pub fn to_lat_lng(self) -> LatLng {
        let lng = self.x * 360.0 - 180.0;
        let n = std::f64::consts::PI * (1.0 - 2.0 * self.y);
        let lat = n.sinh().atan().to_degrees();
        LatLng { lat, lng }
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: developers use `LatLng <-> WorldPoint` to drive the
    //! renderer. The contract is: equator/meridian land at (0.5, 0.5),
    //! projection round-trips within float precision, and latitudes outside
    //! the Web-Mercator range are clamped (not infinite).

    use super::*;

    const EPS: f64 = 1e-9;

    #[test]
    fn equator_and_prime_meridian_land_at_world_centre() {
        let w = LatLng::new(0.0, 0.0).to_world();
        assert!((w.x - 0.5).abs() < EPS, "x = {}", w.x);
        assert!((w.y - 0.5).abs() < EPS, "y = {}", w.y);
    }

    #[test]
    fn antimeridian_corners_land_at_world_edges() {
        assert!((LatLng::new(0.0, -180.0).to_world().x - 0.0).abs() < EPS);
        assert!((LatLng::new(0.0, 180.0).to_world().x - 1.0).abs() < EPS);
    }

    #[test]
    fn web_mercator_pole_clamping_keeps_y_finite() {
        // The north pole proper is infinite under Web-Mercator. Anything past
        // ±MAX_LATITUDE_DEG must clamp to ~0 / ~1 — never NaN, never inf.
        let north = LatLng::new(90.0, 0.0).to_world();
        let south = LatLng::new(-90.0, 0.0).to_world();
        assert!(north.y.is_finite() && south.y.is_finite());
        assert!(north.y >= 0.0 && north.y < 1e-6);
        assert!(south.y <= 1.0 && south.y > 1.0 - 1e-6);
    }

    #[test]
    fn round_trip_identity_within_float_precision() {
        // A handful of real-world points across the Northern hemisphere.
        let samples = [
            LatLng::new(60.39, 5.32),    // Bergen
            LatLng::new(69.65, 18.96),   // Tromsø
            LatLng::new(0.0, 0.0),       // Null Island
            LatLng::new(-33.86, 151.21), // Sydney
            LatLng::new(40.71, -74.00),  // New York
        ];
        for p in samples {
            let r = p.to_world().to_lat_lng();
            assert!((r.lat - p.lat).abs() < 1e-9, "lat: {} vs {}", r.lat, p.lat);
            assert!((r.lng - p.lng).abs() < 1e-9, "lng: {} vs {}", r.lng, p.lng);
        }
    }

    #[test]
    fn northern_hemisphere_projects_to_upper_half() {
        // y < 0.5 ⇒ north of equator. A guard against an inverted y axis.
        let w = LatLng::new(60.39, 5.32).to_world();
        assert!(w.y < 0.5, "Bergen y must be in northern half, got {}", w.y);
    }
}
