//! Coordinate translation between map projections (CRS).
//!
//! The renderer's *world space* is, and stays, normalised Web-Mercator
//! (see [`crate::geo`]): one shared frame every layer, the camera, and the
//! tile pyramid agree on. To show data authored in a *different* projection
//! on that same map — a marker whose position came from a Norwegian dataset
//! in ETRS89/UTM, a GeoJSON in a national grid — the coordinate has to be
//! translated into the shared frame first, or it lands in the wrong place
//! and appears to "drift" relative to the basemap.
//!
//! This module is that translator. Every supported [`Crs`] knows how to go
//! to and from WGS84 lat/lng, which is the hub: WGS84 is what
//! [`LatLng::to_world`](crate::geo::LatLng::to_world) consumes, so once a
//! coordinate is in lat/lng it places identically to everything else on the
//! map. Translating between any two projections is therefore
//! `dst.from_lat_lng(src.to_lat_lng(x, y))` — exactly what [`reproject`]
//! does.
//!
//! Pure f64 math, no I/O, no GPU — a value boundary the host binds to.

use crate::geo::{LatLng, WorldPoint};

/// WGS84 / GRS80 ellipsoid. ETRS89 (the datum behind EPSG:25832/25833) is
/// fixed to the stable part of the Eurasian plate and coincides with WGS84
/// to within a few centimetres — well below tile-pixel scale — so a single
/// ellipsoid serves both.
const SEMI_MAJOR_A: f64 = 6_378_137.0;
const INV_FLATTENING: f64 = 298.257_223_563;

/// UTM scale factor on the central meridian, and the false easting every
/// UTM zone applies so eastings stay positive across the 6°-wide zone.
const UTM_K0: f64 = 0.9996;
const UTM_FALSE_EASTING: f64 = 500_000.0;
/// Northern-hemisphere false northing. Norway is entirely north of the
/// equator, so the southern-hemisphere 10 000 000 m offset never applies.
const UTM_FALSE_NORTHING: f64 = 0.0;

/// A coordinate reference system this renderer can translate to and from the
/// shared map frame. The hub is WGS84 lat/lng; planar systems carry
/// `(x = easting, y = northing)` in metres, geographic ones
/// `(x = longitude, y = latitude)` in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crs {
    /// WGS84 geographic coordinates (EPSG:4326). `(x = lng°, y = lat°)`.
    /// The identity hub — translating to/from this is free.
    Wgs84,
    /// ETRS89 / UTM zone 32N (EPSG:25832). Central meridian 9°E — western
    /// Norway and the common choice for southern Norway. Metres.
    Utm32N,
    /// ETRS89 / UTM zone 33N (EPSG:25833). Central meridian 15°E —
    /// Kartverket's primary projection for mainland Norway. Metres.
    Utm33N,
}

impl Crs {
    /// The UTM zone number for the UTM variants (`None` for geographic CRS).
    fn utm_zone(self) -> Option<u32> {
        match self {
            Crs::Wgs84 => None,
            Crs::Utm32N => Some(32),
            Crs::Utm33N => Some(33),
        }
    }

    /// Translate a coordinate in this CRS to WGS84 lat/lng. For
    /// [`Crs::Wgs84`] this is the identity (`x → lng`, `y → lat`); for UTM
    /// it inverts the transverse-Mercator projection.
    pub fn to_lat_lng(self, x: f64, y: f64) -> LatLng {
        match self.utm_zone() {
            None => LatLng::new(y, x),
            Some(zone) => utm_to_lat_lng(x, y, zone),
        }
    }

    /// Translate a WGS84 lat/lng into this CRS's native coordinates,
    /// returning `(x, y)` — `(lng°, lat°)` for geographic, `(easting,
    /// northing)` in metres for UTM. Inverse of [`Crs::to_lat_lng`].
    pub fn from_lat_lng(self, ll: LatLng) -> (f64, f64) {
        match self.utm_zone() {
            None => (ll.lng, ll.lat),
            Some(zone) => lat_lng_to_utm(ll, zone),
        }
    }

