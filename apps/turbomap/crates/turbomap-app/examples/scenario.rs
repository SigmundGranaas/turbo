//! Headless 3D-terrain crash/inspection harness — runs the REAL engine
//! against REAL Bodø/Sjunkhatten tiles + DEM over HTTP, drives a scripted
//! camera session (tilt down hard, orbit, pan, over-zoom) that mirrors the
//! on-device interactions, and after every step:
//!   * wraps the work in `catch_unwind` so a Rust panic is reported with
//!     the step + camera state (run with `RUST_BACKTRACE=1` for the trace),
//!   * finite-checks the camera matrix (NaN here → the GPU gets NaN → driver
//!     hang on device), and
//!   * dumps the frame to `/tmp/turbomap-scenario/NNN.png` so it can be
//!     eyeballed without a device.
//!
//! This is the local stand-in for shipping to a phone: it hits the same
//! `Map` code paths the Android FFI does.
//!
//! Example:
//!   RUST_BACKTRACE=1 cargo run -p turbomap-app --example scenario -- \
//!     --center 67.23,15.30 --zoom 13 --pitch 80
//!
//! `--dem-url`/`--basemap-url` default to the prod tileserver + Kartverket.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use image::{ImageEncoder, RgbaImage};
use turbomap_core::{
    Camera, Color, DemEncoding, Feature, Filter, GeomType, Geometry, HillshadeStyle, IconSpec,
    LatLng, Map, MapOptions, Marker, MarkerId, Paint, PendingTile, RadarFrame, RasterFormat,
    RasterTile, Rule, SunPosition, TileError, TileId, TileSource, VectorStyle, VectorTile,
    VectorTileLayer, VectorTileSource, VectorValue,
};

/// A coarse synthetic radar grid (precip + coverage) so the cloud overlay
/// runs end-to-end — including the camera-ray pitch-parallax path that only
/// engages when tilted + geo-registered (a known steep-pitch hazard).
fn synthetic_radar(w: u32, h: u32, phase: f32) -> RadarFrame {
    let mut precip = vec![0u8; (w * h) as usize];
    let mut coverage = vec![0u8; (w * h) as usize];
    let front = -0.2 + phase * 1.3;
    for y in 0..h {
        for x in 0..w {
            let nx = (x as f32 + 0.5) / w as f32;
            let ny = (y as f32 + 0.5) / h as f32;
            let band = (-((nx + (ny - 0.5) * 0.3 - front).powi(2)) / (2.0 * 0.10 * 0.10)).exp();
            let i = (y * w + x) as usize;
            coverage[i] = (band.clamp(0.0, 1.0) * 230.0) as u8;
            precip[i] = ((band * band).clamp(0.0, 1.0) * 200.0) as u8;
        }
    }
    RadarFrame::from_u8(w, h, &precip, &coverage)
}

/// In-memory vector source that exercises EVERY vector render path the engine
/// supports, deterministically and offline, so the diagnostic suite can
/// observe each one draped on real 3D terrain:
///   - `streets`   LineStrings  → `Paint::Line` + along-line road labels
///   - `water`     Polygon      → `Paint::Fill`
///   - `buildings` Polygons     → `Paint::FillExtrusion` (3D prisms, height-driven)
///   - `pois`      Points       → `Paint::Text` + `IconSpec` (POI label + sprite)
///
/// The same tile content repeats at every zoom (the geometry is tile-local),
/// which is all the renderer needs to light up each pipeline.
struct SyntheticVectors;

fn prop(pairs: &[(&str, VectorValue)]) -> HashMap<String, VectorValue> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

