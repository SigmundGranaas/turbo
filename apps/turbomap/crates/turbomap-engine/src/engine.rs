//! `TurbomapEngine` — the wgpu renderer behind the [`MapEngine`] contract.
//!
//! It owns a `turbomap_core::Map` and translates an applied
//! [`Scene`] into the core's imperative pipeline calls, driven by the
//! diff so the host only ever describes *what the map should be*. This
//! slice renders the layer types the core supports directly — raster and
//! hillshade — and records any others as unsupported (surfaced through
//! the inspect tooling); vector/symbol/custom layers land in later
//! slices.

use std::collections::HashMap;
use std::sync::Arc;

use turbomap_core::{
    Camera, Color as CoreColor, Filter as CoreFilter, HillshadeStyle, HitResult,
    LatLng as CoreLatLng, Map, MapError, MapOptions, Marker, MarkerId, Paint as CorePaint,
    PendingTile, Rule as CoreRule, TerrainOptions, TileId, TileSource, VectorStyle, VectorTileSource,
};
use turbomap_scene::{
    diff, Capabilities, CameraState, Color, Filter, FilterValue, Hit, LatLng, Layer, MapEngine,
    Paint, Scene, SceneDelta, ScreenPoint, SourceDef,
};

use crate::geojson::GEOJSON_LAYER;
use crate::resolver::{ResolvedSource, SourceResolver};

/// Pixels-to-extent-units for vector line widths. Core line width is in
/// tile-extent units (~0.0625 px each at extent 4096), so a `W`-px line is
/// roughly `W * 16` extent units.
const PX_TO_EXTENT: f32 = 16.0;

/// Counts from one [`TurbomapEngine::pump_tiles`] drain — useful both as
/// a convergence guard and as an inspectable signal.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DrainStats {
    pub rounds: u32,
    pub raster_tiles: u32,
    pub terrain_tiles: u32,
    pub vector_tiles: u32,
}

/// A wgpu map engine driven by the renderer-agnostic [`Scene`] IR.
pub struct TurbomapEngine {
    map: Map,
    scene: Scene,
    resolver: Box<dyn SourceResolver>,
    /// Resolved raster source per layer id — lets the engine answer its
    /// own pending raster tiles in [`TurbomapEngine::pump_tiles`].
    raster_sources: HashMap<String, Arc<dyn TileSource>>,
    /// The single resolved DEM source feeding terrain/hillshade.
    terrain_source: Option<Arc<dyn TileSource>>,
    /// Resolved vector source per layer id (MVT or GeoJSON).
    vector_sources: HashMap<String, Arc<dyn VectorTileSource>>,
    /// Layer ids the current backend cannot render (recorded each apply).
    unsupported: Vec<String>,
    max_texture_size: u32,
}

