//! The projection boundary (C4).
//!
//! The routing engine is planar-only: it takes metres and returns
//! metres, and has no coordinate reference system to be right or wrong
//! about. The HTTP contract is WGS84 lon/lat. This module is where the
//! two meet, and it is deliberately the *only* place in the API layer
//! that converts route geometry between them.
//!
//! Keeping it in one file is what makes the boundary checkable. A
//! projection scattered across handlers is a projection that eventually
//! gets applied twice, or not at all, on some path nobody tested.

use turbo_geo_frame::{utm33n_to_wgs84, wgs84_to_utm33n};
use turbo_tiles_pathfind::Point;

/// `[lon, lat]` from a request → the engine's planar frame.
#[inline]
pub(crate) fn planar(lonlat: [f64; 2]) -> Point {
    wgs84_to_utm33n(lonlat[0], lonlat[1])
}

/// The engine's planar frame → `[lon, lat]` for a response.
#[inline]
pub(crate) fn lonlat(p: Point) -> [f64; 2] {
    let (lon, lat) = utm33n_to_wgs84(p.x, p.y);
    [lon, lat]
}

pub(crate) fn planar_all(pts: &[[f64; 2]]) -> Vec<Point> {
    pts.iter().copied().map(planar).collect()
}

pub(crate) fn lonlat_all(pts: &[Point]) -> Vec<[f64; 2]> {
    pts.iter().copied().map(lonlat).collect()
}

/// Polylines, for `Prefs::avoid` and refused-region rings.
pub(crate) fn planar_rings(rings: &[Vec<[f64; 2]>]) -> Vec<Vec<Point>> {
    rings.iter().map(|r| planar_all(r)).collect()
}

pub(crate) fn lonlat_rings(rings: &[Vec<Point>]) -> Vec<Vec<[f64; 2]>> {
    rings.iter().map(|r| lonlat_all(r)).collect()
}

/// Project, then rename to the *artifact's* point type.
///
/// The DEM-serving endpoints (`/v1/elev`, `/v1/slope`, `/v1/dem`,
/// `/v1/mask`, `/v1/search`) do not route: they project a request and
/// query the artifact directly, so they legitimately hold both
/// vocabularies. Defining it once here rather than per handler keeps
/// the "projection happens in exactly one place" invariant true.
#[inline]
pub(crate) fn artifact_xy(lon: f64, lat: f64) -> turbo_tiles_elev::PointXY {
    turbo_geodata_artifacts::xy(wgs84_to_utm33n(lon, lat))
}