impl VectorTileSource for SyntheticVectors {
    fn request(&self, _tile: TileId) -> Result<VectorTile, TileError> {
        let e = 4096i32;
        let streets = VectorTileLayer {
            name: "streets".into(),
            version: 2,
            extent: e as u32,
            features: vec![Feature {
                id: 1,
                geom_type: GeomType::LineString,
                geometry: Geometry::LineString(vec![
                    vec![(e / 16, e / 16), (e * 15 / 16, e * 15 / 16)],
                    vec![(e / 16, e * 15 / 16), (e * 15 / 16, e / 16)],
                ]),
                properties: prop(&[("name", VectorValue::String("Diagonalveien".into()))]),
            }],
        };
        let rect = |x0: i32, y0: i32, x1: i32, y1: i32| {
            Geometry::Polygon(vec![vec![
                (x0, y0),
                (x1, y0),
                (x1, y1),
                (x0, y1),
                (x0, y0),
            ]])
        };
        let water = VectorTileLayer {
            name: "water".into(),
            version: 2,
            extent: e as u32,
            features: vec![Feature {
                id: 10,
                geom_type: GeomType::Polygon,
                geometry: rect(e / 20, e * 11 / 20, e * 9 / 20, e * 19 / 20),
                properties: HashMap::new(),
            }],
        };
        // Two building footprints with different heights → distinct prisms
        // under a tilted camera (and the height-driven walls/roof shading).
        let buildings = VectorTileLayer {
            name: "buildings".into(),
            version: 2,
            extent: e as u32,
            features: vec![
                Feature {
                    id: 20,
                    geom_type: GeomType::Polygon,
                    geometry: rect(e * 11 / 20, e * 2 / 20, e * 16 / 20, e * 7 / 20),
                    properties: prop(&[("render_height", VectorValue::Int(120))]),
                },
                Feature {
                    id: 21,
                    geom_type: GeomType::Polygon,
                    geometry: rect(e * 12 / 20, e * 9 / 20, e * 15 / 20, e * 12 / 20),
                    properties: prop(&[("render_height", VectorValue::Int(40))]),
                },
            ],
        };
        let pois = VectorTileLayer {
            name: "pois".into(),
            version: 2,
            extent: e as u32,
            features: vec![
                Feature {
                    id: 30,
                    geom_type: GeomType::Point,
                    geometry: Geometry::Point(vec![(e * 5 / 16, e * 6 / 16)]),
                    properties: prop(&[
                        ("name", VectorValue::String("Stortind".into())),
                        ("rank", VectorValue::Int(3)),
                    ]),
                },
                Feature {
                    id: 31,
                    geom_type: GeomType::Point,
                    geometry: Geometry::Point(vec![(e * 11 / 16, e * 10 / 16)]),
                    properties: prop(&[
                        ("name", VectorValue::String("Havna".into())),
                        ("rank", VectorValue::Int(2)),
                    ]),
                },
            ],
        };
        Ok(VectorTile {
            layers: vec![streets, water, buildings, pois],
        })
    }
    fn min_zoom(&self) -> u8 {
        0
    }
    fn max_zoom(&self) -> u8 {
        18
    }
}

/// Minimal vector style — one line rule + one fill — just enough to push
/// real MVT geometry through the vector pipeline so its terrain-draping
/// vertex shader (the DEM sample) actually executes with terrain present.
fn demo_style() -> VectorStyle {
    let rule = |layer: &str, paint: Paint, interactive: bool| Rule {
        source_layer: layer.into(),
        filter: Filter::Always,
        paint,
        min_zoom: 0,
        max_zoom: 22,
        interactive,
    };
    VectorStyle {
        background: Color::rgba(0, 0, 0, 0),
        rules: vec![
            // Fill: a lake polygon draped on the terrain.
            rule("water", Paint::Fill { color: Color::rgb(0x9E, 0xC2, 0xDF) }, false),
            // Line: the road network (interactive → exercises the hit-test index).
            rule(
                "streets",
                Paint::Line { color: Color::rgb(0xBD, 0xB3, 0xA1), width: 14.0 },
                true,
            ),
            // 3D extrusion: buildings as height-driven prisms (only shows tilted).
            rule(
                "buildings",
                Paint::FillExtrusion {
                    color: Color::rgb(0xC9, 0xB8, 0x9A),
                    height_m: 30.0,
                    height_property: Some("render_height".into()),
                    min_height_property: None,
                },
                false,
            ),
            // Along-line label: the road name follows the street centreline.
            rule(
                "streets",
                Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 22.0,
                    color: Color::rgb(0x33, 0x2A, 0x1E),
                    halo_color: Color::rgb(255, 255, 255),
                    halo_width: 2.0,
                    rank_field: None,
                    along_line: true,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 0.0,
                },
                false,
            ),
            // Point label + icon: POI markers with a sprite and a name.
            rule(
                "pois",
                Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 24.0,
                    color: Color::rgb(0x1A, 0x1A, 0x1A),
                    halo_color: Color::rgb(255, 255, 255),
                    halo_width: 2.5,
                    rank_field: Some("rank".into()),
                    along_line: false,
                    icon: Some(IconSpec {
                        sprite: "marker".into(),
                        size_px: 32.0,
                        color: Color::rgb(0xD8, 0x3A, 0x2A),
                    }),
                    left_anchor: true,
                    letter_spacing: 0.0,
                    weight: 0.0,
                },
                false,
            ),
        ],
    }
}

const WIDTH: u32 = 900;
const HEIGHT: u32 = 1600; // tall, like a phone — exercises the steep-pitch footprint

/// A blocking, in-process-cached HTTP tile source. One type serves both the
/// basemap raster and the DEM (just different templates / halo). Caching
/// keeps the multi-step scenario from re-fetching the same tile each time the
/// camera revisits an area.
struct HttpTiles {
    /// URL template with `{z}`/`{x}`/`{y}` placeholders (order varies by server).
    template: String,
    min_zoom: u8,
    max_zoom: u8,
    halo_px: u32,
    client: reqwest::blocking::Client,
    cache: Mutex<HashMap<TileId, Option<Vec<u8>>>>,
}

