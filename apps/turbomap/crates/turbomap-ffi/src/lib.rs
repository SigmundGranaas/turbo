//! uniffi bindings for the turbomap engine — the control plane that
//! Kotlin/Swift hosts talk to.
//!
//! Scope (by design, per the map-engine architecture):
//! - The **control plane** crosses uniffi: scene (as the Scene-IR JSON),
//!   camera, projection, hit-testing, and pull/push tile IO.
//! - The **GPU plane does not**: you cannot pass an `ANativeWindow` /
//!   `CAMetalLayer` through uniffi. Surface creation + the vsync render
//!   loop are small pieces of hand-written per-platform glue that wrap
//!   the same [`TurboMap`] object.
//! - Tile IO is **host-driven**: `pendingTiles()` lists what the engine
//!   needs; the host fetches (it owns auth/caching/offline) and pushes
//!   encoded bytes back via the `ingest*` methods. Inline GeoJSON needs
//!   no IO and is drained in-process by `pumpLocalTiles()`.
//!
//! Everything here is exercised from Rust tests acting as a foreign host
//! (`tests/roundtrip.rs`), and the Kotlin/Swift sources are generated in
//! CI via the bundled `uniffi-bindgen` binary.

use std::sync::Mutex;
use std::time::Duration;

use turbomap_core::{MapOptions, TileId};
use turbomap_engine::{CameraState, HostDrivenResolver, MapEngine, TurbomapEngine};
use turbomap_scene::{LatLng, Scene, ScreenPoint};

pub mod offscreen;

// Structured per-frame trace (Slice-1 instrumentation). Ungated so its pure
// JSON serialization is host-compiled + unit-tested; the `surface` FFI that
// publishes it is Android-only.
mod trace;

// On-screen render path for Android: a `wgpu::Surface` built from a Java
// `Surface` through hand-written JNI (uniffi can't carry an `ANativeWindow`).
#[cfg(target_os = "android")]
mod surface;

uniffi::setup_scaffolding!();

// ---- value types ----------------------------------------------------------

/// Camera pose. Mirrors the Scene-IR `CameraState`.
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct Camera {
    pub lat: f64,
    pub lng: f64,
    pub zoom: f64,
    pub pitch_deg: f64,
    pub bearing_deg: f64,
}

#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}

/// The camera's locked zoom range, `[min, max]`.
#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct ZoomRange {
    pub min: f64,
    pub max: f64,
}

/// A supported ETRS89/UTM zone for coordinate translation. Maps to
/// [`turbomap_core::Crs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum UtmZone {
    /// EPSG:25832 — central meridian 9°E.
    Zone32N,
    /// EPSG:25833 — central meridian 15°E (Kartverket's primary projection).
    Zone33N,
}

