//! Headless terrain-corpus evaluation — the autonomous routing
//! development loop.
//!
//! `tileserver eval-terrain` loads the production routing artifacts and
//! layer stack IN-PROCESS (no HTTP server, no database), solves every
//! ground-truth hike in `tools/terrain-corpus.toml` with the
//! force-off-trail prefs the quality harness uses, and emits one
//! machine-readable JSON per hike plus a run summary. It samples
//! elevation profiles directly from the loaded DEM, so the proven
//! `terrain_metrics.py` scoring can run fully offline against this
//! output (see `--score`).
//!
//! This is the substrate that lets an agent iterate on the router
//! (change → measure quality/latency/geometry-drift → verdict) with no
//! human in the loop. Solves are deterministic, so a per-route geometry
//! hash is an exact change-detector.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use turbo_tiles_elev::{wgs84_to_utm33n, Dem, PointXY};
use turbo_tiles_pathfind::{CostMode, PathStrategy, Pathfinder, Prefs};

use crate::routing_setup;

/// Elevation profile sample spacing (m) along a polyline. Matches the
/// ~20 m sampling `terrain_metrics.py` requests from `/v1/elev/profile`
/// so the offline-scored metrics line up with the HTTP path.
const PROFILE_STEP_M: f64 = 20.0;

#[derive(serde::Deserialize)]
struct Corpus {
    #[serde(default)]
    hike: Vec<Hike>,
}

#[derive(serde::Deserialize)]
struct Hike {
    id: i64,
    region: String,
    /// Corpus-reported length; we recompute from the polyline, so this
    /// is only read for cross-checking.
    #[serde(default)]
    #[allow(dead_code)]
    length_m: f64,
    from: [f64; 2],
    to: [f64; 2],
    polyline: Vec<[f64; 2]>,
}

/// One polyline plus its elevation profile, ready for offline scoring.
#[derive(serde::Serialize)]
struct ProfiledLine {
    /// [lon, lat] vertices.
    polyline: Vec<[f64; 2]>,
    /// Cumulative horizontal distance (m) at each elevation sample.
    distances_m: Vec<f64>,
    /// Elevation (m) at each sample; `null` where the DEM has no data.
    elev_m: Vec<Option<f64>>,
    length_m: f64,
}

/// Per-hike evaluation record. `terrain_metrics.py --offline` reads
/// these; the summary aggregates them.
#[derive(serde::Serialize)]
struct HikeResult {
    id: i64,
    region: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strategy: Option<String>,
    solve_ms: f64,
    /// Deterministic hash of the solver geometry — the exact
    /// change-detector for regression diffing.
    geometry_hash: String,
    /// Ground-truth hike (corpus polyline) + its profile.
    truth: ProfiledLine,
    /// Solver output + its profile (empty polyline when the solve
    /// failed).
    solver: ProfiledLine,
}

#[derive(serde::Serialize)]
struct RunSummary {
    corpus: String,
    total: usize,
    ok: usize,
    failed: usize,
    solve_ms_mean: f64,
    solve_ms_p50: f64,
    solve_ms_p95: f64,
    solve_ms_max: f64,
    /// Stable hash of all per-hike geometry hashes in corpus order —
    /// one number that flips iff any route changed.
    corpus_geometry_hash: String,
    /// Peak resident set size of this process (MiB) — includes faulted
    /// DEM/mask mmap pages, so it reflects the memory the solver
    /// actually touches over the corpus. The memory axis of the loop.
    peak_rss_mb: f64,
    /// DEM tile-cache lookups (hits + misses) over the corpus solve —
    /// ~4 per `dem.sample()`. DETERMINISTic (unlike wall-clock), so it's
    /// the noise-free proxy for "how much DEM work the solver does": the
    /// exact before/after signal for sampling optimisations, and a
    /// regression guard.
    dem_cache_lookups: u64,
    /// Set when `--check-determinism` ran a second pass and every hash
    /// matched (or lists the hikes that differed).
    #[serde(skip_serializing_if = "Option::is_none")]
    determinism_ok: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    determinism_mismatches: Vec<i64>,
}