impl HttpTiles {
    fn new(template: String, min_zoom: u8, max_zoom: u8, halo_px: u32) -> Self {
        Self {
            template,
            min_zoom,
            max_zoom,
            halo_px,
            client: reqwest::blocking::Client::builder()
                .user_agent("turbomap-scenario/0.1")
                .timeout(Duration::from_secs(20))
                .build()
                .expect("http client"),
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl TileSource for HttpTiles {
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        if let Some(hit) = self.cache.lock().unwrap().get(&tile) {
            return match hit {
                Some(bytes) => Ok(RasterTile {
                    bytes: bytes.clone(),
                    format: RasterFormat::Png,
                }),
                None => Err(TileError::Network("cached miss".into())),
            };
        }
        let url = self
            .template
            .replace("{z}", &tile.z.to_string())
            .replace("{x}", &tile.x.to_string())
            .replace("{y}", &tile.y.to_string());
        let fetched: Result<Vec<u8>, TileError> = (|| {
            let resp = self
                .client
                .get(&url)
                .send()
                .map_err(|e| TileError::Network(e.to_string()))?;
            if !resp.status().is_success() {
                return Err(TileError::Network(format!("HTTP {} {url}", resp.status())));
            }
            Ok(resp
                .bytes()
                .map_err(|e| TileError::Network(e.to_string()))?
                .to_vec())
        })();
        let mut cache = self.cache.lock().unwrap();
        match fetched {
            Ok(bytes) => {
                cache.insert(tile, Some(bytes.clone()));
                Ok(RasterTile {
                    bytes,
                    format: RasterFormat::Png,
                })
            }
            Err(e) => {
                // Cache the miss so a doomed tile (e.g. above coverage) isn't
                // re-hammered every step; the harness tolerates misses.
                cache.insert(tile, None);
                Err(e)
            }
        }
    }

    fn min_zoom(&self) -> u8 {
        self.min_zoom
    }
    fn max_zoom(&self) -> u8 {
        self.max_zoom
    }
    fn dem_halo_px(&self) -> u32 {
        self.halo_px
    }
}

fn encode_png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);
    image::codecs::png::PngEncoder::new(Cursor::new(&mut out))
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("png encode");
    out
}

struct Cli {
    center: LatLng,
    zoom: f64,
    max_pitch: f64,
    bearing: f64,
    basemap_url: String,
    dem_url: String,
    out_dir: String,
}

fn parse_cli() -> Cli {
    // Sjunkhatten (near Bodø): steep coastal fjord terrain at ~67°N.
    let mut center = LatLng { lat: 67.23, lng: 15.30 };
    let mut zoom = 13.0;
    let mut max_pitch = 80.0;
    let mut bearing = 20.0;
    let api = std::env::var("TURBO_API_URL")
        .unwrap_or_else(|_| "https://kart-api.sandring.no".into());
    let mut basemap_url =
        "https://cache.kartverket.no/v1/wmts/1.0.0/topo/default/webmercator/{z}/{y}/{x}.png"
            .to_string();
    let mut dem_url = format!("{}/v1/dem/rgb/{{z}}/{{x}}/{{y}}.png?halo=1", api.trim_end_matches('/'));
    let mut out_dir = "/tmp/turbomap-scenario".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--center" => {
                let v = args.next().expect("--center LAT,LNG");
                let (lat, lng) = v.split_once(',').expect("LAT,LNG");
                center = LatLng {
                    lat: lat.parse().expect("lat"),
                    lng: lng.parse().expect("lng"),
                };
            }
            "--zoom" => zoom = args.next().expect("--zoom").parse().expect("z"),
            "--pitch" => max_pitch = args.next().expect("--pitch").parse().expect("deg"),
            "--bearing" => bearing = args.next().expect("--bearing").parse().expect("deg"),
            "--basemap-url" => basemap_url = args.next().expect("--basemap-url"),
            "--dem-url" => dem_url = args.next().expect("--dem-url"),
            "--out-dir" => out_dir = args.next().expect("--out-dir"),
            other => panic!("unknown arg {other}"),
        }
    }
    Cli { center, zoom, max_pitch, bearing, basemap_url, dem_url, out_dir }
}

/// One scripted camera pose + a human label for crash reports / frame names.
struct Step {
    label: String,
    camera: Camera,
    /// Optional sun override applied before this frame (the time-of-day sweep);
    /// `None` keeps whatever the previous step left set.
    sun: Option<SunPosition>,
}

