//! Offscreen visual smoke test for the turbomap layer stack.
//!
//! Headless wgpu, no winit, no network — feeds the Map a synthetic
//! basemap (checkerboard PNGs) plus a synthetic Mapbox-Terrain-RGB DEM
//! (a Gaussian bump near Bergen) and writes the final composite to
//! `/tmp/turbomap-snapshot.png` so we can eyeball that the hillshade
//! pipeline + raster basemap actually paint pixels to the target.

use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use image::{ImageEncoder, RgbaImage};
use turbomap_clouds::DebugView;
use turbomap_core::{
    Camera, CloudParams, HillshadeStyle, LatLng, Map, MapOptions, PendingTile, RadarFrame,
    RasterFormat, RasterTile, TileError, TileId, TileSource,
};

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 768;
const TILE_PX: u32 = 256;
const DEFAULT_OUT: &str = "/tmp/turbomap-snapshot.png";

/// Live DEM tile source backed by an HTTP server (typically our own
/// `/v1/dem/rgb/{z}/{x}/{y}.png`). Lets the snapshot exercise the real
/// GPU hillshade pipeline against real Norwegian elevation data instead
/// of the synthetic Gaussian fallback.
struct HttpDemSource {
    base: String,
    client: reqwest::blocking::Client,
}

impl HttpDemSource {
    fn new(base: String) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            client: reqwest::blocking::Client::builder()
                .user_agent("turbomap-snapshot/0.1")
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("client"),
        }
    }
}

impl TileSource for HttpDemSource {
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        // `?halo=1` gives us 258×258 PNGs so the GPU gradient kernel
        // can step into the neighbour terrain without ClampToEdge
        // seams. Paired with `dem_halo_px(&self) -> 1` below.
        let url = format!(
            "{}/v1/dem/rgb/{}/{}/{}.png?halo=1",
            self.base, tile.z, tile.x, tile.y
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| TileError::Network(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(TileError::Network(format!(
                "HTTP {}: {}",
                resp.status(),
                url
            )));
        }
        let bytes = resp
            .bytes()
            .map_err(|e| TileError::Network(e.to_string()))?
            .to_vec();
        Ok(RasterTile {
            bytes,
            format: RasterFormat::Png,
        })
    }

    fn min_zoom(&self) -> u8 {
        6
    }
    fn max_zoom(&self) -> u8 {
        14
    }
    fn dem_halo_px(&self) -> u32 {
        1
    }
}

/// Neutral parchment basemap — uniform sepia, no tile-id stamps. The
/// snapshot is meant to surface flaws in the *hillshade* pipeline, so
/// the basemap deliberately contributes nothing that could be
/// mis-attributed to it. An earlier version drew coloured 2 px tile
/// borders for debugging, but those bled through translucent hillshade
/// and looked like rendering artifacts in agent-shared screenshots.
struct ParchmentBasemap;

impl TileSource for ParchmentBasemap {
    fn request(&self, _tile: TileId) -> Result<RasterTile, TileError> {
        let mut img = RgbaImage::new(TILE_PX, TILE_PX);
        for px in img.pixels_mut() {
            *px = image::Rgba([226, 218, 198, 255]);
        }
        Ok(RasterTile {
            bytes: encode_png(&img),
            format: RasterFormat::Png,
        })
    }

    fn min_zoom(&self) -> u8 {
        0
    }
    fn max_zoom(&self) -> u8 {
        20
    }
}

/// Synthetic Mapbox-Terrain-RGB DEM. A Gaussian peak (~1500 m) centred
/// on Bergen gives the hillshade pipeline a real gradient to shade.
struct GaussianTerrainSource {
    peak_lng: f64,
    peak_lat: f64,
    peak_height_m: f64,
    sigma_deg: f64,
}

impl GaussianTerrainSource {
    fn bergen() -> Self {
        Self {
            peak_lng: 5.32,
            peak_lat: 60.39,
            peak_height_m: 1500.0,
            sigma_deg: 0.6,
        }
    }
}