/// Which router a lane exercises. The corpus endpoints are sti graph
/// nodes, so the two modes dispatch to DIFFERENT solvers (see
/// `Pathfinder::solve_inner`):
///   - `OffTrail`: `force_off_trail=true`, snapping disabled — the FMM
///     grade-limited solver re-derives the route from terrain. Quality
///     vs ground truth is meaningful here.
///   - `Unified`: production-default prefs — endpoints snap to the
///     graph and the unified A* (mesh ∪ trail) solves it. Quality vs
///     truth is trivially high (the route retraces the trail), but
///     geometry hash / latency / DEM work are real regression signals
///     for the router users actually hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvalMode {
    OffTrail,
    Unified,
}

impl std::str::FromStr for EvalMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "off-trail" => Ok(Self::OffTrail),
            "unified" => Ok(Self::Unified),
            other => Err(format!("unknown mode `{other}` (off-trail|unified)")),
        }
    }
}

/// Entry point for the `eval-terrain` subcommand.
pub fn run(
    corpus_path: PathBuf,
    artifacts_dir: Option<PathBuf>,
    out_dir: PathBuf,
    filter: Option<String>,
    limit: Option<usize>,
    check_determinism: bool,
    mode: EvalMode,
) -> Result<()> {
    let corpus_text = std::fs::read_to_string(&corpus_path)
        .with_context(|| format!("reading corpus {}", corpus_path.display()))?;
    let corpus: Corpus = toml::from_str(&corpus_text)
        .with_context(|| format!("parsing corpus {}", corpus_path.display()))?;

    let hikes: Vec<Hike> = corpus
        .hike
        .into_iter()
        .filter(|h| match &filter {
            Some(f) => h.region.contains(f.as_str()) || h.id.to_string().contains(f.as_str()),
            None => true,
        })
        .take(limit.unwrap_or(usize::MAX))
        .collect();

    // Build the production pathfinder in-process — no DB, no server.
    let art = routing_setup::load_routing_artifacts(artifacts_dir.as_deref());
    let dem = art
        .dem
        .clone()
        .context("eval-terrain needs a DEM artifact (norway.dem) for elevation profiles")?;
    let cost_config = routing_setup::load_cost_config();
    let (pf, _landcover) =
        routing_setup::build_pathfinder(artifacts_dir.as_deref(), &art, cost_config);

    eprintln!(
        "eval-terrain: {} hikes, layers={:?}",
        hikes.len(),
        pf.layer_names()
    );

    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating out dir {}", out_dir.display()))?;

    // Capture DEM cache lookups across exactly the first solve pass
    // (the determinism re-run below would otherwise double-count).
    let cache_before = dem.cache_stats();
    let results = solve_corpus(&pf, &dem, &hikes, mode);
    let cache_after = dem.cache_stats();
    let dem_cache_lookups = (cache_after.hits + cache_after.misses)
        .saturating_sub(cache_before.hits + cache_before.misses);

    // Optional determinism gate: solve again, compare geometry hashes.
    let mut determinism_ok = None;
    let mut mismatches = Vec::new();
    if check_determinism {
        let second = solve_corpus(&pf, &dem, &hikes, mode);
        for (a, b) in results.iter().zip(second.iter()) {
            if a.geometry_hash != b.geometry_hash {
                mismatches.push(a.id);
            }
        }
        determinism_ok = Some(mismatches.is_empty());
        eprintln!(
            "eval-terrain: determinism {} ({} mismatches)",
            if mismatches.is_empty() { "OK" } else { "FAILED" },
            mismatches.len()
        );
    }

    // Write per-hike JSON.
    for r in &results {
        let fname = format!("{}-{}.json", r.region, r.id);
        let path = out_dir.join(&fname);
        let json = serde_json::to_string(r).context("serializing hike result")?;
        std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    }

    let summary = summarize(
        &corpus_path,
        &results,
        determinism_ok,
        mismatches,
        peak_rss_mb(),
        dem_cache_lookups,
    );
    let summary_path = out_dir.join("_summary.json");
    std::fs::write(
        &summary_path,
        serde_json::to_string_pretty(&summary).context("serializing summary")?,
    )
    .with_context(|| format!("writing {}", summary_path.display()))?;

    // Human-readable headline on stdout (the durable signal is the
    // JSON files; this is for a glance).
    println!(
        "eval-terrain: {}/{} ok, solve_ms mean={:.1} p50={:.1} p95={:.1} max={:.1}, peak_rss={:.0}MiB, dem_lookups={}, corpus_hash={}",
        summary.ok,
        summary.total,
        summary.solve_ms_mean,
        summary.solve_ms_p50,
        summary.solve_ms_p95,
        summary.solve_ms_max,
        summary.peak_rss_mb,
        summary.dem_cache_lookups,
        summary.corpus_geometry_hash,
    );
    eprintln!("eval-terrain: wrote {} + per-hike JSON", summary_path.display());
    Ok(())
}