/// Build the session: warm up flat, ramp the pitch down hard, orbit a full
/// turn at max tilt, pan across the terrain, then over-zoom in and back.
/// This is the interaction sequence that crashes on device.
fn build_steps(cli: &Cli) -> Vec<Step> {
    let mut steps = Vec::new();
    let cam = |pitch: f64, bearing: f64, center: LatLng, zoom: f64| {
        Camera::new(center, zoom).with_pitch(pitch).with_bearing(bearing)
    };
    let mut push = |label: String, camera: Camera| steps.push(Step { label, camera, sun: None });
    push("warmup-flat".into(), cam(0.0, 0.0, cli.center, cli.zoom));
    // Sweep pitch across EVERY allowed level in 5° increments (0 → max). The
    // crash hunt + the profiling pass both care about behaviour at every tilt,
    // not just the endpoints: this is where the steep-pitch footprint grows
    // (more visible tiles → more prepare cost) and where any near-horizon NaN
    // would surface. `max_pitch` is the camera's hard cap (default 80°).
    let mut p = 5.0;
    while p <= cli.max_pitch + 1e-6 {
        let pitch = p.min(cli.max_pitch);
        push(format!("pitch-{pitch:.0}"), cam(pitch, cli.bearing, cli.center, cli.zoom));
        p += 5.0;
    }
    // Orbit a full turn at max pitch (the 3D-mode 1-finger orbit).
    for i in 1..=12 {
        let b = cli.bearing + 360.0 * i as f64 / 12.0;
        push(format!("orbit-{:.0}", b % 360.0), cam(cli.max_pitch, b, cli.center, cli.zoom));
    }
    // Pan north across fjord + peak at max pitch.
    for i in 1..=6 {
        let c = LatLng { lat: cli.center.lat + 0.02 * i as f64, lng: cli.center.lng + 0.01 * i as f64 };
        push(format!("pan-{i}"), cam(cli.max_pitch, cli.bearing, c, cli.zoom));
    }
    // Over-zoom in past native max and back out (upsample path), staying tilted.
    for i in 1..=5 {
        let z = cli.zoom + i as f64;
        push(format!("zoomin-{z:.0}"), cam(cli.max_pitch, cli.bearing, cli.center, z));
    }
    for i in (0..=5).rev() {
        let z = cli.zoom + i as f64 - 3.0;
        push(format!("zoomout-{z:.0}"), cam(cli.max_pitch.min(60.0), cli.bearing, cli.center, z.max(5.0)));
    }

    // Time-of-day sweep at a scenic tilt: vary the sun from dawn → noon → dusk
    // so the diagnostic observes the terrain shading, hillshade self-shadows
    // and the analytic sky/atmosphere palette respond to sun azimuth+altitude.
    let tod_cam = cam(55.0, cli.bearing, cli.center, cli.zoom);
    for (label, azimuth_deg, altitude_deg) in [
        ("tod-dawn", 80.0_f32, 6.0_f32),
        ("tod-morning", 120.0, 25.0),
        ("tod-noon", 180.0, 55.0),
        ("tod-evening", 250.0, 18.0),
        ("tod-dusk", 292.0, 4.0),
    ] {
        steps.push(Step {
            label: label.into(),
            camera: tod_cam,
            sun: Some(SunPosition { azimuth_deg, altitude_deg }),
        });
    }
    steps
}

/// Synchronously drain the engine's pending tiles via blocking HTTP. Tolerates
/// misses (above-coverage / sea tiles) — those just stay unloaded, exactly as
/// on device. Bounded so a runaway pending list surfaces rather than spins.
fn drain_tiles(
    map: &mut Map,
    basemap: &Arc<dyn TileSource>,
    dem: &Arc<dyn TileSource>,
    vector: &Arc<dyn VectorTileSource>,
) {
    for _round in 0..40 {
        let pending = map.pending_tiles();
        if pending.is_empty() {
            return;
        }
        let mut progressed = false;
        for req in pending {
            match req {
                PendingTile::Raster { layer_id, tile } => {
                    if let Ok(raw) = basemap.request(tile) {
                        if let Ok(img) = image::load_from_memory(&raw.bytes) {
                            let img = img.to_rgba8();
                            let (w, h) = img.dimensions();
                            map.ingest_raster(&layer_id, tile, img.as_raw(), w, h);
                            progressed = true;
                        }
                    }
                }
                PendingTile::Terrain { tile } => {
                    if let Ok(raw) = dem.request(tile) {
                        if let Ok(img) = image::load_from_memory(&raw.bytes) {
                            let img = img.to_rgba8();
                            let (w, h) = img.dimensions();
                            map.ingest_terrain_tile(tile, img.as_raw(), w, h);
                            progressed = true;
                        }
                    }
                }
                PendingTile::Vector { layer_id, tile } => {
                    if let Ok(vt) = vector.request(tile) {
                        map.ingest_vector_tile(&layer_id, tile, &vt);
                        progressed = true;
                    }
                }
                PendingTile::Hillshade { .. } => {}
            }
        }
        if !progressed {
            // Everything left is a tolerated miss (sea / above coverage).
            return;
        }
    }
}

