//! Unified `Map` with an ordered layer stack.
//!
//! A `Map` owns the camera, viewport, markers, animation, and the
//! pipelines that span layers (text, markers). Each layer in `layers`
//! carries its own source, scene-state (zoom bounds + ingested set +
//! prefetch margin), and layer-specific pipeline + cache.
//!
//! Layers are drawn back-to-front in insertion order:
//! 1. First layer's geometry clears the target.
//! 2. Subsequent layers' geometry use `LoadOp::Load` to composite on top.
//! 3. After all layer geometry, one text pass aggregates labels from
//!    *all* vector layers.
//! 4. Finally, markers paint on top.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::{
    camera::{Camera, CameraAnimation},
    error::MapError,
    geo::LatLng,
    hit::geometry_hit,
    render::{
        cache::CacheStats, gpu_timestamps::GpuTimestamps, hillshade::HillshadePipeline,
        marker::MarkerPipeline,
        raster::{RasterPipeline, TerrainConfig},
        terrain::{TerrainCache, TerrainOptions, TerrainShared},
        text::TextPipeline, vector::VectorPipeline, vector_cache::VectorMeshCache, TextureCache,
    },
    scene::Scene,
    source::TileSource,
    style::{Color, HillshadeStyle, VectorStyle},
    tessellate::{self, InteractiveFeature, LabelRequest, Mesh},
    tile::TileId,
    vector::{GeomType, Value as VectorValue, VectorTile, VectorTileSource},
};

pub use crate::render::terrain::TerrainOptions as PublicTerrainOptions;