impl UtmZone {
    fn to_crs(self) -> turbomap_core::Crs {
        match self {
            UtmZone::Zone32N => turbomap_core::Crs::Utm32N,
            UtmZone::Zone33N => turbomap_core::Crs::Utm33N,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Hit {
    pub layer_id: String,
    pub feature_id: Option<String>,
    /// Struck feature's stringified properties (name/class/…), for a
    /// "tap a place → info" popup. Empty for hits with no attributes.
    pub properties: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, uniffi::Record)]
pub struct Capabilities {
    pub custom_layers: bool,
    pub terrain: bool,
    pub data_driven_paint: bool,
    pub max_texture_size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TileKind {
    Raster,
    Terrain,
    Vector,
}

/// One tile the engine is waiting on. The host fetches it (using the URL
/// template from its own scene) and pushes bytes via the matching ingest.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TileRequest {
    pub kind: TileKind,
    /// Target layer; `None` for the shared terrain DEM.
    pub layer_id: Option<String>,
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

/// What an `applyScene` changed, as counts (the full delta is a Rust-side
/// concept; hosts mostly need "did anything change").
#[derive(Debug, Clone, Copy, Default, uniffi::Record)]
pub struct DeltaSummary {
    pub sources_changed: u32,
    pub layers_added: u32,
    pub layers_removed: u32,
    pub layers_updated: u32,
    pub layers_moved: u32,
}

#[derive(Debug, Clone, Copy, Default, uniffi::Record)]
pub struct DrainStats {
    pub rounds: u32,
    pub raster_tiles: u32,
    pub terrain_tiles: u32,
    pub vector_tiles: u32,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FfiError {
    #[error("no usable GPU adapter: {0}")]
    NoAdapter(String),
    #[error("invalid scene: {0}")]
    InvalidScene(String),
    #[error("engine error: {0}")]
    Engine(String),
    #[error("render error: {0}")]
    Render(String),
}

// ---- the map object -------------------------------------------------------

struct Inner {
    engine: TurbomapEngine,
    gpu: offscreen::GpuContext,
    width: u32,
    height: u32,
}

/// The map handle hosts hold. Thread-safe; uniffi hands out `Arc`s.
#[derive(uniffi::Object)]
pub struct TurboMap {
    inner: Mutex<Inner>,
}

#[uniffi::export]
impl TurboMap {
    /// Build a headless map (offscreen rendering only). The on-screen
    /// constructors live in the per-platform surface glue, which wraps
    /// the same engine — every method below behaves identically there.
    #[uniffi::constructor]
    pub fn headless(width: u32, height: u32, camera: Camera) -> Result<Self, FfiError> {
        let gpu = offscreen::headless()
            .ok_or_else(|| FfiError::NoAdapter("no wgpu adapter available".into()))?;
        let engine = TurbomapEngine::new(
            gpu.device.clone(),
            gpu.queue.clone(),
            offscreen::TARGET_FORMAT,
            (width, height),
            to_camera_state(camera),
            MapOptions::default(),
            Box::new(HostDrivenResolver),
        )
        .map_err(|e| FfiError::Engine(e.to_string()))?;
        Ok(Self {
            inner: Mutex::new(Inner {
                engine,
                gpu,
                width,
                height,
            }),
        })
    }

    /// Replace the whole map state with a Scene-IR JSON document. The
    /// engine diffs against the previous scene and does minimal GPU work.
    pub fn apply_scene(&self, scene_json: String) -> Result<DeltaSummary, FfiError> {
        let scene: Scene = serde_json::from_str(&scene_json)
            .map_err(|e| FfiError::InvalidScene(e.to_string()))?;
        scene
            .validate()
            .map_err(|e| FfiError::InvalidScene(e.to_string()))?;
        let mut inner = self.lock();
        let delta = inner.engine.apply(scene);
        Ok(summarize(&delta))
    }

    /// The currently applied scene, as Scene-IR JSON.
    pub fn scene_json(&self) -> String {
        let inner = self.lock();
        serde_json::to_string(inner.engine.scene()).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn set_camera(&self, camera: Camera) {
        self.lock().engine.set_camera(to_camera_state(camera));
    }

    pub fn camera(&self) -> Camera {
        from_camera_state(self.lock().engine.camera())
    }

    /// Lock the camera's zoom to `[min, max]` so the user can't zoom past
    /// the map's accuracy (past where real tiles exist, the raster
    /// upsamples and overlays drift). The current zoom is clamped into range
    /// immediately. Call [`clear_zoom_bounds`](Self::clear_zoom_bounds) to
    /// return to tracking the active tile sources automatically.
    pub fn set_zoom_bounds(&self, min: f64, max: f64) {
        self.lock().engine.set_zoom_bounds(Some((min, max)));
    }

    /// Drop the manual zoom lock and track the active tile sources' declared
    /// zoom ranges automatically (the default).
    pub fn clear_zoom_bounds(&self) {
        self.lock().engine.set_zoom_bounds(None);
    }

    /// The zoom range the camera is currently locked to.
    pub fn zoom_bounds(&self) -> ZoomRange {
        let (min, max) = self.lock().engine.zoom_bounds();
        ZoomRange { min, max }
    }

    /// Animate to `target` over `duration_ms`; call `tick()` every frame
    /// while it returns `true`.
    pub fn ease_to(&self, target: Camera, duration_ms: u64) {
        self.lock()
            .engine
            .ease_to(to_camera_state(target), Duration::from_millis(duration_ms));
    }

    /// Advance camera animation. `true` = still animating, keep rendering.
    pub fn tick(&self) -> bool {
        self.lock().engine.tick_now()
    }

    pub fn resize(&self, width: u32, height: u32) {
        let mut inner = self.lock();
        inner.width = width;
        inner.height = height;
        inner.engine.resize(width, height);
    }

    pub fn project(&self, geo: GeoPoint) -> Option<Point> {
        self.lock()
            .engine
            .project(LatLng::new(geo.lat, geo.lng))
            .map(|p| Point { x: p.x, y: p.y })
    }

    pub fn unproject(&self, screen: Point) -> Option<GeoPoint> {
        self.lock()
            .engine
            .unproject(ScreenPoint::new(screen.x, screen.y))
            .map(|g| GeoPoint {
                lat: g.lat,
                lng: g.lng,
            })
    }

    pub fn hit_test(&self, screen: Point, tolerance_px: f64) -> Vec<Hit> {
        self.lock()
            .engine
            .hit_test(ScreenPoint::new(screen.x, screen.y), tolerance_px)
            .into_iter()
            .map(|h| Hit {
                layer_id: h.layer_id,
                feature_id: h.feature_id,
                properties: h.properties,
            })
            .collect()
    }

    pub fn capabilities(&self) -> Capabilities {
        let caps = self.lock().engine.capabilities();
        Capabilities {
            custom_layers: caps.custom_layers,
            terrain: caps.terrain,
            data_driven_paint: caps.data_driven_paint,
            max_texture_size: caps.max_texture_size,
        }
    }

    /// Layer ids the engine couldn't render from the last `applyScene`.
    pub fn unsupported_layers(&self) -> Vec<String> {
        self.lock().engine.unsupported_layers().to_vec()
    }

    // ---- host-driven tile IO ----------------------------------------------

    /// Tiles the engine is waiting on. Fetch each host-side and push the
    /// bytes back through the matching `ingest*`.
    pub fn pending_tiles(&self) -> Vec<TileRequest> {
        use turbomap_core::PendingTile;
        self.lock()
            .engine
            .pending_tiles()
            .into_iter()
            .map(|p| match p {
                PendingTile::Raster { layer_id, tile } => tile_request(TileKind::Raster, Some(layer_id), tile),
                PendingTile::Hillshade { layer_id, tile } => tile_request(TileKind::Terrain, Some(layer_id), tile),
                PendingTile::Terrain { tile } => tile_request(TileKind::Terrain, None, tile),
                PendingTile::Vector { layer_id, tile } => tile_request(TileKind::Vector, Some(layer_id), tile),
            })
            .collect()
    }

    /// Push a fetched raster tile (encoded PNG/JPEG/WebP bytes, exactly as
    /// served). Returns `false` if the bytes don't decode.
    pub fn ingest_raster_tile(&self, layer_id: String, z: u8, x: u32, y: u32, bytes: Vec<u8>) -> bool {
        self.lock()
            .engine
            .ingest_raster_encoded(&layer_id, TileId::new(z, x, y), &bytes)
    }

    /// Push a fetched DEM tile (encoded Terrain-RGB / Terrarium image).
    pub fn ingest_terrain_tile(&self, z: u8, x: u32, y: u32, bytes: Vec<u8>) -> bool {
        self.lock()
            .engine
            .ingest_terrain_encoded(TileId::new(z, x, y), &bytes)
    }

    /// Push a fetched vector tile (raw MVT protobuf bytes).
    pub fn ingest_vector_tile(&self, layer_id: String, z: u8, x: u32, y: u32, bytes: Vec<u8>) -> bool {
        self.lock()
            .engine
            .ingest_mvt(&layer_id, TileId::new(z, x, y), &bytes)
    }

    // ---- weather-cloud overlay --------------------------------------------

    /// Enable the procedural cloud overlay with a radar grid of
    /// `grid_w × grid_h` cells. Push frames with [`ingest_radar_frame`] and
    /// scrub the time slider with [`set_cloud_time`]. The overlay draws on
    /// every subsequent `render`/on-screen frame until disabled.
    ///
    /// [`ingest_radar_frame`]: Self::ingest_radar_frame
    /// [`set_cloud_time`]: Self::set_cloud_time
    pub fn enable_clouds(&self, grid_w: u32, grid_h: u32) {
        self.lock().engine.enable_clouds(grid_w, grid_h);
    }

    /// Enable terrain cast shadows at `strength` in `[0,1]` (0 = off). Only
    /// affects 3D terrain; off costs nothing. See
    /// [`turbomap_engine::Engine::set_terrain_shadows`].
    pub fn set_terrain_shadows(&self, strength: f32) {
        self.lock().engine.set_terrain_shadows(strength);
    }

    /// Drive the realistic-water surface from the MET wave/wind forecast: wave
    /// direction + ferocity, whitecaps when the sea turns extreme, shoreline
    /// foam. Each input is optional (`None` for fields MET omits); all-`None` ⇒
    /// calm. Bearings are degrees the wave/wind comes *from*. See
    /// [`turbomap_engine::TurbomapEngine::set_water_conditions`].
    pub fn set_water_conditions(
        &self,
        wave_from_deg: Option<f32>,
        wave_height_m: Option<f32>,
        wind_speed_ms: Option<f32>,
        wind_from_deg: Option<f32>,
    ) {
        self.lock().engine.set_water_conditions(
            wave_from_deg,
            wave_height_m,
            wind_speed_ms,
            wind_from_deg,
        );
    }

    /// Tear the cloud overlay down, freeing its GPU resources.
    pub fn disable_clouds(&self) {
        self.lock().engine.disable_clouds();
    }

    /// Show/hide the overlay without discarding uploaded frames.
    pub fn set_clouds_visible(&self, visible: bool) {
        self.lock().engine.set_clouds_visible(visible);
    }

    /// Upload a radar frame into `slot` 0 (current timestep) or 1 (next),
    /// from two `grid_w * grid_h` byte planes — `precip` and `coverage`,
    /// each `0..=255`. The host samples MET radar/cloud rasters for the
    /// viewport and normalises them to this grid.
    pub fn ingest_radar_frame(
        &self,
        slot: u32,
        grid_w: u32,
        grid_h: u32,
        precip: Vec<u8>,
        coverage: Vec<u8>,
    ) {
        self.lock()
            .engine
            .ingest_radar_frame(slot, grid_w, grid_h, &precip, &coverage);
    }

    /// Set the cloud animation clock (`time`, seconds — drives drift/boil)
    /// and the slot-0→slot-1 crossfade (`blend`, `0..=1`). A time slider
    /// drives `blend`; it can run forward or backward.
    pub fn set_cloud_time(&self, time: f32, blend: f32) {
        self.lock().engine.set_cloud_time(time, blend);
    }

    /// Drain sources that need no IO (inline GeoJSON) in-process. Remote
    /// tiles are untouched — they stay in `pendingTiles()` for the host.
    pub fn pump_local_tiles(&self) -> DrainStats {
        let stats = self.lock().engine.pump_tiles();
        DrainStats {
            rounds: stats.rounds,
            raster_tiles: stats.raster_tiles,
            terrain_tiles: stats.terrain_tiles,
            vector_tiles: stats.vector_tiles,
        }
    }

    // ---- offscreen rendering ----------------------------------------------

    /// Render the current frame offscreen and return it as PNG bytes —
    /// snapshot/verification path usable from any host language. The
    /// on-screen path renders the same engine through the surface glue.
    pub fn render_png(&self) -> Result<Vec<u8>, FfiError> {
        let mut inner = self.lock();
        let (w, h) = (inner.width, inner.height);
        let Inner { engine, gpu, .. } = &mut *inner;
        let rgba = offscreen::render_to_rgba(gpu, w, h, |enc, view| engine.render(enc, view))
            .map_err(FfiError::Render)?;
        engine.after_submit();
        let mut png = Vec::new();
        {
            use image::ImageEncoder;
            image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut png))
                .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
                .map_err(|e| FfiError::Render(e.to_string()))?;
        }
        Ok(png)
    }
}

impl TurboMap {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A panic while holding the lock leaves the map unusable either
        // way; recover the guard rather than poisoning every later call.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// ---- projection helpers -----------------------------------------------------

/// Translate an ETRS89/UTM `easting`/`northing` (metres) into WGS84 lat/lng.
/// Markers, the camera centre, and the Scene IR all speak lat/lng, so a host
/// holding coordinates from a Norwegian UTM dataset runs them through here
/// once and they place on the Web-Mercator map exactly where the basemap
/// puts that location — no drift.
#[uniffi::export]
pub fn utm_to_lat_lng(easting: f64, northing: f64, zone: UtmZone) -> GeoPoint {
    let ll = turbomap_core::Crs::to_lat_lng(zone.to_crs(), easting, northing);
    GeoPoint {
        lat: ll.lat,
        lng: ll.lng,
    }
}

/// Project a WGS84 lat/lng to ETRS89/UTM `(easting, northing)` metres in
/// `zone`. Inverse of [`utm_to_lat_lng`].
#[uniffi::export]
pub fn lat_lng_to_utm(geo: GeoPoint, zone: UtmZone) -> Point {
    let (easting, northing) = turbomap_core::Crs::from_lat_lng(
        zone.to_crs(),
        turbomap_core::LatLng::new(geo.lat, geo.lng),
    );
    Point {
        x: easting,
        y: northing,
    }
}

// ---- conversions ------------------------------------------------------------

fn to_camera_state(c: Camera) -> CameraState {
    CameraState {
        center: LatLng::new(c.lat, c.lng),
        zoom: c.zoom,
        pitch_deg: c.pitch_deg,
        bearing_deg: c.bearing_deg,
    }
}

fn from_camera_state(c: CameraState) -> Camera {
    Camera {
        lat: c.center.lat,
        lng: c.center.lng,
        zoom: c.zoom,
        pitch_deg: c.pitch_deg,
        bearing_deg: c.bearing_deg,
    }
}

fn tile_request(kind: TileKind, layer_id: Option<String>, tile: TileId) -> TileRequest {
    TileRequest {
        kind,
        layer_id,
        z: tile.z,
        x: tile.x,
        y: tile.y,
    }
}

fn summarize(delta: &turbomap_scene::SceneDelta) -> DeltaSummary {
    use turbomap_scene::LayerChange;
    let mut out = DeltaSummary {
        sources_changed: delta.sources.len() as u32,
        ..Default::default()
    };
    for change in &delta.layers {
        match change {
            LayerChange::Added { .. } => out.layers_added += 1,
            LayerChange::Removed { .. } => out.layers_removed += 1,
            LayerChange::Updated { .. } => out.layers_updated += 1,
            LayerChange::Moved { .. } => out.layers_moved += 1,
        }
    }
    out
}