/// Render the current Map state and read the framebuffer back into an image.
/// Single encoder: render + copy_texture_to_buffer, then poll the device until
/// the readback maps. The captured image lets a caller both dump a PNG and
/// measure the frame (e.g. mean luma for the cast-shadow A/B proof).
fn render_capture(
    map: &mut Map,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::Texture,
    target_view: &wgpu::TextureView,
) -> RgbaImage {
    let mut encoder = device.create_command_encoder(&Default::default());
    map.render(&mut encoder, target_view);
    let bpp = 4u32;
    let unpadded = WIDTH * bpp;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scenario-readback"),
        size: (padded * HEIGHT) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    map.after_submit();

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    let started = std::time::Instant::now();
    loop {
        let _ = device.poll(wgpu::PollType::Poll);
        if let Ok(Ok(())) = rx.recv_timeout(Duration::from_millis(10)) {
            break;
        }
        if started.elapsed() > Duration::from_secs(8) {
            panic!("readback map timed out");
        }
    }
    let data = slice.get_mapped_range();
    let mut tight = Vec::with_capacity((unpadded * HEIGHT) as usize);
    for row in 0..HEIGHT {
        let s = (row * padded) as usize;
        tight.extend_from_slice(&data[s..s + unpadded as usize]);
    }
    RgbaImage::from_raw(WIDTH, HEIGHT, tight).expect("rgba")
}

/// Render + read back + write a PNG. Thin wrapper over [`render_capture`].
#[allow(clippy::too_many_arguments)]
fn render_to_png(
    map: &mut Map,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::Texture,
    target_view: &wgpu::TextureView,
    path: &str,
) {
    let img = render_capture(map, device, queue, target, target_view);
    std::fs::write(path, encode_png(&img)).expect("write png");
}

/// Compare two frames for the cast-shadow proof: the count of pixels that
/// darkened (the shadow footprint) and the mean luma drop over just those
/// pixels. A whole-frame mean is swamped by the bright sky (most of a tilted
/// frame), so we measure the shadowed region directly.
fn darkening_stats(off: &RgbaImage, on: &RgbaImage) -> (usize, f64, f64) {
    let luma = |p: &[u8]| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
    let mut changed = 0usize;
    let mut drop_sum = 0.0f64;
    let mut max_drop = 0.0f64;
    for (a, b) in off.pixels().zip(on.pixels()) {
        let d = luma(&a.0) - luma(&b.0);
        if d > 0.5 {
            changed += 1;
            drop_sum += d;
            if d > max_drop {
                max_drop = d;
            }
        }
    }
    let mean = if changed > 0 { drop_sum / changed as f64 } else { 0.0 };
    (changed, mean, max_drop)
}

fn matrix_is_finite(m: &[[f32; 4]; 4]) -> bool {
    m.iter().all(|row| row.iter().all(|v| v.is_finite()))
}

