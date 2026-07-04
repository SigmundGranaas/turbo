//! Tile-pipeline profiling + diagnostic suite (Part A: network saturation).
//!
//! Establishes empirical baselines for the question "how fast *can* tile
//! loading be, and where does it saturate?" — by punishing the real hosts the
//! app uses (Kartverket CDN for raster, our tileserver for DEM) at increasing
//! client concurrency and measuring throughput + latency percentiles.
//!
//! It shares ONE pooled `reqwest::blocking::Client` across N worker threads —
//! the same connection-reuse model the engine's `HttpRasterSource` uses — so
//! the numbers reflect the real transport (keep-alive, TLS reuse, HTTP/2).
//!
//! Run:
//!   cargo run -p turbomap-app --release --example tile_profiler
//!   TURBO_API_URL=https://kart-api.sandring.no cargo run ... --example tile_profiler
//!
//! Output is a per-host concurrency sweep (warm + cold) with the saturation
//! knee and the Little's-law minimum concurrency to hide latency — the inputs
//! for choosing the scheduler's max/min in-flight limits.

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn flush() {
    let _ = std::io::stdout().flush();
}

/// Web-mercator tile (z,x,y) for a lat/lng — matches `LatLng::tile_at`.
fn deg2num(lat: f64, lng: f64, z: u8) -> (u32, u32) {
    let n = (1u64 << z) as f64;
    let x = ((lng + 180.0) / 360.0 * n).floor() as u32;
    let lat_rad = lat.to_radians();
    let y = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n).floor() as u32;
    (x, y)
}

/// A `span × span` block of tiles centred on `(lat,lng)` at zoom `z`.
fn block(lat: f64, lng: f64, z: u8, span: i64) -> Vec<(u8, u32, u32)> {
    let (cx, cy) = deg2num(lat, lng, z);
    let half = span / 2;
    let mut out = Vec::new();
    for dy in -half..half {
        for dx in -half..half {
            let x = cx as i64 + dx;
            let y = cy as i64 + dy;
            if x >= 0 && y >= 0 {
                out.push((z, x as u32, y as u32));
            }
        }
    }
    out
}

fn raster_url(z: u8, x: u32, y: u32, cb: Option<u64>) -> String {
    let bust = cb.map(|n| format!("&_cb={n}")).unwrap_or_default();
    format!(
        "https://cache.atgcp1-prod.kartverket.cloud/v1/service\
         ?layer=topo&style=default&tilematrixset=webmercator&Service=WMTS\
         &Request=GetTile&Version=1.0.0&Format=image/png\
         &TileMatrix={z}&TileCol={x}&TileRow={y}{bust}"
    )
}

fn dem_url(base: &str, z: u8, x: u32, y: u32, cb: Option<u64>) -> String {
    let bust = cb.map(|n| format!("&_cb={n}")).unwrap_or_default();
    format!(
        "{}/v1/dem/rgb/{z}/{x}/{y}.png?halo=1{bust}",
        base.trim_end_matches('/')
    )
}

#[derive(Default, Clone)]
struct Sample {
    ms: f64,
    ok: bool,
    bytes: usize,
}

