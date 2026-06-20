//! Unified `Map` with an ordered layer stack.
//!
//! A `Map` owns the camera, viewport, markers, animation, and the
//! pipelines that span layers (text, markers). Each layer in `layers`
//! carries its own source, scene-state (zoom bounds + ingested set +
//! prefetch margin), and layer-specific pipeline + cache.
//!
//! Layers are drawn back-to-front in insertion order, all inside ONE
//! render pass per frame (tile-based mobile GPUs pay a full
//! framebuffer load/store per pass):
//! 1. The pass clears to the first visible layer's background (vector
//!    layers' style background; otherwise the shared backdrop).
//! 2. Each visible layer's geometry draws in order, compositing on top.
//! 3. After all layer geometry, labels draw per visible vector layer.
//! 4. Finally, markers paint on top.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::{
    camera::{Camera, CameraAnimation, FlingAnimation, ZoomBounds, ZoomFlingAnimation, ZoomLock},
    error::MapError,
    geo::{LatLng, WorldPoint},
    hit::geometry_hit,
    render::{
        cache::CacheStats,
        frame::{RenderFrame, TerrainFrameInputs},
        gpu_timestamps::GpuTimestamps,
        hillshade::{HillshadePipeline, PreparedHillshade},
        icon::{IconPipeline, PreparedIcons},
        marker::MarkerPipeline,
        raster::{PreparedRaster, RasterPipeline},
        sky::SkyPipeline,
        targets::FrameTargets,
        terrain::{Terrain, TerrainCache, TerrainOptions, TerrainShared},
        text::{PreparedText, TextPipeline},
        vector::{PreparedVector, VectorPipeline},
        vector_cache::VectorMeshCache,
        TextureCache, BACKGROUND_CLEAR,
    },
    lighting::Lighting,
    markers::MarkerManager,
    scene::Scene,
    source::TileSource,
    style::{Color, HillshadeStyle, VectorStyle},
    sun::SunPosition,
    tessellate::{self, IconRequest, InteractiveFeature, LabelRequest, Mesh},
    tile::TileId,
    vector::{GeomType, Value as VectorValue, VectorTile, VectorTileSource},
};

pub use crate::render::terrain::TerrainOptions as PublicTerrainOptions;
pub use turbomap_clouds::{CloudParams, RadarFrame};

/// The procedural weather-cloud overlay, drawn after the map's frame
/// pass. Holds the GPU pipeline state plus the current per-frame
/// parameters (animation clock + A→B radar crossfade) the host updates.
struct CloudOverlay {
    scene: turbomap_clouds::CloudScene,
    params: CloudParams,
    /// Radar grid dimensions the two data textures were allocated for.
    grid: (u32, u32),
    enabled: bool,
    /// Geographic box the radar covers, in normalised-Mercator world coords
    /// `(min, max)` (x: west→east, y: north→south). When set, the overlay is
    /// world-locked: each screen pixel is mapped into this box so the clouds
    /// pan and zoom with the terrain. `None` → screen-locked (the field affine
    /// stays identity, the legacy look).
    radar_geo: Option<(WorldPoint, WorldPoint)>,
}

/// Cloud slab thickness as a fraction of the radar box (mercator). Sets the
/// pitch-parallax magnitude: the per-pixel field-uv rake between the slab floor
/// and ceiling is `(ray.xy/ray.z) · CLOUD_SLAB_FRAC` (the box scale cancels into
/// `world_to_field`, so the depth feels the same at any zoom). At map_scale 8 a
/// puff is ~0.125 of the field, so this is kept to a fraction of a puff: 0.15
/// (the old proxy-matrix value) raked ~3 puffs near the grazing top of a tilted
/// view and averaged the cloud/gap structure into a flat white wash. 0.04 gives
/// a subtle, structure-preserving side-reveal. Pair with the shift clamp in
/// `clouds.wgsl` (`view_parallax`) that caps the grazing-ray blow-up.
const CLOUD_SLAB_FRAC: f64 = 0.04;

/// Screen-uv → radar-box-uv affine for the cloud overlay. Samples the real
/// camera projection at three viewport corners (exact for top-down; bearing-
/// and inset-correct via [`Camera::pixel_to_world`]) and expresses them in the
/// radar's geo box `[min, max]` (normalised Mercator). The shader then samples
/// the field at `origin + uv.x·dx + uv.y·dy`, so a fixed world point lands at
/// the same field-uv under any camera — i.e. the clouds are world-locked
/// (pan + zoom with the terrain). Returns `(origin, dx, dy)`.
fn cloud_field_affine(
    cam: Camera,
    viewport: (f64, f64),
    min: WorldPoint,
    max: WorldPoint,
) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let (vw, vh) = viewport;
    // Non-zero span (signed) so a degenerate box can't divide by zero.
    let sx = (max.x - min.x).abs().max(1e-12) * (max.x - min.x).signum();
    let sy = (max.y - min.y).abs().max(1e-12) * (max.y - min.y).signum();
    let w00 = cam.pixel_to_world((0.0, 0.0), (vw, vh));
    let w10 = cam.pixel_to_world((vw, 0.0), (vw, vh));
    let w01 = cam.pixel_to_world((0.0, vh), (vw, vh));
    (
        [((w00.x - min.x) / sx) as f32, ((w00.y - min.y) / sy) as f32],
        [((w10.x - w00.x) / sx) as f32, ((w10.y - w00.y) / sy) as f32],
        [((w01.x - w00.x) / sx) as f32, ((w01.y - w00.y) / sy) as f32],
    )
}

/// The one camera animation in flight. `Map` samples whichever is active in
/// `tick`; they are mutually exclusive — starting any one replaces the rest.
#[derive(Clone, Copy)]
enum ActiveAnim {
    Ease(CameraAnimation),
    PanFling(FlingAnimation),
    ZoomFling(ZoomFlingAnimation),
}

impl ActiveAnim {
    fn sample(&self, now: Instant) -> Camera {
        match self {
            ActiveAnim::Ease(a) => a.sample(now),
            ActiveAnim::PanFling(f) => f.sample(now),
            ActiveAnim::ZoomFling(z) => z.sample(now),
        }
    }

    fn is_finished(&self, now: Instant) -> bool {
        match self {
            ActiveAnim::Ease(a) => a.is_finished(now),
            ActiveAnim::PanFling(f) => f.is_finished(now),
            ActiveAnim::ZoomFling(z) => z.is_finished(now),
        }
    }
}

/// Build the depth attachment matching the surface size. Re-created
/// on resize.
// Re-export the marker/hit types that used to live on `VectorMap` — they
// belong with the public Map facade now.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkerId(pub u64);

#[derive(Debug, Clone)]
pub struct Marker {
    pub id: MarkerId,
    pub lng_lat: LatLng,
    pub radius_px: f32,
    pub color: Color,
    pub data: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct HitFeature {
    pub layer_id: String,
    pub tile_id: TileId,
    pub source_layer: String,
    pub feature_id: u64,
    pub geom_type: GeomType,
    pub properties: std::collections::HashMap<String, VectorValue>,
}

#[derive(Debug, Clone)]
pub struct HitMarker {
    pub id: MarkerId,
    pub lng_lat: LatLng,
    pub data: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum HitResult {
    Feature(HitFeature),
    Marker(HitMarker),
}

#[derive(Debug, Clone)]
pub struct MapOptions {
    pub cache_budget_bytes: usize,
    pub prefetch_margin_px: u32,
    pub fade_in_secs: f32,
    /// Device pixel ratio: how many physical framebuffer pixels per
    /// logical "style pixel". Style-authored sizes (line widths, font
    /// sizes, dash lengths, icon sizes) are multiplied by this, so a map
    /// on a 3× phone screen renders crisp instead of upscaled. 1.0 =
    /// desktop/1:1.
    pub pixel_ratio: f32,
}

impl Default for MapOptions {
    fn default() -> Self {
        Self {
            // Per-layer GPU texture budget. There are up to THREE live
            // caches at once (raster + vector + terrain DEM), so this is
            // ×3 of real VRAM. 128 MiB each (384 MiB total) risks OOM /
            // ANR on a phone under sustained tilted 3D; 80 MiB (≈240 MiB
            // total) is safe, and the eviction↔ingested coherence fix
            // means a smaller cache just re-requests rather than greying
            // out. Comfortably holds a screen's working set at any pitch.
            cache_budget_bytes: 80 * 1024 * 1024,
            prefetch_margin_px: 256,
            pixel_ratio: 1.0,
            // Fade-in is currently DISABLED (0 = tiles snap to fully
            // opaque on ingest) while the per-frame-alpha flicker
            // diagnosis is open. History: 0.18 popped (per-layer
            // arrivals read as discrete additions), 0.4 read as a
            // smooth fade but was suspected as the flicker source.
            // If the flicker is fixed elsewhere, restore 0.4 and
            // update `default_options_are_sensible` accordingly.
            fade_in_secs: 0.0,
        }
    }
}

/// One pending tile request from `Map::pending_tiles`. The host uses
/// `layer_id` to route ingest calls back to the right layer.
///
/// `Terrain` is a special variant — there is at most one terrain
/// source per Map (registered via `set_terrain_source`), used as the
/// heightmap by every ground-plane pipeline. The host fetches it the
/// same way as a raster source (PNG bytes) and pushes back via
/// `ingest_terrain_tile`.
#[derive(Debug, Clone)]
pub enum PendingTile {
    Raster {
        layer_id: String,
        tile: TileId,
    },
    Vector {
        layer_id: String,
        tile: TileId,
    },
    /// A DEM tile (raster bytes, special semantic). Same fetch shape as
    /// `Raster`; the host's pump can usually route both through the same
    /// network worker pool and differentiate by `layer_id`.
    Hillshade {
        layer_id: String,
        tile: TileId,
    },
    /// DEM tile for the shared terrain heightmap. Map-level, not
    /// layer-scoped. Host fetches it as raster bytes and routes back
    /// via `Map::ingest_terrain_tile`.
    Terrain {
        tile: TileId,
    },
}

/// Each entry in `Map::layers`. We `Box` the variants because both layer
/// types own substantial GPU state (kilobytes), and clippy flags the
/// unboxed enum as too large to be moved around freely.
enum LayerEntry {
    Raster(Box<RasterLayer>),
    Vector(Box<VectorLayer>),
    Hillshade(Box<HillshadeLayer>),
}

/// Per-layer output of the render prep phase (Phase A). Tagged with
/// the layer's index in `Map::layers` so the draw phase can pair each
/// prepared item back up with an *immutable* borrow of its layer.
enum PreparedLayer {
    Raster(PreparedRaster),
    Vector(PreparedVector),
    Hillshade(PreparedHillshade),
}

struct RasterLayer {
    id: String,
    source: Arc<dyn TileSource>,
    scene: Scene,
    pipeline: RasterPipeline,
    cache: TextureCache,
    fade_in_secs: f32,
    visible: bool,
}

struct VectorLayer {
    id: String,
    source: Arc<dyn VectorTileSource>,
    style: VectorStyle,
    scene: Scene,
    pipeline: VectorPipeline,
    cache: VectorMeshCache,
    fade_in_secs: f32,
    visible: bool,
    /// Per-frame paint colour override (linear RGBA in `[0,1]`). When set,
    /// the shader uses it instead of the baked vertex colour — the path
    /// zoom-interpolated / data-driven paint takes. `None` keeps the baked
    /// colour.
    paint_override: Option<[f32; 4]>,
    /// `(dash_len_px, gap_len_px)` for a dashed line layer; `None` = solid.
    dash: Option<(f32, f32)>,
    /// Per-frame multiplier on baked line widths — the zoom curve that lets
    /// roads thicken as you zoom in without re-tessellating. 1.0 = baked.
    width_scale: f32,
}

struct HillshadeLayer {
    id: String,
    style: HillshadeStyle,
    pipeline: HillshadePipeline,
    fade_in_secs: f32,
    visible: bool,
}

/// The ordered stack of render layers (raster basemaps, vector overlays,
/// hillshade), bottom-to-top. Owns the `Vec<LayerEntry>` plus the by-id query
/// and mutation logic that used to live as a dozen scanning loops on `Map`.
///
/// Derefs to the inner `Vec` so the render hot-loop, hit-test, ingest and
/// metrics keep iterating/indexing it directly; the named methods give the
/// layer *semantics* (find-by-id, visibility, source ranges) a single home.
#[derive(Default)]
struct LayerStack {
    entries: Vec<LayerEntry>,
}

impl std::ops::Deref for LayerStack {
    type Target = Vec<LayerEntry>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}
impl std::ops::DerefMut for LayerStack {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}

impl LayerStack {
    fn new() -> Self {
        Self::default()
    }