fn main() {
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Warn)
        .init();
    let cli = parse_cli();
    std::fs::create_dir_all(&cli.out_dir).expect("mkdir out-dir");

    let instance = wgpu::Instance::new({
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        d.backends = wgpu::Backends::PRIMARY;
        d
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("scenario-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    .expect("device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    let target_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scenario-target"),
        size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
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
        Camera::new(cli.center, cli.zoom),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
    )
    .expect("map");

    let basemap: Arc<dyn TileSource> =
        Arc::new(HttpTiles::new(cli.basemap_url.clone(), 0, 18, 0));
    let dem: Arc<dyn TileSource> =
        Arc::new(HttpTiles::new(cli.dem_url.clone(), 6, 14, 1));
    // Public OSM vector tiles (no auth) + a minimal line/fill style, so the
    // vector pipeline runs WITH terrain — exercising the draping vertex
    // shader's DEM sample (the path that never executes in the sim, since the
    // sim has no terrain → zscale 0 → the sample is branched out).
    let vector: Arc<dyn VectorTileSource> = Arc::new(SyntheticVectors);
    map.add_raster_layer("basemap", basemap.clone());
    map.set_terrain_source(dem.clone(), turbomap_core::TerrainOptions::default());
    // Terrain CAST shadows on for the whole run: a peak occludes the valley
    // behind it (distinct from the always-on Lambertian self-shading). This
    // exercises the CPU horizon-march + the shadow texture upload + the
    // raster shader's group(3) sample across the pitch sweep, time-of-day
    // sweep and animated nav. The dedicated A/B proof below quantifies it.
    map.set_terrain_shadows(0.85);
    // Hillshade over the basemap: sun-relit relief shadows from the shared DEM
    // — exercises the hillshade pipeline + the terrain self-shadowing the sun
    // drives. Drawn between basemap and vectors so roads/labels sit on top.
    map.add_hillshade_layer(
        "hillshade",
        HillshadeStyle {
            encoding: DemEncoding::MapboxRgb,
            sun_azimuth_deg: 315.0,
            sun_altitude_deg: 45.0,
            exaggeration: 1.4,
            shadow_color: Color::rgb(0x2A, 0x33, 0x40),
            highlight_color: Color::rgb(0xFF, 0xFB, 0xF0),
            opacity: 0.55,
        },
    );
    map.add_vector_layer("osm", vector.clone(), demo_style());
    // Fixed sun so frames are deterministic (no wall-clock dependence). The
    // time-of-day sweep below overrides this per-step.
    map.set_sun_position(Some(SunPosition { azimuth_deg: 145.0, altitude_deg: 30.0 }));

    // Markers around the camera — exercises the marker pipeline + the
    // elevation-aware projection (hit_test → lng_lat_to_screen → world_to_
    // screen_z) on the 3D terrain, a path the raster+DEM-only run skipped.
    for (i, (dlat, dlng)) in [(0.0, 0.0), (0.03, 0.02), (-0.02, 0.04), (0.05, -0.03)]
        .iter()
        .enumerate()
    {
        let mut data = std::collections::HashMap::new();
        data.insert("name".to_owned(), format!("m{i}"));
        map.add_marker(Marker {
            id: MarkerId(0),
            lng_lat: LatLng::new(cli.center.lat + dlat, cli.center.lng + dlng),
            radius_px: 10.0,
            color: Color::rgb(0xE5, 0x39, 0x35),
            data,
        });
    }
    // The styled "you are here" location puck — a larger, distinct blue dot at
    // the camera centre, the app's current-location marker.
    let mut puck = std::collections::HashMap::new();
    puck.insert("kind".to_owned(), "location".to_owned());
    map.add_marker(Marker {
        id: MarkerId(0),
        lng_lat: cli.center,
        radius_px: 16.0,
        color: Color::rgb(0x1E, 0x88, 0xE5),
        data: puck,
    });

    // Cloud overlay, geo-registered so the camera-ray parallax engages under
    // tilt (otherwise the steep-pitch cloud path is never tested).
    const GW: u32 = 64;
    const GH: u32 = 48;
    map.enable_clouds(GW, GH);
    map.ingest_radar_frame(0, &synthetic_radar(GW, GH, 0.40));
    map.ingest_radar_frame(1, &synthetic_radar(GW, GH, 0.55));
    map.set_cloud_time(7.0, 0.5);
    map.set_cloud_geo_bounds(
        cli.center.lng - 1.2,
        cli.center.lat - 0.6,
        cli.center.lng + 1.2,
        cli.center.lat + 0.6,
    );

    let steps = build_steps(&cli);
    eprintln!(
        "scenario: {} steps @ {:.4},{:.4} z{} → pitch {}°, frames to {}",
        steps.len(), cli.center.lat, cli.center.lng, cli.zoom, cli.max_pitch, cli.out_dir,
    );

    // Debug fast path: skip the scripted sweep + nav and run only the
    // cast-shadow proof (warm tiles + 2 renders). ~30 s instead of minutes.
    let shadow_only = std::env::var("TURBO_SHADOW_ONLY").is_ok();
    let mut crashed = false;
    // Per-step profiling: we snapshot the engine's always-on FrameMetrics
    // after every render so we can table the load + timing at each pitch /
    // orbit / zoom level and flag any slow frame. (label, pitch, metrics)
    let mut profiles: Vec<(String, f64, turbomap_core::FrameMetrics)> = Vec::new();
    eprintln!(
        "  {:>3} {:<14} {:>5} | {:>7} {:>7} {:>7} {:>7} | {:>4} {:>4} {:>6}",
        "#", "step", "pitch", "cpu ms", "prep", "pass", "cloud", "lyr", "dc", "tiles",
    );
    for (i, step) in steps.iter().enumerate() {
        if shadow_only {
            break;
        }
        let path = format!("{}/{:03}-{}.png", cli.out_dir, i, step.label);
        let cam = step.camera;
        // Catch a Rust panic per step so we know exactly which interaction
        // broke (and the backtrace prints above this line).
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            map.set_camera(cam);
            if let Some(sun) = step.sun {
                map.set_sun_position(Some(sun));
            }
            // The clamp may have lowered the pitch; report what actually rendered.
            let eff = map.camera();
            let mat = eff.view_projection_matrix((WIDTH, HEIGHT));
            if !eff.pitch_deg.is_finite() || !eff.zoom.is_finite() || !matrix_is_finite(&mat) {
                panic!(
                    "NON-FINITE camera at step {i} ({}): pitch={} zoom={} matrix_finite={}",
                    step.label, eff.pitch_deg, eff.zoom, matrix_is_finite(&mat)
                );
            }
            drain_tiles(&mut map, &basemap, &dem, &vector);
            // Hit-test a few screen points — drives marker reprojection through
            // the elevation-aware path on the 3D surface.
            for p in [(WIDTH as f64 * 0.5, HEIGHT as f64 * 0.5), (120.0, 200.0), (700.0, 1300.0)] {
                let _ = map.hit_test(p, 16.0);
            }
            render_to_png(&mut map, &device, &queue, &target, &target_view, &path);
        }));
        match result {
            Ok(()) => {
                let m = map.last_frame_metrics().clone();
                let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
                let gpu = m
                    .gpu_time
                    .map(|g| format!(" gpu={:.2}ms", ms(g)))
                    .unwrap_or_default();
                eprintln!(
                    "  {i:>3} {:<14} {:>5.0} | {:>7.2} {:>7.2} {:>7.2} {:>7.2} | {:>4} {:>4} {:>6}{}",
                    step.label,
                    map.camera().pitch_deg,
                    ms(m.cpu_time),
                    ms(m.phases.prepare),
                    ms(m.phases.pass),
                    ms(m.phases.clouds),
                    m.visible_layers,
                    m.draw_calls,
                    m.tiles_drawn,
                    gpu,
                );
                profiles.push((step.label.clone(), map.camera().pitch_deg, m));
            }
            Err(_) => {
                eprintln!(
                    "  [{i:>2}] {:<14} *** CRASHED *** requested pitch={:.0} bearing={:.0} zoom={:.1} center={:.4},{:.4}",
                    step.label, cam.pitch_deg, cam.bearing_deg, cam.zoom, cam.center.lat, cam.center.lng,
                );
                crashed = true;
                break;
            }
        }
    }

    // ---- Terrain cast-shadow proof ----------------------------------------
    // Prove cast shadows are real occlusion, not just a tint: render the SAME
    // tilted view under a low raking sun with shadows OFF then ON and require
    // the terrain to measurably darken (occluded valleys lose the direct sun
    // term; ambient skylight still reaches them, so it darkens, not blackens).
    // Both frames are dumped for an eyeball diff of the long shadows.
    if !crashed {
        // Look north onto the land mass (the default centre sits on the fjord;
        // a tilted view frames the mountainous terrain ahead). Pitch 55 reads
        // the 3D relief without the extreme-tilt horizon haze.
        let shadow_cam = Camera::new(cli.center, cli.zoom)
            .with_pitch(55.0)
            .with_bearing(cli.bearing);
        map.set_camera(shadow_cam);
        // A low-mid sun (18°) toward the east casts shadows westward across the
        // relief while keeping sun-facing slopes bright enough that occluded
        // ones visibly darken.
        map.set_sun_position(Some(SunPosition { azimuth_deg: 95.0, altitude_deg: 18.0 }));
        drain_tiles(&mut map, &basemap, &dem, &vector);

        map.set_terrain_shadows(0.0);
        let off = render_capture(&mut map, &device, &queue, &target, &target_view);
        std::fs::write(format!("{}/shadow-off.png", cli.out_dir), encode_png(&off))
            .expect("write png");

        map.set_terrain_shadows(0.85);
        let on = render_capture(&mut map, &device, &queue, &target, &target_view);
        std::fs::write(format!("{}/shadow-on.png", cli.out_dir), encode_png(&on))
            .expect("write png");

        let (changed, mean_drop, max_drop) = darkening_stats(&off, &on);
        let pct = 100.0 * changed as f64 / (WIDTH * HEIGHT) as f64;
        eprintln!("  ── terrain cast-shadow proof (sun alt 18°, pitch 45°) ──");
        eprintln!(
            "  shadow footprint: {changed} px ({pct:.2}% of frame) darkened, mean drop {mean_drop:.1}, max drop {max_drop:.1} luma"
        );
        // Real cast shadows occlude a real, localised patch of terrain. Require a
        // non-trivial footprint AND a genuinely dark core: a wiring no-op leaves
        // 0 changed pixels, and a uniform-tint bug would darken broadly but never
        // produce a deep core. (The mean over the soft penumbra edge is modest by
        // nature, so we gate on the footprint + the core depth, not the mean.)
        if changed < 1_500 || max_drop < 12.0 {
            eprintln!(
                "SCENARIO FAILED — cast shadows too weak ({changed} px, max drop {max_drop:.1}); expected ≥1500 px and ≥12.0 luma core."
            );
            std::process::exit(1);
        }
        // Restore a normal sun; shadows stay ON to exercise the recompute path
        // as the camera moves through the nav phase below.
        map.set_sun_position(Some(SunPosition { azimuth_deg: 145.0, altitude_deg: 30.0 }));
    }

    // ---- Animated-navigation perf -----------------------------------------
    // The scripted steps above teleport the camera (set_camera). This phase
    // exercises the ANIMATION path the device actually uses while navigating:
    // an eased fly-to, then an inertial pan fling, advanced by `tick()` every
    // frame (each tick re-syncs scenes + redraws). It profiles the per-frame
    // cost while the camera is genuinely in motion — the "tilt + navigate"
    // workload — at ~30 fps wall-clock so the time-based physics steps.
    if !crashed && !shadow_only {
        let home = Camera::new(cli.center, cli.zoom)
            .with_pitch(35.0)
            .with_bearing(cli.bearing);
        let fly_target = Camera::new(
            LatLng::new(cli.center.lat + 0.03, cli.center.lng + 0.02),
            cli.zoom + 2.0,
        )
        .with_pitch(70.0)
        .with_bearing(cli.bearing + 60.0);
        // Pre-warm BOTH endpoints' tiles (blocking network) before timing, so
        // the loop measures the per-frame animation cost — tick → scene re-sync
        // → redraw — not tile fetches. (Without this, draining the fly-to
        // destination's cold tiles dominates and starves the loop.)
        for c in [fly_target, home] {
            map.set_camera(c);
            drain_tiles(&mut map, &basemap, &dem, &vector);
        }
        map.ease_to(fly_target, Duration::from_millis(1200));
        eprintln!("  ── animated navigation (ease-to + fling, tick-driven, warm tiles) ──");
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        // Fixed frame budget (~2 s at 30 fps): the ease (1.2 s) then a pan
        // fling carry the camera the whole time, so every frame is in motion.
        const NAV_FRAMES: u32 = 60;
        for nav_idx in 0..NAV_FRAMES {
            map.tick(std::time::Instant::now());
            // Mid-flight, hand off to an inertial pan fling (gesture momentum).
            if nav_idx == 35 {
                map.fling((-650.0, 280.0));
            }
            // Tiles are warm → this is cheap (cache hits); keeps the visible
            // set current as the camera moves without dominating the timing.
            drain_tiles(&mut map, &basemap, &dem, &vector);
            let path = format!("{}/nav-{nav_idx:03}.png", cli.out_dir);
            render_to_png(&mut map, &device, &queue, &target, &target_view, &path);
            let m = map.last_frame_metrics().clone();
            profiles.push((format!("nav-{nav_idx}"), map.camera().pitch_deg, m.clone()));
            if nav_idx.is_multiple_of(12) {
                eprintln!(
                    "  nav {nav_idx:>3}        {:>5.0} | {:>7.2} {:>7.2} {:>7.2} {:>7.2} | {:>4} {:>4} {:>6}",
                    map.camera().pitch_deg,
                    ms(m.cpu_time),
                    ms(m.phases.prepare),
                    ms(m.phases.pass),
                    ms(m.phases.clouds),
                    m.visible_layers,
                    m.draw_calls,
                    m.tiles_drawn,
                );
            }
            std::thread::sleep(Duration::from_millis(33));
        }
        eprintln!("  animated navigation: {NAV_FRAMES} frames");
    }

    print_profile_summary(&profiles);

    if crashed {
        eprintln!("SCENARIO FAILED — reproduced a crash locally (see backtrace above).");
        std::process::exit(1);
    }
    eprintln!("SCENARIO OK — {} frames in {} (no panic, no non-finite camera).", steps.len(), cli.out_dir);
}

