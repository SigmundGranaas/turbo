//! The session driver — a headless stand-in for a device.
//!
//! What a phone gives a map renderer, and what replaces it here:
//!
//! | Device                       | Simulator                                |
//! | ---------------------------- | ---------------------------------------- |
//! | Surface + vsync loop         | Offscreen target, fixed `step()` cadence  |
//! | Touch gestures               | Scripted camera intents (`ease_to`, pans) |
//! | Network tile latency         | Frame-indexed delivery queue              |
//! | A user's eyes                | Per-frame pixel analysis + keyframe PNGs  |
//!
//! Each `step()` is one "vsync": advance the camera animation, deliver
//! tiles whose simulated latency elapsed, render a real frame through the
//! real engine, and measure it (blank coverage, frame-to-frame change,
//! CPU time). Tests assert on the *behaviour over the session* — the
//! things a user would notice — rather than on single stills.

use std::collections::HashMap;

use image::RgbaImage;
use turbomap_core::{MapOptions, PendingTile, TileId};
use turbomap_engine::{CameraState, HostDrivenResolver, MapEngine, TurbomapEngine};
use turbomap_golden::{render_to_image, Gpu};
use turbomap_scene::style::MatchCase;
use turbomap_scene::{
    Color, DemEncoding, Filter, FilterValue, Layer, LatLng, Paint, Scene, SourceDef,
    SymbolPlacement, TextAnchor,
};

use crate::world::world_tile;

/// The sRGB colour the renderer clears to before any tile covers a pixel.
/// A high fraction of these on screen is exactly what a user perceives as
/// "blank map while loading".
pub const CLEAR_SRGB: [u8; 3] = [172, 172, 168];

// A cohesive, muted cartographic palette (warm paper land, soft blue
// water, two greens for landuse, white roads cased in cool grey, amber
// arterials, dark-slate labels with a near-white halo).
pub const LAND_SRGB: [u8; 3] = [242, 239, 232];
pub const WATER_SRGB: [u8; 3] = [166, 204, 222];
pub const PARK_SRGB: [u8; 3] = [201, 224, 192];
pub const WOOD_SRGB: [u8; 3] = [180, 210, 172];
pub const ROAD_CASING_SRGB: [u8; 3] = [214, 216, 220];
pub const ROAD_INNER_SRGB: [u8; 3] = [255, 255, 255];
pub const ROAD_MAJOR_SRGB: [u8; 3] = [247, 220, 150];
pub const LABEL_SRGB: [u8; 3] = [70, 74, 84];
pub const HALO_SRGB: [u8; 3] = [248, 248, 250];

// What the authored colours look like ON SCREEN. Screen-space assertions
// compare against these, never the authored constants directly: the two
// coincide only while no post/grading pass runs. When the leftover HDR
// bloom + ACES tonemap from the reverted water feature was silently applied
// (June 24 → July 3), authored-vs-screen diverged and every blank-map gate
// was defanged (nothing on screen matched the authored clear, so
// `blank_frac` measured 0 forever and the gates could not fail). Keeping the
// seam makes that failure mode impossible to reintroduce silently.
//
// Baseline EMPIRICALLY with the inspection tool whenever the render pipeline
// intentionally regrades colour (same discipline as goldens):
//   cargo run -p turbomap-sim --example coldload_dump --release
pub const ONSCREEN_CLEAR_SRGB: [u8; 3] = CLEAR_SRGB;
pub const ONSCREEN_LAND_SRGB: [u8; 3] = LAND_SRGB;
pub const ONSCREEN_WATER_SRGB: [u8; 3] = WATER_SRGB;
pub const ONSCREEN_ROAD_INNER_SRGB: [u8; 3] = ROAD_INNER_SRGB;
pub const ONSCREEN_ROAD_MAJOR_SRGB: [u8; 3] = ROAD_MAJOR_SRGB;

/// A width-by-road-class paint: `cases` give (kind → px), `default` the
/// fallback px. Drives the data-driven width hierarchy.
fn road_width_by_class(cases: &[(&str, f32)], default: f32) -> Paint<f32> {
    Paint::Match {
        property: "kind".to_string(),
        cases: cases
            .iter()
            .map(|&(kind, px)| MatchCase {
                value: FilterValue::String(kind.to_string()),
                result: px,
            })
            .collect(),
        default: Box::new(default),
    }
}

