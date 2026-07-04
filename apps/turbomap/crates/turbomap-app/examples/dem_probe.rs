//! Agent-first DEM endpoint inspector.
//!
//! Fetches Terrain-RGB tiles from any `/v1/dem/rgb/{z}/{x}/{y}.png`
//! endpoint, computes per-tile elevation statistics, generates an
//! analytic hillshade preview for visual sanity checking, and (in
//! `--grid` mode) stitches a rectangle of tiles into a single composite
//! PNG so you can eyeball a region of interest end-to-end.
//!
//! Output is line-oriented: one JSON object per tile to stdout (greppable
//! and easy for another agent or script to consume), plus the rendered
//! PNGs on disk.
//!
//! Examples:
//!
//!   # Single tile, raw + hillshade saved next to it.
//!   dem_probe --server http://127.0.0.1:8086 --tile 9/268/151 \
//!             --out /tmp/dem-bergen
//!
//!   # 4×4 grid of z=10 tiles centred on Jotunheimen.
//!   dem_probe --server http://127.0.0.1:8086 --center 61.50,8.30 \
//!             --zoom 10 --grid 4 --out /tmp/dem-jotunheimen
//!
//!   # Latency benchmark (warm-cache after the first request).
//!   dem_probe --server http://127.0.0.1:8086 --tile 9/268/151 \
//!             --bench 25
//!
//! The tool *does not* re-implement the Map's GPU hillshade — that's
//! verified separately in the wgpu snapshot example. This is the CPU
//! shaded preview the agent uses to decide whether the elevation field
//! coming out of the server is plausible without spinning up wgpu.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use image::{ImageEncoder, Rgba, RgbaImage};

const TILE_PX: u32 = 256;

#[derive(Debug, Clone)]
struct Args {
    server: String,
    /// Either a single tile (`z/x/y`) or a centre (`lat,lng`) + zoom + grid.
    mode: Mode,
    out: Option<PathBuf>,
    bench: Option<u32>,
}

#[derive(Debug, Clone)]
enum Mode {
    Single { z: u8, x: u32, y: u32 },
    Grid { z: u8, x0: u32, y0: u32, n: u32 },
}

fn parse_args() -> Args {
    let mut server = String::from("http://127.0.0.1:8086");
    let mut tile: Option<(u8, u32, u32)> = None;
    let mut center: Option<(f64, f64)> = None;
    let mut zoom: Option<u8> = None;
    let mut grid: Option<u32> = None;
    let mut out: Option<PathBuf> = None;
    let mut bench: Option<u32> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server" => server = args.next().expect("--server VALUE"),
            "--tile" => {
                let v = args.next().expect("--tile Z/X/Y");
                let parts: Vec<_> = v.split('/').collect();
                if parts.len() != 3 {
                    panic!("--tile must be Z/X/Y, got {v}");
                }
                tile = Some((
                    parts[0].parse().expect("z u8"),
                    parts[1].parse().expect("x u32"),
                    parts[2].parse().expect("y u32"),
                ));
            }
            "--center" => {
                let v = args.next().expect("--center LAT,LNG");
                let (lat, lng) = v.split_once(',').expect("LAT,LNG");
                center = Some((lat.parse().expect("lat f64"), lng.parse().expect("lng f64")));
            }
            "--zoom" => zoom = Some(args.next().expect("--zoom Z").parse().expect("z u8")),
            "--grid" => grid = Some(args.next().expect("--grid N").parse().expect("n u32")),
            "--out" => out = Some(args.next().expect("--out PATH").into()),
            "--bench" => bench = Some(args.next().expect("--bench N").parse().expect("n u32")),
            "-h" | "--help" => {
                eprintln!(
                    "usage: dem_probe --server URL (--tile Z/X/Y | --center LAT,LNG --zoom Z) \
                     [--grid N] [--out PATH] [--bench N]\n\nSee the module doc-comment in \
                     examples/dem_probe.rs for examples."
                );
                std::process::exit(0);
            }
            other => panic!("unknown arg: {other}"),
        }
    }

    let mode = if let Some((z, x, y)) = tile {
        let n = grid.unwrap_or(1);
        if n == 1 {
            Mode::Single { z, x, y }
        } else {
            Mode::Grid { z, x0: x, y0: y, n }
        }
    } else if let (Some((lat, lng)), Some(z)) = (center, zoom) {
        let (cx, cy) = lnglat_to_tile(lng, lat, z);
        let n = grid.unwrap_or(1).max(1);
        // Centre the NxN grid on the requested tile, biased down-left
        // so even sizes still cover the centre.
        let half = (n / 2) as i64;
        let x0 = (cx as i64 - half).clamp(0, (1i64 << z) - n as i64) as u32;
        let y0 = (cy as i64 - half).clamp(0, (1i64 << z) - n as i64) as u32;
        if n == 1 {
            Mode::Single { z, x: cx, y: cy }
        } else {
            Mode::Grid { z, x0, y0, n }
        }
    } else {
        panic!("need either --tile Z/X/Y or --center LAT,LNG --zoom Z");
    };

    Args {
        server,
        mode,
        out,
        bench,
    }
}

