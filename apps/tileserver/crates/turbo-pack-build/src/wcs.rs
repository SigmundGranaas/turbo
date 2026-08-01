//! Kartverket's DTM over WCS, fetched a tile at a time.
//!
//! # Why 1.0.0
//!
//! The service is ArcGIS behind `wcs.geonorge.no`. WCS 2.0.1 is
//! advertised in its capabilities and `GetCapabilities` works, but
//! `GetCoverage` with `subset=x(...)&subset=y(...)` answers **HTTP 400**
//! with an ArcGIS error page whose body says only "Error occurred while
//! processing request". Both `E`/`N` and `x`/`y` axis labels were tried;
//! the axis labels are `x y` per `DescribeCoverage`, and it still 400s.
//!
//! WCS 1.0.0 with `BBOX` + `WIDTH`/`HEIGHT` works first time. So this
//! speaks 1.0.0 deliberately, and the version is not a detail to
//! "modernise" later without re-testing against the live service.
//!
//! # Resolution is a request parameter
//!
//! `DescribeCoverage` reports an `offsetVector` of 1 m — the source is
//! NHM DTM1 — and the server resamples to whatever `WIDTH`/`HEIGHT`
//! asks for. Asking for 10 m is therefore a choice, made here to match
//! what the PostGIS pipeline loads, so the two builders produce
//! comparable DEMs.

use crate::BuildError;

/// `nhm_dtm_topo_25833` is published in EPSG:25833 — which is the pack's
/// own `utm33n` frame, so nothing in this pipeline reprojects.
pub const DEFAULT_ENDPOINT: &str = "https://wcs.geonorge.no/skwms1/wcs.hoyde-dtm-nhm-25833";
pub const COVERAGE: &str = "nhm_dtm_topo_25833";
pub const EPSG: u32 = 25833;

/// Metres per DEM sample. Must match what `turbo-tiles-build` loads, or
/// the two builders are not comparable.
pub const RESOLUTION_M: f64 = 10.0;

/// Samples per side of one request.
///
/// Not the same number as the DEM's own 256-cell tile: at 10 m a 256 px
/// request covers 2.56 km, which needs 441 round trips for a 53 km
/// region. 1024 px is 10.24 km and ~4 MB, so the same region is 36
/// requests, each split into 16 tiles locally. The artifact's tile size
/// constrains the *file*, not the fetch.
pub const REQUEST_PX: usize = 1024;

/// A half-open box in projected metres (EPSG:25833).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxUtm {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl BoxUtm {
    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Snap a box outward to a multiple of [`RESOLUTION_M`].
///
/// Outward, so the plan always covers at least what was asked for. A
/// box snapped inward loses a strip at the edge, and the edge of a
/// routing pack is exactly where someone put a waypoint on purpose.
pub fn snap_out(b: BoxUtm) -> BoxUtm {
    let r = RESOLUTION_M;
    BoxUtm {
        min_x: (b.min_x / r).floor() * r,
        min_y: (b.min_y / r).floor() * r,
        max_x: (b.max_x / r).ceil() * r,
        max_y: (b.max_y / r).ceil() * r,
    }
}

/// The fetch plan for a region: aligned request boxes, west→east then
/// north→south.
///
/// Deterministic given the region, which is what makes a build
/// resumable and two builds of the same region diffable.
pub fn plan(region: BoxUtm) -> Vec<BoxUtm> {
    let region = snap_out(region);
    let step = REQUEST_PX as f64 * RESOLUTION_M;
    let mut out = Vec::new();
    let mut y = region.max_y;
    while y > region.min_y {
        let bottom = (y - step).max(region.min_y);
        let mut x = region.min_x;
        while x < region.max_x {
            let right = (x + step).min(region.max_x);
            out.push(BoxUtm {
                min_x: x,
                min_y: bottom,
                max_x: right,
                max_y: y,
            });
            x = right;
        }
        y = bottom;
    }
    out
}

/// The `GetCoverage` URL for one request box.
pub fn url(endpoint: &str, b: BoxUtm) -> String {
    // Sizes come from the box, so a partial edge tile stays at the same
    // ground resolution as a full one rather than being stretched.
    let w = (b.width() / RESOLUTION_M).round().max(1.0) as usize;
    let h = (b.height() / RESOLUTION_M).round().max(1.0) as usize;
    format!(
        "{endpoint}?service=WCS&version=1.0.0&request=GetCoverage\
         &coverage={COVERAGE}&CRS=EPSG:{EPSG}\
         &BBOX={},{},{},{}&WIDTH={w}&HEIGHT={h}&FORMAT=GeoTIFF",
        b.min_x, b.min_y, b.max_x, b.max_y
    )
}