impl TurbomapEngine {
    /// Build an engine over a host-provided GPU context. `resolver` maps
    /// the scene's declarative sources to concrete tile sources.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        size: (u32, u32),
        camera: CameraState,
        options: MapOptions,
        resolver: Box<dyn SourceResolver>,
    ) -> Result<Self, MapError> {
        let max_texture_size = device.limits().max_texture_dimension_2d;
        let map = Map::new(device, queue, surface_format, size, to_core_camera(camera), options)?;
        Ok(Self {
            map,
            scene: Scene::new(),
            resolver,
            raster_sources: HashMap::new(),
            terrain_source: None,
            vector_sources: HashMap::new(),
            unsupported: Vec::new(),
            max_texture_size,
        })
    }

    /// Bring the core `Map` in line with the new scene (`self.scene`),
    /// given the previous `old`, doing the *minimal* GPU work.
    ///
    /// Positional GPU layers (raster/line/fill/hillshade) reconcile by a
    /// longest-unchanged-prefix + tail rebuild: a layer is "unchanged"
    /// only if both its definition and its source data are unchanged, so
    /// appending an overlay or repainting the top layer leaves the rest of
    /// the stack — and its GPU tile caches — untouched. Circle layers
    /// (markers) rebuild only when a circle layer or its data changes.
    fn reconcile(&mut self, old: &Scene) {
        let new = self.scene.clone();
        let dirty = dirty_sources(old, &new);

        // --- positional GPU layers: longest-unchanged-prefix + tail rebuild
        let old_pos = positional_layers(old);
        let new_pos = positional_layers(&new);
        let mut prefix = 0;
        while prefix < old_pos.len()
            && prefix < new_pos.len()
            && old_pos[prefix] == new_pos[prefix]
            && new_pos[prefix].source().map(|s| !dirty.contains(s)).unwrap_or(true)
        {
            prefix += 1;
        }
        // Remove the divergent tail (reverse order keeps ids stable).
        let current = self.map.layer_ids();
        for id in current.iter().skip(prefix).rev() {
            self.map.remove_layer(id);
            self.raster_sources.remove(id);
            self.vector_sources.remove(id);
        }
        // Re-install the new tail in order (core appends).
        for layer in new_pos.iter().skip(prefix) {
            self.install_positional(layer, &new);
        }
        // Terrain is global; if the new scene has no hillshade, drop it.
        if !new.layers.iter().any(|l| matches!(l, Layer::Hillshade { .. })) {
            self.map.clear_terrain();
            self.terrain_source = None;
        }

        // --- circle layers → markers: rebuild only when they or their data change
        let circles_changed = circle_layers(old) != circle_layers(&new)
            || circle_layers(&new)
                .iter()
                .any(|l| l.source().map(|s| dirty.contains(s)).unwrap_or(false));
        if circles_changed {
            self.rebuild_markers(&new);
        }

        // --- unsupported report: a pure scan over the whole scene
        self.unsupported = new
            .layers
            .iter()
            .filter(|l| !is_supportable(l, &new))
            .map(|l| l.id().to_string())
            .collect();
    }

    /// Install one positional layer into the core map (append).
    fn install_positional(&mut self, layer: &Layer, scene: &Scene) {
        match layer {
            Layer::Raster { id, source, .. } => {
                if let Some(ResolvedSource::Raster(src)) = self.resolve(scene, source) {
                    self.map.add_raster_layer(id.clone(), src.clone());
                    self.raster_sources.insert(id.clone(), src);
                }
            }
            Layer::Hillshade {
                id,
                source,
                exaggeration,
            } => {
                if let Some(ResolvedSource::Dem(dem)) = self.resolve(scene, source) {
                    self.map.set_terrain_source(
                        dem.clone(),
                        TerrainOptions {
                            exaggeration: *exaggeration,
                            ..Default::default()
                        },
                    );
                    self.map.add_hillshade_layer(id.clone(), HillshadeStyle::default());
                    self.terrain_source = Some(dem);
                }
            }
            Layer::Line {
                id,
                source,
                source_layer,
                filter,
                color,
                width,
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = line_style(name, filter, color, width, zoom);
                    self.map.add_vector_layer(id.clone(), vsrc.clone(), style);
                    self.vector_sources.insert(id.clone(), vsrc);
                }
            }
            Layer::Fill {
                id,
                source,
                source_layer,
                filter,
                color,
                opacity,
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = fill_style(name, filter, color, opacity, zoom);
                    self.map.add_vector_layer(id.clone(), vsrc.clone(), style);
                    self.vector_sources.insert(id.clone(), vsrc);
                }
            }
            // Circles and unsupported kinds are handled outside the
            // positional stack.
            _ => {}
        }
    }

    fn rebuild_markers(&mut self, scene: &Scene) {
        self.map.clear_markers();
        let zoom = self.map.camera().zoom;
        for layer in &scene.layers {
            let Layer::Circle {
                id,
                source,
                color,
                radius,
                ..
            } = layer
            else {
                continue;
            };
            if let Some(SourceDef::GeoJson { data }) = scene.sources.get(source) {
                let c = color.at(zoom);
                let r = radius.at(zoom);
                for (lng, lat) in crate::geojson::parse_points(data) {
                    self.map.add_marker(Marker {
                        id: MarkerId(0),
                        lng_lat: CoreLatLng { lng, lat },
                        radius_px: r,
                        color: CoreColor::rgba(c.r, c.g, c.b, c.a),
                        data: HashMap::from([("layer".to_string(), id.clone())]),
                    });
                }
            }
        }
    }

    fn resolve(&self, scene: &Scene, source: &str) -> Option<ResolvedSource> {
        scene.sources.get(source).map(|def| self.resolver.resolve(source, def))
    }

    /// Synchronously drain pending tiles against the resolved sources and
    /// ingest them. A convenience for headless rendering, inspection, and
    /// tests; an async host instead drives `pending`/ingest itself.
    pub fn pump_tiles(&mut self) -> DrainStats {
        let mut stats = DrainStats::default();
        loop {
            let pending = self.map.pending_tiles();
            if pending.is_empty() {
                break;
            }
            for req in pending {
                match req {
                    PendingTile::Raster { layer_id, tile } => {
                        if let Some(src) = self.raster_sources.get(&layer_id).cloned() {
                            if let Some((rgba, w, h)) = fetch_decode(src.as_ref(), tile) {
                                self.map.ingest_raster(&layer_id, tile, &rgba, w, h);
                                stats.raster_tiles += 1;
                            }
                        }
                    }
                    PendingTile::Terrain { tile } => {
                        if let Some(src) = self.terrain_source.clone() {
                            if let Some((rgba, w, h)) = fetch_decode(src.as_ref(), tile) {
                                self.map.ingest_terrain_tile(tile, &rgba, w, h);
                                stats.terrain_tiles += 1;
                            }
                        }
                    }
                    PendingTile::Vector { layer_id, tile } => {
                        if let Some(src) = self.vector_sources.get(&layer_id).cloned() {
                            if let Ok(vtile) = src.request(tile) {
                                self.map.ingest_vector_tile(&layer_id, tile, &vtile);
                                stats.vector_tiles += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
            stats.rounds += 1;
            if stats.rounds > 64 {
                // Synthetic/HTTP sources converge in a handful of rounds;
                // this only trips on a scene-state bug.
                break;
            }
        }
        stats
    }

    /// Record one frame into the host's encoder + target view.
    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        self.map.render(encoder, target);
    }

    /// Finalize per-frame bookkeeping after the queue submit.
    pub fn after_submit(&mut self) {
        self.map.after_submit();
    }

    /// Metrics for the last rendered frame (cpu/gpu time, per-layer cache
    /// stats) — the inspection tooling reads these.
    pub fn last_frame_metrics(&self) -> &turbomap_core::map::FrameMetrics {
        self.map.last_frame_metrics()
    }

    /// Layer ids the backend skipped at the last apply.
    pub fn unsupported_layers(&self) -> &[String] {
        &self.unsupported
    }

    /// Inspection escape hatch: the wrapped core map.
    pub fn map(&self) -> &Map {
        &self.map
    }
}

impl MapEngine for TurbomapEngine {
    fn apply(&mut self, scene: Scene) -> SceneDelta {
        let delta = diff(&self.scene, &scene);
        if !delta.is_empty() {
            // Swap the new scene in first so `reconcile` reads it as the
            // target and `old` holds the previous one.
            let old = std::mem::replace(&mut self.scene, scene);
            self.reconcile(&old);
        }
        delta
    }

    fn scene(&self) -> &Scene {
        &self.scene
    }

    fn camera(&self) -> CameraState {
        from_core_camera(self.map.camera())
    }

    fn set_camera(&mut self, camera: CameraState) {
        self.map.set_camera(to_core_camera(camera));
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.map.resize(width, height);
    }

    fn project(&self, geo: LatLng) -> Option<ScreenPoint> {
        let (x, y) = self.map.lng_lat_to_screen(CoreLatLng {
            lng: geo.lng,
            lat: geo.lat,
        });
        Some(ScreenPoint { x, y })
    }

    fn unproject(&self, screen: ScreenPoint) -> Option<LatLng> {
        let ll = self.map.screen_to_lng_lat((screen.x, screen.y));
        Some(LatLng {
            lat: ll.lat,
            lng: ll.lng,
        })
    }

    fn hit_test(&self, screen: ScreenPoint, tol_px: f64) -> Vec<Hit> {
        self.map
            .hit_test((screen.x, screen.y), tol_px)
            .into_iter()
            .map(|hit| match hit {
                HitResult::Feature(f) => Hit {
                    layer_id: f.layer_id,
                    feature_id: Some(f.feature_id.to_string()),
                },
                HitResult::Marker(m) => Hit {
                    // Circle layers stash their id in marker data so a hit
                    // attributes back to the right layer.
                    layer_id: m
                        .data
                        .get("layer")
                        .cloned()
                        .unwrap_or_else(|| "<marker>".to_string()),
                    feature_id: Some(m.id.0.to_string()),
                },
            })
            .collect()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            custom_layers: false,
            terrain: true,
            data_driven_paint: false,
            max_texture_size: self.max_texture_size,
        }
    }
}

fn to_core_camera(c: CameraState) -> Camera {
    Camera::new(
        CoreLatLng {
            lng: c.center.lng,
            lat: c.center.lat,
        },
        c.zoom,
    )
    .with_pitch(c.pitch_deg)
    .with_bearing(c.bearing_deg)
}

fn from_core_camera(c: Camera) -> CameraState {
    CameraState {
        center: LatLng {
            lat: c.center.lat,
            lng: c.center.lng,
        },
        zoom: c.zoom,
        pitch_deg: c.pitch_deg,
        bearing_deg: c.bearing_deg,
    }
}

/// Positional GPU layers in scene order — the ones that occupy a slot in
/// the core layer stack (everything but circles/symbol/custom).
fn positional_layers(scene: &Scene) -> Vec<Layer> {
    scene
        .layers
        .iter()
        .filter(|l| {
            matches!(
                l,
                Layer::Raster { .. }
                    | Layer::Line { .. }
                    | Layer::Fill { .. }
                    | Layer::Hillshade { .. }
            )
        })
        .cloned()
        .collect()
}

fn circle_layers(scene: &Scene) -> Vec<Layer> {
    scene
        .layers
        .iter()
        .filter(|l| matches!(l, Layer::Circle { .. }))
        .cloned()
        .collect()
}

/// Source keys whose definition was added or changed between scenes — the
/// signal that a layer drawing from them must be rebuilt (e.g. a live GPS
/// trace whose GeoJSON data updated while the layer itself didn't).
fn dirty_sources(old: &Scene, new: &Scene) -> std::collections::BTreeSet<String> {
    new.sources
        .iter()
        .filter(|(k, v)| old.sources.get(*k) != Some(v))
        .map(|(k, _)| k.clone())
        .collect()
}

/// Whether this backend can render a layer, by layer kind × source kind.
/// Drives the inspect tool's `unsupported` report.
fn is_supportable(layer: &Layer, scene: &Scene) -> bool {
    let source_is = |want: fn(&SourceDef) -> bool| layer.source().and_then(|s| scene.sources.get(s)).map(want).unwrap_or(false);
    match layer {
        Layer::Raster { .. } => source_is(|d| matches!(d, SourceDef::RasterXyz { .. })),
        Layer::Hillshade { .. } => source_is(|d| matches!(d, SourceDef::DemXyz { .. })),
        Layer::Line { .. } | Layer::Fill { .. } => {
            source_is(|d| matches!(d, SourceDef::GeoJson { .. } | SourceDef::VectorXyz { .. }))
        }
        Layer::Circle { .. } => source_is(|d| matches!(d, SourceDef::GeoJson { .. })),
        Layer::Symbol { .. } | Layer::Custom { .. } => false,
    }
}

/// GeoJSON sources emit a fixed layer name; MVT sources use the layer's
/// declared source-layer.
fn geojson_or_declared(scene: &Scene, source: &str, declared: &Option<String>) -> String {
    if matches!(scene.sources.get(source), Some(SourceDef::GeoJson { .. })) {
        GEOJSON_LAYER.to_string()
    } else {
        declared.clone().unwrap_or_default()
    }
}

/// Build a core `VectorStyle` from a Scene `Fill` layer. Opacity folds
/// into the colour's alpha (core fill paint has no separate opacity).
fn fill_style(
    layer_name: String,
    filter: &Filter,
    color: &Paint<Color>,
    opacity: &Paint<f32>,
    zoom: f64,
) -> VectorStyle {
    let c = color.at(zoom);
    let a = (c.a as f32 * opacity.at(zoom)).round().clamp(0.0, 255.0) as u8;
    VectorStyle {
        background: CoreColor::rgba(0, 0, 0, 0),
        rules: vec![CoreRule {
            source_layer: layer_name,
            filter: map_filter(filter),
            paint: CorePaint::Fill {
                color: CoreColor::rgba(c.r, c.g, c.b, a),
            },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        }],
    }
}

/// Build a core `VectorStyle` from a Scene `Line` layer. Paints are
/// resolved at the current zoom (data-driven/zoom GPU paint is Phase 3);
/// line width is converted from pixels to core's extent units.
fn line_style(
    layer_name: String,
    filter: &Filter,
    color: &Paint<Color>,
    width: &Paint<f32>,
    zoom: f64,
) -> VectorStyle {
    let c = color.at(zoom);
    let w = (width.at(zoom) * PX_TO_EXTENT).max(1.0);
    VectorStyle {
        background: CoreColor::rgba(0, 0, 0, 0),
        rules: vec![CoreRule {
            source_layer: layer_name,
            filter: map_filter(filter),
            paint: CorePaint::Line {
                color: CoreColor::rgba(c.r, c.g, c.b, c.a),
                width: w,
            },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        }],
    }
}

/// Map the IR filter onto core's narrower matcher. Compound forms
/// (`Not`/`All`/`Any`) have no core equivalent yet, so they degrade to
/// `Always` rather than failing.
fn map_filter(filter: &Filter) -> CoreFilter {
    match filter {
        Filter::Always => CoreFilter::Always,
        Filter::Eq(key, value) => CoreFilter::Eq(key.clone(), filter_value_to_string(value)),
        Filter::In(key, values) => {
            CoreFilter::In(key.clone(), values.iter().map(filter_value_to_string).collect())
        }
        Filter::Not(_) | Filter::All(_) | Filter::Any(_) => {
            log::warn!("compound filter unsupported by core style; treating as Always");
            CoreFilter::Always
        }
    }
}

fn filter_value_to_string(value: &FilterValue) -> String {
    match value {
        FilterValue::Bool(b) => b.to_string(),
        FilterValue::Number(n) => n.to_string(),
        FilterValue::String(s) => s.clone(),
    }
}

/// Fetch one tile and decode it to RGBA8. Returns `None` on any failure —
/// a missing tile degrades to "not yet loaded", never a panic.
fn fetch_decode(src: &dyn TileSource, tile: TileId) -> Option<(Vec<u8>, u32, u32)> {
    let raw = src.request(tile).ok()?;
    let img = image::load_from_memory(&raw.bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}
