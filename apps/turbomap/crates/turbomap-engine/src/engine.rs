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
    Paint, Scene, SceneDelta, ScreenPoint, SourceDef, SymbolPlacement, TextAnchor,
};

use crate::geojson::GEOJSON_LAYER;
use crate::resolver::{ResolvedSource, SourceResolver};

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
    /// Colour paint per line/fill layer, re-evaluated each frame so zoom
    /// curves animate on the GPU without re-tessellation.
    layer_colors: HashMap<String, Paint<Color>>,
    /// Layer ids the current backend cannot render (recorded each apply).
    unsupported: Vec<String>,
    max_texture_size: u32,
    /// Device pixel ratio from `MapOptions` — multiplies style-authored
    /// sizes (line widths, fonts, dashes, icons, marker radii) at compile
    /// time so the frame is crisp at the host's native DPI.
    pixel_ratio: f32,
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
        let pixel_ratio = options.pixel_ratio.max(0.5);
        let map = Map::new(device, queue, surface_format, size, to_core_camera(camera), options)?;
        Ok(Self {
            map,
            scene: Scene::new(),
            resolver,
            raster_sources: HashMap::new(),
            terrain_source: None,
            vector_sources: HashMap::new(),
            layer_colors: HashMap::new(),
            unsupported: Vec::new(),
            max_texture_size,
            pixel_ratio,
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
            self.layer_colors.remove(id);
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
                dash_array,
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = line_style(name, filter, color, width, zoom, self.pixel_ratio);
                    self.map.add_vector_layer(id.clone(), vsrc.clone(), style);
                    // A `[dash, gap]` array makes the layer dashed (pixels).
                    if let Some((d, g)) = dash_to_pair(dash_array) {
                        let r = self.pixel_ratio;
                        self.map.set_vector_layer_dash(id, Some((d * r, g * r)));
                    }
                    self.vector_sources.insert(id.clone(), vsrc);
                    // Single-colour layers get a per-frame GPU override
                    // (zoom curves animate); data-driven Match colours are
                    // baked per-feature, so leave the override off.
                    if !color.is_data_driven() {
                        self.layer_colors.insert(id.clone(), color.clone());
                    }
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
                    if !color.is_data_driven() {
                        self.layer_colors.insert(id.clone(), color.clone());
                    }
                }
            }
            Layer::Symbol {
                id,
                source,
                source_layer,
                filter,
                text_field,
                text_size,
                color,
                halo_color,
                halo_width,
                sort_key,
                placement,
                icon_image,
                icon_size,
                icon_color,
                text_anchor,
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = symbol_style(
                        name, filter, text_field, text_size, color, halo_color, halo_width,
                        sort_key, *placement, icon_image, icon_size, icon_color, zoom,
                        self.pixel_ratio, *text_anchor,
                    );
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
                let r = radius.at(zoom) * self.pixel_ratio;
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
            let before_round = stats.raster_tiles + stats.terrain_tiles + stats.vector_tiles;
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
            let ingested = stats.raster_tiles + stats.terrain_tiles + stats.vector_tiles;
            if ingested == before_round {
                // No progress this round — remaining pending tiles come
                // from sources this pump can't serve (e.g. remote stubs a
                // host feeds via the ingest API instead). Stop rather than
                // spin.
                break;
            }
            if stats.rounds > 64 {
                // Synthetic/HTTP sources converge in a handful of rounds;
                // this only trips on a scene-state bug.
                break;
            }
        }
        stats
    }

    /// Tiles the engine is waiting on — the pull half of host-driven tile
    /// IO. Hosts fetch these (they know the URL templates from their own
    /// scene) and push results through the `ingest_*` methods below.
    pub fn pending_tiles(&self) -> Vec<PendingTile> {
        self.map.pending_tiles()
    }

    /// Push one fetched raster tile (encoded PNG/JPEG/WebP bytes, exactly
    /// as served). Returns `false` if the bytes don't decode.
    pub fn ingest_raster_encoded(&mut self, layer_id: &str, tile: TileId, bytes: &[u8]) -> bool {
        let Ok(img) = image::load_from_memory(bytes) else {
            return false;
        };
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        self.map.ingest_raster(layer_id, tile, img.as_raw(), w, h);
        true
    }

    /// Push one fetched DEM tile (encoded Terrain-RGB/Terrarium image).
    pub fn ingest_terrain_encoded(&mut self, tile: TileId, bytes: &[u8]) -> bool {
        let Ok(img) = image::load_from_memory(bytes) else {
            return false;
        };
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        self.map.ingest_terrain_tile(tile, img.as_raw(), w, h);
        true
    }

    /// Push one fetched vector tile (raw MVT protobuf bytes). Returns
    /// `false` if the bytes don't decode.
    pub fn ingest_mvt(&mut self, layer_id: &str, tile: TileId, bytes: &[u8]) -> bool {
        let Ok(vtile) = turbomap_core::vector::decode_mvt(bytes) else {
            return false;
        };
        self.map.ingest_vector_tile(layer_id, tile, &vtile);
        true
    }

    /// Animate the camera to `target` over `duration`; drive with
    /// [`tick_now`](Self::tick_now) each frame.
    pub fn ease_to(&mut self, target: CameraState, duration: std::time::Duration) {
        self.map.ease_to(to_core_camera(target), duration);
    }

    /// Start an inertial fling (momentum pan) from the current camera at the
    /// drag-release velocity `velocity_px` (screen px/s). Drive it with
    /// [`tick_now`](Self::tick_now) each frame; a new pan/zoom cancels it.
    pub fn fling(&mut self, velocity_px: (f64, f64)) {
        self.map.fling(velocity_px);
    }

    /// Pan the map by a screen-pixel drag delta (the per-move gesture step).
    pub fn pan_by_pixels(&mut self, dx: f64, dy: f64) {
        self.map.pan_by_pixels(dx, dy);
    }

    /// Zoom by `factor` (2.0 = one level in) about `focus_px`, keeping that
    /// pixel over the same place — the immediate scroll/pinch step.
    pub fn zoom_around(&mut self, factor: f64, focus_px: (f64, f64)) {
        self.map.zoom_around(factor, focus_px);
    }

    /// Start a zoom fling (pinch-release momentum) at `zoom_velocity`
    /// (zoom-levels/s) about `focus_px`. Drive with
    /// [`tick_now`](Self::tick_now); a new pan/zoom cancels it.
    pub fn zoom_fling(&mut self, zoom_velocity: f64, focus_px: (f64, f64)) {
        self.map.zoom_fling(zoom_velocity, focus_px);
    }

    /// Rotate the compass bearing by `delta_deg` — the two-finger rotate
    /// gesture (wraps to [0, 360)).
    pub fn rotate_by(&mut self, delta_deg: f64) {
        self.map.rotate_by(delta_deg);
    }

    /// Tilt by `delta_deg` — the two-finger vertical-drag gesture (clamped
    /// to the pitch limit).
    pub fn pitch_by(&mut self, delta_deg: f64) {
        self.map.pitch_by(delta_deg);
    }

    /// Rotate by `delta_deg` about `focus_px` (the two-finger centroid),
    /// keeping that pixel anchored — the natural pivot for the gesture.
    pub fn rotate_around(&mut self, delta_deg: f64, focus_px: (f64, f64)) {
        self.map.rotate_around(delta_deg, focus_px);
    }

    /// Tilt by `delta_deg` about `focus_px`, keeping that pixel anchored.
    pub fn pitch_around(&mut self, delta_deg: f64, focus_px: (f64, f64)) {
        self.map.pitch_around(delta_deg, focus_px);
    }

    /// Animated focus-invariant zoom over `duration` — the smooth double-tap
    /// zoom. Drive with [`tick_now`](Self::tick_now).
    pub fn zoom_around_animated(
        &mut self,
        factor: f64,
        focus_px: (f64, f64),
        duration: std::time::Duration,
    ) {
        self.map.zoom_around_animated(factor, focus_px, duration);
    }

    /// Advance any running camera animation. Returns `true` while the
    /// animation is still in flight (i.e. keep rendering frames).
    pub fn tick_now(&mut self) -> bool {
        self.map.tick(std::time::Instant::now())
    }

    /// Re-evaluate line/fill colour paints at the current zoom and push
    /// them to the GPU as per-layer overrides. Cheap (one eval per layer),
    /// no re-tessellation — this is how zoom-curve / data-driven colour
    /// stays live. Called automatically before every [`render`](Self::render).
    pub fn update_dynamic_paint(&mut self) {
        let zoom = self.map.camera().zoom;
        for (id, paint) in &self.layer_colors {
            let c = paint.at(zoom);
            // Same colour-management contract as baked vertex colours:
            // authored sRGB, decoded to linear once before the shader.
            let linear = CoreColor::rgba(c.r, c.g, c.b, c.a).to_linear_f32();
            self.map.set_vector_layer_color(id, Some(linear));
        }
    }

    /// Record one frame into the host's encoder + target view.
    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        self.update_dynamic_paint();
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

    /// Register a fallback font face for scripts the bundled Latin default
    /// doesn't cover (CJK, Arabic, …). The host supplies the font bytes
    /// (bundled asset or platform font). Returns `false` if they don't
    /// parse. Glyphs pack into the shared atlas on first use.
    pub fn add_fallback_font(&mut self, bytes: Vec<u8>) -> bool {
        self.map.add_fallback_font(bytes)
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
                    properties: f
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), vector_value_to_string(v)))
                        .collect(),
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
                    properties: m.data,
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
                    | Layer::Symbol { .. }
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
        Layer::Line { .. } | Layer::Fill { .. } | Layer::Symbol { .. } => {
            source_is(|d| matches!(d, SourceDef::GeoJson { .. } | SourceDef::VectorXyz { .. }))
        }
        Layer::Circle { .. } => source_is(|d| matches!(d, SourceDef::GeoJson { .. })),
        Layer::Custom { .. } => false,
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