/// A Google-Maps-shaped basemap scene over the synthetic world: land
/// raster, water fills, cased roads (inner colour data-driven by road
/// kind), and place labels.
pub fn basemap_scene() -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "land".to_string(),
        SourceDef::RasterXyz {
            tiles: vec!["sim://land/{z}/{x}/{y}".to_string()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 22,
            attribution: None,
        },
    );
    scene.sources.insert(
        "world".to_string(),
        SourceDef::VectorXyz {
            tiles: vec!["sim://world/{z}/{x}/{y}".to_string()],
            min_zoom: 0,
            max_zoom: 22,
        },
    );
    scene.layers.push(Layer::Raster {
        id: "land".to_string(),
        source: "land".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "water".to_string(),
        source: "world".to_string(),
        source_layer: Some("water".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(WATER_SRGB[0], WATER_SRGB[1], WATER_SRGB[2])),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "landuse".to_string(),
        source: "world".to_string(),
        source_layer: Some("landuse".to_string()),
        filter: Filter::Always,
        // Park vs wood, data-driven on the landuse class.
        color: Paint::Match {
            property: "kind".to_string(),
            cases: vec![MatchCase {
                value: FilterValue::String("wood".to_string()),
                result: Color::rgb(WOOD_SRGB[0], WOOD_SRGB[1], WOOD_SRGB[2]),
            }],
            default: Box::new(Color::rgb(PARK_SRGB[0], PARK_SRGB[1], PARK_SRGB[2])),
        },
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Line {
        id: "roads-casing".to_string(),
        source: "world".to_string(),
        source_layer: Some("roads".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(ROAD_CASING_SRGB[0], ROAD_CASING_SRGB[1], ROAD_CASING_SRGB[2])),
        width: road_width_by_class(&[("major", 12.0), ("minor", 8.0), ("local", 5.5)], 4.0),
        dash_array: None,
    });
    scene.layers.push(Layer::Line {
        id: "roads".to_string(),
        source: "world".to_string(),
        source_layer: Some("roads".to_string()),
        filter: Filter::Always,
        color: Paint::Match {
            property: "kind".to_string(),
            cases: vec![MatchCase {
                value: FilterValue::String("major".to_string()),
                result: Color::rgb(ROAD_MAJOR_SRGB[0], ROAD_MAJOR_SRGB[1], ROAD_MAJOR_SRGB[2]),
            }],
            default: Box::new(Color::rgb(
                ROAD_INNER_SRGB[0],
                ROAD_INNER_SRGB[1],
                ROAD_INNER_SRGB[2],
            )),
        },
        // Inner fill, narrower than the casing — the road hierarchy a real
        // basemap shows: arterials fat, side streets thin.
        width: road_width_by_class(&[("major", 8.5), ("minor", 4.5), ("local", 3.0)], 2.0),
        dash_array: None,
    });
    scene.layers.push(Layer::Symbol {
        id: "labels".to_string(),
        source: "world".to_string(),
        source_layer: Some("places".to_string()),
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(14.0),
        color: Paint::Const(Color::rgb(LABEL_SRGB[0], LABEL_SRGB[1], LABEL_SRGB[2])),
        // Dark label ink with a white halo — the readability outline real
        // basemaps use so labels stay legible over roads and water.
        halo_color: Paint::Const(Color::rgb(HALO_SRGB[0], HALO_SRGB[1], HALO_SRGB[2])),
        halo_width: Paint::Const(1.5),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
        icon_color: Paint::Const(Color::rgb(70, 78, 92)),
        text_anchor: TextAnchor::Center,
        letter_spacing: 0.0,
        font_weight: 0.0,
    });
    scene
}

/// Per-tile halo (px) the synthetic DEM bakes in: each tile is `256 + 2·halo`,
/// the outer ring overscanning the neighbours so adjacent terrain-mesh edges
/// agree. Matches what `decode_height_grid` trims.
const DEM_HALO: u32 = 1;

