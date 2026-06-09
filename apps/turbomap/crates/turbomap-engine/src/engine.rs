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

    /// Bring the core `Map` in line with `new`. This slice does a full
    /// rebuild whenever anything changed — correctness first; an
    /// incremental reconcile that preserves unchanged layers' GPU caches
    /// is a follow-up. The returned [`SceneDelta`] is still the minimal
    /// logical change (computed in [`MapEngine::apply`]).
    fn reconcile(&mut self, new: &Scene) {
        for id in self.map.layer_ids() {
            self.map.remove_layer(&id);
        }
        self.map.clear_terrain();
        self.map.clear_markers();
        self.raster_sources.clear();
        self.terrain_source = None;
        self.vector_sources.clear();
        self.unsupported.clear();

        for layer in &new.layers {
            match layer {
                Layer::Raster { id, source, .. } => {
                    match new.sources.get(source).map(|def| self.resolver.resolve(source, def)) {
                        Some(ResolvedSource::Raster(src)) => {
                            self.map.add_raster_layer(id.clone(), src.clone());
                            self.raster_sources.insert(id.clone(), src);
                        }
                        _ => self.unsupported.push(id.clone()),
                    }
                }
                Layer::Hillshade {
                    id,
                    source,
                    exaggeration,
                } => {
                    match new.sources.get(source).map(|def| self.resolver.resolve(source, def)) {
                        Some(ResolvedSource::Dem(dem)) => {
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
                        _ => self.unsupported.push(id.clone()),
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
                    let def = new.sources.get(source);
                    match def.map(|d| self.resolver.resolve(source, d)) {
                        Some(ResolvedSource::Vector(vsrc)) => {
                            let zoom = self.map.camera().zoom;
                            // GeoJSON sources emit a fixed layer name; MVT
                            // sources use the layer's declared source-layer.
                            let layer_name = if matches!(def, Some(SourceDef::GeoJson { .. })) {
                                GEOJSON_LAYER.to_string()
                            } else {
                                source_layer.clone().unwrap_or_default()
                            };
                            let style = line_style(layer_name, filter, color, width, zoom);
                            self.map.add_vector_layer(id.clone(), vsrc.clone(), style);
                            self.vector_sources.insert(id.clone(), vsrc);
                        }
                        _ => self.unsupported.push(id.clone()),
                    }
                }
                Layer::Circle {
                    id,
                    source,
                    color,
                    radius,
                    ..
                } => {
                    // Circles render as core markers (screen-space discs),
                    // positioned in lng/lat — no tiling. We read points
                    // straight from the GeoJSON source's data.
                    match new.sources.get(source) {
                        Some(SourceDef::GeoJson { data }) => {
                            let zoom = self.map.camera().zoom;
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
                        _ => self.unsupported.push(id.clone()),
                    }
                }
                other => self.unsupported.push(other.id().to_string()),
            }
        }
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
            self.reconcile(&scene);
        }
        self.scene = scene;
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
