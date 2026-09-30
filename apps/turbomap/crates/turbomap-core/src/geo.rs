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

/// A geographic rectangle, in degrees. `west > east` means it crosses the
/// antimeridian (Fiji: west 177°, east −178°). Built only through
/// [`LatLngBounds::new`] / [`LatLngBounds::containing`], which refuse a
/// rectangle that is not one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLngBounds {
    south: f64,
    west: f64,
    north: f64,
    east: f64,
}

impl LatLngBounds {
    /// The rectangle from its south-west to its north-east corner.
    pub fn new(south_west: LatLng, north_east: LatLng) -> Result<Self, crate::error::FitError> {
        use crate::error::FitError;
        for c in [south_west, north_east] {
            if !(c.lat.is_finite() && c.lng.is_finite()) {
                return Err(FitError::NonFiniteCorner {
                    lat: c.lat,
                    lng: c.lng,
                });
            }
            if !(-180.0..=180.0).contains(&c.lng) {
                return Err(FitError::Longitude { lng: c.lng });
            }
        }
        let (south, north) = (south_west.lat, north_east.lat);
        if !(-90.0 <= south && south <= north && north <= 90.0) {
            return Err(FitError::Latitudes { south, north });
        }
        Ok(Self {
            south,
            west: south_west.lng,
            north,
            east: north_east.lng,
        })
    }

    /// The smallest rectangle holding every point, or `None` for no points.
    /// Longitude takes the shorter way round: the rectangle is the
    /// complement of the widest gap between the points' longitudes, so
    /// photographs either side of the antimeridian make a narrow rectangle
    /// across it, not one spanning the world.
    pub fn containing(
        points: impl IntoIterator<Item = LatLng>,
    ) -> Result<Option<Self>, crate::error::FitError> {
        let points: Vec<LatLng> = points.into_iter().collect();
        let Some(first) = points.first() else {
            return Ok(None);
        };
        let (mut south, mut north) = (first.lat, first.lat);
        let mut lngs = Vec::with_capacity(points.len());
        for p in &points {
            // Validate each point through the one constructor's rules.
            Self::new(*p, *p)?;
            south = south.min(p.lat);
            north = north.max(p.lat);
            lngs.push(p.lng);
        }
        lngs.sort_by(f64::total_cmp);
        // The widest gap, the wrap from the last longitude back round to
        // the first included. The rectangle runs from the gap's east side
        // to its west side.
        let mut gap = (
            lngs[0] + 360.0 - lngs[lngs.len() - 1],
            lngs[0],
            lngs[lngs.len() - 1],
        );
        for w in lngs.windows(2) {
            if w[1] - w[0] > gap.0 {
                gap = (w[1] - w[0], w[1], w[0]);
            }
        }
        let (_, west, east) = gap;
        Ok(Some(Self {
            south,
            west,
            north,
            east,
        }))
    }

    pub fn south(&self) -> f64 {
        self.south
    }
    pub fn west(&self) -> f64 {
        self.west
    }
    pub fn north(&self) -> f64 {
        self.north
    }
    pub fn east(&self) -> f64 {
        self.east
    }

    /// The rectangle in world space: `(x0, y0)` north-west, `(x1, y1)`
    /// south-east, with `x1 > 1` when it crosses the antimeridian.
    pub(crate) fn world_rect(&self) -> (WorldPoint, WorldPoint) {
        let nw = LatLng::new(self.north, self.west).to_world();
        let mut se = LatLng::new(self.south, self.east).to_world();
        if self.west > self.east {
            se.x += 1.0;
        }
        (nw, se)
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