    /// The id of any layer kind.
    fn id_of(entry: &LayerEntry) -> &str {
        match entry {
            LayerEntry::Raster(r) => &r.id,
            LayerEntry::Vector(v) => &v.id,
            LayerEntry::Hillshade(h) => &h.id,
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|l| Self::id_of(l) == id)
    }

    fn ids(&self) -> Vec<String> {
        self.entries.iter().map(|l| Self::id_of(l).to_string()).collect()
    }

    /// Remove the layer with `id` (if any). Returns whether the stack changed
    /// — the caller recomputes the zoom lock when a source leaves.
    fn remove(&mut self, id: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|l| Self::id_of(l) != id);
        self.entries.len() != before
    }

    fn vector_source(&self, id: &str) -> Option<Arc<dyn VectorTileSource>> {
        self.entries.iter().find_map(|l| match l {
            LayerEntry::Vector(v) if v.id == id => Some(v.source.clone()),
            _ => None,
        })
    }

    fn raster_source(&self, id: &str) -> Option<Arc<dyn TileSource>> {
        self.entries.iter().find_map(|l| match l {
            LayerEntry::Raster(r) if r.id == id => Some(r.source.clone()),
            _ => None,
        })
    }

    fn vector_style(&self, id: &str) -> Option<VectorStyle> {
        self.entries.iter().find_map(|l| match l {
            LayerEntry::Vector(v) if v.id == id => Some(v.style.clone()),
            _ => None,
        })
    }

    fn visibility(&self, id: &str) -> Option<bool> {
        self.entries.iter().find_map(|l| match l {
            LayerEntry::Raster(r) if r.id == id => Some(r.visible),
            LayerEntry::Vector(v) if v.id == id => Some(v.visible),
            LayerEntry::Hillshade(h) if h.id == id => Some(h.visible),
            _ => None,
        })
    }

    fn set_visibility(&mut self, id: &str, visible: bool) -> bool {
        for layer in self.entries.iter_mut() {
            let (lid, lvis): (&str, &mut bool) = match layer {
                LayerEntry::Raster(r) => (&r.id, &mut r.visible),
                LayerEntry::Vector(v) => (&v.id, &mut v.visible),
                LayerEntry::Hillshade(h) => (&h.id, &mut h.visible),
            };
            if lid == id {
                *lvis = visible;
                return true;
            }
        }
        false
    }

    fn set_fade_in(&mut self, id: &str, secs: f32) -> bool {
        for layer in self.entries.iter_mut() {
            let (lid, fade): (&str, &mut f32) = match layer {
                LayerEntry::Raster(r) => (&r.id, &mut r.fade_in_secs),
                LayerEntry::Vector(v) => (&v.id, &mut v.fade_in_secs),
                LayerEntry::Hillshade(h) => (&h.id, &mut h.fade_in_secs),
            };
            if lid == id {
                *fade = secs.max(0.0);
                return true;
            }
        }
        false
    }

    /// Run `f` on the vector layer with `id`; returns `false` if none matches.
    fn with_vector_mut(&mut self, id: &str, f: impl FnOnce(&mut VectorLayer)) -> bool {
        for layer in self.entries.iter_mut() {
            if let LayerEntry::Vector(v) = layer {
                if v.id == id {
                    f(v);
                    return true;
                }
            }
        }
        false
    }

    /// Index of the first visible layer (the one that "clears" the frame), or
    /// `None` if the stack is empty / all hidden.
    fn first_visible_index(&self) -> Option<usize> {
        self.entries.iter().position(|l| match l {
            LayerEntry::Raster(r) => r.visible,
            LayerEntry::Vector(v) => v.visible,
            LayerEntry::Hillshade(h) => h.visible,
        })
    }

    /// The `(min_zoom, max_zoom)` of every layer that owns a tile source —
    /// the input to the camera's source-derived zoom lock. Hillshade has no
    /// own source (it reads the shared terrain).
    fn source_ranges(&self) -> Vec<(u8, u8)> {
        self.entries
            .iter()
            .filter_map(|l| match l {
                LayerEntry::Raster(r) => Some((r.source.min_zoom(), r.source.max_zoom())),
                LayerEntry::Vector(v) => Some((v.source.min_zoom(), v.source.max_zoom())),
                LayerEntry::Hillshade(_) => None,
            })
            .collect()
    }

    /// Any layer's `Scene` (they all sync from the same camera) — used as the
    /// projection source for markers. Prefers raster/vector (which own a
    /// scene); hillshade reads the shared terrain so contributes none.
    fn marker_scene(&self) -> Option<&Scene> {
        self.entries.iter().find_map(|l| match l {
            LayerEntry::Raster(r) => Some(&r.scene),
            LayerEntry::Vector(v) => Some(&v.scene),
            LayerEntry::Hillshade(_) => None,
        })
    }
}

/// The GPU rendering toolbox: the screen-space pipelines shared across all
/// layers (text, icon, marker, sky), the frame's colour/depth attachments, the
/// optional GPU timer, and the terrain bind-group layout + placeholder.
///
/// This is everything the frame pass needs that is NOT per-layer — the
/// raster/vector/hillshade pipelines stay on their `LayerEntry`. Grouping it
/// gives the rendering subsystem one owner instead of seven loose fields on
/// the `Map` god-object; `Map::render` reads them as `self.renderer.*`.
struct Renderer {
    /// Single text pipeline, shared across all vector layers.
    text_pipeline: TextPipeline,
    /// Single icon/sprite pipeline, shared across all vector layers.
    icon_pipeline: IconPipeline,
    marker_pipeline: MarkerPipeline,
    /// Analytic atmosphere sky, drawn first in the frame pass when the camera
    /// is tilted (so the horizon band shows behind the terrain).
    sky_pipeline: SkyPipeline,
    /// Optional GPU-side frame timing — `Some` only when the device negotiated
    /// `Features::TIMESTAMP_QUERY`.
    gpu_timestamps: Option<GpuTimestamps>,
    /// Map-level terrain bind-group layout + sampler + 1×1 placeholder bind
    /// group. Always present so displacement-capable pipelines can be built
    /// before any terrain source is registered — they bind the placeholder
    /// and render flat.
    terrain_shared: TerrainShared,
    /// The frame's MSAA colour + depth attachments, recreated together on
    /// resize. See [`FrameTargets`].
    targets: FrameTargets,
}

impl Renderer {
    fn new(
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        initial_size: (u32, u32),
    ) -> Self {
        Self {
            text_pipeline: TextPipeline::new(device.clone(), queue.clone(), surface_format),
            icon_pipeline: IconPipeline::new(device.clone(), queue.clone(), surface_format),
            marker_pipeline: MarkerPipeline::new(device.clone(), queue.clone(), surface_format),
            sky_pipeline: SkyPipeline::new(device.clone(), queue.clone(), surface_format),
            gpu_timestamps: GpuTimestamps::new(device, queue),
            terrain_shared: TerrainShared::new(device, queue),
            targets: FrameTargets::new(device, surface_format, initial_size),
        }
    }
}

/// The camera's pose and the state that drives + constrains it: the shared
/// [`Camera`], the single in-flight animation (eased move / pan fling / zoom
/// fling), the bottom viewport inset, and the [`ZoomLock`].
///
/// Mutating a `CameraState` only changes the pose — it deliberately does NOT
/// reach into layers or terrain. The `Map` owns the side effects: after any
/// camera change it runs its scene-sync seam (`sync_scenes`: re-sync each
/// layer's `Scene`, clamp pitch against the terrain). That separation is what
/// lets the camera be one cohesive "moving piece" instead of a scatter of
/// fields whose every write had to remember to also re-sync the world.
struct CameraState {
    /// The pose. Layers' `Scene` is synced from this each camera change.
    camera: Camera,
    /// The single in-flight animation, if any. Sampled in `tick`; starting one
    /// replaces whatever was running.
    active: Option<ActiveAnim>,
    /// Bottom viewport inset (px), re-stamped onto `camera` on every update so
    /// projection + render stay inset-aware. 0 = none.
    viewport_inset_px: f64,
    /// The zoom range the camera is locked to (+ optional host override),
    /// re-stamped onto `camera` on every update. See [`ZoomLock`].
    zoom: ZoomLock,
}

impl CameraState {
    fn new(initial: Camera) -> Self {
        Self {
            camera: initial,
            active: None,
            viewport_inset_px: initial.viewport_inset_px,
            zoom: ZoomLock::new(initial.zoom_bounds),
        }
    }

