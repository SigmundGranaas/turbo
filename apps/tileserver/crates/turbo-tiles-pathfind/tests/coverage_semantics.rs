//! Coverage semantics: `Pathfinder::point_covered` (D1, was E6).
//!
//! **These tests were inverted in B2.** They originally *documented the
//! defect*: coverage was `layers.iter().any(|l| l.covers(x, y))`, so an
//! advisory landcover mask extending past the DEM granted routing coverage
//! at points with no elevation data. They now assert the fix — coverage is
//! the **intersection of `Requirement::Required` contributors**.
//!
//! Original finding, kept for context:
//!
//! From the assumption audit (finding A3). `point_covered` is
//!
//! ```ignore
//! self.layers.iter().any(|l| l.covers(x, y))
//! ```
//!
//! so **any** layer claiming coverage marks the point covered. But the
//! layers mean different things by `covers`:
//!
//! | Layer | `covers` | Meaning |
//! |---|---|---|
//! | `SlopeLayer` / `AvalancheTerrainLayer` | `dem.sample().is_ok()` | authoritative terrain |
//! | `MaskRefusalLayer` | `mask.refused().is_ok()` | authoritative water/glacier |
//! | `LandcoverLayer` | `mask.refused().is_ok()` | **advisory** — "is there forest here" |
//! | `TrailProximityLayer` | `any_near()` | **deliberately** narrowed, see its comment |
//!
//! `TrailProximityLayer` already carries a comment explaining that returning
//! `true` unconditionally "would short-circuit the no-terrain-data precheck",
//! so the Required/Advisory distinction is understood in the codebase — it is
//! just enforced ad hoc, per layer, by hand. `LandcoverLayer` did not get the
//! same treatment.
//!
//! The pre-check exists to stop the solver building a uniform-cost mesh and
//! returning a straight line, which `pathfinder.rs` calls "semantically a
//! lie". These tests pin down when that guarantee holds.

use std::io::Write;
use std::sync::Arc;

use turbo_tiles_artifacts::{write_header as write_art_header, ArtifactKind, Header, HEADER_BYTES};
use turbo_tiles_mask::{write_meta as write_mask_meta, Mask, MaskMeta, MASK_FORMAT_VERSION};
use turbo_tiles_pathfind::Pathfinder;

/// A DEM tile at 10 m covering `cells × cells` from upper-left `(ulx, uly)`.
fn write_flat_dem(path: &std::path::Path, ulx: f64, uly: f64, cells: u32, elev: f32) {
    use turbo_tiles_elev::{
        write_meta as write_dem_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD,
        DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
    };
    let meta = DemMeta {
        tile_count: 1,
        tile_cells: cells,
        pixel_size_m: 10.0,
        nodata: NODATA_SENTINEL,
        compression: COMPRESSION_ZSTD,
    };
    let mut f = std::fs::File::create(path).unwrap();
    write_art_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Dem,
            format_version: DEM_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .unwrap();
    write_dem_meta(&mut f, &meta).unwrap();
    let data = vec![elev; (cells * cells) as usize];
    let compressed = zstd::encode_all(bytemuck::cast_slice::<f32, u8>(&data), 1).unwrap();
    let dir_offset = (HEADER_BYTES + DEM_META_BYTES) as u64;
    let entry = TileEntry {
        ulx,
        uly,
        offset: dir_offset + TILE_ENTRY_BYTES as u64,
        compressed_size: compressed.len() as u32,
    };
    write_tile_entry(&mut f, &entry).unwrap();
    f.write_all(&compressed).unwrap();
    f.sync_all().unwrap();
}

/// A 2-bit mask over `[min_x, max_x] × [min_y, max_y]`, every cell `class`.
/// `class = 0` is "nothing here" — which is still *coverage*: the mask can
/// answer the question over this extent.
fn write_mask(path: &std::path::Path, min_x: f64, min_y: f64, extent_m: f64, class: u8) {
    let res = 100.0f32;
    let cells = (extent_m / res as f64).ceil() as u32;
    let meta = MaskMeta {
        min_x,
        min_y,
        max_x: min_x + extent_m,
        max_y: min_y + extent_m,
        cells_x: cells,
        cells_y: cells,
        resolution_m: res,
    };
    let mut f = std::fs::File::create(path).unwrap();
    write_art_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Mask,
            format_version: MASK_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .unwrap();
    write_mask_meta(&mut f, &meta).unwrap();
    // 2 bits per cell, 4 cells per byte.
    let packed = (class & 0b11) * 0b0101_0101;
    let bytes = vec![packed; ((cells as usize * cells as usize) + 3) / 4];
    f.write_all(&bytes).unwrap();
    f.sync_all().unwrap();
}

/// DEM over a 2.56 km tile at UTM33N (500000, 7500000); landcover mask over a
/// 20 km square with the same lower-left corner, so it extends far past the
/// DEM. `probe` is well outside the DEM but inside the mask.
struct Fixture {
    _dir: tempfile::TempDir,
    dem: Arc<turbo_tiles_elev::Dem>,
    forest: Arc<Mask>,
    inside_dem: (f64, f64),
    outside_dem_inside_mask: (f64, f64),
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let (ulx, uly) = (500_000.0f64, 7_502_560.0f64);
    let dem_path = dir.path().join("t.dem");
    write_flat_dem(&dem_path, ulx, uly, 256, 100.0);

    let mask_path = dir.path().join("forest.mask");
    write_mask(&mask_path, 500_000.0, 7_500_000.0, 20_000.0, 1);

