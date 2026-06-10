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
    Color, Filter, FilterValue, Layer, LatLng, Paint, Scene, SourceDef, SymbolPlacement,
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
    });
    scene
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
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TileKey {
    Raster(String, TileId),
    Vector(String, TileId),
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
                // No terrain in the basemap sessions.
                _ => continue,
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
            }
            self.in_flight.remove(&key);
        }

        // Render a real frame and measure it.
        let engine = &mut self.engine;
        let img = render_to_image(&self.gpu, self.width, self.height, |enc, view| {
            engine.render(enc, view)
        });
        self.engine.after_submit();

        let blank_frac = fraction_near(&img, CLEAR_SRGB, 6);
        let diff_frac = match &self.prev {
            Some(prev) => diff_fraction(prev, &img, 8),
            None => 1.0,
        };
        let cpu_ms = self.engine.last_frame_metrics().cpu_time.as_secs_f64() * 1000.0;
        self.stats.push(FrameStats {
            frame: self.frame,
            animating,
            zoom: self.engine.camera().zoom,
            delivered,
            in_flight: self.in_flight.len(),
            blank_frac,
            diff_frac,
            cpu_ms,
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