    /// Re-stamp the sticky inset + zoom lock onto the pose. Called after any
    /// pose change so a host-supplied or animation-sampled camera can't escape
    /// the map's inset / accuracy bounds.
    fn restamp(&mut self) {
        self.camera.viewport_inset_px = self.viewport_inset_px;
        self.camera.set_zoom_bounds(self.zoom.active());
    }

    /// Replace the pose (sanitising untrusted input), clear any animation, and
    /// re-stamp the inset + zoom lock. The `Map` follows this with its scene
    /// re-sync seam.
    fn set(&mut self, camera: Camera) {
        self.camera = camera.sanitized();
        self.active = None;
        self.restamp();
    }

    /// Sample the active animation at `now`, re-stamping the pose. Returns
    /// `(advanced, still_animating)`: `advanced` = a frame was sampled (the
    /// `Map` re-syncs scenes when so), `still_animating` = keep ticking.
    fn tick(&mut self, now: Instant) -> (bool, bool) {
        let Some(anim) = self.active else {
            return (false, false);
        };
        self.camera = anim.sample(now);
        self.restamp();
        if anim.is_finished(now) {
            self.active = None;
            (true, false)
        } else {
            (true, true)
        }
    }
}

pub struct Map {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface_format: wgpu::TextureFormat,
    viewport_px: (u32, u32),
    /// The camera's pose + drive state: the shared camera, the single in-flight
    /// animation (fling/ease/zoom-fling), the viewport inset, and the zoom
    /// lock. Mutating it never touches layers/terrain — `Map` runs the
    /// `sync_after_camera` seam after every camera change to re-sync scenes
    /// and clamp pitch against the terrain. See [`CameraState`].
    cam: CameraState,
    options: MapOptions,
    layers: LayerStack,
    /// The GPU rendering toolbox: the shared screen-space pipelines (text,
    /// icon, marker, sky), the frame attachments, the GPU timer, and the
    /// terrain bind-group layout/placeholder. The per-layer raster/vector/
    /// hillshade pipelines live on their `LayerEntry`; this holds everything
    /// the frame pass needs that *isn't* per-layer. See [`Renderer`].
    renderer: Renderer,
    markers: MarkerManager,
    last_frame_metrics: FrameMetrics,
    /// Optional shared heightmap. When set, ground-plane pipelines
    /// (raster, hillshade, vector) sample the DEM in their vertex
    /// shaders and displace by elevation. See [`TerrainOptions`].
    terrain: Option<Terrain>,
    /// Optional procedural weather-cloud overlay. Drawn in its own pass
    /// over the resolved surface (it's a single-sampled, depth-less
    /// fullscreen composite, so it can't share the MSAA frame pass).
    clouds: Option<CloudOverlay>,
    /// Scene lighting — the sun driving terrain shading, sky, aerial
    /// perspective + clouds. One explicit mode (default / time-tracked /
    /// fixed), see [`crate::lighting::Lighting`].
    lighting: Lighting,
}

impl Map {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        initial_size: (u32, u32),
        initial_camera: Camera,
        options: MapOptions,
    ) -> Result<Self, MapError> {
        let renderer = Renderer::new(&device, &queue, surface_format, initial_size);
        Ok(Self {
            device,
            queue,
            surface_format,
            viewport_px: initial_size,
            cam: CameraState::new(initial_camera),
            options,
            layers: LayerStack::new(),
            renderer,
            markers: MarkerManager::default(),
            last_frame_metrics: FrameMetrics::default(),
            terrain: None,
            clouds: None,
            lighting: Lighting::default(),
        })
    }

    // ---- Procedural weather-cloud overlay -----------------------------

    /// Turn on the procedural cloud overlay, allocating its two radar
    /// data textures at `grid_w × grid_h`. Idempotent for a given grid
    /// size; calling with a new size rebuilds the textures (and clears any
    /// uploaded frames). Upload radar frames with [`Map::ingest_radar_frame`]
    /// and drive the animation with [`Map::set_cloud_time`].
    pub fn enable_clouds(&mut self, grid_w: u32, grid_h: u32) {
        if let Some(c) = &mut self.clouds {
            if c.grid == (grid_w, grid_h) {
                c.enabled = true;
                return;
            }
        }
        let scene = turbomap_clouds::CloudScene::new(
            &self.device,
            &self.queue,
            self.surface_format,
            grid_w,
            grid_h,
        );
        // Preserve any geo box across a grid-size rebuild so world-lock sticks.
        let radar_geo = self.clouds.as_ref().and_then(|c| c.radar_geo);
        self.clouds = Some(CloudOverlay {
            scene,
            params: CloudParams::default(),
            grid: (grid_w, grid_h),
            enabled: true,
            radar_geo,
        });
    }

    /// Geo-register the radar to the lat/lng box it covers, so the overlay is
    /// world-locked (pans + zooms with the map). Pass the bounds the radar
    /// frames were sampled for. No-op if clouds aren't enabled.
    pub fn set_cloud_geo_bounds(&mut self, west: f64, south: f64, east: f64, north: f64) {
        if let Some(c) = &mut self.clouds {
            // Mercator y grows southward, so north is the min-y corner.
            let min = LatLng::new(north, west).to_world();
            let max = LatLng::new(south, east).to_world();
            c.radar_geo = Some((min, max));
        }
    }

    /// Show/hide the overlay without discarding its GPU state or uploaded
    /// frames. No-op if clouds were never enabled.
    pub fn set_clouds_visible(&mut self, visible: bool) {
        if let Some(c) = &mut self.clouds {
            c.enabled = visible;
        }
    }

    /// Tear the overlay down entirely, freeing its GPU resources.
    pub fn disable_clouds(&mut self) {
        self.clouds = None;
    }

    /// Whether the overlay is currently enabled and will draw.
    pub fn clouds_enabled(&self) -> bool {
        self.clouds.as_ref().map(|c| c.enabled).unwrap_or(false)
    }

    /// Upload a radar frame into slot 0 (current timestep) or 1 (next).
    /// The frame's dimensions must match the grid passed to
    /// [`Map::enable_clouds`]. No-op if clouds aren't enabled.
    pub fn ingest_radar_frame(&mut self, slot: u32, frame: &RadarFrame) {
        if let Some(c) = &self.clouds {
            c.scene.upload(&self.queue, slot as usize, frame);
        }
    }

    /// Set the per-frame animation state: `time` is a free-running clock
    /// (seconds) driving cloud drift/boil; `blend` in `0..=1` crossfades
    /// the slot-0 radar frame into the slot-1 frame — this is what a time
    /// slider scrubs (and can run backward). No-op if clouds aren't enabled.
    pub fn set_cloud_time(&mut self, time: f32, blend: f32) {
        if let Some(c) = &mut self.clouds {
            c.params.time = time;
            c.params.blend = blend.clamp(0.0, 1.0);
        }
    }

    /// Replace the cloud overlay's full look parameters (wind, sun, feature
    /// scale, opacity, …). `resolution` is overwritten per frame from the
    /// viewport, so its value here is ignored. No-op if clouds aren't enabled.
    pub fn set_cloud_params(&mut self, params: CloudParams) {
        if let Some(c) = &mut self.clouds {
            let (time, blend) = (c.params.time, c.params.blend);
            c.params = params;
            c.params.time = time;
            c.params.blend = blend;
        }
    }

    /// Enable 3D terrain. Future ground-plane draws will displace
    /// their vertices by the elevation sampled from `source`. There
    /// is at most one terrain source per Map — calling this again
    /// replaces it. Halo > 0 on the source is required so adjacent
    /// tile-edge vertices agree and the mesh doesn't crack at tile
    /// boundaries.
    pub fn set_terrain_source(&mut self, source: Arc<dyn TileSource>, options: TerrainOptions) {
        let halo = source.dem_halo_px();
        let cache = TerrainCache::new(
            self.device.clone(),
            self.queue.clone(),
            &self.renderer.terrain_shared,
            self.options.cache_budget_bytes,
            halo,
            options.encoding,
        );
        let scene = Scene::with_margin(
            self.cam.camera,
            self.viewport_px,
            source.min_zoom(),
            source.max_zoom(),
            self.options.prefetch_margin_px,
        );
        self.terrain = Some(Terrain::new(source, cache, scene, options));
    }

    pub fn clear_terrain(&mut self) {
        self.terrain = None;
    }

    pub fn has_terrain(&self) -> bool {
        self.terrain.is_some()
    }

    // ---- Sun / time-of-day --------------------------------------------

    /// Make the sun (and therefore terrain shading, aerial perspective
    /// and the sky) track a real instant in time. `unix_seconds` is UTC
    /// seconds since the epoch; the position is solved per frame at the
    /// camera's current location, so the light follows both the clock
    /// and where the user is looking. `None` clears it (back to the
    /// fixed default unless [`Map::set_sun_position`] is also set).
    pub fn set_sun_time(&mut self, unix_seconds: Option<f64>) {
        self.lighting.set_time(unix_seconds);
    }

    /// Pin the sun to an explicit azimuth/altitude, overriding any
    /// time-based tracking. Used for deterministic goldens and manual
    /// control. `None` clears the override.
    pub fn set_sun_position(&mut self, sun: Option<SunPosition>) {
        self.lighting.set_fixed(sun);
    }

    /// The sun position used this frame, resolved by the [`Lighting`] mode
    /// at the camera's current location.
    fn effective_sun(&self) -> SunPosition {
        self.lighting.sun_at(self.cam.camera.center)
    }

    /// Push decoded RGBA back into the shared terrain heightmap.
    /// Host drives this from the same fetch pump it uses for raster
    /// tiles. Silently no-ops when no terrain source is registered
    /// (e.g. host sent us a stale tile after `clear_terrain`).
    pub fn ingest_terrain_tile(&mut self, tile: TileId, rgba: &[u8], width: u32, height: u32) {
        if let Some(t) = self.terrain.as_mut() {
            let evicted = t.cache.ingest(tile, rgba, width, height);
            t.scene.ingest(tile);
            for e in &evicted {
                t.scene.un_ingest(e);
            }
        }
    }

    // ---- layer management ----------------------------------------------