fn lnglat_to_tile(lng: f64, lat: f64, z: u8) -> (u32, u32) {
    let n = (1u64 << z) as f64;
    let x = ((lng + 180.0) / 360.0 * n).floor() as u32;
    let lat_rad = lat.to_radians();
    let y = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n)
        .floor() as u32;
    let max = (1u32 << z) - 1;
    (x.min(max), y.min(max))
}

/// Mapbox Terrain-RGB → metres. Alpha=0 ⇒ nodata.
fn decode(pixel: Rgba<u8>) -> Option<f32> {
    if pixel.0[3] == 0 {
        return None;
    }
    let r = pixel.0[0] as f32;
    let g = pixel.0[1] as f32;
    let b = pixel.0[2] as f32;
    Some(-10000.0 + (r * 256.0 * 256.0 + g * 256.0 + b) * 0.1)
}

#[derive(Debug, Clone)]
struct TileStats {
    min: f32,
    max: f32,
    mean: f32,
    stddev: f32,
    nodata_pct: f32,
    samples: u32,
}

fn stats(img: &RgbaImage) -> TileStats {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sum = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut n = 0u32;
    let mut nodata = 0u32;
    for p in img.pixels() {
        match decode(*p) {
            Some(h) => {
                min = min.min(h);
                max = max.max(h);
                sum += h as f64;
                sum_sq += (h as f64) * (h as f64);
                n += 1;
            }
            None => nodata += 1,
        }
    }
    if n == 0 {
        return TileStats {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            stddev: 0.0,
            nodata_pct: 100.0,
            samples: 0,
        };
    }
    let mean = (sum / n as f64) as f32;
    let var = (sum_sq / n as f64) - (mean as f64 * mean as f64);
    let stddev = var.max(0.0).sqrt() as f32;
    let total = (img.width() * img.height()) as f32;
    TileStats {
        min,
        max,
        mean,
        stddev,
        nodata_pct: 100.0 * nodata as f32 / total,
        samples: n,
    }
}

/// CPU analytic hillshade: 3-tap horizontal/vertical gradient, sun
/// 315° azimuth / 45° altitude (the classic carto default). Output is
/// 8-bit grey that mirrors what an agent would see when eyeballing a
/// terrain DEM in QGIS.
fn hillshade(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let mut out = RgbaImage::new(w, h);

    let sun_az = 315.0_f32.to_radians();
    let sun_alt = 45.0_f32.to_radians();
    let cos_alt = sun_alt.cos();
    let sin_alt = sun_alt.sin();
    let sun_x = cos_alt * sun_az.sin();
    let sun_y = cos_alt * sun_az.cos();
    let sun_z = sin_alt;

    // 10 m horizontal step at z=9 covers most DTM10 tiles within ~50 m;
    // this is a back-of-envelope value used only to scale the gradient
    // for the visual. Not pretending to be physically accurate.
    let dx_m = 10.0_f32;

    let sample = |x: i32, y: i32| -> f32 {
        let xc = x.clamp(0, w as i32 - 1) as u32;
        let yc = y.clamp(0, h as i32 - 1) as u32;
        decode(*img.get_pixel(xc, yc)).unwrap_or(0.0)
    };

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let zx = (sample(x + 1, y) - sample(x - 1, y)) / (2.0 * dx_m);
            let zy = (sample(x, y + 1) - sample(x, y - 1)) / (2.0 * dx_m);
            let nx = -zx;
            let ny = -zy;
            let nz = 1.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            let dot = (nx * sun_x + ny * sun_y + nz * sun_z) / len;
            let i = (dot.max(0.0) * 255.0) as u8;
            // Tint slightly warm so peaks are easy to spot against the
            // raw greyscale hillshade.
            out.put_pixel(x as u32, y as u32, Rgba([i, i, i.saturating_sub(8), 255]));
        }
    }
    out
}

fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);
    {
        let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut out));
        encoder
            .write_image(
                img.as_raw(),
                img.width(),
                img.height(),
                image::ExtendedColorType::Rgba8,
            )
            .expect("png encode");
    }
    out
}

fn fetch(
    client: &reqwest::blocking::Client,
    base: &str,
    z: u8,
    x: u32,
    y: u32,
) -> (RgbaImage, Duration, u16, usize) {
    let url = format!(
        "{}/v1/dem/rgb/{}/{}/{}.png",
        base.trim_end_matches('/'),
        z,
        x,
        y
    );
    let started = Instant::now();
    let resp = client.get(&url).send().expect("send");
    let status = resp.status().as_u16();
    let bytes = resp.bytes().expect("body");
    let took = started.elapsed();
    let img = image::load_from_memory(&bytes)
        .unwrap_or_else(|e| panic!("decode {url} ({status}): {e}"))
        .to_rgba8();
    (img, took, status, bytes.len())
}