fn solve_corpus(pf: &Pathfinder, dem: &Dem, hikes: &[Hike], mode: EvalMode) -> Vec<HikeResult> {
    hikes.iter().map(|h| solve_one(pf, dem, h, mode)).collect()
}

fn solve_one(pf: &Pathfinder, dem: &Dem, h: &Hike, mode: EvalMode) -> HikeResult {
    let prefs = match mode {
        // Force the off-trail solver so the route is re-derived from
        // terrain, not retraced from the corpus trail. Mirrors
        // `terrain_metrics.py --force-off-trail`.
        EvalMode::OffTrail => Prefs {
            force_off_trail: true,
            snap_radius_m: 0.0,
            bridge_radius_m: 0.0,
            allow_off_trail: true,
            max_off_trail_km: 20.0,
            cost_mode: CostMode::FastMarching,
            ..Default::default()
        },
        // Production defaults — the unified A* over mesh ∪ trail, the
        // router users actually hit.
        EvalMode::Unified => Prefs::default(),
    };

    let truth = profile_line(dem, &h.polyline);

    let t0 = Instant::now();
    let solved = pf.solve(h.from, h.to, prefs);
    let solve_ms = t0.elapsed().as_secs_f64() * 1e3;

    match solved {
        Ok(path) => {
            let geometry_hash = hash_geometry(&path.geometry);
            let solver = profile_line(dem, &path.geometry);
            HikeResult {
                id: h.id,
                region: h.region.clone(),
                ok: true,
                error: None,
                strategy: Some(strategy_str(path.strategy)),
                solve_ms,
                geometry_hash,
                truth,
                solver,
            }
        }
        Err(e) => HikeResult {
            id: h.id,
            region: h.region.clone(),
            ok: false,
            error: Some(e.to_string()),
            strategy: None,
            solve_ms,
            geometry_hash: "none".to_string(),
            truth,
            solver: ProfiledLine {
                polyline: Vec::new(),
                distances_m: Vec::new(),
                elev_m: Vec::new(),
                length_m: 0.0,
            },
        },
    }
}

/// Sample an elevation profile along a [lon,lat] polyline at
/// `PROFILE_STEP_M` spacing. Distances are measured in UTM33N metres
/// (the DEM's CRS), so they line up with the elevation samples.
fn profile_line(dem: &Dem, poly: &[[f64; 2]]) -> ProfiledLine {
    if poly.len() < 2 {
        return ProfiledLine {
            polyline: poly.to_vec(),
            distances_m: Vec::new(),
            elev_m: Vec::new(),
            length_m: 0.0,
        };
    }
    // Project vertices to UTM33N and build cumulative distances.
    let utm: Vec<PointXY> = poly.iter().map(|p| wgs84_to_utm33n(p[0], p[1])).collect();
    let mut seg_len = Vec::with_capacity(utm.len() - 1);
    let mut total = 0.0;
    for w in utm.windows(2) {
        let d = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        seg_len.push(d);
        total += d;
    }
    // Walk along the polyline emitting a sample every PROFILE_STEP_M.
    let n_samples = (total / PROFILE_STEP_M).floor() as usize + 1;
    let mut sample_pts = Vec::with_capacity(n_samples + 1);
    let mut distances = Vec::with_capacity(n_samples + 1);
    for s in 0..=n_samples {
        let target = (s as f64 * PROFILE_STEP_M).min(total);
        let (x, y) = point_at_distance(&utm, &seg_len, target);
        sample_pts.push(PointXY { x, y });
        distances.push(target);
        if target >= total {
            break;
        }
    }
    // Always include the exact endpoint.
    if *distances.last().unwrap_or(&0.0) < total {
        let last = utm[utm.len() - 1];
        sample_pts.push(last);
        distances.push(total);
    }

    let elev = match dem.profile(&sample_pts) {
        Ok(v) => v.into_iter().map(|o| o.map(|f| f as f64)).collect(),
        Err(_) => vec![None; sample_pts.len()],
    };

    ProfiledLine {
        polyline: poly.to_vec(),
        distances_m: distances,
        elev_m: elev,
        length_m: total,
    }
}