/// Roll up the per-step FrameMetrics into a load/performance summary: overall
/// CPU min/avg/max, the slowest frame, and — because the user is hunting
/// steep-pitch behaviour — the CPU cost at the flat (0°) vs. max-tilt frames,
/// which is where the visible-tile footprint (and thus prepare cost) blows up.
fn print_profile_summary(profiles: &[(String, f64, turbomap_core::FrameMetrics)]) {
    if profiles.is_empty() {
        return;
    }
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    let cpu: Vec<f64> = profiles.iter().map(|(_, _, m)| ms(m.cpu_time)).collect();
    let n = cpu.len() as f64;
    let sum: f64 = cpu.iter().sum();
    let min = cpu.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = cpu.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let (slow_label, slow_pitch, slow_m) = profiles
        .iter()
        .max_by(|a, b| ms(a.2.cpu_time).partial_cmp(&ms(b.2.cpu_time)).unwrap())
        .unwrap();
    // Flat vs. steepest pitch frames, for the tilt-cost correlation.
    let flattest = profiles.iter().min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let steepest = profiles.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    eprintln!("\n  ── profile summary ({} frames) ──", profiles.len());
    eprintln!(
        "  cpu/frame: min {:.2}ms  avg {:.2}ms  max {:.2}ms",
        min,
        sum / n,
        max
    );
    eprintln!(
        "  slowest: '{}' @ pitch {:.0}° — cpu {:.2}ms (prep {:.2} / pass {:.2} / cloud {:.2}), {} tiles, {} draws",
        slow_label,
        slow_pitch,
        ms(slow_m.cpu_time),
        ms(slow_m.phases.prepare),
        ms(slow_m.phases.pass),
        ms(slow_m.phases.clouds),
        slow_m.tiles_drawn,
        slow_m.draw_calls,
    );
    if let (Some((_, fp, fm)), Some((_, sp, sm))) = (flattest, steepest) {
        eprintln!(
            "  tilt cost: pitch {:.0}° → {:.2}ms ({} tiles)   vs   pitch {:.0}° → {:.2}ms ({} tiles)",
            fp,
            ms(fm.cpu_time),
            fm.tiles_drawn,
            sp,
            ms(sm.cpu_time),
            sm.tiles_drawn,
        );
    }
}
