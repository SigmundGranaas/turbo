//! **L5 — geographic ⇄ planar.** The one place a coordinate reference
//! system is named.
//!
//! See `docs/architecture/2026-07-routing-engine-module-design.md` §7.3.
//!
//! Both functions moved here verbatim: the forward transform from
//! `turbo-tiles-elev` (where a projection had no business living inside
//! an elevation primitive — every DEM consumer inherited a CRS opinion
//! it never asked for) and the inverse from the pathfinder (where it
//! made the engine's public API geographic, and therefore un-portable
//! to any frame but Norway's).
//!
//! # Why this is not an engine port
//!
//! The design rev. 1 proposed `Projection` as a trait the engine holds.
//! That is a port with exactly one implementation, and worse, it keeps
//! the *concept* of a CRS inside the engine — so every future feature
//! gets to ask "which frame is this in?" and eventually one of them
//! answers wrongly. Removing the concept entirely is stronger than
//! abstracting it: the engine takes metres, returns metres, and a game
//! engine driving it needs no projection at all rather than an identity
//! stub.
//!
//! Requests arrive here in WGS84 and answers leave in WGS84. Everything
//! between is planar.

#![forbid(unsafe_code)]

use turbo_route_model::Point;

/// WGS84 (lon, lat) → EPSG:25833 (UTM33N) using the inverse of the
/// standard ellipsoidal UTM formulas (WGS84 ellipsoid). Accurate to
/// well under a metre for the entire UTM33N zone (Norway interior).
pub fn wgs84_to_utm33n(lon_deg: f64, lat_deg: f64) -> Point {
    const A: f64 = 6_378_137.0;
    const F: f64 = 1.0 / 298.257_223_563;
    let e2 = F * (2.0 - F);
    let ep2 = e2 / (1.0 - e2);
    let k0 = 0.9996;
    let lon0 = 15.0_f64.to_radians();
    let false_e = 500_000.0;
    let false_n = 0.0;

    let phi = lat_deg.to_radians();
    let lam = lon_deg.to_radians();
    let dlam = lam - lon0;

    let sin_phi = phi.sin();
    let cos_phi = phi.cos();
    let tan_phi = phi.tan();
    let n = A / (1.0 - e2 * sin_phi * sin_phi).sqrt();
    let t = tan_phi * tan_phi;
    let c = ep2 * cos_phi * cos_phi;
    let a_term = cos_phi * dlam;

    let m = A
        * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0) * phi
            - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * e2 * e2 * e2 / 1024.0)
                * (2.0 * phi).sin()
            + (15.0 * e2 * e2 / 256.0 + 45.0 * e2 * e2 * e2 / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e2 * e2 * e2 / 3072.0) * (6.0 * phi).sin());

    let x = k0
        * n
        * (a_term
            + (1.0 - t + c) * a_term.powi(3) / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a_term.powi(5) / 120.0)
        + false_e;
    let y = k0
        * (m + n
            * tan_phi
            * (a_term * a_term / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * a_term.powi(4) / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * a_term.powi(6) / 720.0))
        + false_n;
    Point { x, y }
}

/// Approximate UTM33N → WGS84 — the inverse of [`wgs84_to_utm33n`].
pub fn utm33n_to_wgs84(x: f64, y: f64) -> (f64, f64) {
    const A: f64 = 6_378_137.0;
    const F: f64 = 1.0 / 298.257_223_563;
    let e2 = F * (2.0 - F);
    let ep2 = e2 / (1.0 - e2);
    let k0 = 0.9996;
    let lon0 = 15.0_f64.to_radians();
    let false_e = 500_000.0;
    let m = y / k0;

    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
    let mu = m / (A * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0));
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1.powi(2) / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin();
    let sin_phi1 = phi1.sin();
    let cos_phi1 = phi1.cos();
    let tan_phi1 = phi1.tan();
    let c1 = ep2 * cos_phi1 * cos_phi1;
    let t1 = tan_phi1 * tan_phi1;
    let n1 = A / (1.0 - e2 * sin_phi1 * sin_phi1).sqrt();
    let r1 = A * (1.0 - e2) / (1.0 - e2 * sin_phi1 * sin_phi1).powf(1.5);
    let d = (x - false_e) / (n1 * k0);
    let phi = phi1
        - (n1 * tan_phi1 / r1)
            * (d * d / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d.powi(4) / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1)
                    * d.powi(6)
                    / 720.0);
    let lambda = lon0
        + (d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1)
                * d.powi(5)
                / 120.0)
            / cos_phi1;
    (lambda.to_degrees(), phi.to_degrees())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oslo_round_trips() {
        let (lon, lat) = (10.7522, 59.9139);
        let p = wgs84_to_utm33n(lon, lat);
        assert!((p.x - 262_000.0).abs() < 1000.0 && (p.y - 6_649_000.0).abs() < 1000.0);
        let (lon2, lat2) = utm33n_to_wgs84(p.x, p.y);
        assert!(
            (lon - lon2).abs() < 1e-6 && (lat - lat2).abs() < 1e-6,
            "round trip lost precision: {lon},{lat} -> {lon2},{lat2}"
        );
    }

    /// Sjunkhatten, the corpus area — near the northern end of the zone
    /// where the series expansions are least comfortable.
    #[test]
    fn sjunkhatten_round_trips() {
        let (lon, lat) = (15.15, 67.42);
        let p = wgs84_to_utm33n(lon, lat);
        let (lon2, lat2) = utm33n_to_wgs84(p.x, p.y);
        assert!((lon - lon2).abs() < 1e-6 && (lat - lat2).abs() < 1e-6);
    }
}