/// Drive exactly `conc` in-flight requests draining `urls`, on a shared pooled
/// client. Returns per-request samples + wall-clock of the whole batch.
fn run_at_concurrency(
    client: &Arc<reqwest::blocking::Client>,
    urls: &Arc<Vec<String>>,
    conc: usize,
) -> (Vec<Sample>, Duration) {
    let next = Arc::new(AtomicUsize::new(0));
    let samples = Arc::new(Mutex::new(Vec::with_capacity(urls.len())));
    let start = Instant::now();
    let mut handles = Vec::with_capacity(conc);
    for _ in 0..conc {
        let client = Arc::clone(client);
        let urls = Arc::clone(urls);
        let next = Arc::clone(&next);
        let samples = Arc::clone(&samples);
        handles.push(std::thread::spawn(move || {
            let mut local = Vec::new();
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= urls.len() {
                    break;
                }
                let t0 = Instant::now();
                let res = client.get(&urls[i]).send().and_then(|r| {
                    let ok = r.status().is_success();
                    r.bytes().map(|b| (ok, b.len()))
                });
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                match res {
                    Ok((ok, bytes)) => local.push(Sample { ms, ok, bytes }),
                    Err(_) => local.push(Sample {
                        ms,
                        ok: false,
                        bytes: 0,
                    }),
                }
            }
            samples.lock().unwrap().extend(local);
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    let wall = start.elapsed();
    let s = std::mem::take(&mut *samples.lock().unwrap());
    (s, wall)
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn sweep(
    name: &str,
    make_url: impl Fn(u8, u32, u32, Option<u64>) -> String,
    tiles: &[(u8, u32, u32)],
    concs: &[usize],
    cold: bool,
) {
    let client = Arc::new(
        reqwest::blocking::Client::builder()
            .pool_max_idle_per_host(64)
            .timeout(Duration::from_secs(15))
            .user_agent("turbo-tile-profiler")
            .build()
            .expect("client"),
    );
    println!(
        "\n=== {name} {} ({} tiles/run) ===",
        if cold { "[COLD/cache-bust]" } else { "[WARM]" },
        tiles.len()
    );
    println!(
        "{:>4} {:>9} {:>7} {:>7} {:>7} {:>7} {:>7} {:>5} {:>7}",
        "conc", "tiles/s", "p50", "p95", "p99", "max", "MB/s", "err", "tileKB"
    );
    let mut best_tps = 0.0_f64;
    let mut knee = 1usize;
    let mut knee_lat = 0.0_f64;
    for (run_idx, &c) in concs.iter().enumerate() {
        // Cache-bust per (conc,run) so cold means cold; warm reuses the same URLs.
        let cb = if cold {
            Some(0xC0FFEE + run_idx as u64 * 1_000_003)
        } else {
            None
        };
        let urls: Arc<Vec<String>> = Arc::new(
            tiles
                .iter()
                .enumerate()
                .map(|(i, &(z, x, y))| make_url(z, x, y, cb.map(|b| b + i as u64)))
                .collect(),
        );
        let (samples, wall) = run_at_concurrency(&client, &urls, c);
        let mut lat: Vec<f64> = samples.iter().map(|s| s.ms).collect();
        lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let oks: Vec<&Sample> = samples.iter().filter(|s| s.ok).collect();
        let errs = samples.len() - oks.len();
        let bytes_ok: usize = oks.iter().map(|s| s.bytes).sum();
        let wall_s = wall.as_secs_f64().max(1e-6);
        let tps = samples.len() as f64 / wall_s;
        let mbps = bytes_ok as f64 / wall_s / 1e6;
        let tile_kb = if oks.is_empty() {
            0.0
        } else {
            bytes_ok as f64 / oks.len() as f64 / 1024.0
        };
        println!(
            "{:>4} {:>9.1} {:>7.0} {:>7.0} {:>7.0} {:>7.0} {:>7.2} {:>5} {:>6.0}K",
            c,
            tps,
            pct(&lat, 0.50),
            pct(&lat, 0.95),
            pct(&lat, 0.99),
            lat.last().copied().unwrap_or(0.0),
            mbps,
            errs,
            tile_kb
        );
        flush();
        // Knee = last concurrency that improved throughput by >=10%.
        if tps > best_tps * 1.10 {
            best_tps = tps;
            knee = c;
            knee_lat = pct(&lat, 0.50);
        }
    }
    // Little's law: to sustain `best_tps` at `knee_lat` latency you need
    // ~ best_tps * knee_lat concurrent requests in flight. That is the MIN
    // in-flight to hide latency; going past the knee adds latency, not tps.
    let little = (best_tps * knee_lat / 1000.0).ceil().max(1.0);
    println!(
        "  → saturates ~conc={knee} ({best_tps:.0} tiles/s, p50 {knee_lat:.0}ms); \
         Little's-law min-in-flight ≈ {little:.0}. Recommended max ≈ {knee}, min ≈ {little:.0}."
    );
}

/// One COLD measurement at a single concurrency (cache-busted) → worst-case
/// first-touch latency. Kept to one small run so we don't force the tileserver
/// to regenerate hundreds of tiles. This seeds the worst-case latency model.
fn cold_sample(
    name: &str,
    make_url: impl Fn(u8, u32, u32, Option<u64>) -> String,
    tiles: &[(u8, u32, u32)],
    conc: usize,
) {
    let client = Arc::new(
        reqwest::blocking::Client::builder()
            .pool_max_idle_per_host(64)
            .timeout(Duration::from_secs(20))
            .user_agent("turbo-tile-profiler")
            .build()
            .expect("client"),
    );
    let urls: Arc<Vec<String>> = Arc::new(
        tiles
            .iter()
            .enumerate()
            .map(|(i, &(z, x, y))| make_url(z, x, y, Some(0xC01D_0000 + i as u64)))
            .collect(),
    );
    let (samples, wall) = run_at_concurrency(&client, &urls, conc);
    let mut lat: Vec<f64> = samples.iter().map(|s| s.ms).collect();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let errs = samples.iter().filter(|s| !s.ok).count();
    println!(
        "  COLD {name} @conc={conc}: {} tiles in {:.1}s | p50 {:.0}ms p95 {:.0}ms p99 {:.0}ms max {:.0}ms | {errs} err",
        samples.len(), wall.as_secs_f64(),
        pct(&lat, 0.50), pct(&lat, 0.95), pct(&lat, 0.99), lat.last().copied().unwrap_or(0.0)
    );
    flush();
}

fn main() {
    let base = std::env::var("TURBO_API_URL")
        .unwrap_or_else(|_| "https://kart-api.sandring.no".to_string());
    // Sjunkhatten / Bodø.
    let (lat, lng) = (67.23, 15.30);
    let concs = [1usize, 2, 4, 8, 16, 24, 32, 48, 64];

    println!("Tile saturation profiler — real hosts. base={base}");
    println!("Centre {lat},{lng}. Latency ms; tiles/s = block size / wall.");
    flush();

    // 8×8 = 64-tile blocks — enough for percentiles, light on the prod server.
    let dem_tiles = block(lat, lng, 13, 8);
    let ras_tiles = block(lat, lng, 14, 8);

    // Steady-state saturation curve (warm) — the input for max/min limits.
    let base_dem = base.clone();
    sweep(
        "DEM  (our tileserver)",
        move |z, x, y, cb| dem_url(&base_dem, z, x, y, cb),
        &dem_tiles,
        &concs,
        false,
    );
    sweep(
        "RASTER (Kartverket CDN)",
        raster_url,
        &ras_tiles,
        &concs,
        false,
    );

    // Worst-case first-touch (cold), one run each.
    println!("\n=== COLD first-touch (cache-busted, worst case) ===");
    flush();
    let base_dem2 = base.clone();
    cold_sample(
        "DEM ",
        move |z, x, y, cb| dem_url(&base_dem2, z, x, y, cb),
        &dem_tiles,
        16,
    );
    cold_sample("RAS ", raster_url, &ras_tiles, 16);
}