/// [`basemap_scene`] plus a synthetic DEM registered as a HEIGHT-ONLY terrain
/// source — so the sim can drive the full 3D path (vertex displacement,
/// sun-lighting, and CAST SHADOWS) deterministically and offline. This is the
/// device-equivalent the flat sessions can't reach: it exercises the exact
/// `Map::render` / `update_terrain_shadows` code the phone runs, so a
/// render-thread cost regression (the kind that froze sun mode) shows up here
/// as a slow frame in a CI test instead of an on-device ANR.
pub fn basemap_scene_3d() -> Scene {
    let mut scene = basemap_scene();
    scene.sources.insert(
        "dem".to_string(),
        SourceDef::DemXyz {
            tiles: vec!["sim://dem/{z}/{x}/{y}".to_string()],
            encoding: DemEncoding::MapboxRgb,
            min_zoom: 0,
            max_zoom: 22,
            halo: DEM_HALO,
        },
    );
    // Height-only: the DEM just displaces the ground and the basemap raster
    // lights itself from the sun (one lit 3D surface) — the path cast shadows
    // ride on. No relief-shading overlay.
    scene.layers.push(Layer::Hillshade {
        id: "terrain".to_string(),
        source: "dem".to_string(),
        exaggeration: 1.5,
        height_only: true,
    });
    scene
}

/// A synthetic Mapbox-Terrain-RGB DEM tile (`256 + 2·DEM_HALO` per side) of
/// procedural ridged hills. Elevation is a continuous function of WORLD
/// position, so adjacent tiles agree at their shared edge (no mesh cracks) and
/// the relief is steep enough to cast real terrain shadows for the test.
fn dem_tile_png(tile: TileId) -> Vec<u8> {
    use image::ImageEncoder;
    let n = 256 + 2 * DEM_HALO;
    let scale = 1.0 / (1u64 << tile.z) as f64; // tile world size
    let ox = tile.x as f64 * scale;
    let oy = tile.y as f64 * scale;
    // ~one ridge every ~1.3 tiles (cycles per world unit at this zoom) — dense
    // enough that any footprint-sized shadow grid contains ridges + valleys, so
    // the cast-shadow test isn't fragile to where the camera centre lands.
    let f = std::f64::consts::TAU * (1u64 << tile.z) as f64 / 1.3;
    let mut img = RgbaImage::new(n, n);
    for py in 0..n {
        for px in 0..n {
            // Interior pixel → world; the halo ring samples the neighbours'
            // world coords (continuous function → seamless edges).
            let u = (px as f64 - DEM_HALO as f64) / 256.0;
            let v = (py as f64 - DEM_HALO as f64) / 256.0;
            let wx = ox + u * scale;
            let wy = oy + v * scale;
            let elev = 700.0
                + 520.0 * (wx * f).sin() * (wy * f).cos()
                + 240.0 * ((wx + wy) * f * 0.5).sin();
            let elev = elev.clamp(0.0, 8000.0);
            // Mapbox Terrain-RGB: h = -10000 + V·0.1  ⇒  V = (h + 10000)·10.
            let val = ((elev + 10000.0) * 10.0) as u32;
            let r = ((val >> 16) & 0xff) as u8;
            let g = ((val >> 8) & 0xff) as u8;
            let b = (val & 0xff) as u8;
            img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
        }
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut out))
        .write_image(img.as_raw(), n, n, image::ExtendedColorType::Rgba8)
        .expect("dem png encode");
    out
}

/// Mean perceptual luminance (Rec. 709) over the last rendered frame, `[0,255]`.
/// Used to prove cast shadows darken the terrain.
pub fn mean_luma(img: &RgbaImage) -> f64 {
    let mut sum = 0.0f64;
    for p in img.pixels() {
        sum += 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
    }
    sum / (img.width() * img.height()) as f64
}

/// Per-frame measurements — the simulator's instrument panel.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FrameStats {
    pub frame: u64,
    pub animating: bool,
    pub zoom: f64,
    /// Tiles delivered (latency elapsed) this frame.
    pub delivered: u32,
    /// Tiles still in flight after this frame.
    pub in_flight: usize,
    /// Fraction of pixels showing the clear colour — "blank map".
    pub blank_frac: f64,
    /// Fraction of pixels that changed vs the previous frame.
    pub diff_frac: f64,
    /// Render CPU time reported by the engine, milliseconds.
    pub cpu_ms: f64,
    /// Engine tile-lifecycle counts this frame (summed across layers +
    /// terrain): the want-list size, and residents no longer wanted (the
    /// eviction candidates). Thrash shows up as `retained` churning while
    /// `desired` is stable — countable now instead of inferred (slice A1).
    pub desired: usize,
    pub retained: usize,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TileKey {
    Raster(String, TileId),
    Vector(String, TileId),
    Terrain(TileId),
}