    pub fn add_raster_layer(&mut self, id: impl Into<String>, source: Arc<dyn TileSource>) {
        let id = id.into();
        let min_zoom = source.min_zoom();
        let max_zoom = source.max_zoom();
        let pipeline = RasterPipeline::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &self.renderer.terrain_shared.bind_group_layout,
        );
        let cache = TextureCache::new(
            self.device.clone(),
            self.queue.clone(),
            pipeline.texture_bind_group_layout.clone(),
            pipeline.sampler.clone(),
            self.options.cache_budget_bytes,
            // Basemaps carry colour data — sRGB so the GPU returns
            // perceptually-correct linear values to the shader.
            wgpu::TextureFormat::Rgba8UnormSrgb,
            // Mipmaps eliminate shimmer when a tile is minified at
            // zoom-out. Cost is ~33 % more GPU memory per tile; the
            // budget eviction loop accounts for the full chain size.
            true,
        );
        let scene = Scene::with_margin(
            self.cam.camera,
            self.viewport_px,
            min_zoom,
            max_zoom,
            self.options.prefetch_margin_px,
        );
        self.layers.push(LayerEntry::Raster(Box::new(RasterLayer {
            id,
            source,
            scene,
            pipeline,
            cache,
            fade_in_secs: self.options.fade_in_secs,
            visible: true,
        })));
        self.recompute_zoom_bounds();
    }

    /// Add a hillshade overlay. The hillshade pipeline samples the
    /// Map-level shared DEM bind group at draw time. If terrain isn't
    /// registered yet, the pipeline is built against the placeholder
    /// layout — calling `set_terrain_source` later activates real
    /// displacement without recompiling the pipeline.
    pub fn add_hillshade_layer(&mut self, id: impl Into<String>, style: HillshadeStyle) {
        let id = id.into();
        let halo = self
            .terrain
            .as_ref()
            .map(|t| t.cache.halo_px())
            .unwrap_or(0);
        let pipeline = HillshadePipeline::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &self.renderer.terrain_shared.bind_group_layout,
            halo,
        );
        self.layers
            .push(LayerEntry::Hillshade(Box::new(HillshadeLayer {
                id,
                style,
                pipeline,
                fade_in_secs: self.options.fade_in_secs,
                visible: true,
            })));
    }

    pub fn add_vector_layer(
        &mut self,
        id: impl Into<String>,
        source: Arc<dyn VectorTileSource>,
        style: VectorStyle,
    ) {
        let id = id.into();
        let min_zoom = source.min_zoom();
        let max_zoom = source.max_zoom();
        let pipeline = VectorPipeline::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &self.renderer.terrain_shared.bind_group_layout,
        );
        let cache = VectorMeshCache::new(self.device.clone(), self.options.cache_budget_bytes);
        let scene = Scene::with_margin(
            self.cam.camera,
            self.viewport_px,
            min_zoom,
            max_zoom,
            self.options.prefetch_margin_px,
        );
        self.layers.push(LayerEntry::Vector(Box::new(VectorLayer {
            id,
            source,
            style,
            scene,
            pipeline,
            cache,
            fade_in_secs: self.options.fade_in_secs,
            visible: true,
            paint_override: None,
            dash: None,
            width_scale: 1.0,
        })));
        self.recompute_zoom_bounds();
    }

    /// Set (or clear) a vector layer's dash pattern, in screen pixels
    /// `(dash_len, gap_len)`. Returns `false` if no vector layer matches.
    pub fn set_vector_layer_dash(&mut self, id: &str, dash: Option<(f32, f32)>) -> bool {
        self.layers.with_vector_mut(id, |v| v.dash = dash)
    }

    /// Set (or clear) a vector layer's per-frame paint colour override.
    /// `color` is linear RGBA in `[0,1]`. Returns `false` if no vector
    /// layer matches `id`.
    pub fn set_vector_layer_color(&mut self, id: &str, color: Option<[f32; 4]>) -> bool {
        self.layers.with_vector_mut(id, |v| v.paint_override = color)
    }

    /// Set a vector layer's per-frame line-width multiplier (the zoom curve).
    /// `1.0` is the baked width. No-op for fills/text (width_px = 0). Returns
    /// `false` if no vector layer matches `id`.
    pub fn set_vector_layer_width_scale(&mut self, id: &str, scale: f32) -> bool {
        self.layers.with_vector_mut(id, |v| v.width_scale = scale)
    }

    pub fn remove_layer(&mut self, id: &str) {
        // Dropping a layer can widen or narrow the source-derived lock.
        if self.layers.remove(id) {
            self.recompute_zoom_bounds();
        }
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn has_layer(&self, id: &str) -> bool {
        self.layers.contains(id)
    }

    pub fn layer_ids(&self) -> Vec<String> {
        self.layers.ids()
    }

    /// Look up the vector source for a vector layer — useful for the host
    /// when constructing the fetch pump.
    pub fn vector_source(&self, id: &str) -> Option<Arc<dyn VectorTileSource>> {
        self.layers.vector_source(id)
    }

    pub fn raster_source(&self, id: &str) -> Option<Arc<dyn TileSource>> {
        self.layers.raster_source(id)
    }

    /// Terrain DEM source (Map-level since the 3D-terrain refactor;
    /// hillshade layers consume this rather than owning their own
    /// source). Returns `None` until `set_terrain_source` is called.
    pub fn terrain_source(&self) -> Option<Arc<dyn TileSource>> {
        self.terrain.as_ref().map(|t| t.source.clone())
    }

    pub fn vector_style(&self, id: &str) -> Option<VectorStyle> {
        self.layers.vector_style(id)
    }

    // ---- camera + viewport ---------------------------------------------

    pub fn camera(&self) -> Camera {
        self.cam.camera
    }

    pub fn set_camera(&mut self, camera: Camera) {
        // `CameraState::set` sanitises the (possibly host-supplied / NaN) pose,
        // clears any animation, and re-stamps the sticky inset + zoom lock so
        // the camera can't escape the map's accuracy. Then run the scene-sync
        // seam — the Map's side of every camera change.
        self.cam.set(camera);
        self.sync_scenes();
    }

    /// The zoom range the camera is currently locked to.
    pub fn zoom_bounds(&self) -> ZoomBounds {
        self.cam.zoom.active()
    }

    /// Lock the camera's zoom to an explicit range, or pass `None` to track
    /// the active tile sources automatically (the default). The current
    /// camera is clamped into the new range immediately, so a map sitting on
    /// an over-zoomed frame snaps back to the deepest accurate level.
    pub fn set_zoom_bounds(&mut self, bounds: Option<ZoomBounds>) {
        self.cam.zoom.set_manual(bounds);
        self.recompute_zoom_bounds();
    }

    /// Recompute the active zoom lock. With a manual override set, that
    /// wins; otherwise the bounds are the union of every layer source's
    /// declared `[min_zoom, max_zoom]` — the camera can reach the deepest
    /// real tile any layer serves, but no further (past that the raster
    /// upsamples and overlays appear to drift). Re-stamps the camera.
    fn recompute_zoom_bounds(&mut self) {
        let bounds = self.cam.zoom.resolve(self.layers.source_ranges());
        self.cam.camera.set_zoom_bounds(bounds);
        // A clamp may have changed the zoom; keep the scenes in step.
        self.sync_scenes();
    }

    /// Reserve `bottom_px` at the bottom of the viewport (e.g. a sheet). Shifts
    /// the projection's principal point up by `bottom_px/2` for projection,
    /// unprojection, and rendering alike. Persisted across camera changes.
    pub fn set_viewport_inset(&mut self, bottom_px: f64) {
        self.cam.viewport_inset_px = bottom_px.max(0.0);
        self.cam.camera.viewport_inset_px = self.cam.viewport_inset_px;
        self.sync_scenes();
    }

    /// Start an inertial fling from the current camera at `velocity_px`
    /// (screen px/s, the drag-release velocity). The map glides and
    /// decelerates as `tick` is pumped. A near-zero velocity is a no-op.
    pub fn fling(&mut self, velocity_px: (f64, f64)) {
        let f = FlingAnimation::new(self.cam.camera, velocity_px);
        if f.is_finished(Instant::now()) {
            return;
        }
        self.cam.active = Some(ActiveAnim::PanFling(f));
    }

    /// Start a zoom fling (pinch-release momentum) at `zoom_velocity`
    /// (levels/s) about `focus_px`. Glides and settles as `tick` is pumped;
    /// a near-zero velocity is a no-op.
    pub fn zoom_fling(&mut self, zoom_velocity: f64, focus_px: (f64, f64)) {
        let (w, h) = self.viewport_px;
        let z = ZoomFlingAnimation::new(self.cam.camera, zoom_velocity, focus_px, (w as f64, h as f64));
        if z.is_finished(Instant::now()) {
            return;
        }
        self.cam.active = Some(ActiveAnim::ZoomFling(z));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_px = (width, height);
        // Depth + MSAA colour must match the surface size or Metal asserts on
        // the next render; FrameTargets recreates both together (no-op when
        // unchanged or degenerate).
        self.renderer.targets
            .resize(&self.device, self.surface_format, (width, height));
        self.sync_scenes();
    }

    pub fn pan_by_pixels(&mut self, dx: f64, dy: f64) {
        let mut c = self.cam.camera;
        c.pan_by_pixels(dx, dy);
        self.set_camera(c);
    }

    pub fn zoom_around(&mut self, factor: f64, focus_px: (f64, f64)) {
        let (w, h) = self.viewport_px;
        let mut c = self.cam.camera;
        c.zoom_around(factor, focus_px, (w as f64, h as f64));
        self.set_camera(c);
    }

    /// Rotate the bearing by `delta_deg` (two-finger rotate), pivoting on
    /// the screen centre.
    pub fn rotate_by(&mut self, delta_deg: f64) {
        let mut c = self.cam.camera;
        c.rotate_by(delta_deg);
        self.set_camera(c);
    }

    /// Tilt by `delta_deg` (two-finger drag), clamped to the pitch limit.
    pub fn pitch_by(&mut self, delta_deg: f64) {
        let mut c = self.cam.camera;
        c.pitch_by(delta_deg);
        self.set_camera(c);
    }

    /// Rotate the bearing by `delta_deg` about `focus_px` (the gesture
    /// centroid), keeping that pixel anchored.
    pub fn rotate_around(&mut self, delta_deg: f64, focus_px: (f64, f64)) {
        let (w, h) = self.viewport_px;
        let c = self
            .cam
            .camera
            .rotated_around(delta_deg, focus_px, (w as f64, h as f64));
        self.set_camera(c);
    }

    /// Tilt by `delta_deg` about `focus_px`, keeping that pixel anchored.
    pub fn pitch_around(&mut self, delta_deg: f64, focus_px: (f64, f64)) {
        let (w, h) = self.viewport_px;
        let c = self
            .cam
            .camera
            .pitched_around(delta_deg, focus_px, (w as f64, h as f64));
        self.set_camera(c);
    }

    /// Animate a focus-invariant zoom by `factor` about `focus_px` over
    /// `duration` — the smooth double-tap / scroll-wheel zoom. Eases to the
    /// same target [`Camera::zoom_around`] would snap to.
    pub fn zoom_around_animated(&mut self, factor: f64, focus_px: (f64, f64), duration: Duration) {
        let (w, h) = self.viewport_px;
        let target = self
            .cam
            .camera
            .zoomed_around(factor, focus_px, (w as f64, h as f64));
        self.ease_to(target, duration);
    }

    pub fn ease_to(&mut self, target: Camera, duration: Duration) {
        self.cam.active = Some(ActiveAnim::Ease(CameraAnimation::new(
            self.cam.camera,
            target,
            duration,
        )));
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        // CameraState samples the animation + re-stamps the pose; the Map runs
        // the scene-sync seam whenever a frame advanced.
        let (advanced, still_animating) = self.cam.tick(now);
        if advanced {
            self.sync_scenes();
        }
        still_animating
    }

    pub fn is_animating(&self) -> bool {
        if self.cam.active.is_some() {
            return true;
        }
        // Any layer with a fading tile keeps the animation flag set.
        self.layers.iter().any(|l| match l {
            LayerEntry::Raster(r) => r.cache.any_younger_than(r.fade_in_secs),
            LayerEntry::Vector(v) => v.cache.any_younger_than(v.fade_in_secs),
            LayerEntry::Hillshade(_) => false,
        })
    }

    /// Push the current camera + viewport into each layer's scene. Called
    /// whenever the camera changes or layers are added/removed.
    /// Keep the camera eye above the 3D terrain: if tilting/zooming would
    /// drop the eye to (or below) the relief under the look-point, cap the
    /// pitch so it stays a clearance above. Without this, lowering the
    /// pitch over tall, exaggerated terrain puts the camera *inside* the
    /// mesh — you render from under the ground, which looks broken and can
    /// hang the GPU driver. No-op without terrain or before its DEM loads.
    fn clamp_pitch_above_terrain(&mut self) {
        if self.terrain.is_none() {
            return;
        }
        let vp = self.viewport_px;
        let terrain_z = self.ground_world_z(self.cam.camera.center.to_world());
        if terrain_z <= 0.0 {
            return; // sea level / DEM not resident yet → nothing to clear
        }
        let alt = self.cam.camera.altitude_world(vp);
        if alt <= 1e-9 {
            return;
        }
        // Require eye_z = alt·cos(pitch) ≥ terrain_z + clearance.
        let clearance = 0.20 * alt;
        let cos_max = ((terrain_z + clearance) / alt).clamp(0.0, 1.0);
        // Guard the acos domain: a non-finite ratio (e.g. terrain_z/alt NaN)
        // would make pitch NaN → NaN view matrix → GPU hang. Skip if so.
        if !cos_max.is_finite() {
            return;
        }
        let pitch_max = (cos_max.acos().to_degrees() as f64).max(0.0);
        if self.cam.camera.pitch_deg > pitch_max {
            self.cam.camera.pitch_deg = pitch_max;
        }
    }

    fn sync_scenes(&mut self) {
        self.clamp_pitch_above_terrain();
        for l in self.layers.iter_mut() {
            match l {
                LayerEntry::Raster(r) => {
                    r.scene.set_camera(self.cam.camera);
                    r.scene.set_viewport_px(self.viewport_px);
                }
                LayerEntry::Vector(v) => {
                    v.scene.set_camera(self.cam.camera);
                    v.scene.set_viewport_px(self.viewport_px);
                }
                // Hillshade no longer owns a scene — it iterates the
                // shared terrain scene at render time.
                LayerEntry::Hillshade(_) => {}
            }
        }
        if let Some(t) = self.terrain.as_mut() {
            t.scene.set_camera(self.cam.camera);
            t.scene.set_viewport_px(self.viewport_px);
        }
    }

    // ---- coordinate conversion -----------------------------------------

    pub fn screen_to_lng_lat(&self, screen_px: (f64, f64)) -> LatLng {
        let (w, h) = self.viewport_px;
        let world = self.cam.camera.pixel_to_world(screen_px, (w as f64, h as f64));
        world.to_lat_lng()
    }

    pub fn lng_lat_to_screen(&self, lng_lat: LatLng) -> (f64, f64) {
        let world = lng_lat.to_world();
        let (w, h) = self.viewport_px;
        let centre = (w as f64 * 0.5, h as f64 * 0.5);
        // When 3D terrain is loaded, lift the point onto the surface so
        // markers/waypoints/photo-pins sit *on* the relief, not on the
        // flat z=0 plane (which makes them float or sink under tilt).
        // `ground_world_z` is the same displaced-z the terrain mesh uses,
        // so they land exactly where the ground is drawn.
        let proj = if self.terrain.is_some() {
            self.cam.camera
                .world_to_screen_z(world, self.ground_world_z(world), (w as f64, h as f64))
        } else {
            // Fall back to the camera centre for off-screen / behind-camera
            // points so callers (e.g. hit-test on markers) get a deterministic
            // value rather than panicking. The cull happens upstream.
            self.cam.camera.world_to_screen(world, (w as f64, h as f64))
        };
        proj.unwrap_or(centre)
    }

    /// World-space displaced height (z) of the ground at `world`, matching
    /// the terrain mesh: `elevation · meters_to_world · exaggeration`. 0
    /// when no terrain is registered or no covering DEM tile is resident
    /// yet. `meters_to_world` is taken at the camera-centre latitude, the
    /// same value the per-frame mesh displacement uses, so overlays align
    /// with the drawn surface.
    fn ground_world_z(&self, world: WorldPoint) -> f32 {
        let Some(t) = self.terrain.as_ref() else {
            return 0.0;
        };
        // `meters_to_world` at the camera-centre latitude — the same factor the
        // per-frame mesh displacement uses, so overlays land on the surface.
        let lat = self.cam.camera.center.lat.to_radians();
        let m2w = (lat.cos().abs() as f32 / 40_075_017.0).max(1e-12);
        t.ground_world_z(world, m2w)
    }

    // ---- tile orchestration --------------------------------------------

    /// Aggregate pending tiles across all layers. Each entry carries the
    /// layer id so the host can route `ingest_raster`/`ingest_vector_mesh`
    /// back correctly.
    pub fn pending_tiles(&self) -> Vec<PendingTile> {
        let mut out = Vec::new();
        for l in self.layers.iter() {
            match l {
                LayerEntry::Raster(r) if r.visible => {
                    for tile in r.scene.pending_tiles() {
                        out.push(PendingTile::Raster {
                            layer_id: r.id.clone(),
                            tile,
                        });
                    }
                }
                LayerEntry::Vector(v) if v.visible => {
                    for tile in v.scene.pending_tiles() {
                        out.push(PendingTile::Vector {
                            layer_id: v.id.clone(),
                            tile,
                        });
                    }
                }
                // Hillshade no longer fetches its own DEM tiles —
                // the shared terrain source below handles that.
                LayerEntry::Hillshade(_) => {}
                _ => {}
            }
        }
        if let Some(t) = self.terrain.as_ref() {
            for tile in t.scene.pending_tiles() {
                out.push(PendingTile::Terrain { tile });
            }
        }
        out
    }

    fn first_visible_layer_index(&self) -> Option<usize> {
        self.layers.first_visible_index()
    }

    /// Back-compat shim. Hillshade no longer owns its own DEM cache —
    /// the data goes to the shared terrain cache, so this just forwards
    /// to [`Map::ingest_terrain_tile`]. The `layer_id` argument is
    /// kept for source compatibility but ignored.
    pub fn ingest_hillshade(&mut self, _layer_id: &str, tile: TileId, rgba: &[u8], w: u32, h: u32) {
        self.ingest_terrain_tile(tile, rgba, w, h);
    }

    pub fn ingest_raster(&mut self, layer_id: &str, tile: TileId, rgba: &[u8], w: u32, h: u32) {
        for l in self.layers.iter_mut() {
            if let LayerEntry::Raster(r) = l {
                if r.id == layer_id {
                    let evicted = r.cache.insert(tile, rgba, w, h);
                    r.scene.ingest(tile);
                    // Keep the "ingested" set in step with what the cache
                    // actually holds — evicted tiles must become re-
                    // requestable, or they grey out permanently.
                    for e in &evicted {
                        r.scene.un_ingest(e);
                    }
                    return;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ingest_vector_mesh(
        &mut self,
        layer_id: &str,
        tile: TileId,
        mesh: &Mesh,
        labels: Vec<LabelRequest>,
        icons: Vec<IconRequest>,
        interactive: Vec<InteractiveFeature>,
    ) {
        for l in self.layers.iter_mut() {
            if let LayerEntry::Vector(v) = l {
                if v.id == layer_id {
                    let evicted = v.cache.insert(tile, mesh, labels, icons, interactive);
                    v.scene.ingest(tile);
                    for e in &evicted {
                        v.scene.un_ingest(e);
                    }
                    return;
                }
            }
        }
    }

    /// Convenience: tessellate `tile` against the layer's current style on
    /// the calling thread, then ingest. Useful for testing and small hosts
    /// that don't want to run the tessellator off-thread.
    pub fn ingest_vector_tile(&mut self, layer_id: &str, tile_id: TileId, tile: &VectorTile) {
        // Need to extract style+id first, then run tessellate, then store
        // — to avoid overlapping borrows.
        let style_opt = self.layers.iter().find_map(|l| match l {
            LayerEntry::Vector(v) if v.id == layer_id => Some(v.style.clone()),
            _ => None,
        });
        let Some(style) = style_opt else {
            return;
        };
        let out = tessellate::tessellate(tile_id, tile, &style);
        self.ingest_vector_mesh(
            layer_id,
            tile_id,
            &out.mesh,
            out.labels,
            out.icons,
            out.interactive,
        );
    }

    // ---- layer visibility ---------------------------------------------

    /// Toggle a layer on/off by id. Invisible layers still own their
    /// cache (so re-enabling is free) but skip both the render pass
    /// and the `pending_tiles` enumeration — no network traffic while
    /// hidden. Returns `false` if no layer matches the id.
    pub fn set_layer_visibility(&mut self, id: &str, visible: bool) -> bool {
        self.layers.set_visibility(id, visible)
    }

    pub fn layer_visibility(&self, id: &str) -> Option<bool> {
        self.layers.visibility(id)
    }

    /// Set the per-layer fade-in duration. Lets the UI tune the
    /// cross-tile blend feel at runtime — 0 disables fading entirely
    /// (instant tile-pop), higher values smear arrivals into a
    /// longer crossfade.
    pub fn set_layer_fade_in(&mut self, id: &str, secs: f32) -> bool {
        self.layers.set_fade_in(id, secs)
    }

    // ---- fonts ---------------------------------------------------------

    /// Register a fallback font face (owned bytes) for scripts the bundled
    /// default doesn't cover (CJK, Arabic, …). Returns `false` if the bytes
    /// don't parse. Faces added earlier win where they have coverage, so
    /// the bundled Latin face is always preferred for Latin text.
    pub fn add_fallback_font(&mut self, bytes: Vec<u8>) -> bool {
        self.renderer.text_pipeline.add_fallback_face(bytes)
    }

    // ---- markers -------------------------------------------------------

    pub fn add_marker(&mut self, marker: Marker) -> MarkerId {
        self.markers.add(marker)
    }

    pub fn remove_marker(&mut self, id: MarkerId) {
        self.markers.remove(id);
    }

    pub fn clear_markers(&mut self) {
        self.markers.clear();
    }

    pub fn markers(&self) -> &[Marker] {
        self.markers.all()
    }

    // ---- hit testing ---------------------------------------------------

    pub fn hit_test(&self, screen_px: (f64, f64), tolerance_px: f64) -> Vec<HitResult> {
        let mut out: Vec<HitResult> = Vec::new();

        // Markers first (top z-order, newest-first within markers).
        for hit in self
            .markers
            .hit(screen_px, tolerance_px, |ll| self.lng_lat_to_screen(ll))
        {
            out.push(HitResult::Marker(hit));
        }

        // Then vector-tile features, top-most layer first.
        let camera = self.cam.camera;
        let ppw = camera.pixels_per_world_unit();
        let centre = camera.center.to_world();
        let (vw, vh) = self.viewport_px;
        let click_world = (
            centre.x + (screen_px.0 - vw as f64 * 0.5) / ppw,
            centre.y + (screen_px.1 - vh as f64 * 0.5) / ppw,
        );
        let world_tol = tolerance_px / ppw;

        for layer in self.layers.iter().rev() {
            let LayerEntry::Vector(v) = layer else {
                continue;
            };
            for tile in v.scene.visible_tiles() {
                let Some(entry) = v.cache.peek(tile) else {
                    continue;
                };
                // Project the click into this tile's local coords once,
                // then ask the spatial index for the small candidate
                // set instead of walking every feature.
                let n = (1u64 << tile.z) as f64;
                // Tile features all share an extent — derive it from
                // the index (constructed with the tile's extent).
                let extent_local = if let Some(f) = entry.interactive.first() {
                    f.extent as f64
                } else {
                    continue;
                };
                let local = (
                    (click_world.0 * n - tile.x as f64) * extent_local,
                    (click_world.1 * n - tile.y as f64) * extent_local,
                );
                let local_tol = world_tol * n * extent_local;
                let local_tol_sq = local_tol * local_tol;

                for &feature_idx in entry.hit_index.query(local.0, local.1) {
                    let hit = &entry.interactive[feature_idx as usize];
                    if !geometry_hit(&hit.feature.geometry, local, local_tol_sq) {
                        continue;
                    }
                    let already = out.iter().any(|r| {
                        matches!(r, HitResult::Feature(f)
                            if f.feature_id == hit.feature.id
                                && f.source_layer == hit.source_layer
                                && f.layer_id == v.id)
                    });
                    if already {
                        continue;
                    }
                    out.push(HitResult::Feature(HitFeature {
                        layer_id: v.id.clone(),
                        tile_id: tile,
                        source_layer: hit.source_layer.clone(),
                        feature_id: hit.feature.id,
                        geom_type: hit.feature.geom_type,
                        properties: hit.feature.properties.clone(),
                    }));
                }
            }
        }
        out
    }

    // ---- render --------------------------------------------------------

    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let started = Instant::now();

        // ---- Master finite gate -----------------------------------------
        // The single chokepoint every GPU upload sits behind. If the camera
        // can't produce a finite view-projection — a NaN/Inf slipped past the
        // input guards, or steep-pitch/zoom math degenerated — DON'T encode
        // the frame: a mobile driver hangs on a NaN matrix where desktop Metal
        // shrugs. Drop the frame (the last presented surface stays on screen)
        // and record it, so a host watching `last_frame_metrics` can see the
        // map is shedding frames rather than silently wedging.
        let gate_vp = self.cam.camera.view_projection_matrix(self.viewport_px);
        if !crate::render::mat4_is_finite(&gate_vp)
            || !self.cam.camera.pitch_deg.is_finite()
            || !self.cam.camera.zoom.is_finite()
        {
            log::warn!(
                "turbomap: dropping non-finite frame (pitch={}, zoom={}, vp_finite={})",
                self.cam.camera.pitch_deg,
                self.cam.camera.zoom,
                crate::render::mat4_is_finite(&gate_vp),
            );
            self.last_frame_metrics = FrameMetrics {
                cpu_time: started.elapsed(),
                frame_dropped: true,
                ..Default::default()
            };
            return;
        }

        if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
            ts.try_drain();
            ts.begin(encoder);
        }
        // The frame is recorded as exactly ONE render pass — on
        // tile-based mobile GPUs every extra pass costs a full
        // framebuffer load/store. Three phases:
        //   A. prepare: every visible layer (in order) does its CPU
        //      work — uniform/instance uploads, batch building, LRU
        //      touches — and returns a draw list.
        //   B. pick the frame clear colour (replicating the old
        //      "first visible layer clears" semantics).
        //   C. begin the single pass and replay the prepared draw
        //      lists in order: layer geometry, then text per visible
        //      vector layer, then markers.
        //
        // All per-frame render globals — metres-to-world, the sun-lit
        // atmosphere, the aerial-perspective haze, the sky uniform and the
        // raster/vector terrain configs — derived once from the camera +
        // sun + terrain. See [`RenderFrame`].
        let frame = RenderFrame::build(
            &self.cam.camera,
            self.viewport_px,
            self.effective_sun(),
            TerrainFrameInputs {
                present: self.terrain.is_some(),
                exaggeration: self
                    .terrain
                    .as_ref()
                    .map(|t| t.options.exaggeration)
                    .unwrap_or(1.0),
                encoding: self
                    .terrain
                    .as_ref()
                    .map(|t| t.options.encoding)
                    .unwrap_or(crate::dem::DemEncoding::MapboxRgb),
                halo_px: self.terrain.as_ref().map(|t| t.cache.halo_px()).unwrap_or(0),
            },
        );
        let first_visible = self.first_visible_layer_index();

        // ---- Phase A: prepare ------------------------------------
        // Split-borrow the parts we need so the loop can mutably
        // borrow `self.layers` while still passing references into
        // `self.terrain`. `terrain_cell` is an Option<&mut Terrain>
        // we reborrow on a per-pipeline basis.
        let prepare_started = Instant::now();
        let mut tiles_drawn = 0usize;
        let mut terrain_cell = self.terrain.as_mut();
        let mut prepared_layers: Vec<(usize, PreparedLayer)> =
            Vec::with_capacity(self.layers.len());
        // One prepared text item per *visible vector layer*, in layer
        // order — preserving the old one-text-pass-per-vector-layer
        // semantics (per-layer label collision sets).
        let mut prepared_text: Vec<PreparedText> = Vec::new();
        let mut prepared_icons: Vec<PreparedIcons> = Vec::new();
        self.renderer.text_pipeline.begin_frame();
        self.renderer.icon_pipeline.begin_frame();
        for (i, layer) in self.layers.iter_mut().enumerate() {
            match layer {
                LayerEntry::Raster(r) if r.visible => {
                    let p = r.pipeline.prepare(
                        &r.scene,
                        &mut r.cache,
                        terrain_cell.as_deref_mut().map(|t| &mut t.cache),
                        frame.raster_terrain_cfg,
                        r.fade_in_secs,
                    );
                    tiles_drawn += r.scene.visible_tiles().len();
                    prepared_layers.push((i, PreparedLayer::Raster(p)));
                }
                LayerEntry::Vector(v) if v.visible => {
                    let p = v.pipeline.prepare(
                        &v.scene,
                        &mut v.cache,
                        v.fade_in_secs,
                        v.paint_override,
                        v.dash,
                        v.width_scale,
                        frame.vec_terrain_zscale,
                        frame.vec_terrain_encoding,
                        frame.vec_terrain_halo_uv,
                    );
                    tiles_drawn += v.scene.visible_tiles().len();
                    prepared_layers.push((i, PreparedLayer::Vector(p)));
                    // Labels come from visible vector layers only —
                    // text on top of a hidden vector layer would look
                    // orphaned. Text runs *before* icons so a POI marker's
                    // dot can be gated on its label surviving collision (dot
                    // + label cull as a unit).
                    prepared_text.push(self.renderer.text_pipeline.prepare(
                        &v.scene,
                        &mut v.cache,
                        self.options.pixel_ratio,
                    ));
                    prepared_icons.push(self.renderer.icon_pipeline.prepare(
                        &v.scene,
                        &mut v.cache,
                        self.renderer.text_pipeline.placed_marker_anchors(),
                    ));
                }
                LayerEntry::Hillshade(h) if h.visible => {
                    // Hillshade reads from the shared TerrainCache.
                    // Without terrain registered the layer is a
                    // no-op — the demo always pairs hillshade with
                    // terrain, but be defensive. Reuse the already-
                    // borrowed `terrain_cell` rather than touching
                    // `self.terrain` again (which the loop has
                    // mutably borrowed for the whole iteration).
                    if let Some(t) = terrain_cell.as_deref_mut() {
                        let p = h.pipeline.prepare(
                            &t.scene,
                            &mut t.cache,
                            h.style,
                            h.fade_in_secs,
                            frame.meters_to_world,
                        );
                        prepared_layers.push((i, PreparedLayer::Hillshade(p)));
                    }
                }
                _ => {}
            }
        }
        self.renderer.text_pipeline.finish_frame();
        self.renderer.icon_pipeline.finish_frame();

        // Markers last. Pick any scene that's around — they all sync
        // from the same camera. Prefer the first raster/vector layer
        // (they have their own scenes); fall back to terrain;
        // otherwise build a one-off from the Map's state.
        let prepared_markers = if self.markers.is_empty() {
            None
        } else {
            let p = if let Some(scene) = self.layers.marker_scene() {
                self.renderer.marker_pipeline.prepare(scene, self.markers.all())
            } else if let Some(t) = self.terrain.as_ref() {
                self.renderer.marker_pipeline.prepare(&t.scene, self.markers.all())
            } else {
                // No layers — build a one-off Scene from the Map's state.
                let scene = Scene::with_margin(self.cam.camera, self.viewport_px, 0, 22, 0);
                self.renderer.marker_pipeline.prepare(&scene, self.markers.all())
            };
            Some(p)
        };
        let prepare_time = prepare_started.elapsed();

        // ---- Phase B: frame clear colour -------------------------
        // Replicates the old "first visible layer clears" semantics:
        // a vector layer clears to its style background; raster and
        // hillshade (and an empty layer stack) clear to the shared
        // backdrop colour.
        let clear = match first_visible.map(|i| &self.layers[i]) {
            Some(LayerEntry::Vector(v)) => {
                let c = srgb_color_to_linear_f32(v.style.background);
                wgpu::Color {
                    r: c[0] as f64,
                    g: c[1] as f64,
                    b: c[2] as f64,
                    a: c[3] as f64,
                }
            }
            _ => BACKGROUND_CLEAR,
        };

        // ---- Phase C: the single render pass ---------------------
        let pass_started = Instant::now();
        {
            let terrain_cache = self.terrain.as_ref().map(|t| &t.cache);
            let placeholder_dem = &self.renderer.terrain_shared.placeholder_bind_group;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("turbomap-frame-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    // Render multisampled, then resolve down to the surface.
                    view: self.renderer.targets.color_view(),
                    resolve_target: Some(target),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        // The multisampled buffer is transient — we only keep
                        // the resolved surface, so it needn't be stored.
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.renderer.targets.depth_view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Atmosphere first, behind everything: a depth-disabled
            // full-screen triangle. Terrain/vectors overdraw it, so it
            // shows only at the horizon when the camera is tilted.
            if let Some(g) = &frame.sky_globals {
                self.renderer.sky_pipeline.draw(g, &mut pass);
            }

            for (i, prepared) in &prepared_layers {
                match (&self.layers[*i], prepared) {
                    (LayerEntry::Raster(r), PreparedLayer::Raster(p)) => {
                        r.pipeline
                            .draw(p, &r.cache, terrain_cache, placeholder_dem, &mut pass);
                    }
                    (LayerEntry::Vector(v), PreparedLayer::Vector(p)) => {
                        v.pipeline.draw(p, &v.cache, terrain_cache, placeholder_dem, &mut pass);
                    }
                    (LayerEntry::Hillshade(h), PreparedLayer::Hillshade(p)) => {
                        if let Some(tc) = terrain_cache {
                            h.pipeline.draw(p, tc, &mut pass);
                        }
                    }
                    // prepare tagged each item with its own layer's
                    // index, and `self.layers` is not mutated between
                    // the phases.
                    _ => unreachable!("prepared layer kind mismatch"),
                }
            }

            // Icons under labels, so a name centred on a shield reads on top.
            for p in &prepared_icons {
                self.renderer.icon_pipeline.draw(p, &mut pass);
            }

            for p in &prepared_text {
                self.renderer.text_pipeline.draw(p, &mut pass);
            }

            if let Some(p) = &prepared_markers {
                self.renderer.marker_pipeline.draw(p, &mut pass);
            }
        }
        let pass_time = pass_started.elapsed();

        // Weather-cloud overlay: a separate, single-sampled, depth-less
        // fullscreen composite over the already-resolved surface. It can't
        // join the MSAA frame pass above (sample-count / depth mismatch),
        // so it pays one extra fullscreen pass — acceptable for an overlay.
        let clouds_started = Instant::now();
        let cam = self.cam.camera;
        let (vw, vh) = (self.viewport_px.0 as f64, self.viewport_px.1 as f64);
        if let Some(c) = &mut self.clouds {
            if c.enabled {
                c.params.resolution = [vw as f32, vh as f32];
                // World-lock: map each screen pixel into the radar's geo box so
                // the clouds pan + zoom with the terrain. Sample the real camera
                // projection at three viewport corners (exact for top-down,
                // bearing/inset-correct via `pixel_to_world`) → screen-uv→field-uv
                // affine. No geo box yet → identity (screen-locked legacy look).
                match c.radar_geo {
                    Some((min, max)) => {
                        let (o, dx, dy) = cloud_field_affine(cam, (vw, vh), min, max);
                        c.params.field_uv_origin = o;
                        c.params.field_uv_dx = dx;
                        c.params.field_uv_dy = dy;

                        // Pitch-3D camera-ray parallax: feed the real camera ray
                        // so the march rakes through the world-locked volume and
                        // reveals the puff sides. WIRED but GATED OFF — validated
                        // via the desktop debug scene (turbomap-app snapshot,
                        // `CLOUD_DEBUG_VIEW=light --pitch 25/45`) that the
                        // camera-ray branch of `render_volume` collapses the
                        // lighting/shadow contrast at moderate tilt: the Light AOV
                        // goes near-uniform pale and the final composite washes to
                        // flat white — worse than the flat look it replaces. The
                        // parallax *shift* itself is sane (bounded, zero at pitch
                        // 0); the bug is in the camera-ray lighting integration.
                        // Until that's fixed, the flat top-down field stays
                        // world-locked under tilt (clouds pan/zoom correctly, just
                        // no side-reveal). Flip to `true` to re-enable.
                        const ENABLE_CAMERA_RAY: bool = true;
                        if ENABLE_CAMERA_RAY && cam.pitch_deg > 0.5 {
                            let sx = (max.x - min.x).abs().max(1e-12);
                            let sy = (max.y - min.y).abs().max(1e-12);
                            c.params.world_to_field = [(1.0 / sx) as f32, (1.0 / sy) as f32];
                            // Slab altitude in world (mercator) units, scaled to
                            // the box so the parallax magnitude is zoom-stable:
                            // ~`CLOUD_SLAB_FRAC` of the box gives a few-puff rake
                            // at a moderate tilt.
                            c.params.cloud_alt_base = 0.0;
                            c.params.cloud_alt_top = (CLOUD_SLAB_FRAC * sy) as f32;
                            let vp = cam.view_projection_matrix(self.viewport_px);
                            let inv = glam::Mat4::from_cols_array_2d(&vp).inverse().to_cols_array_2d();
                            // Guard the inverse: a degenerate VP → NaN → the
                            // cloud raymarch would hang the driver. Fall back to
                            // the world-locked flat field (no camera ray).
                            if crate::render::mat4_is_finite(&inv) {
                                c.params.inv_view_proj = inv;
                                c.params.use_camera_ray = true;
                            } else {
                                c.params.use_camera_ray = false;
                            }
                        } else {
                            c.params.use_camera_ray = false;
                        }
                    }
                    None => {
                        c.params.field_uv_origin = [0.0, 0.0];
                        c.params.field_uv_dx = [1.0, 0.0];
                        c.params.field_uv_dy = [0.0, 1.0];
                        c.params.use_camera_ray = false;
                    }
                }

                // Half-res cloud buffer: the volumetric march is the cost, so
                // render it at half resolution and upscale-composite — keeps
                // the live overlay within budget on mobile / software GPUs.
                c.scene
                    .render_overlay_downsampled(&self.queue, encoder, target, &c.params, 2);
            }
        }

        if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
            ts.end(encoder);
        }
        let clouds_time = clouds_started.elapsed();
        // sky + each drawn layer + icons + text + markers = the pass's draw calls.
        let draw_calls = frame.sky_globals.is_some() as usize
            + prepared_layers.len()
            + prepared_icons.len()
            + prepared_text.len()
            + prepared_markers.is_some() as usize;
        self.last_frame_metrics = FrameMetrics {
            cpu_time: started.elapsed(),
            frame_dropped: false,
            phases: PhaseTimings {
                prepare: prepare_time,
                pass: pass_time,
                clouds: clouds_time,
            },
            gpu_time: self.renderer.gpu_timestamps.as_ref().and_then(|t| {
                if t.last_duration_ns == 0 {
                    None
                } else {
                    Some(Duration::from_nanos(t.last_duration_ns))
                }
            }),
            layer_count: self.layers.len(),
            marker_count: self.markers.len(),
            visible_layers: prepared_layers.len(),
            draw_calls,
            tiles_drawn,
            layers: self
                .layers
                .iter()
                .map(|l| match l {
                    LayerEntry::Raster(r) => LayerMetrics {
                        id: r.id.clone(),
                        kind: LayerKind::Raster,
                        cache: r.cache.stats(),
                    },
                    LayerEntry::Vector(v) => LayerMetrics {
                        id: v.id.clone(),
                        kind: LayerKind::Vector,
                        // VectorMeshCache has its own stats; map to
                        // the common shape so callers can iterate
                        // uniformly. We fill the fields it tracks
                        // (entries + bytes) and leave hit-rate at 0
                        // until the vector cache grows counters too.
                        cache: CacheStats {
                            entries: v.cache.len(),
                            bytes_used: v.cache.bytes_used(),
                            budget_bytes: v.cache.budget_bytes(),
                            ..Default::default()
                        },
                    },
                    LayerEntry::Hillshade(h) => LayerMetrics {
                        id: h.id.clone(),
                        kind: LayerKind::Hillshade,
                        // Hillshade no longer owns its cache — borrow
                        // the shared terrain stats. Empty if no
                        // terrain registered.
                        cache: self
                            .terrain
                            .as_ref()
                            .map(|t| t.cache.stats())
                            .unwrap_or_default(),
                    },
                })
                .collect(),
        };
    }

    /// Metrics for the most recent `render()` call. Includes GPU
    /// wall time when the device supports `Features::TIMESTAMP_QUERY`
    /// (one-frame delay because the readback is async).
    pub fn last_frame_metrics(&self) -> &FrameMetrics {
        &self.last_frame_metrics
    }

    /// Call this **after** `queue.submit(...)` of the frame's encoder.
    /// Arms the async readback for the timestamps written in
    /// `render()` so the *next* frame's `try_drain` can pick them up.
    /// A no-op when GPU timestamps aren't supported on the device.
    /// Safe to call every frame even if you don't care about GPU
    /// timing — the negligible cost is two atomic-bool flips.
    pub fn after_submit(&mut self) {
        if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
            ts.kick_async();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Raster,
    Vector,
    Hillshade,
}

#[derive(Debug, Clone)]
pub struct LayerMetrics {
    pub id: String,
    pub kind: LayerKind,
    pub cache: CacheStats,
}

/// CPU wall-time of each render phase, measured every frame (a handful of
/// `Instant` deltas — always on, negligible cost). Lets any host see *where*
/// a frame's time goes, not just the total: a slow tilt is usually `prepare`
/// (more visible tiles + label layout) or `clouds` (the extra fullscreen
/// raymarch pass), and telling them apart is the whole point of profiling
/// "at all points".
#[derive(Debug, Clone, Copy, Default)]
pub struct PhaseTimings {
    /// Phase A — CPU prep of every visible layer plus the text, icon and
    /// marker pipelines (uniform/instance uploads, batch building, layout).
    pub prepare: Duration,
    /// Phase C — encoding the single frame render pass: sky, then each
    /// layer's geometry, then icons, text and markers.
    pub pass: Duration,
    /// The separate weather-cloud overlay pass. `Duration::ZERO` when clouds
    /// are disabled or absent.
    pub clouds: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct FrameMetrics {
    pub cpu_time: Duration,
    /// `true` when the renderer's finite gate rejected this frame (a
    /// non-finite camera/view-projection) and skipped encoding it entirely —
    /// the previous surface stays on screen. A host seeing this set is
    /// shedding frames, not wedged; sustained drops point at an upstream
    /// camera-math bug. See `Map::render`.
    pub frame_dropped: bool,
    /// Per-phase CPU breakdown of `cpu_time`. See [`PhaseTimings`].
    pub phases: PhaseTimings,
    /// GPU wall time for the most recent COMPLETED frame's passes.
    /// `None` when the device lacks `Features::TIMESTAMP_QUERY`, or
    /// when no frame has finished yet (the readback arrives one
    /// frame after submit). Stable between frames once populated.
    pub gpu_time: Option<Duration>,
    pub layer_count: usize,
    pub marker_count: usize,
    /// Load this frame, for performance correlation: how many layers were
    /// actually drawn (visible), the total draw-call count (sky + layers +
    /// icons + text + markers), and the total visible tiles across all drawn
    /// layers. These are the levers that move `cpu_time` as the camera tilts.
    pub visible_layers: usize,
    pub draw_calls: usize,
    pub tiles_drawn: usize,
    pub layers: Vec<LayerMetrics>,
}

fn srgb_color_to_linear_f32(c: Color) -> [f32; 4] {
    // Canonical decode lives on Color; backgrounds use the same contract
    // as vertex/text/marker colours.
    c.to_linear_f32()
}

#[cfg(test)]
mod tests {
    //! Value boundary: the Map's layer-stack API. Tests don't construct
    //! GPU resources; they exercise the layer add/remove/lookup logic and
    //! marker handling on a stub `Map`-ish shell that doesn't actually
    //! render. (The wgpu-bound paths are covered by the smoke test.)
    //!
    //! We can't build a real `Map` without a wgpu Device, so the tests
    //! here cover the parts of the API that don't need GPU: `MarkerId`
    //! generation and `PendingTile` shapes.
    use super::*;

    // Evaluate the cloud field-uv affine at the screen position where world
    // point `p` currently sits (top-down ortho world→screen), so we can assert
    // a fixed world point maps to the same field-uv under any camera.
    fn field_uv_of(
        cam: Camera,
        vp: (f64, f64),
        min: WorldPoint,
        max: WorldPoint,
        p: WorldPoint,
    ) -> (f64, f64) {
        let (o, dx, dy) = cloud_field_affine(cam, vp, min, max);
        let ppw = cam.pixels_per_world_unit();
        let c = cam.center.to_world();
        let uvx = 0.5 + (p.x - c.x) * ppw / vp.0;
        let uvy = 0.5 + (p.y - c.y) * ppw / vp.1;
        (
            o[0] as f64 + uvx * dx[0] as f64 + uvy * dy[0] as f64,
            o[1] as f64 + uvx * dx[1] as f64 + uvy * dy[1] as f64,
        )
    }

    #[test]
    fn cloud_field_world_locks_under_pan() {
        // Radar box over southern Norway (min = NW corner, max = SE corner).
        let vp = (1080.0, 2400.0);
        let min = LatLng::new(63.0, 8.0).to_world();
        let max = LatLng::new(60.0, 12.0).to_world();
        // A fixed world point (a peak the cloud should stay glued to).
        let peak = LatLng::new(61.5, 10.0).to_world();
        // Its field-uv is camera-independent by construction: (peak - min)/span.
        let want = (
            (peak.x - min.x) / (max.x - min.x),
            (peak.y - min.y) / (max.y - min.y),
        );

        let cam_a = Camera::new(LatLng::new(61.5, 10.0), 9.0); // peak at centre
        let cam_b = Camera::new(LatLng::new(61.2, 10.6), 9.0); // panned SE, same zoom
        let a = field_uv_of(cam_a, vp, min, max, peak);
        let b = field_uv_of(cam_b, vp, min, max, peak);

        // Same world point → same field-uv under both cameras (world-locked),
        // and equal to the geo-derived target.
        assert!(
            (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4,
            "pan: {a:?} vs {b:?}"
        );
        assert!(
            (a.0 - want.0).abs() < 1e-4 && (a.1 - want.1).abs() < 1e-4,
            "geo: {a:?} vs {want:?}"
        );
    }

    #[test]
    fn cloud_field_world_locks_under_bearing() {
        // Rotating the map (bearing) must keep the field geo-pinned: the screen
        // centre always shows the camera centre, so its field-uv must equal the
        // camera centre's geo position regardless of bearing. `pixel_to_world`
        // carries the rotation, so the corner-sampled affine handles it.
        let vp = (1080.0, 2400.0);
        let min = LatLng::new(63.0, 8.0).to_world();
        let max = LatLng::new(60.0, 12.0).to_world();
        let centre = LatLng::new(61.5, 10.0);
        let cw = centre.to_world();
        let want = (
            (cw.x - min.x) / (max.x - min.x),
            (cw.y - min.y) / (max.y - min.y),
        );
        for bearing in [0.0, 30.0, 90.0, 215.0] {
            let cam = Camera::new(centre, 10.0).with_bearing(bearing);
            let (o, dx, dy) = cloud_field_affine(cam, vp, min, max);
            // field_uv at screen centre (uv = 0.5, 0.5).
            let cx = o[0] as f64 + 0.5 * dx[0] as f64 + 0.5 * dy[0] as f64;
            let cy = o[1] as f64 + 0.5 * dx[1] as f64 + 0.5 * dy[1] as f64;
            // Tolerance ~1e-3 of the box (≈a few hundred m over a ~300 km box):
            // the residual is the perspective projection's tiny non-linearity at
            // pitch 0 (corner-sampling an affine), not a geo-lock error — well
            // below a pixel on screen.
            assert!(
                (cx - want.0).abs() < 2e-3 && (cy - want.1).abs() < 2e-3,
                "bearing {bearing}: centre field-uv ({cx},{cy}) != geo {want:?}"
            );
        }
    }

    #[test]
    fn cloud_field_scales_with_zoom() {
        // Zooming IN must shrink the field-uv the viewport spans (fewer, bigger
        // puffs) — the world-locked zoom behaviour.
        let vp = (1080.0, 2400.0);
        let min = LatLng::new(63.0, 8.0).to_world();
        let max = LatLng::new(60.0, 12.0).to_world();
        let centre = LatLng::new(61.5, 10.0);
        let (_, dx_out, _) = cloud_field_affine(Camera::new(centre, 8.0), vp, min, max);
        let (_, dx_in, _) = cloud_field_affine(Camera::new(centre, 11.0), vp, min, max);
        assert!(
            dx_in[0].abs() < dx_out[0].abs() * 0.5,
            "zoom-in should shrink field span: in={} out={}",
            dx_in[0],
            dx_out[0]
        );
    }

    #[test]
    fn marker_id_zero_means_auto_assign() {
        // Constructing a marker with id=0 should trigger auto-assignment.
        // We assert the API shape without actually building a Map.
        let m = Marker {
            id: MarkerId(0),
            lng_lat: LatLng::new(0.0, 0.0),
            radius_px: 8.0,
            color: Color::rgb(0, 0, 0),
            data: std::collections::HashMap::new(),
        };
        // The struct is well-formed; behaviour of auto-assignment is
        // covered by smoke-test usage. This test exists to pin the API.
        assert_eq!(m.id, MarkerId(0));
    }

    #[test]
    fn pending_tile_carries_layer_id_for_routing() {
        // Round-trip: a host receives a PendingTile, extracts layer_id +
        // tile, and routes to the right ingest method. Test the enum
        // shape stays stable.
        let raster_pt = PendingTile::Raster {
            layer_id: "basemap".into(),
            tile: TileId::new(5, 0, 0),
        };
        let vector_pt = PendingTile::Vector {
            layer_id: "overlay".into(),
            tile: TileId::new(11, 1054, 590),
        };
        match raster_pt {
            PendingTile::Raster { layer_id, tile } => {
                assert_eq!(layer_id, "basemap");
                assert_eq!(tile.z, 5);
            }
            _ => panic!("expected raster variant"),
        }
        match vector_pt {
            PendingTile::Vector { layer_id, tile } => {
                assert_eq!(layer_id, "overlay");
                assert_eq!(tile.z, 11);
            }
            _ => panic!("expected vector variant"),
        }
    }

    #[test]
    fn default_options_are_sensible() {
        let o = MapOptions::default();
        assert!(o.cache_budget_bytes >= 64 * 1024 * 1024);
        assert!(o.prefetch_margin_px > 0);
        // Fade is deliberately disabled (0.0) while the flicker
        // diagnosis is open — see the MapOptions::default comment.
        // Anything inside [0, 1) is sane; sub-zero or 1 s+ is not.
        assert!((0.0..1.0).contains(&o.fade_in_secs));
    }
}