/// Fetch and decode one request box.
pub async fn fetch(
    http: &reqwest::Client,
    endpoint: &str,
    b: BoxUtm,
) -> Result<crate::geotiff::Raster, BuildError> {
    let u = url(endpoint, b);
    let resp = http
        .get(&u)
        .send()
        .await
        .map_err(|e| BuildError::Fetch(format!("WCS request: {e}")))?;
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| BuildError::Fetch(format!("WCS body: {e}")))?;
    if !status.is_success() {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]).to_string();
        return Err(BuildError::Fetch(format!("WCS {status}: {head}")));
    }
    let r = crate::geotiff::decode(&bytes)?;

    // The server is free to honour WIDTH/HEIGHT loosely. Check the
    // ground geometry rather than the pixel count: what matters is that
    // the samples land where this build thinks they do.
    if (r.pixel_size_x - RESOLUTION_M).abs() > 0.01 || (r.pixel_size_y - RESOLUTION_M).abs() > 0.01
    {
        return Err(BuildError::Decode(format!(
            "WCS returned {} x {} m pixels, asked for {RESOLUTION_M} m",
            r.pixel_size_x, r.pixel_size_y
        )));
    }
    if (r.origin_x - b.min_x).abs() > RESOLUTION_M || (r.origin_y - b.max_y).abs() > RESOLUTION_M {
        return Err(BuildError::Decode(format!(
            "WCS returned a raster at ({}, {}), asked for ({}, {})",
            r.origin_x, r.origin_y, b.min_x, b.max_y
        )));
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> BoxUtm {
        BoxUtm {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    #[test]
    fn the_plan_covers_the_whole_region() {
        let region = b(480_000.0, 7_400_000.0, 505_000.0, 7_420_000.0);
        let tiles = plan(region);
        assert!(!tiles.is_empty());
        let snapped = snap_out(region);
        let min_x = tiles.iter().map(|t| t.min_x).fold(f64::MAX, f64::min);
        let max_x = tiles.iter().map(|t| t.max_x).fold(f64::MIN, f64::max);
        let min_y = tiles.iter().map(|t| t.min_y).fold(f64::MAX, f64::min);
        let max_y = tiles.iter().map(|t| t.max_y).fold(f64::MIN, f64::max);
        assert_eq!((min_x, max_x), (snapped.min_x, snapped.max_x));
        assert_eq!((min_y, max_y), (snapped.min_y, snapped.max_y));
    }

    /// Gaps are the failure that hides: a hole in the DEM reads as
    /// "no terrain", which the router treats as untraversable and walks
    /// around. It never throws.
    #[test]
    fn the_plan_has_no_gaps_and_no_overlaps() {
        let tiles = plan(b(480_000.0, 7_400_000.0, 505_000.0, 7_420_000.0));
        let area: f64 = tiles.iter().map(|t| t.width() * t.height()).sum();
        let snapped = snap_out(b(480_000.0, 7_400_000.0, 505_000.0, 7_420_000.0));
        let whole = snapped.width() * snapped.height();
        assert!(
            (area - whole).abs() < 1.0,
            "tiles cover {area} m², region is {whole} m² — overlap or gap"
        );
    }

    #[test]
    fn snapping_is_outward_on_both_axes() {
        let s = snap_out(b(480_001.0, 7_400_001.0, 480_009.0, 7_400_009.0));
        assert_eq!(s, b(480_000.0, 7_400_000.0, 480_010.0, 7_400_010.0));
    }

    /// A region already on the grid must not grow — otherwise every
    /// rebuild of the same bbox would drift outward.
    #[test]
    fn snapping_an_aligned_region_is_a_no_op() {
        let aligned = b(480_000.0, 7_400_000.0, 480_100.0, 7_400_100.0);
        assert_eq!(snap_out(aligned), aligned);
    }

    #[test]
    fn the_url_asks_for_the_resolution_the_box_implies() {
        let u = url(
            DEFAULT_ENDPOINT,
            b(480_000.0, 7_420_000.0, 490_240.0, 7_430_240.0),
        );
        assert!(u.contains("version=1.0.0"), "{u}");
        assert!(u.contains("WIDTH=1024"), "{u}");
        assert!(u.contains("HEIGHT=1024"), "{u}");
        assert!(u.contains("BBOX=480000,7420000,490240,7430240"), "{u}");
    }

    /// A partial edge box must keep 10 m pixels, not stretch to fill.
    #[test]
    fn a_partial_edge_box_keeps_the_ground_resolution() {
        let u = url(
            DEFAULT_ENDPOINT,
            b(480_000.0, 7_420_000.0, 482_560.0, 7_420_640.0),
        );
        assert!(u.contains("WIDTH=256"), "{u}");
        assert!(u.contains("HEIGHT=64"), "{u}");
    }
}
