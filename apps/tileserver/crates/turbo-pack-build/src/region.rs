//! The whole pipeline: bbox in, routing pack out, no database.
//!
//! Order matters and is not arbitrary. The DEM is built first because
//! the graph samples it for climb and slope — a graph built before the
//! terrain would have zero gain on every edge, which does not fail, it
//! just makes every route prefer the mountain.

use std::path::Path;

use turbo_tiles_elev::Dem;
use turbo_tiles_mask::RefusalKind;

use crate::{gml, graph, mask, n50, pack, wcs, wfs, BuildError, Kommuner};
use crate::IoAt;

/// Progress across the whole build, so a caller can drive one bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Dem,
    Vector,
    Mask,
    Graph,
    Manifest,
}

#[derive(Debug, Clone, Default)]
pub struct PackReport {
    pub dem_tiles: u64,
    pub dem_bytes: u64,
    pub water_polygons: u32,
    pub glacier_polygons: u32,
    pub refused_cells: u64,
    pub total_cells: u64,
    pub trails: usize,
    pub roads: usize,
    pub nodes: u32,
    pub edges_directed: u32,
    pub edges_without_terrain: u32,
    pub total_bytes: u64,
    pub seconds: f64,
}

/// Budget for the DEM tile cache while building a pack. This runs on
/// phones; `turbo_tiles_elev::DEFAULT_CACHE_BYTES` is sized for a server
/// holding a country.
const DEM_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// Build a complete pack for `extent` (WGS84 `[w, s, e, n]`).
#[allow(clippy::too_many_arguments)]
pub fn build_pack(
    http: &dyn crate::fetch::Fetch,
    out_dir: &Path,
    extent: [f64; 4],
    halo_m: f64,
    kommuner: &Kommuner,
    wcs_endpoint: &str,
    wfs_endpoint: &str,
    concurrency: usize,
    mut on_progress: impl FnMut(Phase, usize, usize) -> bool,
) -> Result<PackReport, BuildError> {
    // Returning false from `on_progress` stops the build. Every phase
    // boundary and every loop iteration is a cancellation point, because
    // the alternative — what this did until recently — is that a
    // cancelled build keeps fetching and computing to completion and
    // only notices on the way out.
    macro_rules! progress {
        ($($arg:expr),* $(,)?) => {
            if !on_progress($($arg),*) {
                return Err(BuildError::Cancelled);
            }
        };
    }
    let started = std::time::Instant::now();
    let mut report = PackReport::default();
    std::fs::create_dir_all(out_dir).at(out_dir)?;

    let region = crate::region_box(extent[0], extent[1], extent[2], extent[3], halo_m);

    // ---- 1. Terrain ----
    // `build_dem` turns a false into `Cancelled` itself, so this one
    // passes the answer along rather than returning out of the closure.
    let dem_report = crate::build_dem(http, wcs_endpoint, region, out_dir, concurrency, |d, t| {
        on_progress(Phase::Dem, d, t)
    })?;
    report.dem_tiles = dem_report.dem_tiles;
    report.dem_bytes = dem_report.dem_bytes;
    // Not `Dem::open`: its default cache is 512 MB, which is a server
    // number. The graph phase samples this DEM across the whole region,
    // so the cache fills with decompressed f32 tiles and nothing evicts
    // them — at the 5 500 km² pack limit that is ~220 MB of native
    // memory on a phone, reached during the one phase that used to
    // report no progress at all. A tile is 256×256×4 B, so this still
    // holds 256 of them, and the loop walks ways in roughly spatial
    // order.
    let dem = Dem::open_with_cache(&dem_report.dem_path, DEM_CACHE_BYTES)
        .map_err(|e| BuildError::Logic(format!("reopen the DEM just written: {e}")))?;

    // ---- 2. Vectors ----
    // N50 first: one order yields both the water/glacier surfaces and
    // the road network, so it is one download for two artifacts.
    progress!(Phase::Vector, 0, kommuner.len() + 1);
    let mut water: Vec<geo::Polygon<f64>> = Vec::new();
    let mut glacier: Vec<geo::Polygon<f64>> = Vec::new();
    let mut ways: Vec<graph::Way> = Vec::new();

    for (i, k) in kommuner.iter().enumerate() {
        let zip = n50::fetch_zip(http, k)?;
        let areal = n50::layer_from_zip(&zip, n50::AREALDEKKE)?;
        // Same clip for surfaces. The scanline fill would reject these
        // anyway, but a kommune holds thousands of lakes the region
        // never sees and each one still costs a bbox and a sort.
        gml::read_surfaces(&areal, mask::WATER_TYPES, |_, p| {
            if ring_touches(&p, region) {
                water.push(p)
            }
        })?;
        gml::read_surfaces(&areal, mask::GLACIER_TYPES, |_, p| {
            if ring_touches(&p, region) {
                glacier.push(p)
            }
        })?;
        drop(areal);

        let samf = n50::layer_from_zip(&zip, n50::SAMFERDSEL)?;
        gml::read_lines(&samf, &["Veglenke"], |f| {
            // A kommune is far bigger than a region, and a road outside
            // the DEM is worse than a road that is merely useless: with
            // no terrain under it, its gain is zero, so it is CHEAP.
            // Left in, those edges are an attractive corridor out of the
            // area the pack actually covers.
            if !touches(&f.coords, region) {
                return;
            }
            let type_veg = f
                .attrs
                .iter()
                .find(|(k, _)| k == "typeVeg")
                .map(|(_, v)| v.as_str());
            ways.push(graph::Way {
                coords: f.coords.iter().map(|c| (c.x, c.y)).collect(),
                fkb_type: road_fkb_type(type_veg),
                marking: 0,
                surface: 0,
                source: SOURCE_N50,
            });
        })?;
        report.roads = ways.len();
        progress!(Phase::Vector, i + 1, kommuner.len() + 1);
    }

    // Trails come from the WFS, which is bbox-scoped — so unlike N50
    // this fetches only the region.
    let cells = wfs::grid_cells(
        wfs::BboxWgs84 {
            west: extent[0],
            south: extent[1],
            east: extent[2],
            north: extent[3],
        },
        wfs::GRID_DEG,
    );
    for cell in &cells {
        for t in wfs::fetch_cell(http, wfs_endpoint, *cell)? {
            ways.push(graph::Way {
                coords: t.coords.iter().map(|c| (c.x, c.y)).collect(),
                fkb_type: wfs::fkb_type_of(&t.kind),
                marking: 0,
                surface: 0,
                source: SOURCE_FKB,
            });
            report.trails += 1;
        }
    }
    progress!(Phase::Vector, kommuner.len() + 1, kommuner.len() + 1);

    // The kommune list is supplied by the caller, and nothing so far has
    // checked that it actually covers the region. Getting it wrong does
    // not fail: the mask refuses nothing, every road lands outside the
    // DEM, and the pack builds, verifies and installs — then routes
    // through lakes. So check it here, where both extents are known.
    // Only the N50-sourced features. The WFS trails were fetched BY
    // bbox, so they are in-region by construction — including them makes
    // the union overlap no matter how wrong the kommune is, which is
    // exactly the check quietly passing.
    let n50_ways: Vec<&graph::Way> = ways.iter().filter(|w| w.source == SOURCE_N50).collect();
    let vector_extent = extent_of(&water, &glacier, &n50_ways);
    match vector_extent {
        None => {
            return Err(BuildError::Logic(format!(
                "kommune {} yielded no water, glacier or road features at all",
                kommuner
                    .iter()
                    .map(|k| k.0.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }
        Some((vx0, vy0, vx1, vy1)) => {
            let overlaps = vx0 < region.max_x
                && vx1 > region.min_x
                && vy0 < region.max_y
                && vy1 > region.min_y;
            if !overlaps {
                return Err(BuildError::Logic(format!(
                    "kommune {} covers ({vx0:.0}, {vy0:.0})-({vx1:.0}, {vy1:.0}) but the region is \
                     ({:.0}, {:.0})-({:.0}, {:.0}) in EPSG:25833 — they do not overlap. The pack \
                     would build with an empty mask and no roads, and route through water.",
                    kommuner.iter().map(|k| k.0.as_str()).collect::<Vec<_>>().join(", "),
                    region.min_x, region.min_y, region.max_x, region.max_y
                )));
            }
        }
    }

    // ---- 3. Mask ----
    progress!(Phase::Mask, 0, water.len() + glacier.len());
    let mut mw = mask::MaskWriter::new(region);
    for p in &water {
        mw.add(p, RefusalKind::Water);
    }
    // After water: last write wins, and glacier over a tarn is glacier.
    for p in &glacier {
        mw.add(p, RefusalKind::Glacier);
    }
    report.water_polygons = mw.water_polygons;
    report.glacier_polygons = mw.glacier_polygons;
    report.refused_cells = mw.refused_cells();
    report.total_cells = mw.total_cells();
    mw.finish(out_dir)?;
    progress!(
        Phase::Mask,
        water.len() + glacier.len(),
        water.len() + glacier.len(),
    );

    // ---- 4. Graph ----
    // Reported per way rather than as one 0%-then-100% pair. This phase
    // samples the DEM at every vertex of every way, so on a big region
    // it is minutes of work, and as a single blocking call it left the
    // host's bar parked on the phase boundary for all of it.
    let (_, _, g) = graph::build(&ways, Some(&dem), out_dir, &mut |done, total| {
        on_progress(Phase::Graph, done, total)
    })?;
    report.nodes = g.nodes;
    report.edges_directed = g.edges_directed;
    report.edges_without_terrain = g.edges_without_terrain;
    progress!(Phase::Graph, ways.len(), ways.len());

    // ---- 5. Manifest ----
    progress!(Phase::Manifest, 0, 1);
    pack::write_manifest(
        out_dir,
        extent,
        halo_m,
        concat!("turbo-pack-build ", env!("CARGO_PKG_VERSION")),
    )?;
    pack::write_provenance(
        out_dir,
        &pack::Provenance {
            sources: vec![
                pack::Source {
                    kind: "wcs".into(),
                    url: wcs_endpoint.to_string(),
                    areas: vec![],
                },
                pack::Source {
                    kind: "n50-gml".into(),
                    url: "https://nedlasting.geonorge.no/api".into(),
                    areas: kommuner.iter().map(|k| k.0.clone()).collect(),
                },
                pack::Source {
                    kind: "fkb-wfs".into(),
                    url: wfs_endpoint.to_string(),
                    areas: vec![],
                },
            ],
            built_at: chrono::Utc::now().to_rfc3339(),
            built_by: concat!("turbo-pack-build ", env!("CARGO_PKG_VERSION")).into(),
            dem_resolution_m: wcs::RESOLUTION_M,
        },
    )?;
    progress!(Phase::Manifest, 1, 1);

    report.total_bytes = std::fs::read_dir(out_dir).at(out_dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    report.seconds = started.elapsed().as_secs_f64();
    Ok(report)
}

/// Does a polyline's bounding box meet the region?
///
/// Bbox rather than a real intersection test: a way whose box overlaps
/// but whose line does not is kept, which costs a few edges at the
/// margin. Dropping one that DOES cross would disconnect the network,
/// and that is the expensive mistake.
fn touches(coords: &[geo::Coord<f64>], region: crate::wcs::BoxUtm) -> bool {
    let mut x0 = f64::MAX;
    let mut y0 = f64::MAX;
    let mut x1 = f64::MIN;
    let mut y1 = f64::MIN;
    for c in coords {
        x0 = x0.min(c.x);
        y0 = y0.min(c.y);
        x1 = x1.max(c.x);
        y1 = y1.max(c.y);
    }
    x0 <= region.max_x && x1 >= region.min_x && y0 <= region.max_y && y1 >= region.min_y
}

fn ring_touches(p: &geo::Polygon<f64>, region: crate::wcs::BoxUtm) -> bool {
    touches(&p.exterior().0, region)
}

/// Bounding box of everything the vector sources produced.
fn extent_of(
    water: &[geo::Polygon<f64>],
    glacier: &[geo::Polygon<f64>],
    ways: &[&graph::Way],
) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    let mut bump = |x: f64, y: f64| {
        b = Some(match b {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    };
    for p in water.iter().chain(glacier.iter()) {
        for c in &p.exterior().0 {
            bump(c.x, c.y);
        }
    }
    for w in ways {
        for &(x, y) in &w.coords {
            bump(x, y);
        }
    }
    b
}

/// `source` byte, matching the server's provenance column.
const SOURCE_N50: u8 = 1;
const SOURCE_FKB: u8 = 2;

/// N50 `typeVeg` → the shared `fkb_type` vocabulary.
///
/// Everything here is a road of some kind; the distinction that matters
/// to the cost model is trail-vs-road, and `Veglenke` is never a trail.
fn road_fkb_type(type_veg: Option<&str>) -> u8 {
    match type_veg.unwrap_or("") {
        "traktorveg" | "traktorvei" => turbo_tiles_graph::encode_fkb_type(Some("traktorvei")),
        "gangOgSykkelveg" | "sykkelveg" => turbo_tiles_graph::encode_fkb_type(Some("sykkelvei")),
        // enkelBilveg, kanalisertVeg, rundkjøring, … all road.
        _ => turbo_tiles_graph::encode_fkb_type(Some("vei")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_road_type_encodes_as_a_road_never_a_trail() {
        for t in [
            "enkelBilveg",
            "kanalisertVeg",
            "rundkjøring",
            "traktorveg",
            "gangOgSykkelveg",
            "",
        ] {
            let code = road_fkb_type(Some(t));
            assert_eq!(code, 2, "{t} encoded as {code}, expected road");
        }
    }

    /// A `Veglenke` must never be classified as `sti`: on foot a trail
    /// is multiplier 1.0 and a road 1.6, so mislabelling roads as trails
    /// would route walkers onto them by preference.
    #[test]
    fn a_road_is_never_cheaper_than_a_trail_on_foot() {
        let road = turbo_tiles_graph::surface_multiplier(road_fkb_type(Some("enkelBilveg")), 0);
        let trail = turbo_tiles_graph::surface_multiplier(1, 0);
        assert!(
            trail < road,
            "trail {trail} should be preferred to road {road}"
        );
    }
}
