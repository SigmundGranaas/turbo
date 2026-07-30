//! E2 — the abstraction penalty for the elevation port.
//!
//! Phase 0 showed the corpus harness cannot resolve a 2% effect: run-to-run
//! spread on identical code is 6-7% sd / 16-19% range. Its geometry hash and
//! DEM lookup count are perfectly stable, so it remains the right
//! *correctness* gate — but it is the wrong *timing* instrument.
//!
//! This measures dispatch directly, against the REAL Sjunkhatten DEM
//! artifact with the access pattern a solve actually produces, then derives
//! the solve-level penalty from the measured per-call delta and the known
//! lookup counts. Measuring a nanosecond effect and multiplying beats trying
//! to see it through 19% noise.
//!
//! Three arms, matching the E2 plan:
//!   A  concrete   `Dem::sample`                    (today)
//!   B  dyn        `&dyn ElevationLike`             (pessimistic bound)
//!   C  generic    `fn f<H: ElevationLike>(h: &H)`  (what the design proposes)
//!
//! Usage: cargo run --release -- <path-to-norway.dem>

use std::hint::black_box;
use std::time::Instant;

use turbo_tiles_elev::{Dem, DemCoverage, PointXY, SlopeAspect};

// ---------------------------------------------------------------------------
// The port, exactly as the module design proposes it. Two hot methods.
// ---------------------------------------------------------------------------
pub trait ElevationLike: Send + Sync {
    fn sample(&self, p: PointXY) -> Option<f32>;
    fn slope_aspect(&self, p: PointXY) -> Option<SlopeAspect>;
    fn coverage(&self) -> DemCoverage;
}

impl ElevationLike for Dem {
    #[inline]
    fn sample(&self, p: PointXY) -> Option<f32> {
        Dem::sample(self, p).ok().flatten()
    }
    #[inline]
    fn slope_aspect(&self, p: PointXY) -> Option<SlopeAspect> {
        Dem::slope_aspect(self, p).ok().flatten()
    }
    #[inline]
    fn coverage(&self) -> DemCoverage {
        Dem::coverage(self)
    }
}

// ---- Arm A: concrete -------------------------------------------------------
#[inline(never)]
fn sum_concrete(dem: &Dem, pts: &[PointXY]) -> f64 {
    let mut acc = 0.0f64;
    for &p in pts {
        if let Some(z) = Dem::sample(dem, p).ok().flatten() {
            acc += z as f64;
        }
    }
    acc
}

// ---- Arm B: dynamic dispatch ----------------------------------------------
#[inline(never)]
fn sum_dyn(dem: &dyn ElevationLike, pts: &[PointXY]) -> f64 {
    let mut acc = 0.0f64;
    for &p in pts {
        if let Some(z) = dem.sample(p) {
            acc += z as f64;
        }
    }
    acc
}

// ---- Arm C: monomorphized generic -----------------------------------------
#[inline(never)]
fn sum_generic<H: ElevationLike + ?Sized>(dem: &H, pts: &[PointXY]) -> f64 {
    let mut acc = 0.0f64;
    for &p in pts {
        if let Some(z) = dem.sample(p) {
            acc += z as f64;
        }
    }
    acc
}

// ---- slope_aspect variants (5 of the 16 hot-path calls) --------------------
#[inline(never)]
fn slope_concrete(dem: &Dem, pts: &[PointXY]) -> f64 {
    let mut acc = 0.0f64;
    for &p in pts {
        if let Some(sa) = Dem::slope_aspect(dem, p).ok().flatten() {
            acc += sa.slope_deg as f64;
        }
    }
    acc
}

#[inline(never)]
fn slope_dyn(dem: &dyn ElevationLike, pts: &[PointXY]) -> f64 {
    let mut acc = 0.0f64;
    for &p in pts {
        if let Some(sa) = dem.slope_aspect(p) {
            acc += sa.slope_deg as f64;
        }
    }
    acc
}