    /// Convenience: translate a coordinate in this CRS straight into the
    /// renderer's normalised Web-Mercator world space, ready to hand to the
    /// camera / tile math. Equivalent to `self.to_lat_lng(x, y).to_world()`.
    pub fn to_world(self, x: f64, y: f64) -> WorldPoint {
        self.to_lat_lng(x, y).to_world()
    }
}

/// Translate `(x, y)` from one CRS to another, via WGS84 lat/lng. The
/// general "show projection A's data on projection B's map" primitive;
/// `reproject(e, n, Crs::Utm33N, Crs::Wgs84)` turns a UTM33 easting/northing
/// into `(lng, lat)`.
pub fn reproject(x: f64, y: f64, from: Crs, to: Crs) -> (f64, f64) {
    if from == to {
        return (x, y);
    }
    to.from_lat_lng(from.to_lat_lng(x, y))
}

impl LatLng {
    /// Build a lat/lng from ETRS89/UTM `easting`/`northing` (metres) in the
    /// given `zone` ([`Crs::Utm32N`] / [`Crs::Utm33N`]). A no-op-friendly
    /// helper so hosts placing markers from a UTM dataset don't have to
    /// reach for [`Crs::to_lat_lng`] directly. Returns the WGS84 lat/lng;
    /// feed it to [`LatLng::to_world`] (or straight to a marker) and it
    /// lines up with the Mercator basemap.
    pub fn from_utm(easting: f64, northing: f64, zone: Crs) -> LatLng {
        zone.to_lat_lng(easting, northing)
    }

    /// Project this lat/lng to ETRS89/UTM `(easting, northing)` metres in
    /// `zone`. Inverse of [`LatLng::from_utm`].
    pub fn to_utm(self, zone: Crs) -> (f64, f64) {
        zone.from_lat_lng(self)
    }
}

/// Central meridian (degrees) of a UTM zone: 6°-wide zones centred so zone 1
/// sits on −177°, zone 31 on 3°, zone 33 on 15°.
fn utm_central_meridian_deg(zone: u32) -> f64 {
    zone as f64 * 6.0 - 183.0
}

/// Forward transverse-Mercator (Snyder / USGS series, accurate to the
/// millimetre within a UTM zone) on the WGS84 ellipsoid.
fn lat_lng_to_utm(ll: LatLng, zone: u32) -> (f64, f64) {
    let f = 1.0 / INV_FLATTENING;
    let a = SEMI_MAJOR_A;
    let e2 = f * (2.0 - f);
    let ep2 = e2 / (1.0 - e2);

    let lat = ll.lat.to_radians();
    let lon0 = utm_central_meridian_deg(zone).to_radians();
    let lon = ll.lng.to_radians();

    let (sin_lat, cos_lat) = lat.sin_cos();
    let tan_lat = sin_lat / cos_lat;

    let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let t = tan_lat * tan_lat;
    let c = ep2 * cos_lat * cos_lat;
    let big_a = (lon - lon0) * cos_lat;

    let m = meridian_arc(lat, a, e2);

    let a2 = big_a * big_a;
    let easting = UTM_FALSE_EASTING
        + UTM_K0
            * n
            * (big_a
                + (1.0 - t + c) * big_a * a2 / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * big_a * a2 * a2 / 120.0);
    let northing = UTM_FALSE_NORTHING
        + UTM_K0
            * (m
                + n
                    * tan_lat
                    * (a2 / 2.0
                        + (5.0 - t + 9.0 * c + 4.0 * c * c) * a2 * a2 / 24.0
                        + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2)
                            * a2 * a2 * a2
                            / 720.0));
    (easting, northing)
}