impl TileSource for GaussianTerrainSource {
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        let n = (1u64 << tile.z) as f64;
        let mut img = RgbaImage::new(TILE_PX, TILE_PX);
        for py in 0..TILE_PX {
            for px in 0..TILE_PX {
                let fx = tile.x as f64 + (px as f64 + 0.5) / TILE_PX as f64;
                let fy = tile.y as f64 + (py as f64 + 0.5) / TILE_PX as f64;
                let lng = fx / n * 360.0 - 180.0;
                let lat = (std::f64::consts::PI * (1.0 - 2.0 * fy / n))
                    .sinh()
                    .atan()
                    .to_degrees();
                let dx = (lng - self.peak_lng) / self.sigma_deg;
                let dy = (lat - self.peak_lat) / self.sigma_deg;
                let h = self.peak_height_m * (-(dx * dx + dy * dy) * 0.5).exp();
                let scaled = ((h + 10000.0) * 10.0).round().clamp(0.0, 16_777_215.0) as u32;
                let r = ((scaled >> 16) & 0xFF) as u8;
                let g = ((scaled >> 8) & 0xFF) as u8;
                let b = (scaled & 0xFF) as u8;
                img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
            }
        }
        Ok(RasterTile {
            bytes: encode_png(&img),
            format: RasterFormat::Png,
        })
    }

    fn min_zoom(&self) -> u8 {
        0
    }
    fn max_zoom(&self) -> u8 {
        20
    }
}

/// A coarse synthetic radar grid (precip + coverage), like the blocky
/// raster MET serves. A drifting frontal band carries the rain; broad
/// cloud surrounds it. `phase` (0..1) slides the band so two frames differ.
fn synthetic_radar(w: u32, h: u32, phase: f32) -> RadarFrame {
    let mut precip = vec![0u8; (w * h) as usize];
    let mut coverage = vec![0u8; (w * h) as usize];
    let front = -0.2 + phase * 1.3;
    for y in 0..h {
        for x in 0..w {
            let nx = (x as f32 + 0.5) / w as f32;
            let ny = (y as f32 + 0.5) / h as f32;
            let band = (-((nx + (ny - 0.5) * 0.3 - front).powi(2)) / (2.0 * 0.10 * 0.10)).exp();
            let mass = (-((nx - 0.7).powi(2) + (ny - 0.35).powi(2)) / (2.0 * 0.16 * 0.16)).exp();
            let cov = (band * 0.85 + mass * 0.8).clamp(0.0, 1.0);
            let pr = (band * band * 0.7).clamp(0.0, 1.0);
            let i = (y * w + x) as usize;
            coverage[i] = (cov * 255.0) as u8;
            precip[i] = (pr * 255.0) as u8;
        }
    }
    RadarFrame::from_u8(w, h, &precip, &coverage)
}

fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 * 1024);
    {
        let encoder = image::codecs::png::PngEncoder::new(Cursor::new(&mut out));
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

/// CLI: snapshot [--dem-url URL] [--center LAT,LNG] [--zoom Z] [--out PATH]
///                [--pitch DEG] [--bearing DEG]
struct CliArgs {
    dem_url: Option<String>,
    center: LatLng,
    zoom: f64,
    pitch: f64,
    bearing: f64,
    out: String,
}

fn parse_cli() -> CliArgs {
    let mut dem_url: Option<String> = None;
    let mut center = LatLng {
        lng: 5.32,
        lat: 60.39,
    };
    let mut zoom = 9.0_f64;
    let mut pitch = 0.0_f64;
    let mut bearing = 0.0_f64;
    let mut out = DEFAULT_OUT.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dem-url" => dem_url = Some(args.next().expect("--dem-url URL")),
            "--center" => {
                let v = args.next().expect("--center LAT,LNG");
                let (lat, lng) = v.split_once(',').expect("LAT,LNG");
                center = LatLng {
                    lng: lng.parse().expect("lng"),
                    lat: lat.parse().expect("lat"),
                };
            }
            "--zoom" => zoom = args.next().expect("--zoom Z").parse().expect("z"),
            "--pitch" => pitch = args.next().expect("--pitch DEG").parse().expect("deg"),
            "--bearing" => bearing = args.next().expect("--bearing DEG").parse().expect("deg"),
            "--out" => out = args.next().expect("--out PATH"),
            other => panic!("unknown arg: {other}"),
        }
    }
    CliArgs {
        dem_url,
        center,
        zoom,
        pitch,
        bearing,
        out,
    }
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn)
        .init();
    let cli = parse_cli();

    // Headless wgpu init. PRIMARY backends + LowPower adapter so we get
    // an integrated GPU when available (matches the demo app).
    let instance = wgpu::Instance::new({
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::PRIMARY;
        desc
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no adapter");
    // The snapshot deliberately does NOT request TIMESTAMP_QUERY: in
    // a one-shot tool we render once and copy_texture_to_buffer in
    // the same encoder. With timestamps enabled Map::render inserts a
    // `resolve_query_set` + buffer-to-buffer copy after its passes,
    // and on the Metal backend that combination ends up serialising
    // weirdly with the trailing texture copy — the texture comes out
    // blank. The long-lived demo app (where each frame is its own
    // encoder + submit pair) doesn't hit this. GPU-side timing belongs
    // there. CPU time + cache stats are still useful here.
    let features = wgpu::Features::empty();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("snapshot-device"),
        required_features: features,
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    .expect("device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    // The Map's pipelines blend against the surface format the caller
    // gives them — Rgba8UnormSrgb mirrors the colour-correct path the
    // live demo runs through.
    let target_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("snapshot-target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());

    let mut map = Map::new(
        device.clone(),
        queue.clone(),
        target_format,
        (WIDTH, HEIGHT),
        Camera::new(cli.center, cli.zoom)
            .with_pitch(cli.pitch)
            .with_bearing(cli.bearing),
        MapOptions {
            // Disable fade-in so the first frame is the final composite —
            // otherwise the snapshot would catch the alpha ramp mid-way.
            fade_in_secs: 0.0,
            ..Default::default()
        },
    )
    .expect("map");

    let basemap: Arc<dyn TileSource> = Arc::new(ParchmentBasemap);
    let dem: Arc<dyn TileSource> = match &cli.dem_url {
        Some(url) => Arc::new(HttpDemSource::new(url.clone())),
        None => Arc::new(GaussianTerrainSource::bergen()),
    };
    let clouds_mode = std::env::var("CLOUDS").is_ok();
    map.add_raster_layer("basemap", basemap.clone());
    // The cloud verification only needs the basemap underneath; skip
    // terrain/hillshade so its tile loop (terrain requests are ignored
    // here) doesn't dominate the run.
    if !clouds_mode {
        map.set_terrain_source(dem.clone(), turbomap_core::TerrainOptions::default());
        map.add_hillshade_layer("hillshade", HillshadeStyle::default());
    }

    // `CLOUDS=1` exercises the weather-cloud overlay end-to-end through the
    // real Map render path: enable it, push two synthetic radar frames, and
    // park the time slider mid-crossfade.
    if clouds_mode {
        const GW: u32 = 64;
        const GH: u32 = 42;
        map.enable_clouds(GW, GH);
        map.ingest_radar_frame(0, &synthetic_radar(GW, GH, 0.40));
        map.ingest_radar_frame(1, &synthetic_radar(GW, GH, 0.55));
        map.set_cloud_time(7.0, 0.5);
        // Geo-register the radar so the world-locked field-uv affine engages
        // and, crucially, so `use_camera_ray` (camera-ray pitch parallax)
        // turns on when the camera is tilted. Without a geo box the overlay
        // falls back to the flat screen-locked path and `--pitch` would test
        // nothing about 3D parallax. Box sized to comfortably fill the view.
        let (clat, clng) = (cli.center.lat, cli.center.lng);
        // CLOUD_BOX_HALF = box half-height in degrees latitude (lng half = 2×).
        // Sweep it to find the box size that keeps puffs screen-sized at a given
        // zoom (the fix for the zoomed-in white wash).
        let half = std::env::var("CLOUD_BOX_HALF")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(2.0);
        map.set_cloud_geo_bounds(
            clng - half * 2.0,
            clat - half,
            clng + half * 2.0,
            clat + half,
        );
        // Render any internal pipeline stage instead of the final composite,
        // for headless diagnosis at a given pitch:
        //   CLOUD_DEBUG_VIEW=parallax CLOUDS=1 cargo run ... --pitch 25
        if let Ok(v) = std::env::var("CLOUD_DEBUG_VIEW") {
            let view = match v.as_str() {
                "final" => DebugView::Final,
                "precip" => DebugView::RadarPrecip,
                "coverage" => DebugView::RadarCoverage,
                "field" => DebugView::CloudField,
                "density" => DebugView::Density,
                "light" => DebugView::Light,
                "alpha" => DebugView::Alpha,
                "albedo" => DebugView::Albedo,
                "parallax" => DebugView::Parallax,
                other => panic!("unknown CLOUD_DEBUG_VIEW {other:?}"),
            };
            let params = CloudParams {
                debug_view: view,
                ..CloudParams::default()
            };
            map.set_cloud_params(params);
        }
    }

    // Drive the host loop synchronously: keep pulling pending tiles
    // until the Map stops asking for more, then render. Both sources
    // are in-process so each request is microseconds.
    let mut iterations = 0;
    loop {
        let pending = map.pending_tiles();
        if pending.is_empty() {
            break;
        }
        for req in pending {
            match req {
                PendingTile::Raster { layer_id, tile } => {
                    let raw = basemap.request(tile).expect("basemap request");
                    let img = image::load_from_memory(&raw.bytes)
                        .expect("basemap decode")
                        .to_rgba8();
                    let (w, h) = img.dimensions();
                    map.ingest_raster(&layer_id, tile, img.as_raw(), w, h);
                }
                PendingTile::Hillshade { layer_id, tile } => {
                    let raw = dem.request(tile).expect("dem request");
                    let img = image::load_from_memory(&raw.bytes)
                        .expect("dem decode")
                        .to_rgba8();
                    let (w, h) = img.dimensions();
                    map.ingest_hillshade(&layer_id, tile, img.as_raw(), w, h);
                }
                PendingTile::Vector { .. } => {
                    // No vector layer in the snapshot; ignore.
                }
                PendingTile::Terrain { .. } => {
                    // Snapshot doesn't register a separate terrain
                    // source — hillshade owns the DEM. Ignore.
                }
            }
        }
        iterations += 1;
        if iterations > 12 {
            // Safety: the synthetic sources never fail so this loop must
            // converge quickly. If it doesn't, the scene state has a bug
            // worth surfacing rather than spinning forever.
            panic!("pending_tiles failed to drain after {iterations} rounds");
        }
    }

    // Real frame: render + copy_texture_to_buffer in a single encoder
    // so the readback captures *this* frame's pixels.
    let mut encoder = device.create_command_encoder(&Default::default());
    map.render(&mut encoder, &target_view);

    let bytes_per_pixel = 4u32;
    let unpadded_bpr = WIDTH * bytes_per_pixel;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bpr = unpadded_bpr.div_ceil(align) * align;
    let buffer_size = (padded_bpr * HEIGHT) as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snapshot-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    map.after_submit();

    // Metrics dump *after* the real frame so gpu_time reflects the
    // warm-up frame (one-frame-delayed readback).
    let m = map.last_frame_metrics();
    let gpu = m
        .gpu_time
        .map(|d| format!("{:.2}ms", d.as_secs_f64() * 1000.0))
        .unwrap_or_else(|| "n/a".into());
    eprintln!(
        "frame: cpu_time={:.2}ms gpu_time={} layers={} markers={}",
        m.cpu_time.as_secs_f64() * 1000.0,
        gpu,
        m.layer_count,
        m.marker_count
    );
    for lm in &m.layers {
        let c = &lm.cache;
        eprintln!(
            "  layer {:>10} ({:?}): entries={} bytes={} hits={} misses={} inserts={} evict={}",
            lm.id, lm.kind, c.entries, c.bytes_used, c.hits, c.misses, c.inserts, c.evictions,
        );
    }

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    // Spin the device until the mapping callback fires. Headless mode
    // doesn't have a vsync to drive it, so we have to ask Poll.
    let started = std::time::Instant::now();
    loop {
        let _ = device.poll(wgpu::PollType::Poll);
        if let Ok(Ok(())) = rx.recv_timeout(Duration::from_millis(10)) {
            break;
        }
        if started.elapsed() > Duration::from_secs(5) {
            panic!("readback map timed out");
        }
    }
    let data = slice.get_mapped_range();

    // Strip the row padding back out and save as PNG.
    let mut tight: Vec<u8> = Vec::with_capacity((unpadded_bpr * HEIGHT) as usize);
    for row in 0..HEIGHT {
        let start = (row * padded_bpr) as usize;
        let end = start + unpadded_bpr as usize;
        tight.extend_from_slice(&data[start..end]);
    }
    // Channels arrive BGRA on some adapters but Rgba8UnormSrgb is well-
    // defined as R-G-B-A in memory; no swizzle needed here.
    let img = RgbaImage::from_raw(WIDTH, HEIGHT, tight).expect("snapshot rgba");
    let bytes = encode_png(&img);
    std::fs::write(&cli.out, &bytes).expect("write snapshot");
    println!("wrote {} ({} bytes)", cli.out, bytes.len());
}