/// Interpolate the (x,y) at cumulative `target` distance along a
/// projected polyline with known segment lengths.
fn point_at_distance(utm: &[PointXY], seg_len: &[f64], target: f64) -> (f64, f64) {
    let mut acc = 0.0;
    for (i, &len) in seg_len.iter().enumerate() {
        if acc + len >= target || i == seg_len.len() - 1 {
            let t = if len > 0.0 { (target - acc) / len } else { 0.0 };
            let a = utm[i];
            let b = utm[i + 1];
            return (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        }
        acc += len;
    }
    let last = utm[utm.len() - 1];
    (last.x, last.y)
}

fn strategy_str(s: PathStrategy) -> String {
    match s {
        PathStrategy::OnGraph => "on_graph",
        PathStrategy::OffTrail => "off_trail",
        PathStrategy::Hybrid => "hybrid",
    }
    .to_string()
}

/// Deterministic geometry hash: quantise coords to ~1e-6° (~0.1 m) so
/// float noise below the routing grid resolution doesn't flip the hash,
/// then hash with the fixed-key `DefaultHasher` (stable across runs).
fn hash_geometry(geom: &[[f64; 2]]) -> String {
    let mut hasher = DefaultHasher::new();
    geom.len().hash(&mut hasher);
    for p in geom {
        ((p[0] * 1e6).round() as i64).hash(&mut hasher);
        ((p[1] * 1e6).round() as i64).hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Peak resident set size of this process in MiB, via `getrusage`.
/// `ru_maxrss` is bytes on macOS, kibibytes on Linux.
fn peak_rss_mb() -> f64 {
    let bytes = unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut ru) != 0 {
            return 0.0;
        }
        let max_rss = ru.ru_maxrss as f64;
        if cfg!(target_os = "macos") {
            max_rss
        } else {
            max_rss * 1024.0
        }
    };
    bytes / (1024.0 * 1024.0)
}

fn summarize(
    corpus_path: &Path,
    results: &[HikeResult],
    determinism_ok: Option<bool>,
    determinism_mismatches: Vec<i64>,
    peak_rss_mb: f64,
    dem_cache_lookups: u64,
) -> RunSummary {
    let total = results.len();
    let ok = results.iter().filter(|r| r.ok).count();
    let mut times: Vec<f64> = results.iter().map(|r| r.solve_ms).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = if times.is_empty() {
        0.0
    } else {
        times.iter().sum::<f64>() / times.len() as f64
    };
    let pct = |p: f64| -> f64 {
        if times.is_empty() {
            return 0.0;
        }
        let idx = ((times.len() as f64 - 1.0) * p).round() as usize;
        times[idx]
    };

    let mut corpus_hasher = DefaultHasher::new();
    for r in results {
        r.geometry_hash.hash(&mut corpus_hasher);
    }

    RunSummary {
        corpus: corpus_path.display().to_string(),
        total,
        ok,
        failed: total - ok,
        solve_ms_mean: mean,
        solve_ms_p50: pct(0.50),
        solve_ms_p95: pct(0.95),
        solve_ms_max: times.last().copied().unwrap_or(0.0),
        corpus_geometry_hash: format!("{:016x}", corpus_hasher.finish()),
        peak_rss_mb,
        dem_cache_lookups,
        determinism_ok,
        determinism_mismatches,
    }
}
