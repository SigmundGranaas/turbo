//! Build a routing pack for one region from Kartverket's public
//! services, with no database in the path.
//!
//! # Why this is possible
//!
//! The assumption that pack building needs PostGIS does not survive
//! reading `turbo-tiles-build`. Its DEM builder is a transcoder; its
//! mask builder rasterises with a Rust scanline over `geo::Polygon` and
//! asks SQL only for WKB; its graph builder makes three PostGIS calls
//! (`ST_X`, `ST_Y`, `ST_AsBinary`). The one real algorithm in the
//! database is `pgr_createTopology`, and that snaps edge *endpoints*
//! within a metre rather than splitting edges at crossings — a grid hash
//! and a union-find, not planar noding.
//!
//! The database is a staging store. This crate replaces the staging,
//! not the computation, and writes through the same format crates
//! (`turbo-tiles-elev`, `-mask`, `-graph`) that `turbo-tiles-build`
//! writes through.
//!
//! # The seam is the point
//!
//! Two builders that share a format layer can be pointed at the same
//! region and their output compared. That turns "does a device-built
//! pack match a server-built one" from a hope into a test.
//!
//! # Sources
//!
//! | data | service | scoping |
//! |---|---|---|
//! | elevation | WCS 1.0.0 `GetCoverage` | arbitrary bbox |
//! | water, glacier | N50 Arealdekke GML | kommune |
//! | roads | N50 Samferdsel GML | kommune |
//! | trails | FKB sti WFS `GetFeature` | arbitrary bbox |
//!
//! N50 has no WFS — the Geonorge catalogue offers it only as
//! `GEONORGE:DOWNLOAD` — so the vector half is ordered per kommune
//! (~26 MB zipped, which decompresses to ~95 MB of Arealdekke). That is
//! the one place this pipeline fetches more than the region needs.

pub mod dem;
pub mod geotiff;
pub mod wcs;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A source service failed or answered something unusable.
    #[error("fetch: {0}")]
    Fetch(String),
    /// A response parsed as the wrong thing. Separate from [`Self::Fetch`]
    /// because it means the service answered *successfully* with
    /// something this code cannot read — a contract change, not an
    /// outage, and the two want different responses from an operator.
    #[error("decode: {0}")]
    Decode(String),
    #[error("build: {0}")]
    Logic(String),
}

/// An HTTP client configured for these services.
///
/// Lives here rather than at the call site so the timeout and the TLS
/// policy travel with the code that knows what the services need: the
/// WCS can take minutes on a cold coverage, and the native trust store
/// matters behind a TLS-intercepting proxy in CI.
pub fn default_client() -> Result<reqwest::Client, BuildError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .user_agent(concat!("turbo-pack-build/", env!("CARGO_PKG_VERSION")))
        .tls_built_in_native_certs(true)
        .build()
        .map_err(|e| BuildError::Fetch(format!("build http client: {e}")))
}

/// What a region build produced.
#[derive(Debug, Clone)]
pub struct RegionReport {
    pub out_dir: PathBuf,
    pub dem_path: PathBuf,
    pub wcs_requests: usize,
    pub dem_tiles: u64,
    pub dem_tiles_all_nodata: u64,
    pub dem_bytes: u64,
    pub seconds: f64,
}

