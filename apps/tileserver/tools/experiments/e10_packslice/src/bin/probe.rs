//! E10b — is `Dem::sample` deterministic with respect to the TILE SET?
//!
//! E10's halo sweep was non-monotonic: one route differed at halos 0/500/
//! 1000/2000/6000 but matched at 3000/4000. A missing-terrain effect must be
//! monotonic, so something else is going on.
//!
//! The DEM v2 reader builds an rstar over tile bboxes at `open()` and answers
//! a sample by finding the tile containing the point. Its own docs say:
//! *"Tiles overlap only when source rasters did (rare); a sample inside an
//! overlap returns the latest source's value (insertion order)."* The
//! Sjunkhatten DEM has **76 overlapping tile pairs** — the four DTM10 sheet
//! quadrants genuinely overlap.
//!
//! If which tile answers a query depends on the *set* of tiles present, then
//! slicing changes elevations, and pack parity is broken at the format level.
//! This samples the same points from the full DEM and from slices, and
//! counts disagreements.
//!
//! Usage: probe <artifacts-dir> <scratch-dir>

use turbo_tiles_elev::format::{read_meta, read_tile_entry};
use turbo_tiles_elev::{Dem, PointXY};

fn main() {
    let art = std::env::args().nth(1).expect("artifacts dir");
    let scratch = std::env::args().nth(2).expect("scratch dir");
    let art = std::path::Path::new(&art);
    let src = art.join("norway.dem");

    // Re-read the directory to find the overlap bands.
    let mut f = std::fs::File::open(&src).unwrap();
    turbo_tiles_artifacts::read_header(&mut f).unwrap();
    let meta = read_meta(&mut f).unwrap();
    let span = meta.tile_cells as f64 * meta.pixel_size_m as f64;
    let mut ents = Vec::new();
    for _ in 0..meta.tile_count {
        let e = read_tile_entry(&mut f).unwrap();
        ents.push((e.ulx, e.uly));
    }
    let mut overlaps = Vec::new();
    for i in 0..ents.len() {
        for j in (i + 1)..ents.len() {
            let (ax, ay) = ents[i];
            let (bx, by) = ents[j];
            if ax < bx + span && bx < ax + span && ay - span < by && by - span < ay {
                overlaps.push((
                    ax.max(bx),
                    (ay - span).max(by - span),
                    (ax + span).min(bx + span),
                    ay.min(by),
                ));
            }
        }
    }
    println!("# E10b — is Dem::sample stable under slicing?");
    println!("tiles={} span={span} m, overlapping pairs={}", meta.tile_count, overlaps.len());
    if let Some(o) = overlaps.first() {
        println!("first overlap band: x [{:.0}, {:.0}]  y [{:.0}, {:.0}]  ({:.0} x {:.0} m)",
                 o.0, o.2, o.1, o.3, o.2 - o.0, o.3 - o.1);
    }

    let full = Dem::open(&src).unwrap();

    // Compare against every slice E10 produced, if still present; otherwise
    // just report the overlap geometry.
    let mut any = false;
    for halo in [0u32, 500, 1000, 2000, 3000, 4000, 6000] {
        let p = std::path::Path::new(&scratch).join(format!("h{halo}.dem"));
        if !p.exists() {
            continue;
        }
        any = true;
        let sl = Dem::open(&p).unwrap();
        let cov = sl.coverage();
        // Dense grid over the slice's own coverage, 50 m steps.
        let (mut n, mut differ, mut only_full, mut only_slice) = (0u64, 0u64, 0u64, 0u64);
        let mut first: Option<(f64, f64, f32, f32)> = None;
        let mut y = cov.min_y + 25.0;
        while y < cov.min_y + cov.cells_y as f64 * 10.0 {
            let mut x = cov.min_x + 25.0;
            while x < cov.min_x + cov.cells_x as f64 * 10.0 {
                let p = PointXY { x, y };
                let a = full.sample(p).ok().flatten();
                let b = sl.sample(p).ok().flatten();
                n += 1;
                match (a, b) {
                    (Some(u), Some(v)) => {
                        if u.to_bits() != v.to_bits() {
                            differ += 1;
                            if first.is_none() {
                                first = Some((x, y, u, v));
                            }
                        }
                    }
                    (Some(_), None) => only_full += 1,
                    (None, Some(_)) => only_slice += 1,
                    (None, None) => {}
                }
                x += 50.0;
            }
            y += 50.0;
        }
        println!(
            "halo={halo:<5} samples={n:<9} differ={differ:<7} only_full={only_full:<8} only_slice={only_slice}"
        );
        if let Some((x, y, u, v)) = first {
            println!("   first disagreement at ({x:.0}, {y:.0}): full={u} slice={v}");
        }
    }
    if !any {
        println!("(no slices in scratch dir — run e10 with --keep, or re-run e10 first)");
    }
}