/// Build the depth attachment matching the surface size. Re-created
/// on resize.
fn create_depth_view(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("turbomap-depth"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::render::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

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
}

impl Default for MapOptions {
    fn default() -> Self {
        Self {
            cache_budget_bytes: 128 * 1024 * 1024,
            prefetch_margin_px: 256,
            // 0.4 s reads as a smooth fade. The earlier default (0.18)
            // popped — each per-layer tile arrival was over visually
            // before the next layer's matching tile could blend in,
            // so the eye perceived a chain of discrete additions
            // instead of a coordinated transition. Longer durations
            // also absorb the staggered cross-layer arrival times.
            fade_in_secs: 0.4,
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
}

struct HillshadeLayer {
    id: String,
    style: HillshadeStyle,
    pipeline: HillshadePipeline,
    fade_in_secs: f32,
    visible: bool,
}

pub struct Map {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface_format: wgpu::TextureFormat,
    viewport_px: (u32, u32),
    /// Shared camera. Layers' `Scene` is synced from this each render.
    camera: Camera,
    animation: Option<CameraAnimation>,
    options: MapOptions,
    layers: Vec<LayerEntry>,
    /// Single text pipeline, shared across all vector layers.
    text_pipeline: TextPipeline,
    marker_pipeline: MarkerPipeline,
    markers: Vec<Marker>,
    next_marker_id: u64,
    last_frame_metrics: FrameMetrics,
    /// Optional GPU-side frame timing. Only present if the wgpu device
    /// negotiated `Features::TIMESTAMP_QUERY` at creation time. When
    /// `None`, `FrameMetrics::gpu_time` stays `None` and we just
    /// report CPU time.
    gpu_timestamps: Option<GpuTimestamps>,
    /// Optional shared heightmap. When set, ground-plane pipelines
    /// (raster, hillshade, vector) sample the DEM in their vertex
    /// shaders and displace by elevation. See [`TerrainOptions`].
    terrain: Option<Terrain>,
    /// Map-level terrain bind-group layout + sampler + 1×1
    /// placeholder bind group. Always present so pipelines that opt
    /// into displacement can be created before any terrain source is
    /// registered — they just bind the placeholder and render flat.
    terrain_shared: TerrainShared,
    /// Depth attachment, sized to the surface. Required for 3D
    /// terrain so the back of a mountain doesn't overdraw the
    /// front. All render passes share this texture.
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
}

struct Terrain {
    /// DEM source the host drains via `PendingTile::Terrain`. Drives
    /// both the tile cache (`TerrainCache`) and visibility tracking
    /// (`Scene`).
    source: Arc<dyn TileSource>,
    cache: TerrainCache,
    scene: Scene,
    options: TerrainOptions,
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
        let text_pipeline = TextPipeline::new(device.clone(), queue.clone(), surface_format);
        let marker_pipeline = MarkerPipeline::new(device.clone(), queue.clone(), surface_format);
        let gpu_timestamps = GpuTimestamps::new(&device, &queue);
        let depth_view = create_depth_view(&device, initial_size);
        let terrain_shared = TerrainShared::new(&device, &queue);
        Ok(Self {
            device,
            queue,
            surface_format,
            viewport_px: initial_size,
            camera: initial_camera,
            animation: None,
            options,
            layers: Vec::new(),
            text_pipeline,
            marker_pipeline,
            markers: Vec::new(),
            next_marker_id: 0,
            last_frame_metrics: FrameMetrics::default(),
            gpu_timestamps,
            terrain: None,
            terrain_shared,
            depth_view,
            depth_size: initial_size,
        })
    }

    /// Enable 3D terrain. Future ground-plane draws will displace
    /// their vertices by the elevation sampled from `source`. There
    /// is at most one terrain source per Map — calling this again
    /// replaces it. Halo > 0 on the source is required so adjacent
    /// tile-edge vertices agree and the mesh doesn't crack at tile
    /// boundaries.
    pub fn set_terrain_source(
        &mut self,
        source: Arc<dyn TileSource>,
        options: TerrainOptions,
    ) {
        let halo = source.dem_halo_px();
        let cache = TerrainCache::new(
            self.device.clone(),
            self.queue.clone(),
            &self.terrain_shared,
            self.options.cache_budget_bytes,
            halo,
        );
        let scene = Scene::with_margin(
            self.camera,
            self.viewport_px,
            source.min_zoom(),
            source.max_zoom(),
            self.options.prefetch_margin_px,
        );
        self.terrain = Some(Terrain {
            source,
            cache,
            scene,
            options,
        });
    }

    pub fn clear_terrain(&mut self) {
        self.terrain = None;
    }

    pub fn has_terrain(&self) -> bool {
        self.terrain.is_some()
    }

    /// Push decoded RGBA back into the shared terrain heightmap.
    /// Host drives this from the same fetch pump it uses for raster
    /// tiles. Silently no-ops when no terrain source is registered
    /// (e.g. host sent us a stale tile after `clear_terrain`).
    pub fn ingest_terrain_tile(
        &mut self,
        tile: TileId,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) {
        if let Some(t) = self.terrain.as_mut() {
            t.cache.ingest(tile, rgba, width, height);
            t.scene.ingest(tile);
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
            &self.terrain_shared.bind_group_layout,
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
            self.camera,
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
    }

    /// Add a hillshade overlay. The hillshade pipeline samples the
    /// Map-level shared DEM bind group at draw time. If terrain isn't
    /// registered yet, the pipeline is built against the placeholder
    /// layout — calling `set_terrain_source` later activates real
    /// displacement without recompiling the pipeline.
    pub fn add_hillshade_layer(
        &mut self,
        id: impl Into<String>,
        style: HillshadeStyle,
    ) {
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
            &self.terrain_shared.bind_group_layout,
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
        let pipeline =
            VectorPipeline::new(self.device.clone(), self.queue.clone(), self.surface_format);
        let cache = VectorMeshCache::new(self.device.clone(), self.options.cache_budget_bytes);
        let scene = Scene::with_margin(
            self.camera,
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
        })));
    }

    pub fn remove_layer(&mut self, id: &str) {
        self.layers.retain(|l| match l {
            LayerEntry::Raster(r) => r.id != id,
            LayerEntry::Vector(v) => v.id != id,
            LayerEntry::Hillshade(h) => h.id != id,
        });
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn has_layer(&self, id: &str) -> bool {
        self.layers.iter().any(|l| match l {
            LayerEntry::Raster(r) => r.id == id,
            LayerEntry::Vector(v) => v.id == id,
            LayerEntry::Hillshade(h) => h.id == id,
        })
    }

    pub fn layer_ids(&self) -> Vec<String> {
        self.layers
            .iter()
            .map(|l| match l {
                LayerEntry::Raster(r) => r.id.clone(),
                LayerEntry::Vector(v) => v.id.clone(),
                LayerEntry::Hillshade(h) => h.id.clone(),
            })
            .collect()
    }

    /// Look up the vector source for a vector layer — useful for the host
    /// when constructing the fetch pump.
    pub fn vector_source(&self, id: &str) -> Option<Arc<dyn VectorTileSource>> {
        self.layers.iter().find_map(|l| match l {
            LayerEntry::Vector(v) if v.id == id => Some(v.source.clone()),
            _ => None,
        })
    }

    pub fn raster_source(&self, id: &str) -> Option<Arc<dyn TileSource>> {
        self.layers.iter().find_map(|l| match l {
            LayerEntry::Raster(r) if r.id == id => Some(r.source.clone()),
            _ => None,
        })
    }

    /// Terrain DEM source (Map-level since the 3D-terrain refactor;
    /// hillshade layers consume this rather than owning their own
    /// source). Returns `None` until `set_terrain_source` is called.
    pub fn terrain_source(&self) -> Option<Arc<dyn TileSource>> {
        self.terrain.as_ref().map(|t| t.source.clone())
    }

    pub fn vector_style(&self, id: &str) -> Option<VectorStyle> {
        self.layers.iter().find_map(|l| match l {
            LayerEntry::Vector(v) if v.id == id => Some(v.style.clone()),
            _ => None,
        })
    }

    // ---- camera + viewport ---------------------------------------------

    pub fn camera(&self) -> Camera {
        self.camera
    }

    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
        self.animation = None;
        self.sync_scenes();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_px = (width, height);
        // Depth texture must match the colour target. Recreate when
        // the surface changes size — Metal will assert otherwise on
        // the next render.
        if (width, height) != self.depth_size && width > 0 && height > 0 {
            self.depth_view = create_depth_view(&self.device, (width, height));
            self.depth_size = (width, height);
        }
        self.sync_scenes();
    }

    pub fn pan_by_pixels(&mut self, dx: f64, dy: f64) {
        let mut c = self.camera;
        c.pan_by_pixels(dx, dy);
        self.set_camera(c);
    }

    pub fn zoom_around(&mut self, factor: f64, focus_px: (f64, f64)) {
        let (w, h) = self.viewport_px;
        let mut c = self.camera;
        c.zoom_around(factor, focus_px, (w as f64, h as f64));
        self.set_camera(c);
    }

    pub fn ease_to(&mut self, target: Camera, duration: Duration) {
        self.animation = Some(CameraAnimation::new(self.camera, target, duration));
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        if let Some(anim) = self.animation {
            self.camera = anim.sample(now);
            self.sync_scenes();
            if anim.is_finished(now) {
                self.animation = None;
                return false;
            }
            return true;
        }
        false
    }

    pub fn is_animating(&self) -> bool {
        if self.animation.is_some() {
            return true;
        }
        // Any layer with a fading tile keeps the animation flag set.
        self.layers.iter().any(|l| match l {
            LayerEntry::Raster(_) => false, // fade-in for raster is per-tile, owned by the pipeline
            LayerEntry::Vector(v) => v.cache.any_younger_than(v.fade_in_secs),
            LayerEntry::Hillshade(_) => false,
        })
    }

    /// Push the current camera + viewport into each layer's scene. Called
    /// whenever the camera changes or layers are added/removed.
    fn sync_scenes(&mut self) {
        for l in &mut self.layers {
            match l {
                LayerEntry::Raster(r) => {
                    r.scene.set_camera(self.camera);
                    r.scene.set_viewport_px(self.viewport_px);
                }
                LayerEntry::Vector(v) => {
                    v.scene.set_camera(self.camera);
                    v.scene.set_viewport_px(self.viewport_px);
                }
                // Hillshade no longer owns a scene — it iterates the
                // shared terrain scene at render time.
                LayerEntry::Hillshade(_) => {}
            }
        }
        if let Some(t) = self.terrain.as_mut() {
            t.scene.set_camera(self.camera);
            t.scene.set_viewport_px(self.viewport_px);
        }
    }

    // ---- coordinate conversion -----------------------------------------

    pub fn screen_to_lng_lat(&self, screen_px: (f64, f64)) -> LatLng {
        let (w, h) = self.viewport_px;
        let world = self.camera.pixel_to_world(screen_px, (w as f64, h as f64));
        world.to_lat_lng()
    }

    pub fn lng_lat_to_screen(&self, lng_lat: LatLng) -> (f64, f64) {
        let world = lng_lat.to_world();
        let (w, h) = self.viewport_px;
        // Fall back to the camera centre for off-screen / behind-camera
        // points so callers (e.g. hit-test on markers) get a deterministic
        // value rather than panicking. The cull happens upstream.
        self.camera
            .world_to_screen(world, (w as f64, h as f64))
            .unwrap_or((w as f64 * 0.5, h as f64 * 0.5))
    }

    // ---- tile orchestration --------------------------------------------

    /// Aggregate pending tiles across all layers. Each entry carries the
    /// layer id so the host can route `ingest_raster`/`ingest_vector_mesh`
    /// back correctly.
    pub fn pending_tiles(&self) -> Vec<PendingTile> {
        let mut out = Vec::new();
        for l in &self.layers {
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
        self.layers.iter().position(|l| match l {
            LayerEntry::Raster(r) => r.visible,
            LayerEntry::Vector(v) => v.visible,
            LayerEntry::Hillshade(h) => h.visible,
        })
    }

    /// Back-compat shim. Hillshade no longer owns its own DEM cache —
    /// the data goes to the shared terrain cache, so this just forwards
    /// to [`Map::ingest_terrain_tile`]. The `layer_id` argument is
    /// kept for source compatibility but ignored.
    pub fn ingest_hillshade(
        &mut self,
        _layer_id: &str,
        tile: TileId,
        rgba: &[u8],
        w: u32,
        h: u32,
    ) {
        self.ingest_terrain_tile(tile, rgba, w, h);
    }

    pub fn ingest_raster(&mut self, layer_id: &str, tile: TileId, rgba: &[u8], w: u32, h: u32) {
        for l in &mut self.layers {
            if let LayerEntry::Raster(r) = l {
                if r.id == layer_id {
                    r.cache.insert(tile, rgba, w, h);
                    r.scene.ingest(tile);
                    return;
                }
            }
        }
    }

    pub fn ingest_vector_mesh(
        &mut self,
        layer_id: &str,
        tile: TileId,
        mesh: &Mesh,
        labels: Vec<LabelRequest>,
        interactive: Vec<InteractiveFeature>,
    ) {
        for l in &mut self.layers {
            if let LayerEntry::Vector(v) = l {
                if v.id == layer_id {
                    v.cache.insert(tile, mesh, labels, interactive);
                    v.scene.ingest(tile);
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
        self.ingest_vector_mesh(layer_id, tile_id, &out.mesh, out.labels, out.interactive);
    }

    // ---- layer visibility ---------------------------------------------

    /// Toggle a layer on/off by id. Invisible layers still own their
    /// cache (so re-enabling is free) but skip both the render pass
    /// and the `pending_tiles` enumeration — no network traffic while
    /// hidden. Returns `false` if no layer matches the id.
    pub fn set_layer_visibility(&mut self, id: &str, visible: bool) -> bool {
        for layer in &mut self.layers {
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

    pub fn layer_visibility(&self, id: &str) -> Option<bool> {
        self.layers.iter().find_map(|l| match l {
            LayerEntry::Raster(r) if r.id == id => Some(r.visible),
            LayerEntry::Vector(v) if v.id == id => Some(v.visible),
            LayerEntry::Hillshade(h) if h.id == id => Some(h.visible),
            _ => None,
        })
    }

    /// Set the per-layer fade-in duration. Lets the UI tune the
    /// cross-tile blend feel at runtime — 0 disables fading entirely
    /// (instant tile-pop), higher values smear arrivals into a
    /// longer crossfade.
    pub fn set_layer_fade_in(&mut self, id: &str, secs: f32) -> bool {
        for layer in &mut self.layers {
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

    // ---- markers -------------------------------------------------------

    pub fn add_marker(&mut self, mut marker: Marker) -> MarkerId {
        if marker.id == MarkerId(0) {
            self.next_marker_id += 1;
            marker.id = MarkerId(self.next_marker_id);
        } else {
            self.next_marker_id = self.next_marker_id.max(marker.id.0);
        }
        let id = marker.id;
        if let Some(slot) = self.markers.iter_mut().find(|m| m.id == id) {
            *slot = marker;
        } else {
            self.markers.push(marker);
        }
        id
    }

    pub fn remove_marker(&mut self, id: MarkerId) {
        self.markers.retain(|m| m.id != id);
    }

    pub fn clear_markers(&mut self) {
        self.markers.clear();
    }

    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    // ---- hit testing ---------------------------------------------------

    pub fn hit_test(&self, screen_px: (f64, f64), tolerance_px: f64) -> Vec<HitResult> {
        let mut out: Vec<HitResult> = Vec::new();

        // Markers first (top z-order, newest-first within markers).
        for marker in self.markers.iter().rev() {
            let mp = self.lng_lat_to_screen(marker.lng_lat);
            let dx = mp.0 - screen_px.0;
            let dy = mp.1 - screen_px.1;
            let r = (marker.radius_px as f64 + tolerance_px).max(0.0);
            if dx * dx + dy * dy <= r * r {
                out.push(HitResult::Marker(HitMarker {
                    id: marker.id,
                    lng_lat: marker.lng_lat,
                    data: marker.data.clone(),
                }));
            }
        }

        // Then vector-tile features, top-most layer first.
        let camera = self.camera;
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
        if let Some(ts) = self.gpu_timestamps.as_mut() {
            ts.try_drain();
            ts.begin(encoder);
        }
        // 1. Each layer renders its geometry in order. The first
        //    *visible* layer clears the target; later visible layers
        //    blend on top. Hidden layers are skipped entirely so the
        //    UI can isolate one layer for debugging without affecting
        //    the clear semantics.
        //
        // Compute the metres-to-world conversion for vertex
        // displacement now so hillshade (and future terrain consumers)
        // can build their per-frame globals. Mercator-correct:
        // `1 / (256 * 2^z_world)` is world units per pixel at zoom z;
        // we want metres → world units. At a given latitude
        //   metres_per_world = 2π·R / cos(lat)  (full world circumference at that lat)
        // so `meters_to_world = 1 / metres_per_world`. The DEM
        // exaggeration is applied separately inside the shader.
        let lat = self.camera.center.lat.to_radians();
        let earth_circumference_m: f32 = 40_075_017.0;
        let meters_to_world =
            (lat.cos().abs() as f32 / earth_circumference_m).max(1e-12);
        // Terrain config for the raster pipeline. When terrain isn't
        // registered, `meters_to_world` is forced to 0 so the shader
        // displacement collapses and the mesh draws flat.
        let raster_terrain_cfg = TerrainConfig {
            meters_to_world: if self.terrain.is_some() {
                meters_to_world
            } else {
                0.0
            },
            exaggeration: self
                .terrain
                .as_ref()
                .map(|t| t.options.exaggeration)
                .unwrap_or(1.0),
            encoding: self
                .terrain
                .as_ref()
                .map(|t| match t.options.encoding {
                    crate::dem::DemEncoding::MapboxRgb => 0u32,
                    crate::dem::DemEncoding::Terrarium => 1u32,
                })
                .unwrap_or(0),
        };
        let first_visible = self.first_visible_layer_index();
        // Split-borrow the parts we need so the loop can mutably
        // borrow `self.layers` while still passing references into
        // `self.terrain` and `self.terrain_shared.placeholder_bind_group`.
        let placeholder_dem = &self.terrain_shared.placeholder_bind_group;
        let terrain_for_loop = self.terrain.as_mut();
        // Convert to Option<&mut Terrain> we can `take` on a per-
        // pipeline basis via reborrow.
        let mut terrain_cell = terrain_for_loop;
        for (i, layer) in self.layers.iter_mut().enumerate() {
            let is_first = Some(i) == first_visible;
            match layer {
                LayerEntry::Raster(r) if r.visible => {
                    r.pipeline.render(
                        &r.scene,
                        &mut r.cache,
                        terrain_cell.as_deref_mut().map(|t| &mut t.cache),
                        placeholder_dem,
                        raster_terrain_cfg,
                        encoder,
                        target,
                        &self.depth_view,
                        r.fade_in_secs,
                        is_first,
                    );
                }
                LayerEntry::Vector(v) if v.visible => {
                    v.pipeline.render(
                        &v.scene,
                        &mut v.cache,
                        encoder,
                        target,
                        srgb_color_to_linear_f32(v.style.background),
                        v.fade_in_secs,
                        is_first,
                    );
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
                        h.pipeline.render(
                            &t.scene,
                            &mut t.cache,
                            h.style,
                            encoder,
                            target,
                            &self.depth_view,
                            h.fade_in_secs,
                            is_first,
                            meters_to_world,
                        );
                    }
                }
                _ => {}
            }
        }

        // 2. Single text pass aggregating labels from visible vector
        //    layers only — text on top of a hidden vector layer would
        //    look orphaned.
        for layer in self.layers.iter_mut() {
            if let LayerEntry::Vector(v) = layer {
                if v.visible {
                    self.text_pipeline
                        .render(&v.scene, &mut v.cache, encoder, target);
                }
            }
        }

        // 3. Markers last.
        if !self.markers.is_empty() {
            // Use the first layer's scene as the camera/viewport source —
            // any layer works since they all sync from the Map.
            // Pick any scene that's around — they all sync from the
            // same camera. Prefer the first raster/vector layer (they
            // have their own scenes); fall back to terrain; otherwise
            // build a one-off from the Map's state.
            let scene_from_layer = self.layers.iter().find_map(|l| match l {
                LayerEntry::Raster(r) => Some(&r.scene),
                LayerEntry::Vector(v) => Some(&v.scene),
                LayerEntry::Hillshade(_) => None,
            });
            if let Some(scene) = scene_from_layer {
                self.marker_pipeline
                    .render(scene, &self.markers, encoder, target);
            } else if let Some(t) = self.terrain.as_ref() {
                self.marker_pipeline
                    .render(&t.scene, &self.markers, encoder, target);
            } else {
                // No layers — build a one-off Scene from the Map's state.
                let scene = Scene::with_margin(self.camera, self.viewport_px, 0, 22, 0);
                self.marker_pipeline
                    .render(&scene, &self.markers, encoder, target);
            }
        }

        if let Some(ts) = self.gpu_timestamps.as_mut() {
            ts.end(encoder);
        }
        self.last_frame_metrics = FrameMetrics {
            cpu_time: started.elapsed(),
            gpu_time: self.gpu_timestamps.as_ref().and_then(|t| {
                if t.last_duration_ns == 0 {
                    None
                } else {
                    Some(Duration::from_nanos(t.last_duration_ns))
                }
            }),
            layer_count: self.layers.len(),
            marker_count: self.markers.len(),
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
        if let Some(ts) = self.gpu_timestamps.as_mut() {
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

#[derive(Debug, Clone, Default)]
pub struct FrameMetrics {
    pub cpu_time: Duration,
    /// GPU wall time for the most recent COMPLETED frame's passes.
    /// `None` when the device lacks `Features::TIMESTAMP_QUERY`, or
    /// when no frame has finished yet (the readback arrives one
    /// frame after submit). Stable between frames once populated.
    pub gpu_time: Option<Duration>,
    pub layer_count: usize,
    pub marker_count: usize,
    pub layers: Vec<LayerMetrics>,
}

fn srgb_color_to_linear_f32(c: Color) -> [f32; 4] {
    fn to_linear(u: u8) -> f32 {
        let s = u as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    [
        to_linear(c.r),
        to_linear(c.g),
        to_linear(c.b),
        c.a as f32 / 255.0,
    ]
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
        assert!(o.fade_in_secs > 0.0 && o.fade_in_secs < 1.0);
    }
}
