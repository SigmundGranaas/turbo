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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use web_time::Instant;

use crate::{
    camera::{Camera, CameraAnimation, FlingAnimation, ZoomBounds, ZoomFlingAnimation, ZoomLock},
    environment::Environment,
    error::MapError,
    geo::{LatLng, WorldPoint},
    hit::geometry_hit,
    lighting::Lighting,
    markers::MarkerManager,
    render::{
        ao::{AoField, AoKey},
        cache::CacheStats,
        custom::{CustomFrameCtx, CustomLayer, CustomLayerInit, CustomPhase},
        floor::FloorPipeline,
        frame::{RenderFrame, TerrainFrameInputs},
        gpu_timestamps::GpuTimestamps,
        graph::{
            DrawList, FrameGraph, FramePhase, MsaaAttachments, PassDesc, PassMask, PassTiming, Res,
        },
        hillshade::{HillshadePipeline, PreparedHillshade},
        icon::{IconPipeline, PreparedIcons},
        marker::MarkerPipeline,
        raster::{PreparedRaster, RasterPipeline},
        route::{build_tube, RoutePipeline, RouteVertex},
        shadow::{ShadowMap, HEIGHT_DIM},
        sky::SkyPipeline,
        targets::FrameTargets,
        terrain::{Terrain, TerrainCache, TerrainOptions, TerrainShared},
        text::{PreparedText, TextPipeline},
        vector::{PreparedVector, VectorPipeline},
        vector_cache::VectorMeshCache,
        TextureCache, BACKGROUND_CLEAR,
    },
    scene::Scene,
    simulation::SimulationSystem,
    source::TileSource,
    style::{Color, HillshadeStyle, VectorStyle},
    subsystem::{BudgetReport, DebugActivation, DebugViewDesc, Subsystem},
    sun::SunPosition,
    surface::Surface,
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
    /// Sim-driven clock (plan E2): the atmosphere's `tick` derives the
    /// drift clock, the one sun and the radar crossfade from the frame's
    /// `Environment`. `set_cloud_time` flips this off — the host owns the
    /// clock then (the time-slider scrub path).
    sim: bool,
    /// Frame-clock stamp of the last slot-1 radar ingest; the sim advects
    /// (crossfades) toward the new frame from here. Pure function of
    /// (ingest time, clock) — deterministic under a pinned clock.
    advect_start: Option<f32>,
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

/// Result of [`Map::screen_to_ground_lng_lat`]: where a screen ray meets the
/// terrain. `world_z` is the displaced surface height at the hit (same units as
/// the mesh); `hit_terrain` is false when the result came from the flat-plane
/// fallback (no terrain / top-down / sky / DEM not resident).
#[derive(Debug, Clone, Copy)]
pub struct GroundHit {
    pub lng_lat: LatLng,
    pub world_z: f32,
    pub hit_terrain: bool,
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
            // Per-layer GPU texture budget (a CEILING — memory is used only as
            // tiles actually resolve, not pre-allocated). Up to THREE live caches
            // (raster + vector + terrain DEM); raster is the one that fills. This
            // MUST exceed the desired working set with real headroom for
            // pan/revisit history, or the LRU evicts tiles the moment they leave
            // view — so looking back re-fetches (slow) and, when the desired set
            // overflows the cache, the resident set churns frame-to-frame and the
            // best-available resolver flip-flops coarse↔fine (visible flicker even
            // when the camera is still). The pitched LOD desired set is ~260 tiles
            // (lod::MAX_TILES + overview); 80 MiB (~240 tiles) couldn't hold it.
            // 512 MiB holds ~1500 raster tiles: the full set plus a deep session
            // history, well within a modern phone's GPU budget. The
            // `capacity` module PROVES `desired ≤ cache` from this at compile
            // time, so the thrash above can't regress silently.
            cache_budget_bytes: crate::capacity::CACHE_BUDGET_BYTES,
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
/// One fetch a plan-driven host should start: the transport payload plus the
/// attempt's identity ([`turbomap_world::RequestId`]) that deliveries,
/// failures, and cancellations are keyed by.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub id: turbomap_world::RequestId,
    pub fetch: PendingTile,
}

/// One streaming step (plan slice B3.2): what to start, what to abort. See
/// [`Map::streaming_plan`].
#[derive(Debug, Clone, Default)]
pub struct StreamingPlan {
    /// Priority-ordered, budget-truncated fetches to begin.
    pub start: Vec<FetchRequest>,
    /// Live attempts the camera moved away from — abort the transport and
    /// report [`Map::fetch_cancelled`]. The verb the pull-only contract
    /// never had: without it, a fast pan leaves stale fetches decoding to
    /// completion while the new viewport waits behind them.
    pub cancel: Vec<turbomap_world::RequestId>,
}

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
    Custom(Box<CustomLayerHolder>),
}

/// Per-layer output of the render prep phase (Phase A). Tagged with
/// the layer's index in `Map::layers` so the draw phase can pair each
/// prepared item back up with an *immutable* borrow of its layer.
enum PreparedLayer {
    Raster(PreparedRaster),
    Vector(PreparedVector),
    Hillshade(PreparedHillshade),
    /// Prepared state lives inside the `CustomLayer` impl (its `prepare`
    /// already ran); this tag just keeps the layer in the draw pairing.
    Custom,
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

/// A host/engine-supplied custom render layer in the stack (plan D4). The
/// boxed impl owns its whole GPU state; `kind` is the registry name the
/// scene IR bound it by (surfaced in inspect).
struct CustomLayerHolder {
    id: String,
    kind: String,
    layer: Box<dyn CustomLayer>,
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
            LayerEntry::Custom(c) => &c.id,
        }
    }

    fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|l| Self::id_of(l) == id)
    }

    fn ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|l| Self::id_of(l).to_string())
            .collect()
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
            LayerEntry::Custom(c) if c.id == id => Some(c.visible),
            _ => None,
        })
    }

    fn set_visibility(&mut self, id: &str, visible: bool) -> bool {
        for layer in self.entries.iter_mut() {
            let (lid, lvis): (&str, &mut bool) = match layer {
                LayerEntry::Raster(r) => (&r.id, &mut r.visible),
                LayerEntry::Vector(v) => (&v.id, &mut v.visible),
                LayerEntry::Hillshade(h) => (&h.id, &mut h.visible),
                LayerEntry::Custom(c) => (&c.id, &mut c.visible),
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
                // Custom layers own their look entirely; fade-in is a
                // tile-layer concept and a silent no-op here.
                LayerEntry::Custom(_) => continue,
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
            LayerEntry::Custom(c) => c.visible,
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
                LayerEntry::Hillshade(_) | LayerEntry::Custom(_) => None,
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
            LayerEntry::Hillshade(_) | LayerEntry::Custom(_) => None,
        })
    }
}

/// The GPU rendering toolbox: the screen-space pipelines shared across all
/// layers (text, icon, marker, sky), the frame's colour/depth attachments, the
/// Frame-level render services shared by every subsystem: the MSAA
/// attachments the single frame pass draws into, and the optional GPU
/// timer. Everything content-shaped lives on a subsystem (slice D2) — this
/// is the residue that is genuinely per-frame, not per-content.
struct Renderer {
    /// Optional GPU-side frame timing — `Some` only when the device negotiated
    /// `Features::TIMESTAMP_QUERY`.
    gpu_timestamps: Option<GpuTimestamps>,
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
            gpu_timestamps: GpuTimestamps::new(device, queue),
            targets: FrameTargets::new(device, initial_size, surface_format),
        }
    }
}

// ---- The five subsystems (slice D2) -----------------------------------
// `Map`'s god-fields, regrouped by the domain that owns them. Each struct
// is one subsystem: its pipelines, its state, its caches — so a subsystem
// is evaluable (and borrowable) alone. The `Subsystem` trait impls (name /
// budgets / inspect / debug views) live in the same file, below `Map`.

/// The tile-layer stack: raster basemaps, vector overlays, hillshade — the
/// content layers a scene declares, in paint order.
struct BasemapSubsystem {
    layers: LayerStack,
}

/// The 3D ground: the shared heightmap + displacement plumbing, the
/// cast-shadow heightfield, and the world-locked AO bake.
struct TerrainSubsystem {
    /// The registered heightmap source + shared tile cache (`None` = 2D map).
    data: Option<Terrain>,
    /// Cast-shadow controls + recompute cache (strength, assembled-region
    /// key, progressive build). See [`TerrainShadowState`].
    shadow: TerrainShadowState,
    /// Map-level terrain bind-group layout + sampler + 1×1 placeholder bind
    /// group. Always present so displacement-capable pipelines can be built
    /// before any terrain source is registered — they bind the placeholder
    /// and render flat.
    shared: TerrainShared,
    /// Frame-global terrain cast-shadow grid (sun-visibility texture + bind
    /// group), bound at `@group(3)` of the raster pipeline. Uploaded from a
    /// CPU horizon-march when the sun/region/DEM changes.
    shadow_map: ShadowMap,
    /// Progressive, world-locked ambient-occlusion bake driven off the
    /// `shadow_map` heightfield. Refines over a few frames, then caches
    /// (it's sun-independent).
    ao: AoField,
}

/// Screen-space symbology: text labels and icon sprites, shared across all
/// vector layers (one collision world per frame).
struct SymbolsSubsystem {
    text: TextPipeline,
    icons: IconPipeline,
}

/// Host-driven overlays: markers and the route/track 3D tubes.
struct OverlaysSubsystem {
    markers: MarkerManager,
    marker_pipeline: MarkerPipeline,
    /// Route/track as raised 3D tubes (a single lit mesh, drawn after the
    /// ground layers so terrain occludes it, before screen-space overlays).
    route_pipeline: RoutePipeline,
    route_tubes: RouteTubeState,
}

/// The environment look: analytic sky, the sub-sea floor backstop, scene
/// lighting, the weather-cloud composite, and the 3D look gates.
struct AtmosphereSubsystem {
    /// Analytic atmosphere sky, drawn first in the frame pass when the camera
    /// is tilted (so the horizon band shows behind the terrain).
    sky: SkyPipeline,
    /// Sub-sea-level ground "floor" backstop, drawn after the sky and before
    /// the terrain so streaming gaps show neutral sea-grey instead of
    /// see-through holes; depth-writes so real terrain overdraws it.
    floor: FloorPipeline,
    /// Scene lighting — the sun driving terrain shading, sky, aerial
    /// perspective + clouds. See [`crate::lighting::Lighting`].
    lighting: Lighting,
    /// Optional procedural weather-cloud overlay (its own composite pass).
    clouds: Option<CloudOverlay>,
    /// Draw the analytic sky behind the scene (when tilted).
    sky_enabled: bool,
    /// Basemap brightness gain for the 3D sun-lit path (1.0 = unchanged).
    basemap_gain: f32,
    /// Apply terrain sun-lighting in 3D (`false` = displaced but unlit).
    terrain_lit: bool,
    /// Apply far-distance atmospheric coloration (aerial perspective) in 3D.
    aerial_haze: bool,
}

// ---- The S7 observability contract, per subsystem (slice D2) ----------
// Budgets come from the caches each subsystem owns; inspect JSON is the
// live-state snapshot; debug views name the frame-graph passes that mask
// the subsystem's stages off (the scenario harness renders every MaskPass
// view automatically in TURBO_PASS_ISOLATE).

impl Subsystem for BasemapSubsystem {
    fn name(&self) -> &'static str {
        "basemap"
    }
    fn passes(&self) -> &'static [&'static str] {
        &["layer", "custom"]
    }
    fn budgets(&self) -> BudgetReport {
        let mut r = BudgetReport::default();
        for l in self.layers.iter() {
            match l {
                LayerEntry::Raster(e) => {
                    let s = e.cache.stats();
                    r.bytes_used += s.bytes_used;
                    r.bytes_budget += s.budget_bytes;
                    r.items += s.entries;
                }
                LayerEntry::Vector(e) => {
                    r.bytes_used += e.cache.bytes_used();
                    r.bytes_budget += e.cache.budget_bytes();
                    r.items += e.cache.len();
                }
                // Hillshade reads the shared terrain cache — counted by the
                // terrain subsystem, not double-counted here. Custom layers
                // own opaque GPU state; byte accounting arrives when the
                // trait grows a budget hook.
                LayerEntry::Hillshade(_) | LayerEntry::Custom(_) => {}
            }
        }
        r
    }
    fn inspect(&self) -> String {
        let layers: Vec<String> = self
            .layers
            .iter()
            .map(|l| {
                let (id, kind, visible) = match l {
                    LayerEntry::Raster(e) => (&e.id, "raster", e.visible),
                    LayerEntry::Vector(e) => (&e.id, "vector", e.visible),
                    LayerEntry::Hillshade(e) => (&e.id, "hillshade", e.visible),
                    LayerEntry::Custom(e) => (&e.id, "custom", e.visible),
                };
                // Custom layers also report the registry kind they were
                // bound by — "which contribution is this" is exactly what
                // an inspect reader wants to know.
                let bound = match l {
                    LayerEntry::Custom(e) => format!(
                        ",\"bound_kind\":\"{}\"",
                        crate::subsystem::json_escape(&e.kind)
                    ),
                    _ => String::new(),
                };
                format!(
                    "{{\"id\":\"{}\",\"kind\":\"{kind}\"{bound},\"visible\":{visible}}}",
                    crate::subsystem::json_escape(id)
                )
            })
            .collect();
        format!("{{\"layers\":[{}]}}", layers.join(","))
    }
    fn debug_views(&self) -> &'static [DebugViewDesc] {
        &[DebugViewDesc {
            name: "no-tile-layers",
            description: "frame without any tile layer (sky/terrain plumbing only)",
            activation: DebugActivation::MaskPass("layer"),
        }]
    }
}

impl Subsystem for TerrainSubsystem {
    fn name(&self) -> &'static str {
        "terrain"
    }
    fn passes(&self) -> &'static [&'static str] {
        &["shadow-assemble", "ao-accumulate"]
    }
    fn budgets(&self) -> BudgetReport {
        match &self.data {
            Some(t) => {
                let s = t.cache.stats();
                BudgetReport {
                    bytes_used: s.bytes_used,
                    bytes_budget: s.budget_bytes,
                    items: s.entries,
                }
            }
            None => BudgetReport::default(),
        }
    }
    fn inspect(&self) -> String {
        let (present, exaggeration, finest, binding) = match &self.data {
            Some(t) => (
                true,
                t.options.exaggeration,
                Some(t.cache.finest_resident_zoom()),
                // The ground representation behind the Surface seam (plan
                // D3) — "heightfield" today, "mesh" when M-TIN lands.
                Some((t as &dyn Surface).ground_binding().kind()),
            ),
            None => (false, 1.0f32, None, None),
        };
        format!(
            "{{\"present\":{present},\"exaggeration\":{exaggeration},\
             \"ground_binding\":{},\
             \"finest_resident_zoom\":{},\"shadow_strength\":{},\
             \"shadow_field_assembled\":{}}}",
            binding
                .map(|b| format!("\"{b}\""))
                .unwrap_or_else(|| "null".into()),
            finest
                .map(|z: u8| z.to_string())
                .unwrap_or_else(|| "null".into()),
            self.shadow.strength,
            self.shadow.key.is_some(),
        )
    }
    fn debug_views(&self) -> &'static [DebugViewDesc] {
        &[
            DebugViewDesc {
                name: "no-cast-shadows",
                description: "zero the cast-shadow uniforms (Lambertian shading only)",
                activation: DebugActivation::MaskPass("shadow-assemble"),
            },
            DebugViewDesc {
                name: "frozen-ao",
                description: "halt AO refinement (cached field keeps rendering)",
                activation: DebugActivation::MaskPass("ao-accumulate"),
            },
        ]
    }
}

impl Subsystem for SymbolsSubsystem {
    fn name(&self) -> &'static str {
        "symbols"
    }
    fn passes(&self) -> &'static [&'static str] {
        &["text", "icons"]
    }
    fn budgets(&self) -> BudgetReport {
        BudgetReport {
            // Atlas byte accounting arrives when the pipelines expose it;
            // the working set is the placed-anchor count.
            bytes_used: 0,
            bytes_budget: 0,
            items: self.text.placed_marker_anchors().len(),
        }
    }
    fn inspect(&self) -> String {
        format!(
            "{{\"placed_anchors\":{}}}",
            self.text.placed_marker_anchors().len()
        )
    }
    fn debug_views(&self) -> &'static [DebugViewDesc] {
        &[
            DebugViewDesc {
                name: "no-text",
                description: "frame without labels (collision world still runs)",
                activation: DebugActivation::MaskPass("text"),
            },
            DebugViewDesc {
                name: "no-icons",
                description: "frame without icon sprites",
                activation: DebugActivation::MaskPass("icons"),
            },
        ]
    }
}