pub struct Sim {
    gpu: Gpu,
    pub engine: TurbomapEngine,
    width: u32,
    height: u32,
    /// Simulated network latency, in frames, for every tile fetch.
    pub latency_frames: u64,
    frame: u64,
    in_flight: HashMap<TileKey, u64>,
    land_png: Vec<u8>,
    prev: Option<RgbaImage>,
    pub last: Option<RgbaImage>,
    pub stats: Vec<FrameStats>,
}

impl Sim {
    /// Build a session over the synthetic world. Returns `None` when no
    /// wgpu adapter exists (callers skip, like every other GPU test).
    pub fn new(width: u32, height: u32, camera: CameraState, options: MapOptions) -> Option<Self> {
        let gpu = turbomap_golden::headless()?;
        let engine = TurbomapEngine::new(
            gpu.device.clone(),
            gpu.queue.clone(),
            turbomap_golden::TARGET_FORMAT,
            (width, height),
            camera,
            options,
            Box::new(HostDrivenResolver),
        )
        .ok()?;
        Some(Self {
            gpu,
            engine,
            width,
            height,
            latency_frames: 0,
            frame: 0,
            in_flight: HashMap::new(),
            land_png: solid_png(LAND_SRGB),
            prev: None,
            last: None,
            stats: Vec::new(),
        })
    }

    pub fn camera(&self) -> CameraState {
        self.engine.camera()
    }

    /// Enable/disable terrain cast shadows (3D sessions, `basemap_scene_3d`).
    /// 0 = off. Passthrough to the engine.
    pub fn set_terrain_shadows(&mut self, strength: f32) {
        self.engine.set_terrain_shadows(strength);
    }

    /// Pin the sun to an explicit azimuth/altitude (degrees). A LOW altitude is
    /// what makes terrain self-occlude, so a shadow test sets e.g. (90, 16).
    pub fn set_sun(&mut self, azimuth_deg: f32, altitude_deg: f32) {
        self.engine.set_sun_position(azimuth_deg, altitude_deg);
    }

    /// A camera centred on the synthetic city. Lat/lng are arbitrary —
    /// the world is global — but fixed so sessions are comparable.
    pub fn start_camera(zoom: f64) -> CameraState {
        CameraState::new(LatLng::new(60.39, 5.32), zoom)
    }

    /// A camera dead on a *major* crossroads of the synthetic world, so
    /// the amber arterial grid is guaranteed in view at any city zoom.
    pub fn camera_at_major_crossroads(zoom: f64) -> CameraState {
        // Major lattice spacing is 2^-9; (264, 147) is the crossroads
        // nearest the default start camera.
        let s = 2f64.powi(-9);
        let center = turbomap_scene::geo::inverse_mercator(264.0 * s, 147.0 * s);
        CameraState::new(center, zoom)
    }