/// Inverse transverse-Mercator: `(easting, northing)` metres → WGS84 lat/lng.
fn utm_to_lat_lng(easting: f64, northing: f64, zone: u32) -> LatLng {
    let f = 1.0 / INV_FLATTENING;
    let a = SEMI_MAJOR_A;
    let e2 = f * (2.0 - f);
    let ep2 = e2 / (1.0 - e2);

    let lon0 = utm_central_meridian_deg(zone).to_radians();

    let m = (northing - UTM_FALSE_NORTHING) / UTM_K0;
    let mu = m / (a * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0));

    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
    let e1_2 = e1 * e1;
    let e1_3 = e1_2 * e1;
    let e1_4 = e1_3 * e1;

    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1_3 / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1_2 / 16.0 - 55.0 * e1_4 / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1_3 / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1_4 / 512.0) * (8.0 * mu).sin();

    let (sin_phi1, cos_phi1) = phi1.sin_cos();
    let tan_phi1 = sin_phi1 / cos_phi1;

    let c1 = ep2 * cos_phi1 * cos_phi1;
    let t1 = tan_phi1 * tan_phi1;
    let n1 = a / (1.0 - e2 * sin_phi1 * sin_phi1).sqrt();
    let r1 = a * (1.0 - e2) / (1.0 - e2 * sin_phi1 * sin_phi1).powf(1.5);
    let d = (easting - UTM_FALSE_EASTING) / (n1 * UTM_K0);

    let d2 = d * d;
    let lat = phi1
        - (n1 * tan_phi1 / r1)
            * (d2 / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d2 * d2 / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1)
                    * d2 * d2 * d2
                    / 720.0);
    let lon = lon0
        + (d - (1.0 + 2.0 * t1 + c1) * d2 * d / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1)
                * d2 * d2 * d
                / 120.0)
            / cos_phi1;

    LatLng::new(lat.to_degrees(), lon.to_degrees())
}

/// Meridian arc length from the equator to `lat` (radians) on the ellipsoid
/// — the northing before false-northing/scale are applied.
fn meridian_arc(lat: f64, a: f64, e2: f64) -> f64 {
    let e4 = e2 * e2;
    let e6 = e4 * e2;
    a * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * lat
        - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * lat).sin()
        + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * lat).sin()
        - (35.0 * e6 / 3072.0) * (6.0 * lat).sin())
}

#[cfg(test)]
mod tests {
    //! Value boundary: a host has a coordinate in some projection and needs
    //! it placed on the Web-Mercator map where the real-world location is.
    //! The contracts: (a) the central-meridian/equator invariants every UTM
    //! zone guarantees, (b) lat/lng → UTM → lat/lng round-trips to sub-mm,
    //! and (c) a UTM-sourced point lands at the *same world point* as the
    //! same place given in lat/lng — i.e. no drift against the basemap.

    use super::*;

    #[test]
    fn on_the_central_meridian_easting_is_the_false_easting() {
        // Anywhere on a zone's central meridian projects to E = 500 000 m
        // exactly — the defining property of the transverse Mercator.
        let (e32, _) = LatLng::new(60.0, 9.0).to_utm(Crs::Utm32N); // CM of zone 32
        let (e33, _) = LatLng::new(63.4, 15.0).to_utm(Crs::Utm33N); // CM of zone 33
        assert!((e32 - 500_000.0).abs() < 1e-3, "zone32 CM easting = {e32}");
        assert!((e33 - 500_000.0).abs() < 1e-3, "zone33 CM easting = {e33}");
    }

    #[test]
    fn equator_on_central_meridian_is_the_grid_origin() {
        // At lat 0 on the central meridian, northing collapses to the false
        // northing (0 in the northern hemisphere) and easting to 500 000.
        let (e, n) = LatLng::new(0.0, 15.0).to_utm(Crs::Utm33N);
        assert!((e - 500_000.0).abs() < 1e-3, "easting = {e}");
        assert!(n.abs() < 1e-3, "northing = {n}");
    }

