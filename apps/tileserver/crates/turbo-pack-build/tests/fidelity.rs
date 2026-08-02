//! Does a DEM built from the WCS describe the same ground as one built
//! from PostGIS?
//!
//! This is the test the shared-format design exists to make possible.
//! `turbo-tiles-build` sources from a PostGIS raster staging table that
//! `raster2pgsql` loaded from GeoTIFFs an operator dropped on a volume;
//! `turbo-pack-build` sources from Kartverket's WCS. A disagreement
//! between them is invisible at every other layer — a wrong DEM does not
//! throw, it produces a route that avoids a slope that is not there.
//!
//! # What this can and cannot assert
//!
//! It cannot demand the two agree to a few centimetres, and a test that
//! did would be lying about what it checks. The two are *different
//! products*: the staging path carries whatever DTM vintage was loaded,
//! while the WCS resamples the current national height model from its
//! 1 m source per request. Lidar and photogrammetric surfaces genuinely
//! differ by metres in mountains.
//!
//! What it can assert is the part this crate could actually get wrong.
//! This code performs **no vertical arithmetic** — float32 samples are
//! copied from the GeoTIFF into the artifact unchanged — so a height
//! error cannot originate here. Placement can: a half-cell or whole-cell
//! shift is exactly the bug a hand-rolled TIFF reader and a hand-rolled
//! tiling loop produce, and it is undetectable by eye. So the gate is
//! **alignment**, measured by finding the offset that best registers the
//! two, plus loose bounds that would still catch a datum or unit blunder.
//!
//! ```text
//! TURBO_DEM_REF=/path/server-built/norway.dem \
//! TURBO_DEM_NEW=/path/wcs-built/norway.dem \
//! TURBO_DEM_BOX=min_x,min_y,max_x,max_y \
//!   cargo test -p turbo-pack-build --test fidelity -- --nocapture
//! ```

use turbo_tiles_elev::{Dem, PointXY};

/// Cell size of both DEMs, in metres.
const CELL_M: f64 = 10.0;

/// A vertical difference beyond this is not a product vintage — it is a
/// datum confusion (geoid vs ellipsoid is ~30 m in Norway) or a unit
/// error, and either is a bug worth failing on.
const MAX_MEAN_DELTA_M: f64 = 5.0;

/// Gross-corruption bound. Two DTM vintages over mountains differ by a
/// few metres RMS; ten is well clear of that and well under anything
/// that would indicate scrambled samples.
const MAX_RMS_M: f64 = 10.0;

fn open(var: &str) -> Option<Dem> {
    let p = std::env::var(var).ok()?;
    Some(Dem::open(&p).unwrap_or_else(|e| panic!("{var} = {p}: {e}")))
}

fn overlap_box() -> (f64, f64, f64, f64) {
    let v = std::env::var("TURBO_DEM_BOX").unwrap_or_default();
    let parts: Vec<f64> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    assert_eq!(
        parts.len(),
        4,
        "set TURBO_DEM_BOX=min_x,min_y,max_x,max_y in EPSG:25833"
    );
    (parts[0], parts[1], parts[2], parts[3])
}

struct Stats {
    compared: usize,
    mean: f64,
    rms: f64,
    only_reference: usize,
    only_built: usize,
}

/// Compare the two over a grid, with `built` shifted by (dx, dy).
fn compare(reference: &Dem, built: &Dem, dx: f64, dy: f64, step: f64) -> Stats {
    let (min_x, min_y, max_x, max_y) = overlap_box();
    let (mut n, mut sum, mut sq) = (0usize, 0.0f64, 0.0f64);
    let (mut only_reference, mut only_built) = (0usize, 0usize);
    let mut y = min_y;
    while y <= max_y {
        let mut x = min_x;
        while x <= max_x {
            let a = reference
                .sample(PointXY { x, y })
                .expect("reference sample");
            let b = built
                .sample(PointXY {
                    x: x + dx,
                    y: y + dy,
                })
                .expect("built sample");
            match (a, b) {
                (Some(a), Some(b)) => {
                    let d = (b - a) as f64;
                    n += 1;
                    sum += d;
                    sq += d * d;
                }
                (Some(_), None) => only_reference += 1,
                (None, Some(_)) => only_built += 1,
                (None, None) => {}
            }
            x += step;
        }
        y += step;
    }
    Stats {
        compared: n,
        mean: sum / n.max(1) as f64,
        rms: (sq / n.max(1) as f64).sqrt(),
        only_reference,
        only_built,
    }
}

#[test]
fn a_wcs_built_dem_registers_against_a_postgis_built_one() {
    let (Some(reference), Some(built)) = (open("TURBO_DEM_REF"), open("TURBO_DEM_NEW")) else {
        eprintln!("skipped: set TURBO_DEM_REF, TURBO_DEM_NEW and TURBO_DEM_BOX");
        return;
    };

    // Sub-cell steps, so a half-cell shift is visible and not aliased
    // away by sampling only on the grid both share.
    let offsets: Vec<f64> = (-4..=4).map(|i| i as f64 * CELL_M / 2.0).collect();
    let mut best = (f64::MAX, 0.0f64, 0.0f64);
    for &dy in &offsets {
        let mut row = String::new();
        for &dx in &offsets {
            let s = compare(&reference, &built, dx, dy, 100.0);
            row.push_str(&format!(" {:6.3}", s.rms));
            if s.rms < best.0 {
                best = (s.rms, dx, dy);
            }
        }
        eprintln!("dy {dy:+6.1} |{row}");
    }
    eprintln!(
        "best alignment: dx {:+.1} m, dy {:+.1} m (rms {:.3} m)",
        best.1, best.2, best.0
    );

    let aligned = compare(&reference, &built, 0.0, 0.0, 50.0);
    eprintln!(
        "at zero offset: {} points, mean {:+.3} m, rms {:.3} m, \
         coverage-only ref {} / built {}",
        aligned.compared, aligned.mean, aligned.rms, aligned.only_reference, aligned.only_built
    );

    assert!(aligned.compared > 100, "not enough overlapping terrain");

    // The gate. A whole-cell placement bug puts the minimum a full cell
    // away; anything inside half a cell is the noise floor of two
    // different surveys of the same mountain.
    assert!(
        best.1.abs() < CELL_M && best.2.abs() < CELL_M,
        "best registration is at ({:+.1}, {:+.1}) m — a placement error, not survey noise. \
         Suspect the tile origin or the tiled-TIFF row stride.",
        best.1,
        best.2
    );

    // No vertical arithmetic happens in this crate, so a large bias
    // means the two sources are on different datums — which is a real
    // problem for a pack, even though it is not this code's bug.
    assert!(
        aligned.mean.abs() < MAX_MEAN_DELTA_M,
        "mean difference {:+.3} m exceeds {MAX_MEAN_DELTA_M} m — suspect a vertical datum \
         (geoid vs ellipsoid is ~30 m here) rather than a DTM vintage",
        aligned.mean
    );
    assert!(
        aligned.rms < MAX_RMS_M,
        "rms {:.3} m exceeds {MAX_RMS_M} m — the surfaces do not describe the same ground",
        aligned.rms
    );

    // The asymmetric one. Terrain the reference lacks but this build
    // claims means nodata was read as ground — and ground at -32767 m
    // is a hole the router will happily fall into.
    assert_eq!(
        aligned.only_built, 0,
        "this build claims terrain the reference does not have — nodata read as ground"
    );
}