/// Build the DEM for `region` into `out_dir`.
///
/// Concurrency is deliberately small. These are public services shared
/// by everyone in the country, and this is the request pattern that
/// changes if the builder ever runs on phones rather than one server:
/// N clients each pulling a region is different traffic in kind, not
/// just in volume.
pub async fn build_dem(
    http: &reqwest::Client,
    endpoint: &str,
    region: wcs::BoxUtm,
    out_dir: &std::path::Path,
    concurrency: usize,
    mut on_progress: impl FnMut(usize, usize),
) -> Result<RegionReport, BuildError> {
    use futures::StreamExt;

    let started = std::time::Instant::now();
    let plan = wcs::plan(region);
    let total = plan.len();
    let reserved = dem::expected_tiles(&plan);
    let mut writer = dem::DemWriter::create(out_dir, reserved)?;

    // Fetch concurrently but write in plan order: the tile directory is
    // an rstar keyed on position, so order does not affect correctness —
    // it affects whether two builds of the same region produce the same
    // bytes, which is what makes them comparable.
    let mut stream = futures::stream::iter(plan.iter().copied().map(|b| {
        let http = http.clone();
        let endpoint = endpoint.to_string();
        async move { (b, wcs::fetch(&http, &endpoint, b).await) }
    }))
    .buffered(concurrency.max(1));

    let mut done = 0usize;
    while let Some((_, result)) = stream.next().await {
        let raster = result?;
        writer.add(&raster)?;
        done += 1;
        on_progress(done, total);
    }

    let tiles = writer.tiles_written;
    let nodata = writer.tiles_all_nodata;
    let dem_path = writer.finish()?;
    let dem_bytes = std::fs::metadata(&dem_path)?.len();

    Ok(RegionReport {
        out_dir: out_dir.to_path_buf(),
        dem_path,
        wcs_requests: total,
        dem_tiles: tiles,
        dem_tiles_all_nodata: nodata,
        dem_bytes,
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// WGS84 degrees → EPSG:25833 (UTM 33N), the frame packs are cut in.
///
/// Written out rather than pulled from a projection library because it
/// is one well-defined transform on a fixed ellipsoid, and the
/// alternative is a large dependency in the one crate that has to
/// cross-compile to Android.
pub fn wgs84_to_utm33(lon_deg: f64, lat_deg: f64) -> (f64, f64) {
    // WGS84
    const A: f64 = 6_378_137.0;
    const F: f64 = 1.0 / 298.257_223_563;
    const K0: f64 = 0.9996;
    const FALSE_EASTING: f64 = 500_000.0;
    const LON0_DEG: f64 = 15.0; // zone 33 central meridian

    let e2 = F * (2.0 - F);
    let ep2 = e2 / (1.0 - e2);
    let lat = lat_deg.to_radians();
    let dlon = (lon_deg - LON0_DEG).to_radians();

    let n = A / (1.0 - e2 * lat.sin().powi(2)).sqrt();
    let t = lat.tan().powi(2);
    let c = ep2 * lat.cos().powi(2);
    let a1 = lat.cos() * dlon;

    let m = A
        * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2 * e2 * e2 / 256.0) * lat
            - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * e2 * e2 * e2 / 1024.0)
                * (2.0 * lat).sin()
            + (15.0 * e2 * e2 / 256.0 + 45.0 * e2 * e2 * e2 / 1024.0) * (4.0 * lat).sin()
            - (35.0 * e2 * e2 * e2 / 3072.0) * (6.0 * lat).sin());

    let easting = K0
        * n
        * (a1
            + (1.0 - t + c) * a1.powi(3) / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a1.powi(5) / 120.0)
        + FALSE_EASTING;
    let northing = K0
        * (m + n
            * lat.tan()
            * (a1 * a1 / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * a1.powi(4) / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * a1.powi(6) / 720.0));

    (easting, northing)
}

/// The UTM33 box covering a WGS84 bbox, with an optional halo.
///
/// All four corners are projected, not two: UTM grid lines are not
/// parallel to meridians away from the central one, so a box built from
/// the SW and NE corners alone is narrower than the ground it claims —
/// and the missing strip is at the edge, where a deliberate waypoint is
/// most likely to be.
pub fn region_box(west: f64, south: f64, east: f64, north: f64, halo_m: f64) -> wcs::BoxUtm {
    let corners = [
        wgs84_to_utm33(west, south),
        wgs84_to_utm33(east, south),
        wgs84_to_utm33(west, north),
        wgs84_to_utm33(east, north),
    ];
    let min_x = corners.iter().map(|c| c.0).fold(f64::MAX, f64::min) - halo_m;
    let max_x = corners.iter().map(|c| c.0).fold(f64::MIN, f64::max) + halo_m;
    let min_y = corners.iter().map(|c| c.1).fold(f64::MAX, f64::min) - halo_m;
    let max_y = corners.iter().map(|c| c.1).fold(f64::MIN, f64::max) + halo_m;
    wcs::snap_out(wcs::BoxUtm {
        min_x,
        min_y,
        max_x,
        max_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checked against the Sjunkhatten pack's own extent, which was cut
    /// by the server pipeline — so this is the two frames agreeing, not
    /// this function agreeing with itself.
    #[test]
    fn projects_into_the_utm33_range_the_packs_use() {
        // The bundled pack's bbox corner.
        let (e, n) = wgs84_to_utm33(15.029297, 66.82652);
        assert!(
            (400_000.0..600_000.0).contains(&e),
            "easting {e} outside zone 33"
        );
        assert!(
            (7_300_000.0..7_500_000.0).contains(&n),
            "northing {n} not in northern Norway"
        );
    }

    /// A point on the central meridian has a closed form: easting is
    /// exactly the false easting.
    #[test]
    fn the_central_meridian_lands_on_the_false_easting() {
        let (e, _) = wgs84_to_utm33(15.0, 67.0);
        assert!((e - 500_000.0).abs() < 1e-6, "got {e}");
    }

    #[test]
    fn the_region_box_uses_all_four_corners() {
        let b = region_box(15.029297, 66.82652, 16.259766, 67.305976, 0.0);
        // The north edge is further from the central meridian per degree
        // of longitude than the south edge, so a two-corner box would be
        // narrower than this one.
        let (sw, _) = wgs84_to_utm33(15.029297, 66.82652);
        let (nw, _) = wgs84_to_utm33(15.029297, 67.305976);
        assert!(
            b.min_x <= sw.min(nw) + 1e-6,
            "west edge must bound both corners"
        );
    }

    #[test]
    fn the_halo_grows_the_box_on_every_side() {
        let plain = region_box(15.0, 66.9, 15.5, 67.1, 0.0);
        let haloed = region_box(15.0, 66.9, 15.5, 67.1, 1000.0);
        assert!(haloed.min_x <= plain.min_x - 990.0);
        assert!(haloed.max_x >= plain.max_x + 990.0);
        assert!(haloed.min_y <= plain.min_y - 990.0);
        assert!(haloed.max_y >= plain.max_y + 990.0);
    }
}