    #[test]
    fn east_of_the_central_meridian_increases_easting() {
        // A point east of the CM has E > 500 000; west has E < 500 000.
        // Guards against a sign flip in the longitude term.
        let (east, _) = LatLng::new(60.0, 12.0).to_utm(Crs::Utm32N); // 3° east of 9°
        let (west, _) = LatLng::new(60.0, 6.0).to_utm(Crs::Utm32N); // 3° west of 9°
        assert!(east > 500_000.0, "east of CM: {east}");
        assert!(west < 500_000.0, "west of CM: {west}");
    }

    #[test]
    fn lat_lng_utm_round_trips_to_well_below_tile_pixel_scale() {
        // Real Norwegian places across both zones. The Snyder truncated
        // series is accurate to the millimetre within a zone; 1e-6° (~0.1 m
        // at these latitudes) is the round-trip tolerance, orders of
        // magnitude finer than a tile pixel.
        let samples = [
            (LatLng::new(60.39, 5.32), Crs::Utm32N),  // Bergen
            (LatLng::new(59.91, 10.75), Crs::Utm32N), // Oslo
            (LatLng::new(63.43, 10.39), Crs::Utm33N), // Trondheim
            (LatLng::new(69.65, 18.96), Crs::Utm33N), // Tromsø
        ];
        for (ll, zone) in samples {
            let (e, n) = ll.to_utm(zone);
            let back = LatLng::from_utm(e, n, zone);
            assert!(
                (back.lat - ll.lat).abs() < 1e-6 && (back.lng - ll.lng).abs() < 1e-6,
                "{ll:?} via {zone:?} -> ({e}, {n}) -> {back:?}",
            );
        }
    }

    #[test]
    fn utm_sourced_point_lands_on_the_same_world_point_as_lat_lng() {
        // The drift contract: take a real place, get its UTM33 coordinates,
        // then place THAT on the map via the projection. It must resolve to
        // the identical world point as the WGS84 lat/lng — otherwise a
        // UTM-sourced marker would sit off the basemap.
        let place = LatLng::new(63.43, 10.39); // Trondheim
        let truth = place.to_world();

        let (e, n) = place.to_utm(Crs::Utm33N);
        let via_utm = Crs::Utm33N.to_world(e, n);

        // World space is [0,1] over the whole globe, so the ~1e-8° series
        // round-trip error shrinks to a few 1e-11 here — sub-pixel even at
        // the deepest zoom.
        assert!(
            (via_utm.x - truth.x).abs() < 1e-9 && (via_utm.y - truth.y).abs() < 1e-9,
            "UTM-sourced {via_utm:?} must equal lat/lng-sourced {truth:?}",
        );
    }

    #[test]
    fn reproject_between_two_utm_zones_round_trips() {
        // Translating a coordinate from zone 33 to zone 32 and back returns
        // the original easting/northing — `reproject` composes cleanly.
        let (e0, n0) = LatLng::new(62.0, 12.0).to_utm(Crs::Utm33N);
        let (e32, n32) = reproject(e0, n0, Crs::Utm33N, Crs::Utm32N);
        let (e_back, n_back) = reproject(e32, n32, Crs::Utm32N, Crs::Utm33N);
        assert!(
            (e_back - e0).abs() < 1e-2 && (n_back - n0).abs() < 1e-2,
            "({e0}, {n0}) -> zone32 ({e32}, {n32}) -> ({e_back}, {n_back})",
        );
        // The two zones really are different frames — the easting must shift.
        assert!((e32 - e0).abs() > 1.0, "zone change should move the easting");
    }

    #[test]
    fn wgs84_crs_is_the_identity_hub() {
        // Crs::Wgs84 carries (x = lng, y = lat) and translating through it
        // is free — the property that lets every other CRS use it as a hub.
        let ll = Crs::Wgs84.to_lat_lng(10.39, 63.43);
        assert_eq!(ll, LatLng::new(63.43, 10.39));
        let (x, y) = Crs::Wgs84.from_lat_lng(LatLng::new(63.43, 10.39));
        assert_eq!((x, y), (10.39, 63.43));
    }
}