impl Subsystem for OverlaysSubsystem {
    fn name(&self) -> &'static str {
        "overlays"
    }
    fn passes(&self) -> &'static [&'static str] {
        &["markers", "route-tubes"]
    }
    fn budgets(&self) -> BudgetReport {
        BudgetReport {
            bytes_used: 0,
            bytes_budget: 0,
            items: self.markers.len() + self.route_tubes.tubes.len(),
        }
    }
    fn inspect(&self) -> String {
        format!(
            "{{\"markers\":{},\"route_tubes\":{},\"tube_radius_px\":{}}}",
            self.markers.len(),
            self.route_tubes.tubes.len(),
            self.route_tubes.radius_px,
        )
    }
    fn debug_views(&self) -> &'static [DebugViewDesc] {
        &[
            DebugViewDesc {
                name: "no-markers",
                description: "frame without host markers",
                activation: DebugActivation::MaskPass("markers"),
            },
            DebugViewDesc {
                name: "no-route-tubes",
                description: "frame without the route/track tubes",
                activation: DebugActivation::MaskPass("route-tubes"),
            },
        ]
    }
}

impl Subsystem for AtmosphereSubsystem {
    fn name(&self) -> &'static str {
        "atmosphere"
    }
    fn passes(&self) -> &'static [&'static str] {
        &["sky", "floor", "clouds"]
    }
    fn budgets(&self) -> BudgetReport {
        // The radar field textures are the only sized asset: two slots of
        // grid-size R8 pairs (precip + coverage).
        let bytes = self
            .clouds
            .as_ref()
            .map(|c| (c.grid.0 as usize) * (c.grid.1 as usize) * 2 * 2)
            .unwrap_or(0);
        BudgetReport {
            bytes_used: bytes,
            bytes_budget: 0,
            items: self
                .clouds
                .as_ref()
                .map(|c| c.enabled as usize)
                .unwrap_or(0),
        }
    }
    fn inspect(&self) -> String {
        let clouds = match &self.clouds {
            Some(c) => format!(
                "{{\"enabled\":{},\"sim\":{},\"time\":{},\"blend\":{},\"grid\":[{},{}],\"world_locked\":{}}}",
                c.enabled,
                c.sim,
                c.params.time,
                c.params.blend,
                c.grid.0,
                c.grid.1,
                c.radar_geo.is_some(),
            ),
            None => "null".to_string(),
        };
        format!(
            "{{\"lighting_mode\":\"{:?}\",\"sky_enabled\":{},\"basemap_gain\":{},\
             \"terrain_lit\":{},\"aerial_haze\":{},\"clouds\":{clouds}}}",
            self.lighting.mode(),
            self.sky_enabled,
            self.basemap_gain,
            self.terrain_lit,
            self.aerial_haze,
        )
    }
    fn debug_views(&self) -> &'static [DebugViewDesc] {
        &[
            DebugViewDesc {
                name: "no-sky",
                description: "frame without the analytic sky dome",
                activation: DebugActivation::MaskPass("sky"),
            },
            DebugViewDesc {
                name: "no-clouds",
                description: "frame without the weather-cloud composite",
                activation: DebugActivation::MaskPass("clouds"),
            },
            DebugViewDesc {
                name: "cloud-aov",
                description: "render a cloud pipeline stage (set_cloud_params debug_view: \
                              precip/coverage/field/density/light/alpha/albedo/parallax)",
                activation: DebugActivation::Param("set_cloud_params(debug_view)"),
            },
        ]
    }
}

/// How long the sim crossfades toward a newly ingested "next" radar frame
/// (slot 1). Real feeds re-ingest faster than this; the demo cadence keeps
/// the advection visibly smooth. Pure function of (ingest stamp, clock), so
/// replay under a pinned clock is exact.
const RADAR_ADVECT_SECS: f32 = 30.0;

/// The weather-cloud simulation (plan E2): clouds tick from the frame's
/// Environment — the same clock, sun and wind everything else samples —
/// instead of waiting for a host to scrub `set_cloud_time` every frame.
/// Deliberately STATELESS between frames (every output is a pure function
/// of `env` + the radar ingest stamps), so `dt_s` goes unused and a pinned
/// clock replays the exact frame.
impl SimulationSystem for AtmosphereSubsystem {
    fn tick(&mut self, _dt_s: f32, env: &Environment) -> bool {
        let Some(c) = self.clouds.as_mut() else {
            return false;
        };
        if !(c.enabled && c.sim) {
            return false;
        }
        // Drift/boil ride the Environment clock.
        c.params.time = env.time_s;
        // Wind-driven drift: a scene/host-supplied wind overrides the
        // tuned default; calm (the current default Environment) leaves it.
        if env.wind != [0.0, 0.0] {
            c.params.wind = env.wind;
        }
        // The ONE Environment sun lights the clouds — self-shadow azimuth
        // (compass → world xy: x=E, y=S) and elevation stay coherent with
        // terrain shading, the sky and aerial perspective by construction.
        let az = env.sun.azimuth_deg.to_radians();
        c.params.sun_dir = [az.sin(), -az.cos()];
        c.params.sun_elevation = (env.sun.altitude_deg / 90.0).clamp(0.0, 1.0);
        // Radar advection: crossfade toward the most recent "next" frame
        // from the moment it arrived.
        if let Some(t0) = c.advect_start {
            c.params.blend = ((env.time_s - t0) / RADAR_ADVECT_SECS).clamp(0.0, 1.0);
        }
        true
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
    /// Right viewport inset (px) — e.g. a desktop side panel; re-stamped like
    /// the bottom inset. 0 = none.
    viewport_inset_right_px: f64,
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
            viewport_inset_right_px: initial.viewport_inset_right_px,
            zoom: ZoomLock::new(initial.zoom_bounds),
        }
    }

    /// Re-stamp the sticky inset + zoom lock onto the pose. Called after any
    /// pose change so a host-supplied or animation-sampled camera can't escape
    /// the map's inset / accuracy bounds.
    fn restamp(&mut self) {
        self.camera.viewport_inset_px = self.viewport_inset_px;
        self.camera.viewport_inset_right_px = self.viewport_inset_right_px;
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
    /// Frame-level render services (MSAA attachments + GPU timer). All
    /// content-shaped state lives on the subsystems below (slice D2).
    renderer: Renderer,
    /// The tile-layer stack subsystem. See [`BasemapSubsystem`].
    basemap: BasemapSubsystem,
    /// The 3D ground subsystem: heightmap, cast shadows, AO.
    /// See [`TerrainSubsystem`].
    terrain: TerrainSubsystem,
    /// Screen-space text + icons. See [`SymbolsSubsystem`].
    symbols: SymbolsSubsystem,
    /// Markers + route tubes. See [`OverlaysSubsystem`].
    overlays: OverlaysSubsystem,
    /// Sky, floor, lighting, clouds, 3D look gates. See
    /// [`AtmosphereSubsystem`].
    atmosphere: AtmosphereSubsystem,
    last_frame_metrics: FrameMetrics,
    /// Renderer wall clock. `elapsed().as_secs_f32()` is stamped into the frame
    /// config each render to drift the procedural haze (so it "rolls in" and
    /// its patchiness moves over time). Animates while frames are produced.
    start: Instant,
    /// Pins the frame's animation clock (haze drift, custom-layer time, the
    /// cloud sim) to a fixed value — deterministic goldens and E2's replay
    /// gate ("same (fields, time, seed) ⇒ identical frame"). `None` = wall
    /// clock.
    time_override: Option<f32>,
    /// The previous frame's clock, for the simulation tick's `dt`.
    last_frame_clock: Option<f32>,
    /// Camera-eye world position at the previous `pending_tiles()` call — the
    /// finite-difference travel direction that feeds the priority score's
    /// motion term (stream WHERE WE'RE HEADING). `None` until the first call;
    /// a stationary camera yields zero alignment, which is the exact parity
    /// case with the historical `(tier, distance)` order. `Cell` because
    /// `pending_tiles` is a `&self` read on the render thread and this is
    /// its private memo, not shared state.
    last_priority_eye: std::cell::Cell<Option<WorldPoint>>,
    /// **Slice B3.1 dual-write.** The ONE lifecycle table
    /// ([`turbomap_world::Lifecycle`]) shadowing the legacy per-scene
    /// `ingested` sets. Written alongside every selection (`pending_tiles`)
    /// and ingest/eviction; [`Map::lifecycle_agreement`] proves the two
    /// bookkeepings agree across the full sim sweep. The legacy sets are
    /// deleted (and this becomes the source of truth feeding the
    /// `StreamingPlan`) only after that gate has held — never before.
    /// `RefCell`: written from the `&self` `pending_tiles` on the render
    /// thread, same discipline as `last_priority_eye`.
    lifecycle: std::cell::RefCell<turbomap_world::Lifecycle>,
    /// Monotonic counter for the table's recency stamps (frame-ish; advances
    /// per `pending_tiles` call — deterministic, no wall clock).
    lifecycle_frame: std::cell::Cell<u64>,
    /// Stable numeric layer ids for [`turbomap_world::ChunkKey`]s.
    /// `WorldLayerId(0)` is reserved for the Map-level terrain source.
    world_layer_ids: HashMap<String, turbomap_world::WorldLayerId>,
    next_world_layer_id: u16,
    /// Frame-graph pass disable-set for isolation debugging: any pass in the
    /// frame can be turned off by name (`"clouds"`, `"layer:hillshade"`, …)
    /// via [`Map::set_pass_enabled`]. Skipped passes still appear in the
    /// frame's pass report (marked `skipped`), so an isolation experiment is
    /// a fully described frame. Empty in production.
    pass_mask: PassMask,
}

/// Route/track 3D-tube state. Each entry is a polyline + style; the combined
/// lit mesh is rebuilt (sampling terrain elevation) when a polyline changes or
/// when new DEM tiles arrive (so the tube re-drapes as terrain streams in).
#[derive(Default)]
struct RouteTubeState {
    /// id → (world-space polyline, color, radius in metres).
    tubes: std::collections::HashMap<String, RouteTube>,
    /// Bumped on every terrain ingest; a tube rebuilt at an older generation is
    /// stale (its baked elevations predate newly-loaded DEM) and re-drapes.
    terrain_gen: u64,
    /// `terrain_gen` the current mesh was built at.
    built_gen: u64,
    /// Set when a polyline/style changed — forces an immediate rebuild.
    dirty: bool,
    /// Absolute world origin the baked mesh xy is relative to (f32 precision).
    origin: (f64, f64),
    /// Tube radius in screen pixels (shared across tubes; the latest set wins).
    radius_px: f32,
    /// Throttle for terrain-driven re-drapes (a DEM burst bumps `terrain_gen`
    /// many times/sec; rebuilding the mesh each time is wasteful for long tracks).
    last_build: Option<Instant>,
}

struct RouteTube {
    points: Vec<(f64, f64)>,
    color: [u8; 4],
}

/// Terrain cast-shadow state held on the `Map`: the user-set strength plus the
/// inputs of the last assembled heightfield, so the (relatively expensive) CPU
/// cross-tile assembly reruns only when something it depends on changed — the
/// sun moved, the camera settled in a new region, or new DEM tiles arrived. The
/// fragment shader marches the held heightfield every frame, so panning within
/// the assembled region stays sharp + welded to the ground with no reassembly.
#[derive(Default)]
struct TerrainShadowState {
    /// 0 = disabled. Blend factor of cast shadows into the direct sun term.
    strength: f32,
    /// Key of the last computed field; `None` forces a recompute.
    key: Option<ShadowKey>,
    /// ABSOLUTE world origin (lower-left, Mercator [0,1]) of the last computed
    /// field. Stored absolute — NOT camera-relative — because the shadow
    /// texture is anchored to the terrain, and the camera-relative (RTC) origin
    /// the shader needs must be rederived against the CURRENT camera each frame
    /// (`origin_abs − current_cam_origin`). Storing an RTC origin instead pinned
    /// the stale texture to the moving camera, so shadows slid in screen space
    /// while panning instead of staying on the ground.
    origin_abs: [f64; 2],
    world_size: f32,
    /// In-flight PROGRESSIVE reassembly. The 256² cross-tile sample is spread
    /// over several frames (a chunk of rows each) into this scratch buffer, with
    /// the previous field kept bound until it completes — so a region change
    /// never blocks one frame on the whole walk (the ~5–20ms settle hitch).
    /// `None` when no build is in flight. Mirrors the AO progressive bake.
    build: Option<ShadowBuild>,
    /// Camera pose (lat/lng/zoom/pitch/bearing bits) at the last render, to
    /// detect "the camera is moving". A REPLACEMENT reassembly is deferred while
    /// moving — a fling is an `active` animation so it was already deferred (and
    /// feels smooth), but a finger-pan is direct `set_camera` (not "animating"),
    /// so it used to reassemble mid-drag and felt choppy. Treating any
    /// frame-to-frame pose change as "moving" makes a pan defer like a fling:
    /// shadows hold their last field during the drag and settle in (progressively)
    /// once you stop.
    last_pose: Option<[u64; 5]>,
}

/// One in-flight progressive heightfield assembly (see [`TerrainShadowState::build`]).
struct ShadowBuild {
    key: ShadowKey,
    origin_abs: [f64; 2],
    size_f: f32,
    cell: f64,
    zscale: f32,
    heights: Vec<f32>,
    /// Rows `[0, next_row)` are sampled; the build commits when it reaches `dim`.
    next_row: usize,
}

/// Diagnostic (env `TURBO_SHADOW_DEBUG`): report the assembled heightfield's
/// relief SPAN in world-z and the finest resident DEM zoom. A near-zero span at
/// high camera zoom = the 256² field is being fed an over-zoomed (coarse) DEM,
/// so it's locally flat and casts no shadow — distinguishing a DEM-resolution
/// limit from a march/extent bug.
fn debug_shadow_relief(tag: &str, heights: &[f32], size_f: f32, zoom: f64, dem_finest_z: u8) {
    if std::env::var("TURBO_SHADOW_DEBUG").is_err() {
        return;
    }
    let (mn, mx) = heights
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &h| {
            (a.min(h), b.max(h))
        });
    let nonzero = heights.iter().filter(|&&h| h != 0.0).count();
    eprintln!(
        "SHADOW[{tag}] zoom={zoom:.1} dem_finest_z={dem_finest_z} field_size={size_f:.6} \
         relief_z=[{mn:.7},{mx:.7}] span_z={:.7} nonzero={nonzero}/{}",
        mx - mn,
        heights.len(),
    );
}