    /// One simulated vsync: animate, deliver due tiles, render, measure.
    pub fn step(&mut self) -> &FrameStats {
        self.frame += 1;
        let animating = self.engine.tick_now();

        // Schedule newly-requested tiles with the configured latency.
        for pending in self.engine.pending_tiles() {
            let key = match pending {
                PendingTile::Raster { layer_id, tile } => TileKey::Raster(layer_id, tile),
                PendingTile::Vector { layer_id, tile } => TileKey::Vector(layer_id, tile),
                // DEM tiles for the 3D sessions (basemap_scene_3d); absent in
                // the flat basemap sessions, where the terrain scene is empty.
                PendingTile::Terrain { tile } => TileKey::Terrain(tile),
                // Height-only terrain draws no relief-shading overlay, so the
                // hillshade tile stream is unused here.
                PendingTile::Hillshade { .. } => continue,
            };
            self.in_flight
                .entry(key)
                .or_insert(self.frame + self.latency_frames);
        }

        // Deliver everything whose latency elapsed.
        let due: Vec<TileKey> = self
            .in_flight
            .iter()
            .filter(|(_, &due)| due <= self.frame)
            .map(|(k, _)| k.clone())
            .collect();
        let delivered = due.len() as u32;
        for key in due {
            match &key {
                TileKey::Raster(layer, tile) => {
                    self.engine.ingest_raster_encoded(layer, *tile, &self.land_png);
                }
                TileKey::Vector(layer, tile) => {
                    let bytes = world_tile(tile.z, tile.x, tile.y);
                    self.engine.ingest_mvt(layer, *tile, &bytes);
                }
                TileKey::Terrain(tile) => {
                    self.engine.ingest_terrain_encoded(*tile, &dem_tile_png(*tile));
                }
            }
            self.in_flight.remove(&key);
        }

        // Render a real frame and measure it.
        let engine = &mut self.engine;
        let img = render_to_image(&self.gpu, self.width, self.height, |enc, view| {
            engine.render(enc, view)
        });
        self.engine.after_submit();

        let blank_frac = fraction_near(&img, ONSCREEN_CLEAR_SRGB, 6);
        let diff_frac = match &self.prev {
            Some(prev) => diff_fraction(prev, &img, 8),
            None => 1.0,
        };
        // Slice-B3.1 dual-write gate, swept across every behavioural test:
        // the lifecycle table and the legacy scene bookkeeping must agree on
        // EVERY frame of every session, or the table may not become the
        // source of truth. (pending_tiles ran during scheduling above, so
        // the table is synced to this frame's camera.)
        self.engine
            .lifecycle_agreement()
            .unwrap_or_else(|e| panic!("frame {}: {e}", self.frame));

        let m = self.engine.last_frame_metrics();
        let cpu_ms = m.cpu_time.as_secs_f64() * 1000.0;
        let (desired, retained) = (m.tiles.desired, m.tiles.retained);
        self.stats.push(FrameStats {
            frame: self.frame,
            animating,
            zoom: self.engine.camera().zoom,
            delivered,
            in_flight: self.in_flight.len(),
            blank_frac,
            diff_frac,
            cpu_ms,
            desired,
            retained,
        });
        self.prev = self.last.replace(img);
        self.stats.last().expect("just pushed")
    }

    /// Step until the picture stabilises: no tiles in flight and the
    /// frame-to-frame diff below `diff_threshold` for 3 consecutive
    /// frames. Returns the frames it took, or `None` if `max_frames`
    /// elapsed first.
    pub fn run_until_stable(&mut self, max_frames: u64, diff_threshold: f64) -> Option<u64> {
        let mut calm = 0;
        for n in 1..=max_frames {
            let stats = self.step();
            if stats.in_flight == 0 && !stats.animating && stats.diff_frac <= diff_threshold {
                calm += 1;
                if calm >= 3 {
                    return Some(n);
                }
            } else {
                calm = 0;
            }
        }
        None
    }
}

/// Fraction of pixels within `tol` per channel of `rgb`.
pub fn fraction_near(img: &RgbaImage, rgb: [u8; 3], tol: u8) -> f64 {
    let total = (img.width() * img.height()) as f64;
    let hits = img
        .pixels()
        .filter(|p| {
            (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol)
        })
        .count();
    hits as f64 / total
}

/// Fraction of pixels differing by more than `tol` on any channel.
pub fn diff_fraction(a: &RgbaImage, b: &RgbaImage, tol: u8) -> f64 {
    debug_assert_eq!(a.dimensions(), b.dimensions());
    let total = (a.width() * a.height()) as f64;
    let diff = a
        .pixels()
        .zip(b.pixels())
        .filter(|(pa, pb)| (0..3).any(|i| pa.0[i].abs_diff(pb.0[i]) > tol))
        .count();
    diff as f64 / total
}

fn solid_png(rgb: [u8; 3]) -> Vec<u8> {
    use image::ImageEncoder;
    let mut img = RgbaImage::new(256, 256);
    for px in img.pixels_mut() {
        *px = image::Rgba([rgb[0], rgb[1], rgb[2], 255]);
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut out))
        .write_image(img.as_raw(), 256, 256, image::ExtendedColorType::Rgba8)
        .expect("png encode");
    out
}