    Fixture {
        dem: Arc::new(turbo_tiles_elev::Dem::open(&dem_path).unwrap()),
        forest: Arc::new(Mask::open(&mask_path).unwrap()),
        // Inside the 2.56 km DEM tile.
        inside_dem: (501_000.0, 7_501_000.0),
        // 15 km east: far outside the DEM, still inside the 20 km mask.
        outside_dem_inside_mask: (515_000.0, 7_501_000.0),
        _dir: dir,
    }
}

#[test]
fn dem_alone_reports_coverage_only_inside_the_dem() {
    let f = fixture();
    let pf = Pathfinder::with_defaults(Some(f.dem.clone()), None, None);

    assert!(
        pf.point_covered(f.inside_dem.0, f.inside_dem.1),
        "a point inside the DEM tile must be covered"
    );
    assert!(
        !pf.point_covered(f.outside_dem_inside_mask.0, f.outside_dem_inside_mask.1),
        "a point outside the DEM must NOT be covered when only a DEM is loaded"
    );
}

/// The fix (B2). An **advisory** landcover layer no longer widens coverage:
/// `point_covered` is the intersection of `Required` contributors, so a point
/// outside the DEM is honestly uncovered however many advisory layers can
/// answer questions there.
#[test]
fn advisory_landcover_layer_does_not_grant_coverage_without_elevation() {
    let f = fixture();
    let (x, y) = f.outside_dem_inside_mask;

    let dem_only = Pathfinder::with_defaults(Some(f.dem.clone()), None, None);
    assert!(
        !dem_only.point_covered(x, y),
        "precondition: the point is outside the DEM"
    );

    // Register the landcover mask exactly as `routing_setup.rs` does.
    let mut pf = Pathfinder::with_defaults(Some(f.dem.clone()), None, None);
    pf.push_native(Arc::new(turbo_tiles_pathfind::LandcoverContributor::new(
        f.forest.clone(),
        "forest",
        0.29,
    )));

    // Ground truth: there is genuinely no elevation here.
    assert!(
        f.dem
            .sample(turbo_tiles_elev::PointXY { x, y })
            .ok()
            .flatten()
            .is_none(),
        "precondition: no DEM sample at the probe point"
    );

    assert!(
        !pf.point_covered(x, y),
        "D1/B2: an advisory landcover layer must NOT grant routing coverage \
         at a point with no elevation data. Coverage is the intersection of \
         Required contributors, not the union of everything that answers."
    );
}

/// Required/Advisory is now a first-class property of the contributor
/// trait, so the distinction the audit asked for is expressible and
/// enforced rather than hand-rolled per layer.
#[test]
fn required_vs_advisory_is_expressible() {
    let f = fixture();
    let mut pf = Pathfinder::with_defaults(Some(f.dem.clone()), None, None);
    pf.push_native(Arc::new(turbo_tiles_pathfind::LandcoverContributor::new(
        f.forest.clone(),
        "forest",
        0.29,
    )));

    use turbo_tiles_pathfind::Requirement;
    let req: Vec<(&str, Requirement)> = pf
        .contributors_for_breakdown()
        .iter()
        .map(|c| (c.name(), c.requirement()))
        .collect();

    // Slope is load-bearing; landcover is advice.
    assert!(
        req.iter().any(|(n, r)| *n == "slope" && *r == Requirement::Required),
        "slope must be Required: {req:?}"
    );
    assert!(
        req.iter()
            .all(|(n, r)| *n != "forest" || *r == Requirement::Advisory),
        "landcover must be Advisory: {req:?}"
    );
    // And coverage inside the DEM still works.
    assert!(pf.point_covered(f.inside_dem.0, f.inside_dem.1));
}

/// B2b: endpoint refusal must agree with the SOLVER's cell refusal.
///
/// The point of porting `point_is_refused` off the legacy point query is
/// consistency: `Pathfinder::contributor_veto_at` now builds the identical
/// synthetic east-west cell edge that `cost_field::LazyCostField::ensure`
/// builds, so a point is refused by the endpoint check exactly when the
/// solver would refuse the cell containing it.
///
/// The legacy check asked a different question (`dem.slope_aspect` at the
/// bare point), so the two could disagree — an endpoint could be snapped
/// away from a cell the solver would happily traverse, or accepted into one
/// it would refuse. Measured on the 90-hike Sjunkhatten corpus, that
/// disagreement moved 11-12 routes.
#[test]
fn endpoint_refusal_agrees_with_solver_on_a_flat_dem() {
    let f = fixture();
    let pf = Pathfinder::with_defaults(Some(f.dem.clone()), None, None);

    // A flat DEM has no cliffs, so nothing inside coverage may be refused.
    // `solve` on two in-coverage points must therefore not fail with
    // EndpointRefused — the check that would fire if the endpoint rule
    // disagreed with the (uniformly passable) cost field.
    let (x, y) = f.inside_dem;
    let a = turbo_tiles_pathfind::utm33n_to_wgs84(x, y);
    let b = turbo_tiles_pathfind::utm33n_to_wgs84(x + 600.0, y + 600.0);

    let mut prefs = turbo_tiles_pathfind::Prefs::default();
    prefs.force_off_trail = true;
    prefs.snap_radius_m = 0.0;
    prefs.bridge_radius_m = 0.0;

    match pf.solve([a.0, a.1], [b.0, b.1], prefs) {
        Err(turbo_tiles_pathfind::PathfindError::EndpointRefused { which, layer }) => {
            panic!("flat terrain inside the DEM must not refuse an endpoint: {which} / {layer}")
        }
        _ => {}
    }
}