/// Identity of the inputs the heightfield was assembled from (floats
/// kept as bit patterns so the key is `Eq`/`Hash`-able). Equality means
/// "nothing the field depends on changed", so the cached GPU upload stands.
#[derive(PartialEq, Eq, Clone)]
struct ShadowKey {
    sun: [u32; 3],
    origin: [u32; 2],
    size: u32,
    /// Monotonic DEM-insert count — bumps when new terrain tiles ingest, so a
    /// freshly-filled region recomputes its shadows.
    dem_inserts: u64,
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
        // Every pipeline draws inside the one frame pass, whose MSAA colour
        // target matches the surface format and resolves straight to it.
        let shadow_map = ShadowMap::new(&device, &queue);
        let ao = AoField::new(&device, queue.clone(), &shadow_map.height_tex_layout);
        // Built before the layer stack so displacement-capable pipelines can
        // borrow its DEM bind-group layout (group 2) for draping.
        let shared = TerrainShared::new(&device, &queue);
        let terrain = TerrainSubsystem {
            data: None,
            shadow: TerrainShadowState::default(),
            shared,
            shadow_map,
            ao,
        };
        let symbols = SymbolsSubsystem {
            text: TextPipeline::new(device.clone(), queue.clone(), surface_format),
            icons: IconPipeline::new(device.clone(), queue.clone(), surface_format),
        };
        let overlays = OverlaysSubsystem {
            markers: MarkerManager::default(),
            marker_pipeline: MarkerPipeline::new(device.clone(), queue.clone(), surface_format),
            route_pipeline: RoutePipeline::new(device.clone(), queue.clone(), surface_format),
            route_tubes: RouteTubeState::default(),
        };
        let atmosphere = AtmosphereSubsystem {
            sky: SkyPipeline::new(device.clone(), queue.clone(), surface_format),
            floor: FloorPipeline::new(device.clone(), queue.clone(), surface_format),
            lighting: Lighting::default(),
            clouds: None,
            sky_enabled: true,
            basemap_gain: 1.0,
            terrain_lit: true,
            aerial_haze: true,
        };
        Ok(Self {
            device,
            queue,
            surface_format,
            viewport_px: initial_size,
            cam: CameraState::new(initial_camera),
            options,
            renderer,
            basemap: BasemapSubsystem {
                layers: LayerStack::new(),
            },
            terrain,
            symbols,
            overlays,
            atmosphere,
            last_frame_metrics: FrameMetrics::default(),
            start: Instant::now(),
            time_override: None,
            last_frame_clock: None,
            last_priority_eye: std::cell::Cell::new(None),
            // Dual-write phase: effectively-unbounded capacity so the table
            // observes without interfering; the real governor activates when
            // the table becomes the source of truth (B3.4 / Slice 4).
            lifecycle: std::cell::RefCell::new(turbomap_world::Lifecycle::with_capacity(
                usize::MAX / 2,
            )),
            lifecycle_frame: std::cell::Cell::new(0),
            world_layer_ids: HashMap::new(),
            next_world_layer_id: 1, // 0 is the terrain reservation
            pass_mask: PassMask::default(),
        })
    }

    /// Enable/disable a frame-graph pass by name — the isolation-debugging
    /// switch (architecture §III.3). Names are the `label`s reported in
    /// [`FrameMetrics::passes`]: a bare kind (`"sky"`, `"clouds"`, `"layer"`)
    /// disables every instance; a qualified instance (`"layer:hillshade"`)
    /// disables just that one. Disabled passes are skipped but still reported
    /// (marked `skipped`), and every skip is safe by construction: persistent
    /// resources (heightfield, AO) simply go stale, draw contributions just
    /// don't paint.
    pub fn set_pass_enabled(&mut self, name: &str, enabled: bool) {
        self.pass_mask.set_enabled(name, enabled);
    }

    /// The subsystem registry (slice D2): every subsystem of this map,
    /// viewable through the S7 observability contract. Typed access stays on
    /// the fields; this is the uniform iteration surface for inspection,
    /// budget sweeps and the registry meta-test.
    pub fn subsystems(&self) -> [&dyn Subsystem; 5] {
        [
            &self.basemap,
            &self.terrain,
            &self.symbols,
            &self.overlays,
            &self.atmosphere,
        ]
    }

    /// One JSON object for the whole map's live state: per-subsystem inspect
    /// snapshots + budget reports, keyed by subsystem name. The "inspect
    /// tool" surface — hosts and the scenario harness dump it verbatim.
    pub fn inspect_json(&self) -> String {
        let parts: Vec<String> = self
            .subsystems()
            .iter()
            .map(|s| {
                let b = s.budgets();
                format!(
                    "\"{}\":{{\"state\":{},\"budgets\":{{\"bytes_used\":{},\
                     \"bytes_budget\":{},\"items\":{}}}}}",
                    s.name(),
                    s.inspect(),
                    b.bytes_used,
                    b.bytes_budget,
                    b.items,
                )
            })
            .collect();
        format!("{{{}}}", parts.join(","))
    }

    /// Set (or clear) a route/track polyline rendered as a raised 3D tube.
    /// `points` are lng/lat; empty clears the tube `id`. `radius_px` is the tube
    /// radius in screen pixels (constant thickness at any zoom). Rebuilt against
    /// the terrain on next render.
    pub fn set_route_tube(&mut self, id: &str, points: &[LatLng], color: Color, radius_px: f64) {
        if points.len() < 2 {
            if self.overlays.route_tubes.tubes.remove(id).is_some() {
                self.overlays.route_tubes.dirty = true;
            }
            return;
        }
        let world: Vec<(f64, f64)> = points
            .iter()
            .map(|p| {
                let w = p.to_world();
                (w.x, w.y)
            })
            .collect();
        self.overlays.route_tubes.radius_px = radius_px as f32;
        self.overlays.route_tubes.tubes.insert(
            id.to_string(),
            RouteTube {
                points: world,
                color: [color.r, color.g, color.b, color.a],
            },
        );
        self.overlays.route_tubes.dirty = true;
    }

    /// Rebuild the combined route-tube mesh from the current polylines + terrain
    /// elevation and upload it. Called from `render` when a polyline changed or
    /// newly-loaded DEM means the baked elevations are stale.
    fn rebuild_route_tubes(&mut self) {
        const SEGMENTS: usize = 8;
        if self.overlays.route_tubes.tubes.is_empty() {
            self.overlays.route_pipeline.upload(&[], &[]);
            self.overlays.route_tubes.built_gen = self.overlays.route_tubes.terrain_gen;
            self.overlays.route_tubes.dirty = false;
            return;
        }
        // Surface height factor: metres → world-z, matching the terrain mesh.
        let lat = self.cam.camera.center.lat.to_radians();
        let m2w = (lat.cos().abs() / 40_075_017.0).max(1e-12);
        let exagg = self
            .terrain
            .data
            .as_ref()
            .map(|t| t.options.exaggeration as f64)
            .unwrap_or(1.0);

        // Stable origin for f32 precision: a deterministic min over tube points
        // (HashMap order is random, so don't depend on it).
        let origin = self
            .overlays
            .route_tubes
            .tubes
            .values()
            .flat_map(|t| t.points.iter())
            .fold((f64::INFINITY, f64::INFINITY), |a, p| {
                (a.0.min(p.0), a.1.min(p.1))
            });

        let mut verts: Vec<RouteVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for tube in self.overlays.route_tubes.tubes.values() {
            // Bake the terrain surface height per centerline point via the
            // Surface query (plan D3); the tube radius + lift above it are
            // applied GPU-side (constant screen size).
            let world_z: Vec<f32> = tube
                .points
                .iter()
                .map(|&(x, y)| {
                    self.terrain
                        .data
                        .as_ref()
                        .and_then(|t| (t as &dyn Surface).elevation_at(WorldPoint::new(x, y)))
                        .map(|e| (e as f64 * m2w * exagg) as f32)
                        .unwrap_or(0.0)
                })
                .collect();
            let (v, i) = build_tube(&tube.points, &world_z, origin, SEGMENTS, tube.color);
            let base = verts.len() as u32;
            verts.extend(v);
            indices.extend(i.into_iter().map(|idx| idx + base));
        }
        self.overlays.route_pipeline.upload(&verts, &indices);
        self.overlays.route_tubes.origin = origin;
        self.overlays.route_tubes.built_gen = self.overlays.route_tubes.terrain_gen;
        self.overlays.route_tubes.dirty = false;
    }

    // ---- Procedural weather-cloud overlay -----------------------------

    /// Turn on the procedural cloud overlay, allocating its two radar
    /// data textures at `grid_w × grid_h`. Idempotent for a given grid
    /// size; calling with a new size rebuilds the textures (and clears any
    /// uploaded frames). Upload radar frames with [`Map::ingest_radar_frame`]
    /// and drive the animation with [`Map::set_cloud_time`].
    pub fn enable_clouds(&mut self, grid_w: u32, grid_h: u32) {
        if let Some(c) = &mut self.atmosphere.clouds {
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
        let radar_geo = self.atmosphere.clouds.as_ref().and_then(|c| c.radar_geo);
        self.atmosphere.clouds = Some(CloudOverlay {
            scene,
            params: CloudParams::default(),
            grid: (grid_w, grid_h),
            enabled: true,
            // Weather moves by default: the sim drives the clock until a
            // host scrubs (`set_cloud_time`).
            sim: true,
            advect_start: None,
            radar_geo,
        });
    }

    /// Geo-register the radar to the lat/lng box it covers, so the overlay is
    /// world-locked (pans + zooms with the map). Pass the bounds the radar
    /// frames were sampled for. No-op if clouds aren't enabled.
    pub fn set_cloud_geo_bounds(&mut self, west: f64, south: f64, east: f64, north: f64) {
        if let Some(c) = &mut self.atmosphere.clouds {
            // Mercator y grows southward, so north is the min-y corner.
            let min = LatLng::new(north, west).to_world();
            let max = LatLng::new(south, east).to_world();
            c.radar_geo = Some((min, max));
        }
    }

    /// Show/hide the overlay without discarding its GPU state or uploaded
    /// frames. No-op if clouds were never enabled.
    pub fn set_clouds_visible(&mut self, visible: bool) {
        if let Some(c) = &mut self.atmosphere.clouds {
            c.enabled = visible;
        }
    }

    /// Tear the overlay down entirely, freeing its GPU resources.
    pub fn disable_clouds(&mut self) {
        self.atmosphere.clouds = None;
    }

    /// Whether the overlay is currently enabled and will draw.
    pub fn clouds_enabled(&self) -> bool {
        self.atmosphere
            .clouds
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    /// Upload a radar frame into slot 0 (current timestep) or 1 (next).
    /// The frame's dimensions must match the grid passed to
    /// [`Map::enable_clouds`]. No-op if clouds aren't enabled.
    pub fn ingest_radar_frame(&mut self, slot: u32, frame: &RadarFrame) {
        let now = self.frame_clock();
        if let Some(c) = &mut self.atmosphere.clouds {
            c.scene.upload(&self.queue, slot as usize, frame);
            // A new "next" frame restarts the sim's advection crossfade
            // from the moment it arrived (slot 0 is the current timestep —
            // rewinding the blend for it would pop already-faded weather).
            if slot == 1 {
                c.advect_start = Some(now);
            }
        }
    }

    /// Set the per-frame animation state: `time` is a free-running clock
    /// (seconds) driving cloud drift/boil; `blend` in `0..=1` crossfades
    /// the slot-0 radar frame into the slot-1 frame — this is what a time
    /// slider scrubs (and can run backward). Calling this hands the clock
    /// to the host (the sim stops driving it — see [`Map::set_cloud_sim`]).
    /// No-op if clouds aren't enabled.
    pub fn set_cloud_time(&mut self, time: f32, blend: f32) {
        if let Some(c) = &mut self.atmosphere.clouds {
            c.sim = false;
            c.params.time = time;
            c.params.blend = blend.clamp(0.0, 1.0);
        }
    }

    /// Hand the cloud clock to the simulation (plan E2, the default): the
    /// overlay drifts on the Environment's frame clock, shades under the
    /// one Environment sun, and crossfades toward newly ingested radar
    /// frames — all deterministic under [`Map::set_time_override`]. While
    /// active the sim counts as animation, so render-on-demand hosts keep
    /// pumping frames without manual redraw nudges. `false` freezes the
    /// sim where it is (a host scrub via [`Map::set_cloud_time`] also
    /// takes the clock). No-op if clouds aren't enabled.
    pub fn set_cloud_sim(&mut self, sim: bool) {
        if let Some(c) = &mut self.atmosphere.clouds {
            c.sim = sim;
        }
    }

    /// Replace the cloud overlay's full look parameters (wind, sun, feature
    /// scale, opacity, …). `resolution` is overwritten per frame from the
    /// viewport, so its value here is ignored. No-op if clouds aren't enabled.
    pub fn set_cloud_params(&mut self, params: CloudParams) {
        if let Some(c) = &mut self.atmosphere.clouds {
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
        // A fresh Terrain starts with an empty ingested set; drop the old
        // one's chunks from the lifecycle table to match (dual-write B3.1).
        self.lifecycle
            .borrow_mut()
            .forget_layer(turbomap_world::WorldLayerId(0));
        let halo = source.dem_halo_px();
        let cache = TerrainCache::new(
            self.device.clone(),
            self.queue.clone(),
            &self.terrain.shared,
            self.options.cache_budget_bytes,
            halo,
        );
        // Cap the DEM LOD at its native resolution. The source is ~10 m/px
        // (DTM10 ≈ z13–z14 at these latitudes); refining the relief mesh past
        // that just re-renders the SAME elevation at a finer grid — extra slow
        // DEM tiles for zero new shape. The basemap imagery still refines to its
        // own (higher) max; the terrain mesh upsamples a z14 DEM tile under it,
        // which is invisible (relief is smooth). Big cut to the high-zoom DEM
        // request count (the after-zoom z15/z16 DEM tiles vanish).
        const DEM_MAX_ZOOM: u8 = 14;
        let mut scene = Scene::with_margin(
            self.cam.camera,
            self.viewport_px,
            source.min_zoom(),
            source.max_zoom().min(DEM_MAX_ZOOM),
            self.options.prefetch_margin_px,
        );
        // Coarsen the DEM LOD relative to the imagery: relief geometry reads
        // fine at a larger on-screen tile target, and the DEM tile server is the
        // slow one — a coarser target cuts the per-view DEM request count
        // (~1.5× target ⇒ roughly half the DEM tiles) so near relief streams in
        // far sooner. Imagery keeps the sharp default.
        scene.set_sse_target_px(crate::scene::TERRAIN_LOD_SSE_TARGET_PX);
        // Proto-clipmap: a small LOD budget for the DEM so best-first spends it
        // on the nearest (highest-error) relief — fine near, coarse far — far
        // fewer of the slow DEM tiles per view than the imagery's full budget.
        // Relief is smooth + the imagery carries the far look, so coarse far DEM
        // is invisible. (A true camera-centred geometry clipmap is the next step.)
        const DEM_LOD_TILE_CAP: usize = 96;
        scene.set_lod_tile_cap(DEM_LOD_TILE_CAP);
        self.terrain.data = Some(Terrain::new(source, cache, scene, options));
    }

    pub fn clear_terrain(&mut self) {
        self.terrain.data = None;
    }

    pub fn has_terrain(&self) -> bool {
        self.terrain.data.is_some()
    }

    /// Enable/disable the analytic sky pass (debug isolation).
    pub fn set_sky_enabled(&mut self, enabled: bool) {
        self.atmosphere.sky_enabled = enabled;
    }

    // ---- Sun / time-of-day --------------------------------------------

    /// Make the sun (and therefore terrain shading, aerial perspective
    /// and the sky) track a real instant in time. `unix_seconds` is UTC
    /// seconds since the epoch; the position is solved per frame at the
    /// camera's current location, so the light follows both the clock
    /// and where the user is looking. `None` clears it (back to the
    /// fixed default unless [`Map::set_sun_position`] is also set).
    pub fn set_sun_time(&mut self, unix_seconds: Option<f64>) {
        self.atmosphere.lighting.set_time(unix_seconds);
    }

    /// Pin the sun to an explicit azimuth/altitude, overriding any
    /// time-based tracking. Used for deterministic goldens and manual
    /// control. `None` clears the override.
    pub fn set_sun_position(&mut self, sun: Option<SunPosition>) {
        self.atmosphere.lighting.set_fixed(sun);
    }

    /// Enable (and set the intensity of) terrain *cast* shadows — a peak
    /// throwing a shadow across the valley behind it, distinct from the
    /// always-on Lambertian self-shading. `strength` in `[0,1]`: 0 disables the
    /// feature entirely (zero per-frame cost), 1 is full occlusion of the
    /// direct sun term (ambient skylight still reaches shadowed ground).
    ///
    /// Only affects 3D terrain (a DEM source must be registered). The shadow
    /// field is computed on the CPU and refreshed when the sun, the visible
    /// region, or the resident DEM changes — so a static view costs nothing
    /// after the first frame. See [`crate::render::shadow`].
    /// Basemap brightness gain for the 3D sun-lit terrain (1.0 = unchanged).
    /// Raise it (~1.8) for dark imagery (satellite) so it reads under the same
    /// lighting that suits bright topo. No effect on the flat 2D map.
    pub fn set_basemap_gain(&mut self, gain: f32) {
        self.atmosphere.basemap_gain = gain.clamp(0.1, 8.0);
    }

    /// Toggle terrain sun-lighting in 3D. `true` (default) keeps the lit look;
    /// `false` draws the bare bright basemap over the displaced relief — used by
    /// hosts to keep a plain 2D→3D switch from darkening the scene (lighting +
    /// shadows belong to "sun mode"), which also skips the per-fragment shading
    /// path. No effect on the flat 2D map (no DEM).
    pub fn set_terrain_lit(&mut self, lit: bool) {
        self.atmosphere.terrain_lit = lit;
    }

    /// Toggle far-distance atmospheric coloration (aerial perspective). `true`
    /// (default) keeps the look; `false` forces the haze gate to 0 so the map is
    /// crisp at every angle/zoom. The web ties this to a "Distance haze" setting.
    pub fn set_aerial_haze(&mut self, on: bool) {
        self.atmosphere.aerial_haze = on;
    }

    pub fn set_terrain_shadows(&mut self, strength: f32) {
        let s = strength.clamp(0.0, 1.0);
        if s != self.terrain.shadow.strength {
            self.terrain.shadow.strength = s;
            // Force a recompute on the next frame (the strength change alone
            // doesn't alter the field, but turning it on from cold must).
            self.terrain.shadow.key = None;
        }
    }

    /// The sun position used this frame, resolved by the [`Lighting`] mode
    /// at the camera's current location.
    fn effective_sun(&self) -> SunPosition {
        self.atmosphere.lighting.sun_at(self.cam.camera.center)
    }

    /// Build the frame's [`Environment`] — THE single derivation site for
    /// every environmental input (plan E1): the clock, the one sun, the
    /// sun-derived palette, the host look gates, and (ahead of their first
    /// consumers) wind + season. `RenderFrame::build` turns it into
    /// uniforms; nothing patches lighting state in after the fact.
    /// The frame clock: the renderer wall clock, or the pinned
    /// [`Map::set_time_override`] value. Everything time-driven — the
    /// Environment, haze drift, custom layers, the cloud sim and its radar
    /// advection stamps — reads this one clock, which is what makes replay
    /// deterministic.
    fn frame_clock(&self) -> f32 {
        self.time_override
            .unwrap_or_else(|| self.start.elapsed().as_secs_f32())
    }

    fn environment(&self) -> Environment {
        let sun = self.effective_sun();
        Environment {
            time_s: self.frame_clock(),
            sun,
            atmosphere: crate::sun::atmosphere(sun),
            aerial_haze: self.atmosphere.aerial_haze,
            terrain_lit: self.atmosphere.terrain_lit,
            basemap_gain: self.atmosphere.basemap_gain,
            // Calm + neutral until a scene/host supplies them (E2 wires
            // wind into cloud drift; season waits on M-MODELS styling).
            wind: [0.0, 0.0],
            season: 0.5,
        }
    }

    /// Push a DECODED DEM tile (real heights — see
    /// [`crate::dem::decode_dem_rgba`]) into the shared terrain heightmap.
    /// Host drives this from the same fetch pump it uses for raster
    /// tiles. Silently no-ops when no terrain source is registered
    /// (e.g. host sent us a stale tile after `clear_terrain`).
    pub fn ingest_terrain_tile(&mut self, tile: TileId, dem: &crate::dem::DecodedDem) {
        let mut delivered: Option<Vec<TileId>> = None;
        if let Some(t) = self.terrain.data.as_mut() {
            let evicted = t.cache.ingest(tile, dem);
            // New elevation data → route tubes baked before this are stale and
            // should re-drape onto the now-finer terrain.
            self.overlays.route_tubes.terrain_gen =
                self.overlays.route_tubes.terrain_gen.wrapping_add(1);
            delivered = Some(evicted);
        }
        if let Some(evicted) = delivered {
            // Account the resident payload as the GPU texel bytes (f16
            // height + coverage), matching what the cache actually holds.
            let bytes = (dem.heights_m.len() * 4) as u64;
            self.lifecycle_delivered(None, tile, bytes, &evicted);
        }
    }

    // ---- lifecycle dual-write helpers (slice B3.1) -----------------------

    /// `ChunkKey` for a layer's tile. Layers not yet registered (never
    /// `add_*_layer`ed — impossible for ingest paths) fall back to the
    /// terrain reservation, which `debug_assert`s instead in dev.
    fn world_key(&self, layer_id: Option<&str>, tile: TileId) -> turbomap_world::ChunkKey {
        const TERRAIN: turbomap_world::WorldLayerId = turbomap_world::WorldLayerId(0);
        let layer = match layer_id {
            None => TERRAIN,
            Some(id) => match self.world_layer_ids.get(id) {
                Some(l) => *l,
                None => {
                    debug_assert!(false, "ingest for unregistered layer {id}");
                    TERRAIN
                }
            },
        };
        turbomap_world::ChunkKey {
            layer,
            node: turbomap_world::QuadKey::new(tile.z, tile.x, tile.y).node_id(),
        }
    }

    fn register_world_layer(&mut self, id: &str) {
        if !self.world_layer_ids.contains_key(id) {
            let l = turbomap_world::WorldLayerId(self.next_world_layer_id);
            self.next_world_layer_id += 1;
            self.world_layer_ids.insert(id.to_string(), l);
        }
    }

    /// Mirror a delivered payload into the table (legacy shim path — see
    /// `Lifecycle::delivered_unrequested`) plus any evictions the cache
    /// reported alongside it.
    fn lifecycle_delivered(
        &self,
        layer_id: Option<&str>,
        tile: TileId,
        bytes: u64,
        evicted: &[TileId],
    ) {
        let frame = self.lifecycle_frame.get();
        let mut lc = self.lifecycle.borrow_mut();
        lc.delivered_unrequested(self.world_key(layer_id, tile), bytes, frame);
        for e in evicted {
            let _ = lc.evicted(self.world_key(layer_id, *e));
        }
    }

    // Slice B3.4: `lifecycle_agreement` is retired with the per-scene sets
    // it compared. The dual-write soak (agreement asserted every sim frame
    // across the whole B4 campaign) is what justified the flip; residency
    // now has exactly one owner — the lifecycle table.

    // ---- layer management ----------------------------------------------

    pub fn add_raster_layer(&mut self, id: impl Into<String>, source: Arc<dyn TileSource>) {
        let id = id.into();
        self.register_world_layer(&id);
        let min_zoom = source.min_zoom();
        let max_zoom = source.max_zoom();
        let pipeline = RasterPipeline::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &self.terrain.shared.bind_group_layout,
            &self.terrain.shadow_map.layout,
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
        self.basemap
            .layers
            .push(LayerEntry::Raster(Box::new(RasterLayer {
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
            .data
            .as_ref()
            .map(|t| t.cache.halo_px())
            .unwrap_or(0);
        let pipeline = HillshadePipeline::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &self.terrain.shared.bind_group_layout,
            halo,
        );
        self.basemap
            .layers
            .push(LayerEntry::Hillshade(Box::new(HillshadeLayer {
                id,
                style,
                pipeline,
                fade_in_secs: self.options.fade_in_secs,
                visible: true,
            })));
    }

    /// Everything a [`CustomLayer`] needs to build pipelines compatible
    /// with the frame's single MSAA pass — hand this to the layer factory
    /// before [`Map::add_custom_layer`].
    pub fn custom_layer_init(&self) -> CustomLayerInit {
        CustomLayerInit {
            device: self.device.clone(),
            queue: self.queue.clone(),
            color_format: self.surface_format,
            depth_format: crate::render::DEPTH_FORMAT,
            sample_count: crate::render::MSAA_SAMPLES,
        }
    }

    /// Append a custom render layer (plan D4): a host-supplied
    /// [`CustomLayer`] joins the frame's MSAA pass in its declared phase as
    /// the graph node `custom:<id>`, in stack order among the other layers.
    /// `kind` is the registry name it was bound by (surfaced in inspect).
    pub fn add_custom_layer(
        &mut self,
        id: impl Into<String>,
        kind: impl Into<String>,
        layer: Box<dyn CustomLayer>,
    ) {
        self.basemap
            .layers
            .push(LayerEntry::Custom(Box::new(CustomLayerHolder {
                id: id.into(),
                kind: kind.into(),
                layer,
                visible: true,
            })));
    }

    /// Pin the frame's animation clock (haze drift, custom-layer `time_s`)
    /// to a fixed value; `None` returns to the wall clock. Deterministic
    /// rendering for goldens/replay.
    pub fn set_time_override(&mut self, secs: Option<f32>) {
        self.time_override = secs;
    }

    pub fn add_vector_layer(
        &mut self,
        id: impl Into<String>,
        source: Arc<dyn VectorTileSource>,
        style: VectorStyle,
    ) {
        let id = id.into();
        self.register_world_layer(&id);
        let min_zoom = source.min_zoom();
        let max_zoom = source.max_zoom();
        let pipeline = VectorPipeline::new(
            self.device.clone(),
            self.queue.clone(),
            self.surface_format,
            &self.terrain.shared.bind_group_layout,
        );
        let cache = VectorMeshCache::new(self.device.clone(), self.options.cache_budget_bytes);
        let scene = Scene::with_margin(
            self.cam.camera,
            self.viewport_px,
            min_zoom,
            max_zoom,
            self.options.prefetch_margin_px,
        );
        self.basemap
            .layers
            .push(LayerEntry::Vector(Box::new(VectorLayer {
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
        self.basemap.layers.with_vector_mut(id, |v| v.dash = dash)
    }

    /// Set (or clear) a vector layer's per-frame paint colour override.
    /// `color` is linear RGBA in `[0,1]`. Returns `false` if no vector
    /// layer matches `id`.
    pub fn set_vector_layer_color(&mut self, id: &str, color: Option<[f32; 4]>) -> bool {
        self.basemap
            .layers
            .with_vector_mut(id, |v| v.paint_override = color)
    }

    /// Set a vector layer's per-frame line-width multiplier (the zoom curve).
    /// `1.0` is the baked width. No-op for fills/text (width_px = 0). Returns
    /// `false` if no vector layer matches `id`.
    pub fn set_vector_layer_width_scale(&mut self, id: &str, scale: f32) -> bool {
        self.basemap
            .layers
            .with_vector_mut(id, |v| v.width_scale = scale)
    }

    pub fn remove_layer(&mut self, id: &str) {
        // Dropping a layer can widen or narrow the source-derived lock.
        if self.basemap.layers.remove(id) {
            if let Some(l) = self.world_layer_ids.get(id) {
                self.lifecycle.borrow_mut().forget_layer(*l);
            }
            self.recompute_zoom_bounds();
        }
    }

    pub fn layer_count(&self) -> usize {
        self.basemap.layers.len()
    }

    pub fn has_layer(&self, id: &str) -> bool {
        self.basemap.layers.contains(id)
    }

    pub fn layer_ids(&self) -> Vec<String> {
        self.basemap.layers.ids()
    }

    /// Look up the vector source for a vector layer — useful for the host
    /// when constructing the fetch pump.
    pub fn vector_source(&self, id: &str) -> Option<Arc<dyn VectorTileSource>> {
        self.basemap.layers.vector_source(id)
    }

    pub fn raster_source(&self, id: &str) -> Option<Arc<dyn TileSource>> {
        self.basemap.layers.raster_source(id)
    }

    /// Terrain DEM source (Map-level since the 3D-terrain refactor;
    /// hillshade layers consume this rather than owning their own
    /// source). Returns `None` until `set_terrain_source` is called.
    pub fn terrain_source(&self) -> Option<Arc<dyn TileSource>> {
        self.terrain.data.as_ref().map(|t| t.source.clone())
    }

    pub fn vector_style(&self, id: &str) -> Option<VectorStyle> {
        self.basemap.layers.vector_style(id)
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
        let bounds = self.cam.zoom.resolve(self.basemap.layers.source_ranges());
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

    /// Reserve `right_px` at the right of the viewport (e.g. a desktop side
    /// panel). Shifts the principal point left by `right_px/2` for projection,
    /// unprojection, and rendering alike. Persisted across camera changes.
    pub fn set_viewport_inset_right(&mut self, right_px: f64) {
        self.cam.viewport_inset_right_px = right_px.max(0.0);
        self.cam.camera.viewport_inset_right_px = self.cam.viewport_inset_right_px;
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
        let z = ZoomFlingAnimation::new(
            self.cam.camera,
            zoom_velocity,
            focus_px,
            (w as f64, h as f64),
        );
        if z.is_finished(Instant::now()) {
            return;
        }
        self.cam.active = Some(ActiveAnim::ZoomFling(z));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_px = (width, height);
        // Depth + MSAA colour must match the surface size or Metal asserts on
        // the next render; FrameTargets recreates them together (no-op when
        // unchanged or degenerate).
        self.renderer.targets.resize(&self.device, (width, height));
        self.sync_scenes();
    }

    pub fn pan_by_pixels(&mut self, dx: f64, dy: f64) {
        let mut c = self.cam.camera;
        c.pan_by_pixels(dx, dy);
        self.set_camera(c);
    }

    /// The camera after a *terrain-anchored* zoom by `factor` about `focus_px`:
    /// the SURFACE point under the cursor stays put (3D), instead of the flat
    /// sea-level point that [`Camera::zoomed_around`] re-pins. Without this,
    /// zooming over a mountainside kept the *downhill* z=0 point under the
    /// cursor, so the view slid up/down the slope and the centre drifted until
    /// the eye sank into the relief — the radiating-mesh blowup. `None` to fall
    /// back to the flat zoom (2D, no terrain, or the ray missed the surface
    /// before/after). Raycasts a candidate camera in place (direct field write,
    /// no scene-sync) then restores the live camera, so it's side-effect-free.
    fn terrain_anchored_zoom_target(
        &mut self,
        factor: f64,
        focus_px: (f64, f64),
    ) -> Option<Camera> {
        if self.terrain.data.is_none()
            || self.cam.camera.pitch_deg == 0.0
            || !factor.is_finite()
            || factor <= 0.0
        {
            return None;
        }
        let before = self.screen_to_ground_lng_lat(focus_px);
        if !before.hit_terrain {
            return None;
        }
        let target = before.lng_lat.to_world();
        let original = self.cam.camera;
        // Candidate: zoom only, centre unchanged. We only raycast against it.
        let mut cand = original;
        cand.zoom = self.zoom_bounds().clamp(original.zoom + factor.log2());
        self.cam.camera = cand;
        let after = self.screen_to_ground_lng_lat(focus_px);
        let result = if after.hit_terrain {
            // Shift the centre so the original surface point lands back under
            // the focus pixel (same algebra as the flat re-pin, on the relief).
            let a = after.lng_lat.to_world();
            let centre = cand.center.to_world();
            let mut c = cand;
            c.center = WorldPoint::new(centre.x + (target.x - a.x), centre.y + (target.y - a.y))
                .to_lat_lng();
            c
        } else {
            // Grazing / over-horizon after zoom: keep the zoom, skip the re-pin.
            cand
        };
        self.cam.camera = original; // restore; caller applies `result`
        Some(result)
    }

    pub fn zoom_around(&mut self, factor: f64, focus_px: (f64, f64)) {
        if let Some(target) = self.terrain_anchored_zoom_target(factor, focus_px) {
            self.set_camera(target);
            return;
        }
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
    ///
    /// The delta is clamped to the terrain pitch limit BEFORE the focus re-pin
    /// inside [`Camera::pitched_around`]. This makes a down-tilt stop dead at
    /// the limit: otherwise `pitched_around` re-pins for the requested pitch and
    /// the later [`clamp_pitch_above_terrain`] lowers the pitch *without*
    /// re-pinning, leaving the centre shifted for a pitch that no longer applies
    /// — so each frame at the limit slid the camera away from the pivot at speed
    /// ("runaway tilt").
    pub fn pitch_around(&mut self, delta_deg: f64, focus_px: (f64, f64)) {
        let (w, h) = self.viewport_px;
        let cur = self.cam.camera.pitch_deg;
        let mut target = cur + delta_deg;
        if let Some(pitch_max) = self.terrain_pitch_max() {
            target = target.min(pitch_max);
        }
        // Global [0, MAX_PITCH] is still enforced inside `pitched_around`.
        let eff_delta = target - cur;
        let c = self
            .cam
            .camera
            .pitched_around(eff_delta, focus_px, (w as f64, h as f64));
        self.set_camera(c);
    }

    /// Animate a focus-invariant zoom by `factor` about `focus_px` over
    /// `duration` — the smooth double-tap / scroll-wheel zoom. Eases to the
    /// same target [`Camera::zoom_around`] would snap to.
    pub fn zoom_around_animated(&mut self, factor: f64, focus_px: (f64, f64), duration: Duration) {
        let (w, h) = self.viewport_px;
        // Terrain-anchored target in 3D (double-tap zooms toward the surface
        // point, not the flat one); flat fallback otherwise.
        let target = self
            .terrain_anchored_zoom_target(factor, focus_px)
            .unwrap_or_else(|| {
                self.cam
                    .camera
                    .zoomed_around(factor, focus_px, (w as f64, h as f64))
            });
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

    /// A camera animation (ease/fling) is in flight — visual MOTION only,
    /// no fades. The engine's decode queue keys its apply budget on this:
    /// tight while the user watches motion, generous when settled (a fading
    /// tile is an *apply arriving*, so counting fades here would throttle
    /// the very work that ends them — cold-load starvation).
    pub fn is_camera_animating(&self) -> bool {
        self.cam.active.is_some()
    }

    pub fn is_animating(&self) -> bool {
        if self.cam.active.is_some() {
            return true;
        }
        // An active simulation (the cloud sim driving its own clock) IS
        // animation — render-on-demand hosts keep pumping frames, which is
        // what deleted the host-side request_redraw wart (plan E2).
        if self
            .atmosphere
            .clouds
            .as_ref()
            .is_some_and(|c| c.enabled && c.sim)
        {
            return true;
        }
        // Any layer with a fading tile keeps the animation flag set. Raster fade
        // is keyed on first-on-screen time (tracked in the pipeline), so the
        // signal must come from there — not the cache's ingest age — or a fading
        // cache tile would park render-on-demand mid-fade and stick translucent.
        self.basemap.layers.iter().any(|l| match l {
            LayerEntry::Raster(r) => r.pipeline.has_active_fade(r.fade_in_secs),
            LayerEntry::Vector(v) => v.cache.any_younger_than(v.fade_in_secs),
            LayerEntry::Hillshade(_) => false,
            // A custom layer animates on its own clock; hosts that want
            // continuous animation drive frames themselves (a custom-layer
            // "I'm animating" hook can join the trait when one needs it).
            LayerEntry::Custom(_) => false,
        })
    }

    /// Push the current camera + viewport into each layer's scene. Called
    /// whenever the camera changes or layers are added/removed.
    /// Keep the camera eye above the 3D terrain: if tilting/orbiting/zooming
    /// would drop the eye to (or below) the relief beneath it, cap the pitch
    /// so it stays a clearance above. Without this, a 1-finger orbit that
    /// lowers the view over tall, exaggerated terrain swings the eye *into* a
    /// ridge — you render from under the ground (everything but the nearest
    /// tile greys out / the frustum degenerates, which can hang the GPU
    /// driver). No-op without terrain or before its DEM loads.
    ///
    /// The eye is not above the look-point: at pitch `p` it sits a horizontal
    /// `alt·sin(p)` *back* from the centre (rotated by bearing) at height
    /// `alt·cos(p)`. Sampling terrain only at the centre (the old behaviour)
    /// misses a ridge the eye swings over — so we sample the whole centre→eye
    /// segment and clear the HIGHEST relief under it.
    /// The largest pitch (degrees) that keeps the eye a clearance above the
    /// relief along the centre→eye segment, or `None` when there's nothing to
    /// clear (no terrain, DEM not resident, sea level, degenerate altitude).
    /// See [`clamp_pitch_above_terrain`] for the geometry; this is the pure
    /// limit so the tilt gesture can clamp its *delta* against it (keeping the
    /// focus re-pin honest) instead of only post-clamping after the re-pin.
    fn terrain_pitch_max(&self) -> Option<f64> {
        self.terrain.data.as_ref()?;
        let vp = self.viewport_px;
        let alt = self.cam.camera.altitude_world(vp) as f64;
        if alt <= 1e-9 {
            return None;
        }
        let center = self.cam.camera.center.to_world();
        let pitch = self.cam.camera.pitch_deg.to_radians();
        // Eye horizontal offset from the centre (world units), matching the
        // view matrix's eye construction: bearing_rot · (0, alt·sin(pitch)).
        let (sb, cb) = self.cam.camera.bearing_deg.to_radians().sin_cos();
        let horiz = alt * pitch.sin();
        let (ex, ey) = (-horiz * sb, horiz * cb);
        // Highest terrain along centre→eye (the binding obstacle).
        let mut terrain_z = self.ground_world_z(center);
        const SEG_SAMPLES: u32 = 6;
        for k in 1..=SEG_SAMPLES {
            let t = k as f64 / SEG_SAMPLES as f64;
            let p = WorldPoint::new(center.x + ex * t, center.y + ey * t);
            terrain_z = terrain_z.max(self.ground_world_z(p));
        }
        if terrain_z <= 0.0 {
            return None; // sea level / DEM not resident yet → nothing to clear
        }
        // Require eye_z = alt·cos(pitch) ≥ terrain_z + clearance.
        let clearance = 0.20 * alt as f32;
        let cos_max = (((terrain_z + clearance) as f64) / alt).clamp(0.0, 1.0);
        // Guard the acos domain: a non-finite ratio (e.g. terrain_z/alt NaN)
        // would make pitch NaN → NaN view matrix → GPU hang. Skip if so.
        if !cos_max.is_finite() {
            return None;
        }
        Some(cos_max.acos().to_degrees().max(0.0))
    }

    fn clamp_pitch_above_terrain(&mut self) {
        if let Some(pitch_max) = self.terrain_pitch_max() {
            if self.cam.camera.pitch_deg > pitch_max {
                self.cam.camera.pitch_deg = pitch_max;
            }
        }
    }

    fn sync_scenes(&mut self) {
        self.clamp_pitch_above_terrain();
        for l in self.basemap.layers.iter_mut() {
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
                // shared terrain scene at render time. Custom layers get
                // the camera per frame via `CustomFrameCtx`.
                LayerEntry::Hillshade(_) | LayerEntry::Custom(_) => {}
            }
        }
        if let Some(t) = self.terrain.data.as_mut() {
            t.scene.set_camera(self.cam.camera);
            t.scene.set_viewport_px(self.viewport_px);
        }
    }

    // ---- coordinate conversion -----------------------------------------

    pub fn screen_to_lng_lat(&self, screen_px: (f64, f64)) -> LatLng {
        let (w, h) = self.viewport_px;
        let world = self
            .cam
            .camera
            .pixel_to_world(screen_px, (w as f64, h as f64));
        world.to_lat_lng()
    }

    /// Terrain-aware screen→ground: where the view ray through `screen_px`
    /// actually hits the relief (NOT the flat z=0 plane that [`screen_to_lng_lat`]
    /// uses).
    ///
    /// This closes the projection round-trip in 3D: [`lng_lat_to_screen`] lifts a
    /// geo point onto the terrain surface (`world_to_screen_z` + `ground_world_z`),
    /// so the inverse must intersect that same surface or a dragged marker lands
    /// at the wrong geo point (the flat unproject drifts further off with tilt and
    /// relief). Marches the view ray against the *stable* DEM sampler — the same
    /// one `ground_world_z` lifts with — so the dropped marker re-projects back to
    /// the exact pixel it was dropped at.
    ///
    /// Used only for marker placement/drag. The flat [`screen_to_lng_lat`] /
    /// `pixel_to_world` path is intentionally kept for panning + freehand capture
    /// (a terrain-aware pan skitters as the finger crosses ridges).
    ///
    /// Falls back to the flat plane (`hit_terrain = false`) when there's no
    /// terrain, the camera is top-down (vertical ray → identical xy anyway), the
    /// ray never crosses the surface (sky / over the horizon), or the covering DEM
    /// isn't resident yet.
    pub fn screen_to_ground_lng_lat(&self, screen_px: (f64, f64)) -> GroundHit {
        let (w, h) = self.viewport_px;
        let vp = (w as f64, h as f64);
        let flat = || GroundHit {
            lng_lat: self.cam.camera.pixel_to_world(screen_px, vp).to_lat_lng(),
            world_z: 0.0,
            hit_terrain: false,
        };
        // No relief to march, or a vertical (top-down) ray that hits the surface
        // at the same xy regardless of height → the flat unproject is exact.
        if self.terrain.data.is_none() || self.cam.camera.pitch_deg == 0.0 {
            return flat();
        }
        let origin = self.cam.camera.center.to_world();
        let Some((near, dir)) = self.cam.camera.pixel_ray_from_origin(screen_px, vp, origin) else {
            return flat();
        };
        let dir = dir.normalize_or_zero();
        if dir == glam::Vec3::ZERO {
            return flat();
        }
        // Surface height (world-z) under the ray at parameter `s`, via the same
        // stable sampler `ground_world_z` lifts with → round-trip closes.
        let surf_z = |s: f32| -> f32 {
            let wx = origin.x + (near.x + dir.x * s) as f64;
            let wy = origin.y + (near.y + dir.y * s) as f64;
            self.ground_world_z(WorldPoint::new(wx, wy))
        };
        // Signed gap: ray height above the surface. >0 = above, crossing at 0.
        let gap = |s: f32| near.z + dir.z * s - surf_z(s);

        // The ray must start above the surface and descend toward it.
        let dz = -dir.z; // descent rate per unit s (dir.z < 0 looking down)
        let mut g0 = gap(0.0);
        if g0 <= 0.0 || dz <= 1.0e-6 {
            return flat();
        }
        // Sphere-trace: advance by the distance the ray would need to reach the
        // CURRENT surface height (`gap/dz`), damped slightly. Over empty air above
        // terrain this takes big strides; near the surface (and as the ground
        // rises ahead, shrinking the gap) it slows automatically — so it converges
        // in a handful of steps without a tiny fixed step, and a rising ridge
        // shrinks the gap rather than being skipped. Bounded by ~2× the flat-plane
        // crossing distance so a grazing/sky ray bails to the flat fallback.
        let s_flat = g0 / dz; // ≈ flat-plane crossing distance (terrain is a small lift)
        let ppw = self.cam.camera.pixels_per_world_unit().max(1e-9);
        let min_step = (1.0 / ppw) as f32; // ~1 px floor so it never stalls
        let mut s = 0.0f32;
        for _ in 0..512 {
            // Cone-bounded sphere trace. The bare `gap/dz` stride is only safe if
            // the surface can't rise faster than the ray descends — FALSE for a
            // mountain rising toward the camera: over the low ground in front of a
            // peak the gap is huge, so an unbounded stride leaps clean over the
            // near face and the trace then "hits" the valley/terrain BEHIND it.
            // Cap each stride to ~3% of the distance already travelled (a cone
            // march): tiny near the camera so a near ridge is sampled, growing
            // with distance so the horizon is still reached in a bounded step
            // count. This is what makes a click land on the FIRST surface.
            let cap = (s * 0.03).max(min_step);
            let advance = (g0 / dz * 0.8).clamp(min_step, cap);
            let s_next = s + advance;
            let g = gap(s_next);
            if g <= 0.0 {
                // Bracketed [s, s_next]; bisect for a sub-pixel crossing.
                let (mut lo, mut hi) = (s, s_next);
                for _ in 0..8 {
                    let mid = 0.5 * (lo + hi);
                    if gap(mid) > 0.0 {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let s_hit = 0.5 * (lo + hi);
                let wx = origin.x + (near.x + dir.x * s_hit) as f64;
                let wy = origin.y + (near.y + dir.y * s_hit) as f64;
                return GroundHit {
                    lng_lat: WorldPoint::new(wx, wy).to_lat_lng(),
                    world_z: surf_z(s_hit),
                    hit_terrain: true,
                };
            }
            s = s_next;
            g0 = g;
            if s > s_flat * 2.0 {
                break;
            }
        }
        flat()
    }

    /// Geo → screen pixel, or `None` when the point can't be projected to a
    /// usable pixel: it's behind the camera (`clip.w <= 0`, where the
    /// projection flips/explodes) or the result is non-finite. Overlays (the
    /// user-location dot, the route line, waypoint handles) MUST hide such
    /// points — projecting them to the centre fallback below is what pinned the
    /// blue dot to screen-centre and made route lines radiate from the middle.
    ///
    /// When 3D terrain is loaded, lifts the point onto the surface so
    /// markers/waypoints/photo-pins sit *on* the relief, not on the flat z=0
    /// plane (which makes them float or sink under tilt). `ground_world_z` is
    /// the same displaced-z the terrain mesh uses, so they land exactly where
    /// the ground is drawn.
    pub fn try_lng_lat_to_screen(&self, lng_lat: LatLng) -> Option<(f64, f64)> {
        let world = lng_lat.to_world();
        let (w, h) = self.viewport_px;
        let vp = (w as f64, h as f64);
        let proj = if self.terrain.data.is_some() {
            self.cam
                .camera
                .world_to_screen_z(world, self.ground_world_z(world), vp)
        } else {
            self.cam.camera.world_to_screen(world, vp)
        }?;
        (proj.0.is_finite() && proj.1.is_finite()).then_some(proj)
    }

    /// Geo → screen pixel with a deterministic centre fallback for
    /// off-screen / behind-camera points, so callers that need a value rather
    /// than an `Option` (the marker hit-test) never panic. Visibility-sensitive
    /// callers (overlays) want [`try_lng_lat_to_screen`] instead.
    pub fn lng_lat_to_screen(&self, lng_lat: LatLng) -> (f64, f64) {
        let (w, h) = self.viewport_px;
        let centre = (w as f64 * 0.5, h as f64 * 0.5);
        self.try_lng_lat_to_screen(lng_lat).unwrap_or(centre)
    }

    /// World-space displaced height (z) of the ground at `world`, matching
    /// the terrain mesh: `elevation · meters_to_world · exaggeration`. 0
    /// when no terrain is registered or no covering DEM tile is resident
    /// yet. `meters_to_world` is taken at the camera-centre latitude, the
    /// same value the per-frame mesh displacement uses, so overlays align
    /// with the drawn surface. Elevation comes from the [`Surface`] query
    /// (plan D3) — this function never knows the ground is a texture.
    fn ground_world_z(&self, world: WorldPoint) -> f32 {
        let Some(t) = self.terrain.data.as_ref() else {
            return 0.0;
        };
        // `meters_to_world` at the camera-centre latitude — the same factor the
        // per-frame mesh displacement uses, so overlays land on the surface.
        let lat = self.cam.camera.center.lat.to_radians();
        let m2w = (lat.cos().abs() as f32 / 40_075_017.0).max(1e-12);
        let surface: &dyn Surface = t;
        match surface.elevation_at(world) {
            Some(elev_m) => elev_m * m2w * t.options.exaggeration,
            None => 0.0,
        }
    }

    // ---- tile orchestration --------------------------------------------

    /// Aggregate pending tiles across all layers, in priority order. Each
    /// entry carries the layer id so the host can route
    /// `ingest_raster`/`ingest_vector_mesh` back correctly.
    ///
    /// **Legacy shim** (plan slice B3.2): the start-only projection of
    /// [`Map::streaming_plan`], for hosts that fetch on their own initiative
    /// and cannot yet cancel. New hosts should consume the plan: it
    /// additionally tracks each fetch attempt (`RequestId`) and names the
    /// in-flight work the camera has moved away from.
    pub fn pending_tiles(&self) -> Vec<PendingTile> {
        self.plan_selection().into_iter().map(|(p, _)| p).collect()
    }

    /// One streaming step for plan-driven hosts: the fetches to `start`
    /// (priority-ordered, truncated to `max_start`, each with a minted
    /// [`turbomap_world::RequestId`]) and the in-flight attempts to `cancel`
    /// (the camera moved away — the transport should abort them and report
    /// [`Map::fetch_cancelled`]). Deliveries go through the existing
    /// `ingest_*` calls (which complete the attempt); failures through
    /// [`Map::fetch_failed`].
    pub fn streaming_plan(&mut self, max_start: usize) -> StreamingPlan {
        let selection = self.plan_selection();
        let mut lc = self.lifecycle.borrow_mut();
        let mut start = Vec::new();
        for (p, _) in selection {
            if start.len() >= max_start {
                break;
            }
            let key = self.world_key_of_pending(&p);
            // Only chunks with no live attempt start a new one; the rest of
            // the selection is already in flight.
            if lc.phase_of(key) == Some(turbomap_world::Phase::Desired) {
                if let Ok(id) = lc.fetch_started(key) {
                    start.push(FetchRequest { id, fetch: p });
                }
            }
        }
        let cancel = lc.cancelable().into_iter().map(|(_, id)| id).collect();
        StreamingPlan { start, cancel }
    }

    /// A plan-issued fetch attempt failed (network error, decode error). The
    /// chunk re-pends if still wanted; retry pacing stays host policy for
    /// now (B4 moves it behind the budgets).
    pub fn fetch_failed(&mut self, request: turbomap_world::RequestId) {
        let mut lc = self.lifecycle.borrow_mut();
        if let Some(key) = lc.key_of_request(request) {
            let _ = lc.failed(key, request);
        }
    }

    /// The host honoured a `cancel` entry (or abandoned an attempt).
    pub fn fetch_cancelled(&mut self, request: turbomap_world::RequestId) {
        let mut lc = self.lifecycle.borrow_mut();
        if let Some(key) = lc.key_of_request(request) {
            let _ = lc.cancelled(key, request);
        }
    }

    /// The shared selection behind [`Map::pending_tiles`] and
    /// [`Map::streaming_plan`]: score, order, and dual-write-sync the
    /// lifecycle table.
    fn plan_selection(&self) -> Vec<(PendingTile, turbomap_world::Priority)> {
        // Merge EVERY layer's pending tiles into one list tagged with its
        // streaming tier + distance-to-eye, then sort GLOBALLY. Per-layer the
        // lists were already (tier, distance)-ordered, but emitting them
        // layer-by-layer meant the host fetched every tile of one source (the
        // fast imagery) before any of the next (the slow DEM the 3D relief
        // needs) — so near relief loaded last. A global sort interleaves them:
        // the Overview floor first (anti-flicker), then the nearest, in-front
        // tile of ANY layer first. (The desktop host already re-sorted like
        // this; now the engine does it for every host.)
        let mut tagged: Vec<(PendingTile, crate::scene::TileTier, f32)> = Vec::new();
        for l in self.basemap.layers.iter() {
            match l {
                LayerEntry::Raster(r) if r.visible => {
                    let res = |t: &TileId| self.chunk_is_resident(self.world_key(Some(&r.id), *t));
                    for (tile, tier, d) in r.scene.pending_prioritized(&res) {
                        tagged.push((
                            PendingTile::Raster {
                                layer_id: r.id.clone(),
                                tile,
                            },
                            tier,
                            d,
                        ));
                    }
                }
                LayerEntry::Vector(v) if v.visible => {
                    let res = |t: &TileId| self.chunk_is_resident(self.world_key(Some(&v.id), *t));
                    for (tile, tier, d) in v.scene.pending_prioritized(&res) {
                        tagged.push((
                            PendingTile::Vector {
                                layer_id: v.id.clone(),
                                tile,
                            },
                            tier,
                            d,
                        ));
                    }
                }
                // Hillshade no longer fetches its own DEM tiles —
                // the shared terrain source below handles that.
                LayerEntry::Hillshade(_) => {}
                _ => {}
            }
        }
        if let Some(t) = self.terrain.data.as_ref() {
            let res = |t: &TileId| self.chunk_is_resident(self.world_key(None, *t));
            for (tile, tier, d) in t.scene.pending_prioritized(&res) {
                tagged.push((PendingTile::Terrain { tile }, tier, d));
            }
        }
        // Order by the ONE explainable score (`turbomap_world::priority`,
        // plan slice B2): tier is the law, effective distance decides within
        // a tier, and "effective" folds in the camera's travel direction so
        // the map streams where the user is heading. With a stationary
        // camera the score reproduces the historical `(tier, distance²)`
        // order exactly — pinned by `scene::tests::pending_priority_matches_
        // the_historical_order_when_stationary`.
        let eye = {
            let centre = self.cam.camera.center.to_world();
            let off = self.cam.camera.eye_offset_world(self.viewport_px);
            WorldPoint::new(centre.x + off[0] as f64, centre.y + off[1] as f64)
        };
        let travel = match self.last_priority_eye.replace(Some(eye)) {
            Some(prev) => {
                let (dx, dy) = (eye.x - prev.x, eye.y - prev.y);
                let len = (dx * dx + dy * dy).sqrt();
                // Sub-nanoworld jitter is "stationary", not a direction.
                (len > 1e-12).then(|| (dx / len, dy / len))
            }
            None => None,
        };
        let world_tier = |t: crate::scene::TileTier| match t {
            crate::scene::TileTier::Overview => turbomap_world::Tier::Overview,
            crate::scene::TileTier::Visible => turbomap_world::Tier::Visible,
            crate::scene::TileTier::Prefetch => turbomap_world::Tier::Prefetch,
        };
        let tile_of = |p: &PendingTile| match p {
            PendingTile::Raster { tile, .. }
            | PendingTile::Vector { tile, .. }
            | PendingTile::Hillshade { tile, .. }
            | PendingTile::Terrain { tile } => *tile,
        };
        let mut scored: Vec<(PendingTile, turbomap_world::Priority)> = tagged
            .into_iter()
            .map(|(p, tier, dist_sq)| {
                let alignment = match travel {
                    Some((vx, vy)) => {
                        let t = tile_of(&p);
                        let (nw, se) = t.world_bounds();
                        let (cx, cy) = ((nw.x + se.x) * 0.5, (nw.y + se.y) * 0.5);
                        let (dx, dy) = (cx - eye.x, cy - eye.y);
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 1e-12 {
                            ((vx * dx + vy * dy) / len) as f32
                        } else {
                            0.0
                        }
                    }
                    None => 0.0,
                };
                let eff = turbomap_world::priority::effective_distance_sq(dist_sq, alignment);
                (p, turbomap_world::priority::score(world_tier(tier), eff))
            })
            .collect();
        scored.sort_by_key(|&(_, prio)| prio);

        // ---- Slice B3.1 dual-write: sync the lifecycle table -------------
        // The table's want-set mirrors the SCENES' full desired sets (all
        // layers with scenes + terrain, visibility-independent — the same
        // universe `tile_histogram` counts), so `lifecycle_agreement()` can
        // hold exactly. Priorities are refreshed from this frame's scores
        // for the pending subset; already-resident chunks just stay wanted.
        {
            let frame = self.lifecycle_frame.get() + 1;
            self.lifecycle_frame.set(frame);
            let mut lc = self.lifecycle.borrow_mut();
            let mut wanted: std::collections::HashSet<turbomap_world::ChunkKey> =
                std::collections::HashSet::new();
            let want_scene = |lc: &mut turbomap_world::Lifecycle,
                              wanted: &mut std::collections::HashSet<turbomap_world::ChunkKey>,
                              layer_id: Option<&str>,
                              scene: &crate::scene::Scene| {
                for (tile, _) in scene.desired_tagged() {
                    let key = self.world_key(layer_id, tile);
                    wanted.insert(key);
                    let _ = lc.want(key, u64::MAX, frame);
                }
            };
            for l in self.basemap.layers.iter() {
                match l {
                    LayerEntry::Raster(r) => {
                        want_scene(&mut lc, &mut wanted, Some(&r.id), &r.scene)
                    }
                    LayerEntry::Vector(v) => {
                        want_scene(&mut lc, &mut wanted, Some(&v.id), &v.scene)
                    }
                    LayerEntry::Hillshade(_) | LayerEntry::Custom(_) => {}
                }
            }
            if let Some(t) = self.terrain.data.as_ref() {
                want_scene(&mut lc, &mut wanted, None, &t.scene);
            }
            for &(ref p, prio) in &scored {
                let _ = lc.want(self.world_key_of_pending(p), prio.0, frame);
            }
            lc.retain_wanted(|k| wanted.contains(&k));
        }

        scored
    }

    fn world_key_of_pending(&self, p: &PendingTile) -> turbomap_world::ChunkKey {
        match p {
            PendingTile::Raster { layer_id, tile } | PendingTile::Vector { layer_id, tile } => {
                self.world_key(Some(layer_id), *tile)
            }
            PendingTile::Hillshade { tile, .. } => self.world_key(None, *tile),
            PendingTile::Terrain { tile } => self.world_key(None, *tile),
        }
    }

    fn first_visible_layer_index(&self) -> Option<usize> {
        self.basemap.layers.first_visible_index()
    }

    /// Back-compat shim. Hillshade no longer owns its own DEM cache —
    /// the data goes to the shared terrain cache, so this just forwards
    /// to [`Map::ingest_terrain_tile`]. The `layer_id` argument is
    /// kept for source compatibility but ignored.
    pub fn ingest_hillshade(
        &mut self,
        _layer_id: &str,
        tile: TileId,
        dem: &crate::dem::DecodedDem,
    ) {
        self.ingest_terrain_tile(tile, dem);
    }

    /// Whether a raster layer already holds `tile` resident. The engine's
    /// async decode queue consults this before re-ingesting: a delivery
    /// that raced the accept→apply window must not re-upload a resident
    /// tile (it would restart the fade — steady-state flicker).
    ///
    /// Slice B3.4 (first step): answered by the LIFECYCLE TABLE, not the
    /// per-scene sets — the dual-write soak (agreement asserted every sim
    /// frame across the whole B4 campaign) proved `Resident|Retained` in
    /// the table equals the scenes' `ingested`, so residency truth now has
    /// one owner. The per-scene sets remain only as the desired-set filter
    /// until the follow-up deletes them.
    pub fn is_raster_ingested(&self, layer_id: &str, tile: TileId) -> bool {
        self.chunk_is_resident(self.world_key(Some(layer_id), tile))
    }

    /// Terrain twin of [`Map::is_raster_ingested`].
    pub fn is_terrain_ingested(&self, tile: TileId) -> bool {
        self.terrain.data.is_some() && self.chunk_is_resident(self.world_key(None, tile))
    }

    /// Vector twin of [`Map::is_raster_ingested`].
    pub fn is_vector_ingested(&self, layer_id: &str, tile: TileId) -> bool {
        self.chunk_is_resident(self.world_key(Some(layer_id), tile))
    }

    fn chunk_is_resident(&self, key: turbomap_world::ChunkKey) -> bool {
        matches!(
            self.lifecycle.borrow().phase_of(key),
            Some(turbomap_world::Phase::Resident | turbomap_world::Phase::Retained)
        )
    }

    /// The style a vector layer would tessellate against right now — a
    /// clone for off-thread tessellation (the engine's decode workers).
    pub fn vector_layer_style(&self, layer_id: &str) -> Option<VectorStyle> {
        self.basemap.layers.iter().find_map(|l| match l {
            LayerEntry::Vector(v) if v.id == layer_id => Some(v.style.clone()),
            _ => None,
        })
    }

    pub fn ingest_raster(&mut self, layer_id: &str, tile: TileId, rgba: &[u8], w: u32, h: u32) {
        let mut delivered: Option<Vec<TileId>> = None;
        for l in self.basemap.layers.iter_mut() {
            if let LayerEntry::Raster(r) = l {
                if r.id == layer_id {
                    let evicted = r.cache.insert(tile, rgba, w, h);
                    // Keep the "ingested" set in step with what the cache
                    // actually holds — evicted tiles must become re-
                    // requestable, or they grey out permanently.
                    delivered = Some(evicted);
                    break;
                }
            }
        }
        if let Some(evicted) = delivered {
            self.lifecycle_delivered(Some(layer_id), tile, rgba.len() as u64, &evicted);
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
        let mut delivered: Option<Vec<TileId>> = None;
        for l in self.basemap.layers.iter_mut() {
            if let LayerEntry::Vector(v) = l {
                if v.id == layer_id {
                    let evicted = v.cache.insert(tile, mesh, labels, icons, interactive);
                    delivered = Some(evicted);
                    break;
                }
            }
        }
        if let Some(evicted) = delivered {
            let bytes = (mesh.vertices.len()
                * std::mem::size_of::<crate::tessellate::VectorVertex>()
                + mesh.indices.len() * 4) as u64;
            self.lifecycle_delivered(Some(layer_id), tile, bytes, &evicted);
        }
    }

    /// Convenience: tessellate `tile` against the layer's current style on
    /// the calling thread, then ingest. Useful for testing and small hosts
    /// that don't want to run the tessellator off-thread.
    pub fn ingest_vector_tile(&mut self, layer_id: &str, tile_id: TileId, tile: &VectorTile) {
        // Need to extract style+id first, then run tessellate, then store
        // — to avoid overlapping borrows.
        let style_opt = self.basemap.layers.iter().find_map(|l| match l {
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
        self.basemap.layers.set_visibility(id, visible)
    }

    pub fn layer_visibility(&self, id: &str) -> Option<bool> {
        self.basemap.layers.visibility(id)
    }

    /// Set the per-layer fade-in duration. Lets the UI tune the
    /// cross-tile blend feel at runtime — 0 disables fading entirely
    /// (instant tile-pop), higher values smear arrivals into a
    /// longer crossfade.
    pub fn set_layer_fade_in(&mut self, id: &str, secs: f32) -> bool {
        self.basemap.layers.set_fade_in(id, secs)
    }

    // ---- fonts ---------------------------------------------------------

    /// Register a fallback font face (owned bytes) for scripts the bundled
    /// default doesn't cover (CJK, Arabic, …). Returns `false` if the bytes
    /// don't parse. Faces added earlier win where they have coverage, so
    /// the bundled Latin face is always preferred for Latin text.
    pub fn add_fallback_font(&mut self, bytes: Vec<u8>) -> bool {
        self.symbols.text.add_fallback_face(bytes)
    }

    // ---- markers -------------------------------------------------------

    pub fn add_marker(&mut self, marker: Marker) -> MarkerId {
        self.overlays.markers.add(marker)
    }

    pub fn remove_marker(&mut self, id: MarkerId) {
        self.overlays.markers.remove(id);
    }

    pub fn clear_markers(&mut self) {
        self.overlays.markers.clear();
    }

    pub fn markers(&self) -> &[Marker] {
        self.overlays.markers.all()
    }

    // ---- hit testing ---------------------------------------------------

    pub fn hit_test(&self, screen_px: (f64, f64), tolerance_px: f64) -> Vec<HitResult> {
        let mut out: Vec<HitResult> = Vec::new();

        // Markers first (top z-order, newest-first within markers).
        for hit in self
            .overlays
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

        for layer in self.basemap.layers.iter().rev() {
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

    /// Refresh the terrain cast-shadow field if its inputs changed, then patch
    /// `frame.raster_terrain_cfg` so the raster pipeline samples it. Called from
    /// `render` only when `self.terrain.shadow.strength > 0`.
    ///
    /// The field is computed in the camera-relative (RTC) world frame — the
    /// same frame the vertex shader emits — so the fragment shader's UV map
    /// needs no extra camera math and f32 precision holds at deep zoom. The
    /// recompute is gated on a [`ShadowKey`] (sun direction, RTC region, DEM
    /// insert count): orbit/tilt at a fixed centre reuse the cached upload.
    /// Returns the CPU wall-time spent REASSEMBLING the heightfield this frame
    /// (`Duration::ZERO` when it was skipped or the cached field was reused) so
    /// the caller can record it as a distinct phase — it's a render-thread spike
    /// worth isolating from the steady per-frame `prepare`.
    fn update_terrain_shadows(&mut self, frame: &mut RenderFrame) -> std::time::Duration {
        let cfg = frame.raster_terrain_cfg;
        // Cast shadows only make sense on real 3D terrain.
        if cfg.meters_to_world <= 0.0 {
            log::debug!(
                "turbomap shadow: skip — meters_to_world={}",
                cfg.meters_to_world
            );
            return std::time::Duration::ZERO;
        }
        let Some(terrain) = self.terrain.data.as_ref() else {
            log::debug!("turbomap shadow: skip — no terrain");
            return std::time::Duration::ZERO;
        };
        let tiles = terrain.scene.visible_tiles();
        if tiles.is_empty() {
            log::debug!("turbomap shadow: skip — terrain scene has no visible tiles");
            return std::time::Duration::ZERO;
        }
        // Assemble a camera-centred HEIGHTFIELD covering the near/mid field with a
        // margin, and let the fragment shader march it toward the sun per-pixel.
        // The on-screen flat footprint is `viewport / pixels-per-world-unit`; we
        // cover SHADOW_MARGIN× that so a pan stays inside the assembled region
        // (the per-fragment march reads world coords, so it's correct + sharp +
        // welded to the ground anywhere the heightfield covers — no per-pan
        // reassembly, no stale precomputed visibility to pop).
        const SHADOW_MARGIN: f32 = 2.5;
        let cam_origin = self.cam.camera.center.to_world();
        let ppw = self.cam.camera.pixels_per_world_unit() as f32;
        let footprint = (self.viewport_px.0.max(self.viewport_px.1) as f32) / ppw.max(1e-9);
        // Quantise the field extent to a power-of-two ladder. Raw `size_f` shrinks
        // continuously as you zoom in (footprint ∝ 1/2^zoom), so an un-quantised
        // extent re-keys the field EVERY zoom frame — the cell size + lattice shift
        // each frame and the shadow/AO discretisation flickers (panning was fine
        // because the extent is constant there). Snapping to the smallest power of
        // two that still covers the footprint holds the extent — and thus the cell
        // size and the global lattice — fixed across a whole zoom octave, so zoom
        // re-assembles only when crossing an octave (one settle, not a flicker).
        let size_raw = (footprint * SHADOW_MARGIN).clamp(1e-6, 0.5);
        let size_f = (2.0_f32.powf(size_raw.log2().ceil())).min(0.5);
        // Snap the field's lower-left corner to a FIXED global cell lattice
        // (multiples of `cell` in absolute world space) so the heightfield samples
        // the SAME ground points no matter where the camera sits. Without this it
        // re-centres on the exact camera position on every re-assembly — and a
        // finger-drag isn't an "animation", so it re-assembles every frame,
        // shifting the grid sub-cell and making the shadow/AO discretisation
        // jitter (the flicker while moving). Snapping welds the field to the world.
        let dim = HEIGHT_DIM;
        let cell = (size_f / (dim - 1) as f32) as f64;
        // Snap to a MULTIPLE of `cell` (4 cells): still aligned to the global
        // cell lattice (so no sub-cell jitter), but the CPU re-assembly fires only
        // every few cells of movement instead of every single one — keeps the
        // finer 256² grid affordable to re-centre while dragging.
        let snap_q = cell * 4.0;
        let snap = |v: f64| (v / snap_q).floor() * snap_q;
        let origin_abs = [
            snap(cam_origin.x - 0.5 * size_f as f64),
            snap(cam_origin.y - 0.5 * size_f as f64),
        ];
        // Vertical scale for the heightfield. The mesh's `meters_to_world`
        // (= cos(lat)/circ) under-scales true relief by ~1/cos²(lat) — acceptable
        // for the rendered surface, but it flattens slopes so far that terrain
        // only occludes the sun near the horizon. For shadows to reflect the REAL
        // steepness (and fall on the correct ground footprint), use the
        // geometrically-correct vertical — world-z per metre = 1/(circ·cos lat),
        // matching the world-xy ground scale at this latitude. The user's
        // exaggeration carries over so shadows track the dialed-in relief.
        let lat = self.cam.camera.center.lat.to_radians();
        let earth_circ = 40_075_017.0_f64;
        let coslat = lat.cos().abs().max(1.0e-6);
        let zscale = (cfg.exaggeration as f64 / (earth_circ * coslat)) as f32;
        let sun_dir = cfg.sun_dir;
        let dem_inserts = terrain.cache.stats().inserts;

        let key = ShadowKey {
            sun: [
                sun_dir[0].to_bits(),
                sun_dir[1].to_bits(),
                sun_dir[2].to_bits(),
            ],
            // Snapped lattice origin + grid size: changes only when the camera
            // crosses a whole cell, and always on the same global lattice, so the
            // field re-assembles seldom and never sub-cell-jitters.
            origin: [
                (origin_abs[0] as f32).to_bits(),
                (origin_abs[1] as f32).to_bits(),
            ],
            size: size_f.to_bits(),
            dem_inserts,
        };

        // Re-assemble the cross-tile heightfield only when the camera has SETTLED
        // in a new region (or the sun / resident DEM changed) — never mid-pan, so
        // the dim² tile-cache walk can't hitch a panning frame. The shader marches
        // the held field every frame, so panning within the assembled region needs
        // no reassembly and stays welded to the ground.
        let animating = self.cam.active.is_some();
        let mut assemble = std::time::Duration::ZERO;
        // Start (or restart) a PROGRESSIVE build when the wanted field differs
        // from both the committed field and any in-flight build. Skipped while a
        // camera animation runs (the field would be obsolete before it finished);
        // a finger-pan isn't an "animation", so a build makes progress across the
        // pan and simply restarts if the region keeps crossing the lattice.
        // "Moving" = an in-flight animation (fling/ease) OR the camera pose
        // changed since the last render (a finger-pan). Both should DEFER a
        // replacement reassembly so the drag/fling stays smooth; the field
        // settles in once motion stops.
        let pose = {
            let c = &self.cam.camera;
            [
                c.center.lat.to_bits(),
                c.center.lng.to_bits(),
                c.zoom.to_bits(),
                c.pitch_deg.to_bits(),
                c.bearing_deg.to_bits(),
            ]
        };
        let moving = animating || self.terrain.shadow.last_pose != Some(pose);
        self.terrain.shadow.last_pose = Some(pose);

        let have_this = self.terrain.shadow.key.as_ref() == Some(&key);
        let building_this = self.terrain.shadow.build.as_ref().map(|b| &b.key) == Some(&key);
        if !have_this && !building_this {
            if self.terrain.shadow.key.is_none() {
                // FIRST field: assemble synchronously (gated only on not-animating)
                // so shadows are present on the first settled frame — there's no
                // previous field to keep bound meanwhile, and a single-frame
                // screenshot / the harness shadow proof depends on it.
                if !animating {
                    let t0 = Instant::now();
                    let mut heights = vec![0.0f32; dim * dim];
                    let surface: &dyn Surface = terrain;
                    surface.sample_height_rows(
                        (origin_abs[0], origin_abs[1]),
                        cell,
                        dim,
                        0,
                        dim,
                        &mut |idx, e| {
                            heights[idx] = e.unwrap_or(0.0) * zscale;
                        },
                    );
                    debug_shadow_relief(
                        "sync",
                        &heights,
                        size_f,
                        self.cam.camera.zoom,
                        terrain.cache.finest_resident_zoom(),
                    );
                    self.terrain.shadow_map.upload_heights(&heights);
                    self.terrain.shadow.origin_abs = origin_abs;
                    self.terrain.shadow.world_size = size_f;
                    self.terrain.shadow.key = Some(key.clone());
                    assemble = t0.elapsed();
                }
            } else if !moving {
                // REPLACEMENT: only start once the camera has SETTLED (not mid
                // pan/fling), then amortise over frames. The old field stays
                // bound meanwhile, so the move itself never pays the reassembly.
                self.terrain.shadow.build = Some(ShadowBuild {
                    key: key.clone(),
                    origin_abs,
                    size_f,
                    cell,
                    zscale,
                    heights: vec![0.0f32; dim * dim],
                    next_row: 0,
                });
            }
        }
        // Advance an in-flight build by a chunk of rows; commit (upload) when the
        // last row lands. CHUNK_ROWS = 64 → a 256² field assembles over 4 frames,
        // each ~¼ the old single-frame cost — so a region change never blocks one
        // frame on the whole cross-tile walk (the settle hitch). The finest DEM
        // zoom is still resolved once per chunk; behaviour-identical output.
        if let Some(mut b) = self.terrain.shadow.build.take() {
            let t0 = Instant::now();
            const CHUNK_ROWS: usize = 64;
            let row1 = (b.next_row + CHUNK_ROWS).min(dim);
            let (o, c, zs, row0) = (b.origin_abs, b.cell, b.zscale, b.next_row);
            let surface: &dyn Surface = terrain;
            surface.sample_height_rows((o[0], o[1]), c, dim, row0, row1, &mut |idx, e| {
                b.heights[idx] = e.unwrap_or(0.0) * zs;
            });
            b.next_row = row1;
            if b.next_row >= dim {
                debug_shadow_relief(
                    "prog",
                    &b.heights,
                    b.size_f,
                    self.cam.camera.zoom,
                    terrain.cache.finest_resident_zoom(),
                );
                self.terrain.shadow_map.upload_heights(&b.heights);
                // ABSOLUTE world space (lattice-snapped); the per-frame block below
                // rebases it into the current RTC frame so it stays welded.
                self.terrain.shadow.origin_abs = b.origin_abs;
                self.terrain.shadow.world_size = b.size_f;
                self.terrain.shadow.key = Some(b.key);
                // Build complete — already `take`n out, so leave `build = None`.
            } else {
                self.terrain.shadow.build = Some(b);
            }
            assemble = t0.elapsed();
        }

        // Feed the per-frame shadow uniforms once a heightfield exists. The march
        // step (one texel) + softness scale with the assembled region, and the
        // origin is rebased into THIS frame's RTC frame every frame so the shadow
        // stays pinned to the terrain through a pan instead of sliding.
        if self.terrain.shadow.key.is_some() {
            let cam_now = self.cam.camera.center.to_world();
            frame.raster_terrain_cfg.shadow_origin = [
                (self.terrain.shadow.origin_abs[0] - cam_now.x) as f32,
                (self.terrain.shadow.origin_abs[1] - cam_now.y) as f32,
            ];
            frame.raster_terrain_cfg.shadow_inv_size = 1.0 / self.terrain.shadow.world_size;
            frame.raster_terrain_cfg.shadow_texel_world =
                self.terrain.shadow.world_size / HEIGHT_DIM as f32;
            // Base penumbra band (world-z): ~10 m of relief excess fades lit→shadow
            // at contact. The shader's contact-hardening widens this with occluder
            // distance, so near edges stay crisp and far ridges throw soft shadows.
            frame.raster_terrain_cfg.shadow_softness = (45.0 * zscale).max(1e-7);
            frame.raster_terrain_cfg.shadow_strength = self.terrain.shadow.strength;
        }
        assemble
    }

    // `draw_layers` is gone (slice D1): each layer's draw is registered as
    // its own named graph node (`layer:<id>`) directly in `render`, so the
    // pass report times layers individually and any one can be masked off.

    /// Draw the route/track 3-D tubes into `pass` (after ground layers so the
    /// terrain occludes them, before screen-space overlays).
    fn draw_route_tubes(&self, pass: &mut wgpu::RenderPass<'_>, frame: &RenderFrame) {
        let cam_origin = self.cam.camera.center.to_world();
        let vp = self
            .cam
            .camera
            .view_projection_matrix_rtc(cam_origin, self.viewport_px);
        let origin_delta = [
            (self.overlays.route_tubes.origin.0 - cam_origin.x) as f32,
            (self.overlays.route_tubes.origin.1 - cam_origin.y) as f32,
        ];
        let cfg = &frame.raster_terrain_cfg;
        let sun = cfg.sun_dir;
        let sun_dir = if sun[0] == 0.0 && sun[1] == 0.0 && sun[2] == 0.0 {
            [0.4, 0.4, 0.82]
        } else {
            sun
        };
        let lc = cfg.light_color;
        let light = if lc[0] + lc[1] + lc[2] < 0.01 {
            [1.0, 1.0, 1.0]
        } else {
            lc
        };
        let ppw = (256.0 * 2f64.powf(self.cam.camera.zoom)) as f32;
        let radius_px = if self.overlays.route_tubes.radius_px > 0.0 {
            self.overlays.route_tubes.radius_px
        } else {
            7.0
        };
        self.overlays.route_pipeline.draw(
            vp,
            origin_delta,
            ppw,
            radius_px,
            1.3,
            sun_dir,
            cfg.ambient.max(0.4),
            light,
            pass,
        );
    }

    // `draw_overlays` is gone (slice D1): icons, text and markers are their
    // own OverlayMsaa graph nodes registered in `render` (same order:
    // icons under labels, then labels, then markers).

    // ---- The frame's pass set (slice D1) --------------------------------
    // Every pass the renderer can run, with its phase and declared data
    // flow. Registration order in `render` is painter's order; the graph
    // schedules phases and validates that every non-persistent read has a
    // producer this frame. `HeightField`/`AoField` are persistent (assembled
    // incrementally, cached across frames), so consumers may run on frames
    // where the producers did nothing — that's the streaming model, not a
    // hazard. The shadow *uniforms* flow to layers via `RenderFrame`, which
    // zeroes them when no heightfield exists — a safe default, so layers
    // don't declare a hard read on them.
    const PASS_SHADOW_ASSEMBLE: PassDesc = PassDesc {
        name: "shadow-assemble",
        phase: FramePhase::BeforeFrame,
        reads: &[],
        writes: &[Res::HeightField, Res::ShadowUniforms],
    };
    const PASS_AO_ACCUMULATE: PassDesc = PassDesc {
        name: "ao-accumulate",
        phase: FramePhase::BeforeFrame,
        reads: &[Res::HeightField],
        writes: &[Res::AoField],
    };
    const PASS_SKY: PassDesc = PassDesc {
        name: "sky",
        phase: FramePhase::GroundMsaa,
        reads: &[],
        writes: &[Res::ColorMsaa],
    };
    const PASS_FLOOR: PassDesc = PassDesc {
        name: "floor",
        phase: FramePhase::GroundMsaa,
        reads: &[],
        writes: &[Res::ColorMsaa, Res::Depth],
    };
    const PASS_LAYER: PassDesc = PassDesc {
        name: "layer",
        phase: FramePhase::GroundMsaa,
        reads: &[Res::HeightField, Res::AoField],
        writes: &[Res::ColorMsaa, Res::Depth],
    };
    /// Custom layers (plan D4), one node per layer (`custom:<id>`), in the
    /// declared phase. Two descriptors of the same kind so mask-by-name
    /// (`custom`) covers both phases.
    const PASS_CUSTOM_GROUND: PassDesc = PassDesc {
        name: "custom",
        phase: FramePhase::GroundMsaa,
        reads: &[],
        writes: &[Res::ColorMsaa, Res::Depth],
    };
    const PASS_CUSTOM_OVERLAY: PassDesc = PassDesc {
        name: "custom",
        phase: FramePhase::OverlayMsaa,
        reads: &[],
        writes: &[Res::ColorMsaa],
    };
    const PASS_ROUTE_TUBES: PassDesc = PassDesc {
        name: "route-tubes",
        phase: FramePhase::GroundMsaa,
        reads: &[Res::HeightField],
        writes: &[Res::ColorMsaa, Res::Depth],
    };
    const PASS_ICONS: PassDesc = PassDesc {
        name: "icons",
        phase: FramePhase::OverlayMsaa,
        reads: &[],
        writes: &[Res::ColorMsaa],
    };
    const PASS_TEXT: PassDesc = PassDesc {
        name: "text",
        phase: FramePhase::OverlayMsaa,
        reads: &[],
        writes: &[Res::ColorMsaa],
    };
    const PASS_MARKERS: PassDesc = PassDesc {
        name: "markers",
        phase: FramePhase::OverlayMsaa,
        reads: &[],
        writes: &[Res::ColorMsaa],
    };
    const PASS_CLOUDS: PassDesc = PassDesc {
        name: "clouds",
        phase: FramePhase::Composite,
        reads: &[Res::FrameTarget],
        writes: &[Res::FrameTarget],
    };

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
        // The frame is a FRAME GRAPH (slice D1, architecture §III.3): every
        // piece of GPU work is a named pass with a phase and declared reads/
        // writes (see the `PASS_*` descriptors above). BeforeFrame passes run
        // at encoder level, all Ground+Overlay contributions share exactly
        // ONE MSAA render pass (on tile-based mobile GPUs every extra pass
        // costs a full framebuffer load/store — the graph makes that rule
        // structural), and Composite passes run over the resolved target.
        // Per-pass CPU timings land in `FrameMetrics::passes`; any pass can
        // be disabled by name for isolation debugging (`set_pass_enabled`).
        //
        // Around the graph, the CPU frame keeps its classic phases:
        //   A. prepare: every visible layer (in order) does its CPU
        //      work — uniform/instance uploads, batch building, LRU
        //      touches — and returns a draw list.
        //   B. pick the frame clear colour (replicating the old
        //      "first visible layer clears" semantics).
        //   C. the graph's MSAA pass replays the prepared draw lists.
        //
        // All per-frame render globals — metres-to-world, the sun-lit
        // atmosphere, the aerial-perspective haze, the sky uniform and the
        // raster/vector terrain configs — derived once from the camera +
        // the frame's ENVIRONMENT (plan E1: one value, sampled by everyone)
        // + terrain. See [`RenderFrame`] and [`Environment`].
        let env = self.environment();
        let frame_time_s = env.time_s;
        // Tick the simulation systems under this frame's Environment (plan
        // E2: the cloud sim derives drift, sun and radar advection from the
        // same value every render pass samples). dt is informational — the
        // systems are pure functions of the clock, which is what makes
        // `set_time_override` replay exact.
        let frame_dt_s = (frame_time_s - self.last_frame_clock.unwrap_or(frame_time_s)).max(0.0);
        self.last_frame_clock = Some(frame_time_s);
        let _sim_active = SimulationSystem::tick(&mut self.atmosphere, frame_dt_s, &env);
        let mut frame = RenderFrame::build(
            &self.cam.camera,
            self.viewport_px,
            &env,
            TerrainFrameInputs {
                present: self.terrain.data.is_some(),
                exaggeration: self
                    .terrain
                    .data
                    .as_ref()
                    .map(|t| t.options.exaggeration)
                    .unwrap_or(1.0),
                halo_px: self
                    .terrain
                    .data
                    .as_ref()
                    .map(|t| t.cache.halo_px())
                    .unwrap_or(0),
            },
            self.atmosphere.sky_enabled,
        );
        // The frame graph: mask snapshot + per-pass bookkeeping for this frame.
        let mut graph = FrameGraph::new(self.pass_mask.clone());

        // Terrain relief field: assemble the camera-centred cross-tile
        // heightfield whenever we have 3D terrain — it drives BOTH cast shadows
        // (per-fragment march, gated by `shadow_strength`) and the world-locked
        // AO bake below. Cheap no-op until the camera settles in a new region or
        // the DEM changes. (Previously gated on `shadow.strength > 0`; AO needs
        // the field even with cast shadows off.)
        let shadow_assemble_time = if self.terrain.data.is_some() {
            graph
                .run_now(&Self::PASS_SHADOW_ASSEMBLE, || {
                    self.update_terrain_shadows(&mut frame)
                })
                .unwrap_or(Duration::ZERO)
        } else {
            std::time::Duration::ZERO
        };

        // Progressive ambient-occlusion bake: refine one direction-batch per
        // frame into the world-locked AO field, then cache it. AO is
        // sun-independent, so the keyed field is reused across the day cycle and
        // recomputed only when the region/DEM changes. Runs after the heightfield
        // upload and before the frame pass that samples the field.
        if self.terrain.data.is_some() && self.terrain.shadow.key.is_some() {
            let key = AoKey {
                origin: [
                    (self.terrain.shadow.origin_abs[0] as f32).to_bits(),
                    (self.terrain.shadow.origin_abs[1] as f32).to_bits(),
                ],
                size: self.terrain.shadow.world_size.to_bits(),
                dem_inserts: self
                    .terrain
                    .data
                    .as_ref()
                    .map(|t| t.cache.stats().inserts)
                    .unwrap_or(0),
            };
            let world_size = self.terrain.shadow.world_size;
            if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
                ts.scope_begin("ao", encoder);
            }
            let t = &mut self.terrain;
            graph.run_encoder(&Self::PASS_AO_ACCUMULATE, encoder, |enc| {
                t.ao.accumulate(
                    enc,
                    &t.shadow_map.height_tex_bind_group,
                    t.shadow_map.ao_view(),
                    key,
                    world_size,
                );
            });
            if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
                ts.scope_end(encoder);
            }
        }

        let first_visible = self.first_visible_layer_index();

        // ---- Phase A: prepare ------------------------------------
        // Split-borrow the parts we need so the loop can mutably
        // borrow `self.basemap.layers` while still passing references into
        // `self.terrain.data`. `terrain_cell` is an Option<&mut Terrain>
        // we reborrow on a per-pipeline basis.
        let prepare_started = Instant::now();
        let mut tiles_drawn = 0usize;
        let mut terrain_cell = self.terrain.data.as_mut();
        let mut prepared_layers: Vec<(usize, PreparedLayer)> =
            Vec::with_capacity(self.basemap.layers.len());
        // One prepared text item per *visible vector layer*, in layer
        // order — preserving the old one-text-pass-per-vector-layer
        // semantics (per-layer label collision sets).
        let mut prepared_text: Vec<PreparedText> = Vec::new();
        let mut prepared_icons: Vec<PreparedIcons> = Vec::new();
        // Per-frame context for custom layers (plan D4): the same RTC frame
        // the built-in ground pipelines use, plus the (overridable) clock.
        let custom_ctx = {
            let cam = &self.cam.camera;
            let origin = cam.center.to_world();
            CustomFrameCtx {
                view_proj: cam.view_projection_matrix_rtc(origin, self.viewport_px),
                origin: (origin.x, origin.y),
                viewport_px: self.viewport_px,
                pixels_per_world_unit: cam.pixels_per_world_unit(),
                zoom: cam.zoom,
                pitch_deg: cam.pitch_deg,
                bearing_deg: cam.bearing_deg,
                time_s: frame_time_s,
            }
        };
        self.symbols.text.begin_frame();
        self.symbols.icons.begin_frame();
        for (i, layer) in self.basemap.layers.iter_mut().enumerate() {
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
                        frame.vec_terrain_halo_uv,
                        terrain_cell.as_deref_mut().map(|t| &mut t.cache),
                    );
                    tiles_drawn += v.scene.visible_tiles().len();
                    prepared_layers.push((i, PreparedLayer::Vector(p)));
                    // Labels come from visible vector layers only —
                    // text on top of a hidden vector layer would look
                    // orphaned. Text runs *before* icons so a POI marker's
                    // dot can be gated on its label surviving collision (dot
                    // + label cull as a unit).
                    prepared_text.push(self.symbols.text.prepare(
                        &v.scene,
                        &mut v.cache,
                        self.options.pixel_ratio,
                    ));
                    prepared_icons.push(self.symbols.icons.prepare(
                        &v.scene,
                        &mut v.cache,
                        self.symbols.text.placed_marker_anchors(),
                    ));
                }
                LayerEntry::Hillshade(h) if h.visible => {
                    // Hillshade reads from the shared TerrainCache.
                    // Without terrain registered the layer is a
                    // no-op — the demo always pairs hillshade with
                    // terrain, but be defensive. Reuse the already-
                    // borrowed `terrain_cell` rather than touching
                    // `self.terrain.data` again (which the loop has
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
                LayerEntry::Custom(c) if c.visible => {
                    c.layer.prepare(&custom_ctx);
                    prepared_layers.push((i, PreparedLayer::Custom));
                }
                _ => {}
            }
        }
        self.symbols.text.finish_frame();
        self.symbols.icons.finish_frame();

        // Markers last. Pick any scene that's around — they all sync
        // from the same camera. Prefer the first raster/vector layer
        // (they have their own scenes); fall back to terrain;
        // otherwise build a one-off from the Map's state.
        let prepared_markers = if self.overlays.markers.is_empty() {
            None
        } else {
            let p = if let Some(scene) = self.basemap.layers.marker_scene() {
                self.overlays
                    .marker_pipeline
                    .prepare(scene, self.overlays.markers.all())
            } else if let Some(t) = self.terrain.data.as_ref() {
                self.overlays
                    .marker_pipeline
                    .prepare(&t.scene, self.overlays.markers.all())
            } else {
                // No layers — build a one-off Scene from the Map's state.
                let scene = Scene::with_margin(self.cam.camera, self.viewport_px, 0, 22, 0);
                self.overlays
                    .marker_pipeline
                    .prepare(&scene, self.overlays.markers.all())
            };
            Some(p)
        };
        // Rebuild the route-tube mesh when a polyline changed (now) or when new
        // DEM means the baked elevations are stale (throttled, since a tile burst
        // bumps the generation many times/sec).
        let terrain_stale = self.overlays.route_tubes.built_gen
            != self.overlays.route_tubes.terrain_gen
            && self
                .overlays
                .route_tubes
                .last_build
                .is_none_or(|t| started.duration_since(t).as_millis() >= 250);
        if self.overlays.route_tubes.dirty || terrain_stale {
            self.rebuild_route_tubes();
            self.overlays.route_tubes.last_build = Some(started);
        }
        let prepare_time = prepare_started.elapsed();
        let visible_layers = prepared_layers.len();

        // ---- Phase B: frame clear colour -------------------------
        // Replicates the old "first visible layer clears" semantics:
        // a vector layer clears to its style background; raster and
        // hillshade (and an empty layer stack) clear to the shared
        // backdrop colour.
        let clear = match first_visible.map(|i| &self.basemap.layers[i]) {
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

        // ---- Phase C: the render pass ----------------------------
        // ONE MSAA pass, owned by the frame graph (`run_msaa`): sky, floor,
        // the ground layers (raster / vector mesh, water is an ordinary
        // vector fill, / hillshade), route tubes and overlays, resolved
        // straight to the frame's target. Each contribution is a named graph
        // node — one per tile layer — so the report shows where the encode
        // time goes and any single layer can be isolated off. (The HDR bloom
        // + ACES post stage was a leftover of the reverted realistic-water
        // feature — it silently regraded the whole authored palette; removed
        // 2026-07-03 to restore the golden-locked cartographic look. HDR post
        // returns deliberately with water v2, goldens re-baselined on purpose.)
        let pass_started = Instant::now();
        if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
            ts.scope_begin("frame-pass", encoder);
        }
        {
            let terrain_cache = self.terrain.data.as_ref().map(|t| &t.cache);
            let placeholder_dem = &self.terrain.shared.placeholder_bind_group;
            let shadow_bg = &self.terrain.shadow_map.bind_group;
            let renderer = &self.renderer;
            let atmosphere = &self.atmosphere;
            let symbols = &self.symbols;
            let overlays = &self.overlays;
            let layers = &self.basemap.layers;
            let this = &*self;
            let frame_ref = &frame;

            let mut draws = DrawList::new();
            if let Some(g) = frame.sky_globals.as_ref() {
                draws.add(&Self::PASS_SKY, None, move |pass| {
                    atmosphere.sky.draw(g, pass)
                });
            }
            if let Some(g) = frame.floor_globals.as_ref() {
                draws.add(&Self::PASS_FLOOR, None, move |pass| {
                    atmosphere.floor.draw(g, pass)
                });
            }
            for (i, prepared) in prepared_layers {
                // Tile layers are `layer:<id>` nodes; custom layers get their
                // own pass kind (`custom:<id>`) in their declared phase.
                let (desc, id) = match &layers[i] {
                    LayerEntry::Raster(r) => (&Self::PASS_LAYER, r.id.clone()),
                    LayerEntry::Vector(v) => (&Self::PASS_LAYER, v.id.clone()),
                    LayerEntry::Hillshade(h) => (&Self::PASS_LAYER, h.id.clone()),
                    LayerEntry::Custom(c) => (
                        match c.layer.phase() {
                            CustomPhase::Ground => &Self::PASS_CUSTOM_GROUND,
                            CustomPhase::Overlay => &Self::PASS_CUSTOM_OVERLAY,
                        },
                        c.id.clone(),
                    ),
                };
                draws.add(desc, Some(id), move |pass| match (&layers[i], &prepared) {
                    (LayerEntry::Raster(r), PreparedLayer::Raster(p)) => {
                        r.pipeline.draw(
                            p,
                            &r.cache,
                            terrain_cache,
                            placeholder_dem,
                            shadow_bg,
                            pass,
                        );
                    }
                    (LayerEntry::Vector(v), PreparedLayer::Vector(p)) => {
                        v.pipeline
                            .draw(p, &v.cache, terrain_cache, placeholder_dem, pass);
                    }
                    (LayerEntry::Hillshade(h), PreparedLayer::Hillshade(p)) => {
                        if let Some(tc) = terrain_cache {
                            h.pipeline.draw(p, tc, pass);
                        }
                    }
                    (LayerEntry::Custom(c), PreparedLayer::Custom) => {
                        c.layer.draw(pass);
                    }
                    _ => unreachable!("prepared layer kind mismatch"),
                });
            }
            draws.add(&Self::PASS_ROUTE_TUBES, None, move |pass| {
                this.draw_route_tubes(pass, frame_ref)
            });
            if !prepared_icons.is_empty() {
                draws.add(&Self::PASS_ICONS, None, move |pass| {
                    for p in &prepared_icons {
                        symbols.icons.draw(p, pass);
                    }
                });
            }
            if !prepared_text.is_empty() {
                draws.add(&Self::PASS_TEXT, None, move |pass| {
                    for p in &prepared_text {
                        symbols.text.draw(p, pass);
                    }
                });
            }
            if let Some(p) = prepared_markers {
                draws.add(&Self::PASS_MARKERS, None, move |pass| {
                    overlays.marker_pipeline.draw(&p, pass)
                });
            }

            graph.run_msaa(
                encoder,
                MsaaAttachments {
                    color_view: renderer.targets.color_view(),
                    resolve_target: target,
                    depth_view: renderer.targets.depth_view(),
                    clear,
                },
                draws,
            );
        }
        if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
            ts.scope_end(encoder);
        }
        let pass_time = pass_started.elapsed();

        // Weather-cloud overlay: a separate, single-sampled, depth-less
        // fullscreen composite over the already-resolved surface. It can't
        // join the MSAA frame pass above (sample-count / depth mismatch),
        // so it pays one extra fullscreen pass — acceptable for an overlay.
        let clouds_started = Instant::now();
        let cam = self.cam.camera;
        let (vw, vh) = (self.viewport_px.0 as f64, self.viewport_px.1 as f64);
        let viewport_px = self.viewport_px;
        let queue = &self.queue;
        if let Some(c) = &mut self.atmosphere.clouds {
            if c.enabled {
                if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
                    ts.scope_begin("clouds", encoder);
                }
                graph.run_encoder(&Self::PASS_CLOUDS, encoder, |enc| {
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
                                let vp = cam.view_projection_matrix(viewport_px);
                                let inv = glam::Mat4::from_cols_array_2d(&vp)
                                    .inverse()
                                    .to_cols_array_2d();
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
                        .render_overlay_downsampled(queue, enc, target, &c.params, 2);
                });
                if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
                    ts.scope_end(encoder);
                }
            }
        }

        if let Some(ts) = self.renderer.gpu_timestamps.as_mut() {
            ts.end(encoder);
        }
        let clouds_time = clouds_started.elapsed();
        // The frame's pass report: what ran (in order), what was skipped, and
        // each pass's CPU encode time. Debug builds validate the declared
        // data flow in `finish`.
        let report = graph.finish();
        // Draw contributions actually encoded into the MSAA pass this frame
        // (sky/floor + each drawn layer + route tubes + icons + text +
        // markers). Since D1 this counts from the graph report, which
        // includes the floor + route-tube nodes the old hand count missed.
        let draw_calls = report
            .passes
            .iter()
            .filter(|p| {
                !p.skipped && matches!(p.phase, FramePhase::GroundMsaa | FramePhase::OverlayMsaa)
            })
            .count();
        self.last_frame_metrics = FrameMetrics {
            cpu_time: started.elapsed(),
            frame_dropped: false,
            phases: PhaseTimings {
                prepare: prepare_time,
                pass: pass_time,
                clouds: clouds_time,
                shadow_assemble: shadow_assemble_time,
            },
            gpu_time: self.renderer.gpu_timestamps.as_ref().and_then(|t| {
                if t.last_duration_ns == 0 {
                    None
                } else {
                    Some(Duration::from_nanos(t.last_duration_ns))
                }
            }),
            gpu_passes: self
                .renderer
                .gpu_timestamps
                .as_ref()
                .map(|t| {
                    t.last_scopes
                        .iter()
                        .map(|(name, ns)| (name.to_string(), Duration::from_nanos(*ns)))
                        .collect()
                })
                .unwrap_or_default(),
            layer_count: self.basemap.layers.len(),
            marker_count: self.overlays.markers.len(),
            visible_layers,
            draw_calls,
            passes: report.passes,
            tiles_drawn,
            tiles: self.tile_histogram(),
            layers: self
                .basemap
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
                            .data
                            .as_ref()
                            .map(|t| t.cache.stats())
                            .unwrap_or_default(),
                    },
                    LayerEntry::Custom(c) => LayerMetrics {
                        id: c.id.clone(),
                        kind: LayerKind::Custom,
                        // Custom layers own opaque GPU state — no shared
                        // cache to report.
                        cache: CacheStats::default(),
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

    /// Tile-lifecycle histogram summed across every layer's scene plus the
    /// terrain scene — the whole map's streaming state in one additive value
    /// (see [`crate::scene::TileHistogram`]). Each scene classifies against
    /// the same camera (`sync_scenes` keeps them coherent), so the sum is the
    /// per-frame truth the trace records.
    fn tile_histogram(&self) -> crate::scene::TileHistogram {
        let mut h = crate::scene::TileHistogram::default();
        for l in self.basemap.layers.iter() {
            match l {
                LayerEntry::Raster(r) => {
                    let res = |t: &TileId| self.chunk_is_resident(self.world_key(Some(&r.id), *t));
                    h += r.scene.phase_histogram(&res);
                }
                LayerEntry::Vector(v) => {
                    let res = |t: &TileId| self.chunk_is_resident(self.world_key(Some(&v.id), *t));
                    h += v.scene.phase_histogram(&res);
                }
                // Hillshade owns no scene/cache — it reads the shared terrain,
                // which is counted once below. Custom layers stream nothing.
                LayerEntry::Hillshade(_) | LayerEntry::Custom(_) => {}
            }
        }
        if let Some(t) = &self.terrain.data {
            let res = |ti: &TileId| self.chunk_is_resident(self.world_key(None, *ti));
            h += t.scene.phase_histogram(&res);
        }
        // Resident-but-unwanted is the lifecycle table's knowledge alone
        // (slice B3.4) — the scenes no longer track residency at all.
        h.retained = self.lifecycle.borrow().histogram().retained;
        h
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
    Custom,
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
    /// CPU wall-time of the cast-shadow / AO heightfield REASSEMBLY (the 256²
    /// cross-tile elevation sample + upload). `Duration::ZERO` on the vast
    /// majority of frames — it only fires when the camera settles in a new
    /// lattice region or the sun / DEM changed. When non-zero it's a
    /// render-thread-blocking spike, so isolating it from `prepare` is the whole
    /// point of profiling the 3D path.
    pub shadow_assemble: Duration,
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
    /// The frame graph's pass report (slice D1): every pass that ran or was
    /// masked off this frame, in execution order, with per-pass CPU encode
    /// time. The always-on decomposition of `phases.pass` — "the frame got
    /// slow" resolves to *which pass* without attaching a profiler.
    pub passes: Vec<PassTiming>,
    /// GPU wall time per encoder-level scope (`ao` / `frame-pass` /
    /// `clouds`), one-frame delayed like `gpu_time` and empty without
    /// `Features::TIMESTAMP_QUERY`. The GPU-side decomposition CPU encode
    /// times can't see — a cheap-to-encode pass (clouds) can dominate here.
    pub gpu_passes: Vec<(String, Duration)>,
    /// Tile-lifecycle histogram summed across every layer scene + terrain —
    /// desired/pending/resident/retained, pending split by tier. The trace's
    /// "is streaming healthy?" counters (plan slice A1). Zero on dropped
    /// frames (the finite gate bails before scenes are consulted).
    pub tiles: crate::scene::TileHistogram,
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