/// Build an access pattern resembling a real corridor solve: a swathe of
/// cell centres walked in row-major order, which is what `CostField` and
/// `DemElevation` produce. Spatial locality matters — a random scatter would
/// exaggerate the DEM's tile-cache cost and mask dispatch.
fn corridor_points(cov: &DemCoverage, n: usize) -> Vec<PointXY> {
    let (w, h) = (cov.cells_x as f64 * 10.0, cov.cells_y as f64 * 10.0);
    // Inset so every sample lands inside coverage.
    let (ox, oy) = (cov.min_x + w * 0.25, cov.min_y + h * 0.25);
    let side = (n as f64).sqrt() as usize + 1;
    let mut v = Vec::with_capacity(n);
    for j in 0..side {
        for i in 0..side {
            if v.len() == n {
                return v;
            }
            v.push(PointXY {
                x: ox + i as f64 * 10.0,
                y: oy + j as f64 * 10.0,
            });
        }
    }
    v
}

/// Min-of-reps: for throughput microbenchmarks the minimum is the cleanest
/// estimator — noise is strictly additive, so the fastest observed run is
/// closest to the true cost.
fn bench(label: &str, reps: usize, pts: &[PointXY], mut f: impl FnMut(&[PointXY]) -> f64) -> f64 {
    // Warm the tile cache so we measure dispatch, not zstd decompression.
    black_box(f(pts));
    black_box(f(pts));
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t = Instant::now();
        let r = f(pts);
        let ns = t.elapsed().as_secs_f64() * 1e9 / pts.len() as f64;
        black_box(r);
        if ns < best {
            best = ns;
        }
    }
    println!("  {label:<28} {best:>8.3} ns/call");
    best
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: e2_dispatch <path-to-norway.dem>");
        std::process::exit(2);
    });
    let dem = Dem::open(&path).expect("open DEM");
    let cov = dem.coverage();
    println!("E2 — elevation port dispatch penalty");
    println!("DEM {} ({}x{} cells)\n", path, cov.cells_x, cov.cells_y);

    const N: usize = 2_000_000;
    const REPS: usize = 7;
    let pts = corridor_points(&cov, N);
    println!("{N} points, corridor-ordered, {REPS} reps, min-of-reps\n");

    let dyn_ref: &dyn ElevationLike = &dem;

    println!("sample():");
    let a = bench("A concrete", REPS, &pts, |p| sum_concrete(&dem, p));
    let b = bench("B dyn", REPS, &pts, |p| sum_dyn(dyn_ref, p));
    let c = bench("C generic (monomorphised)", REPS, &pts, |p| sum_generic(&dem, p));

    println!("\nslope_aspect():");
    let sa = bench("A concrete", REPS, &pts, |p| slope_concrete(&dem, p));
    let sb = bench("B dyn", REPS, &pts, |p| slope_dyn(dyn_ref, p));

    println!("\n--- per-call deltas ---");
    println!("  sample  B-A = {:+.3} ns  ({:+.2}%)", b - a, 100.0 * (b - a) / a);
    println!("  sample  C-A = {:+.3} ns  ({:+.2}%)", c - a, 100.0 * (c - a) / a);
    println!("  slope   B-A = {:+.3} ns  ({:+.2}%)", sb - sa, 100.0 * (sb - sa) / sa);

    // ---- derive the solve-level penalty from measured deltas --------------
    // Lookup counts and clean N=5 solve means from phase 0.
    println!("\n--- derived solve-level penalty ---");
    println!("  {:<12} {:>12} {:>12} {:>14} {:>10}", "lane", "lookups", "solve_ms", "dyn penalty", "% of solve");
    for (lane, lookups, total_ms) in [
        ("off-trail", 1_921_389f64, 588.17 * 12.0),
        ("unified", 249_976f64, 18.05 * 12.0),
    ] {
        let pen_ms = (b - a) * lookups / 1e6;
        println!(
            "  {:<12} {:>12.0} {:>12.1} {:>11.3} ms {:>9.3}%",
            lane, lookups, total_ms, pen_ms, 100.0 * pen_ms / total_ms
        );
    }
    println!("\n  (C is monomorphised, so its solve-level penalty is whatever");
    println!("   C-A shows above — expected to be indistinguishable from 0.)");
    println!("\n  Budget is 2%. Phase 0 noise floor was 16-19% range, which is");
    println!("  why this is measured per-call and multiplied out rather than");
    println!("  read off the corpus.");
}