fn save(out: &Path, name: &str, png: &[u8]) {
    let path = out.with_file_name(format!(
        "{}.{}.png",
        out.file_name().unwrap_or_default().to_string_lossy(),
        name
    ));
    std::fs::write(&path, png).expect("write");
    eprintln!("saved {}", path.display());
}

fn main() {
    let args = parse_args();
    let client = reqwest::blocking::Client::builder()
        .user_agent("dem-probe/0.1")
        .timeout(Duration::from_secs(30))
        .build()
        .expect("client");

    if let Some(iters) = args.bench {
        let (z, x, y) = match args.mode {
            Mode::Single { z, x, y } => (z, x, y),
            Mode::Grid { z, x0, y0, .. } => (z, x0, y0),
        };
        let mut times = Vec::with_capacity(iters as usize);
        for _ in 0..iters {
            let (_, took, status, bytes) = fetch(&client, &args.server, z, x, y);
            if status != 200 {
                eprintln!("bench: HTTP {status} (aborting)");
                std::process::exit(1);
            }
            times.push((took.as_secs_f64() * 1000.0, bytes));
        }
        times.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let n = times.len();
        let p50 = times[n / 2].0;
        let p95 = times[(n * 95) / 100].0;
        let max = times[n - 1].0;
        let mean: f64 = times.iter().map(|(t, _)| t).sum::<f64>() / n as f64;
        let bytes_mean: f64 = times.iter().map(|(_, b)| *b as f64).sum::<f64>() / n as f64;
        println!(
            "{{\"mode\":\"bench\",\"tile\":\"{z}/{x}/{y}\",\"n\":{n},\
              \"latency_ms\":{{\"mean\":{mean:.2},\"p50\":{p50:.2},\"p95\":{p95:.2},\"max\":{max:.2}}},\
              \"bytes_mean\":{bytes_mean:.0}}}"
        );
        return;
    }

    match args.mode {
        Mode::Single { z, x, y } => {
            let (img, took, status, bytes) = fetch(&client, &args.server, z, x, y);
            let s = stats(&img);
            println!(
                "{{\"mode\":\"single\",\"tile\":\"{z}/{x}/{y}\",\"status\":{status},\
                  \"latency_ms\":{:.2},\"bytes\":{bytes},\
                  \"elev_m\":{{\"min\":{:.1},\"max\":{:.1},\"mean\":{:.1},\"stddev\":{:.1}}},\
                  \"nodata_pct\":{:.1},\"samples\":{}}}",
                took.as_secs_f64() * 1000.0,
                s.min,
                s.max,
                s.mean,
                s.stddev,
                s.nodata_pct,
                s.samples,
            );
            if let Some(out) = &args.out {
                save(out, "raw", &encode_png(&img));
                save(out, "hillshade", &encode_png(&hillshade(&img)));
            }
        }
        Mode::Grid { z, x0, y0, n } => {
            let cw = TILE_PX * n;
            let ch = TILE_PX * n;
            let mut raw = RgbaImage::new(cw, ch);
            let mut hs = RgbaImage::new(cw, ch);
            for dy in 0..n {
                for dx in 0..n {
                    let x = x0 + dx;
                    let y = y0 + dy;
                    let (img, took, status, bytes) = fetch(&client, &args.server, z, x, y);
                    let s = stats(&img);
                    println!(
                        "{{\"mode\":\"grid\",\"tile\":\"{z}/{x}/{y}\",\"status\":{status},\
                          \"latency_ms\":{:.2},\"bytes\":{bytes},\
                          \"elev_m\":{{\"min\":{:.1},\"max\":{:.1},\"mean\":{:.1},\"stddev\":{:.1}}},\
                          \"nodata_pct\":{:.1}}}",
                        took.as_secs_f64() * 1000.0,
                        s.min,
                        s.max,
                        s.mean,
                        s.stddev,
                        s.nodata_pct,
                    );
                    let hs_tile = hillshade(&img);
                    let ox = dx * TILE_PX;
                    let oy = dy * TILE_PX;
                    for py in 0..TILE_PX {
                        for px in 0..TILE_PX {
                            raw.put_pixel(ox + px, oy + py, *img.get_pixel(px, py));
                            hs.put_pixel(ox + px, oy + py, *hs_tile.get_pixel(px, py));
                        }
                    }
                }
            }
            if let Some(out) = &args.out {
                save(out, "raw", &encode_png(&raw));
                save(out, "hillshade", &encode_png(&hs));
            }
        }
    }
}