fn to_core_color(c: Color) -> CoreColor {
    CoreColor::rgba(c.r, c.g, c.b, c.a)
}

/// Compile a colour paint into `(filter, colour)` pairs — the per-feature
/// rules a core `VectorStyle` needs. `Const`/`Zoom` collapse to a single
/// rule (resolved at zoom); `Match` expands to one rule per case (matched
/// on the feature property) plus a default. Cases are emitted before the
/// default so core's first-match-wins picks the specific case.
fn color_rules(layer_filter: &Filter, color: &Paint<Color>, zoom: f64) -> Vec<(CoreFilter, CoreColor)> {
    match color {
        Paint::Match {
            property,
            cases,
            default,
        } => {
            let mut rules: Vec<(CoreFilter, CoreColor)> = cases
                .iter()
                .map(|case| {
                    (
                        CoreFilter::Eq(property.clone(), filter_value_to_string(&case.value)),
                        to_core_color(case.result),
                    )
                })
                .collect();
            rules.push((map_filter(layer_filter), to_core_color(**default)));
            rules
        }
        _ => vec![(map_filter(layer_filter), to_core_color(color.at(zoom)))],
    }
}

/// Build a core `VectorStyle` from a Scene `Fill` layer. Opacity folds
/// into each colour's alpha (core fill paint has no separate opacity).
fn fill_style(
    layer_name: String,
    filter: &Filter,
    color: &Paint<Color>,
    opacity: &Paint<f32>,
    zoom: f64,
) -> VectorStyle {
    let op = opacity.at(zoom);
    let rules = color_rules(filter, color, zoom)
        .into_iter()
        .map(|(f, c)| {
            let a = (c.a as f32 * op).round().clamp(0.0, 255.0) as u8;
            CoreRule {
                source_layer: layer_name.clone(),
                filter: f,
                paint: CorePaint::Fill {
                    color: CoreColor::rgba(c.r, c.g, c.b, a),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }
        })
        .collect();
    VectorStyle {
        background: CoreColor::rgba(0, 0, 0, 0),
        rules,
    }
}

/// Build a core `VectorStyle` from a Scene `Symbol` layer — a text label
/// per point feature, reading the label from each feature's `text_field`
/// property.
#[allow(clippy::too_many_arguments)]
fn symbol_style(
    layer_name: String,
    filter: &Filter,
    text_field: &str,
    text_size: &Paint<f32>,
    color: &Paint<Color>,
    halo_color: &Paint<Color>,
    halo_width: &Paint<f32>,
    sort_key: &Option<String>,
    placement: SymbolPlacement,
    icon_image: &Option<String>,
    icon_size: &Paint<f32>,
    icon_color: &Paint<Color>,
    zoom: f64,
    pixel_ratio: f32,
    text_anchor: TextAnchor,
) -> VectorStyle {
    let hc = halo_color.at(zoom);
    let core_halo = CoreColor::rgba(hc.r, hc.g, hc.b, hc.a);
    // Halo width already scales with font size on screen (it is in glyph
    // raster px), so it must NOT be multiplied by pixel_ratio again.
    let hw = halo_width.at(zoom);
    let font = text_size.at(zoom) * pixel_ratio;
    let along_line = matches!(placement, SymbolPlacement::Line);
    let left_anchor = matches!(text_anchor, TextAnchor::Left);
    let ic = icon_color.at(zoom);
    let icon = icon_image.as_ref().map(|sprite| turbomap_core::IconSpec {
        sprite: sprite.clone(),
        size_px: (icon_size.at(zoom) * pixel_ratio).max(1.0),
        color: CoreColor::rgba(ic.r, ic.g, ic.b, ic.a),
    });
    // Text colour is data-driven too (e.g. a different colour per place
    // class), so it expands to per-feature rules like lines/fills.
    let rules = color_rules(filter, color, zoom)
        .into_iter()
        .map(|(f, c)| CoreRule {
            source_layer: layer_name.clone(),
            filter: f,
            paint: CorePaint::Text {
                text_field: text_field.to_string(),
                font_size_px: font,
                color: c,
                halo_color: core_halo,
                halo_width: hw,
                rank_field: sort_key.clone(),
                along_line,
                icon: icon.clone(),
                left_anchor,
            },
            min_zoom: 0,
            max_zoom: 22,
            // Symbol layers carrying an icon are POI markers — retain their
            // features so a tap can report the place. Plain text labels
            // (place/street names) aren't tappable, so they stay light.
            interactive: icon.is_some(),
        })
        .collect();
    VectorStyle {
        background: CoreColor::rgba(0, 0, 0, 0),
        rules,
    }
}

/// Build a core `VectorStyle` from a Scene `Line` layer. Paints are
/// resolved at the current zoom (data-driven/zoom GPU paint is Phase 3);
/// line width is converted from pixels to core's extent units.
/// View a paint as a data-driven `Match`, if it is one.
fn as_match<T>(paint: &Paint<T>) -> Option<(&str, &[turbomap_scene::MatchCase<T>], &T)> {
    match paint {
        Paint::Match {
            property,
            cases,
            default,
        } => Some((property.as_str(), cases.as_slice(), &**default)),
        _ => None,
    }
}

/// Style widths are logical px; scale by the device pixel ratio and clamp
/// to a sane physical minimum (widths are extruded GPU-side per frame).
fn line_width_px(px: f32, pixel_ratio: f32) -> f32 {
    (px * pixel_ratio).max(0.5)
}

/// Reduce a scene `dash_array` to the `(dash, gap)` pixel pair the renderer
/// consumes. A single positive value means equal dash/gap; an empty or
/// all-zero array (or `None`) means solid (`None`).
fn dash_to_pair(dash_array: &Option<Vec<f32>>) -> Option<(f32, f32)> {
    let a = dash_array.as_ref()?;
    let dash = *a.first()?;
    if dash <= 0.0 {
        return None;
    }
    let gap = a.get(1).copied().unwrap_or(dash).max(0.0);
    Some((dash, gap))
}

/// Compile a line layer's colour + width into `(filter, colour, width)`
/// rules — the road hierarchy. When both colour and width are `Match` on
/// the *same* feature property (e.g. road `kind`), their cases align into
/// one rule per class: major roads get their colour *and* their (wider)
/// width together. A single `Match` drives the cases while the other paint
/// stays constant; neither `Match` collapses to one rule.
fn line_rules(
    layer_filter: &Filter,
    color: &Paint<Color>,
    width: &Paint<f32>,
    zoom: f64,
    pixel_ratio: f32,
) -> Vec<(CoreFilter, CoreColor, f32)> {
    let cm = as_match(color);
    let wm = as_match(width);
    if cm.is_none() && wm.is_none() {
        return vec![(
            map_filter(layer_filter),
            to_core_color(color.at(zoom)),
            line_width_px(width.at(zoom), pixel_ratio),
        )];
    }

    // The property whose cases drive the rules (colour's if both exist).
    let driving = cm.map(|(p, ..)| p).or(wm.map(|(p, ..)| p)).unwrap();
    if let (Some((cp, ..)), Some((wp, ..))) = (cm, wm) {
        if cp != wp {
            log::warn!(
                "line layer colour ({cp}) and width ({wp}) keyed on different properties; \
                 width uses its default over colour's cases"
            );
        }
    }

    // Case values from whichever Match paints key on `driving` (colour and
    // width have different `T`, so collect from each separately).
    let mut values: Vec<&FilterValue> = Vec::new();
    if let Some((p, cases, _)) = cm {
        if p == driving {
            for case in cases {
                if !values.iter().any(|v| **v == case.value) {
                    values.push(&case.value);
                }
            }
        }
    }
    if let Some((p, cases, _)) = wm {
        if p == driving {
            for case in cases {
                if !values.iter().any(|v| **v == case.value) {
                    values.push(&case.value);
                }
            }
        }
    }

    let resolve_color = |value: Option<&FilterValue>| -> CoreColor {
        match cm {
            Some((p, cases, default)) if p == driving => to_core_color(
                value
                    .and_then(|v| cases.iter().find(|c| &c.value == v))
                    .map(|c| c.result)
                    .unwrap_or(*default),
            ),
            _ => to_core_color(color.at(zoom)),
        }
    };
    let resolve_width = |value: Option<&FilterValue>| -> f32 {
        match wm {
            Some((p, cases, default)) if p == driving => line_width_px(
                value
                    .and_then(|v| cases.iter().find(|c| &c.value == v))
                    .map(|c| c.result)
                    .unwrap_or(*default),
                pixel_ratio,
            ),
            _ => line_width_px(width.at(zoom), pixel_ratio),
        }
    };

    let mut rules: Vec<(CoreFilter, CoreColor, f32)> = values
        .iter()
        .map(|v| {
            (
                CoreFilter::Eq(driving.to_string(), filter_value_to_string(v)),
                resolve_color(Some(v)),
                resolve_width(Some(v)),
            )
        })
        .collect();
    rules.push((map_filter(layer_filter), resolve_color(None), resolve_width(None)));
    rules
}

fn line_style(
    layer_name: String,
    filter: &Filter,
    color: &Paint<Color>,
    width: &Paint<f32>,
    zoom: f64,
    pixel_ratio: f32,
) -> VectorStyle {
    let rules = line_rules(filter, color, width, zoom, pixel_ratio)
        .into_iter()
        .map(|(f, c, w)| CoreRule {
            source_layer: layer_name.clone(),
            filter: f,
            paint: CorePaint::Line { color: c, width: w },
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        })
        .collect();
    VectorStyle {
        background: CoreColor::rgba(0, 0, 0, 0),
        rules,
    }
}

/// Map the IR filter onto core's matcher, including the compound
/// `Not`/`All`/`Any` forms (used for e.g. "roads that aren't tunnels").
fn map_filter(filter: &Filter) -> CoreFilter {
    match filter {
        Filter::Always => CoreFilter::Always,
        Filter::Eq(key, value) => CoreFilter::Eq(key.clone(), filter_value_to_string(value)),
        Filter::In(key, values) => {
            CoreFilter::In(key.clone(), values.iter().map(filter_value_to_string).collect())
        }
        Filter::Not(inner) => CoreFilter::Not(Box::new(map_filter(inner))),
        Filter::All(fs) => CoreFilter::All(fs.iter().map(map_filter).collect()),
        Filter::Any(fs) => CoreFilter::Any(fs.iter().map(map_filter).collect()),
    }
}

fn filter_value_to_string(value: &FilterValue) -> String {
    match value {
        FilterValue::Bool(b) => b.to_string(),
        FilterValue::Number(n) => n.to_string(),
        FilterValue::String(s) => s.clone(),
    }
}

/// Stringify an MVT feature value for a hit-test result's property map —
/// what a host shows when a place is tapped.
fn vector_value_to_string(value: &turbomap_core::VectorValue) -> String {
    use turbomap_core::VectorValue as V;
    match value {
        V::String(s) => s.clone(),
        V::Float(f) => f.to_string(),
        V::Int(i) => i.to_string(),
        V::UInt(u) => u.to_string(),
        V::Bool(b) => b.to_string(),
        V::Null => String::new(),
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
