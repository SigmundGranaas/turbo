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
    Camera, Color as CoreColor, CustomLayer, CustomLayerInit, Filter as CoreFilter, HillshadeStyle,
    HitResult, LatLng as CoreLatLng, Map, MapError, MapOptions, Marker, MarkerId,
    Paint as CorePaint, PendingTile, RadarFrame, Rule as CoreRule, TerrainOptions, TileId,
    TileSource, VectorStyle, VectorTileSource, ZoomBounds,
};
use turbomap_scene::{
    diff, CameraState, Capabilities, Color, Filter, FilterValue, Hit, LatLng, Layer, MapEngine,
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
    /// Ids of every `Line` layer, so the per-frame width-zoom curve can grow
    /// their baked widths on the GPU (no re-tessellation). Data-driven colour
    /// lines aren't in `layer_colors`, so width needs its own roster.
    line_layers: Vec<String>,
    /// Layer ids the current backend cannot render (recorded each apply).
    unsupported: Vec<String>,
    max_texture_size: u32,
    /// Device pixel ratio from `MapOptions` — multiplies style-authored
    /// sizes (line widths, fonts, dashes, icons, marker radii) at compile
    /// time so the frame is crisp at the host's native DPI.
    pixel_ratio: f32,
    /// Raster/DEM image decode + MVT tessellation off the render thread
    /// (plan B4.1/B4.2): the `ingest_*` methods enqueue bytes here;
    /// `render()` applies decoded tiles under a per-frame budget. See
    /// [`crate::codec`].
    decode_queue: crate::codec::DecodeQueue,
    /// Per-vector-layer style generation, bumped on every (re)install. A
    /// tessellation result carries the epoch it was built against; apply
    /// drops it on mismatch — a repaint/rebuild that raced a decode must
    /// not paint stale style onto the map.
    vector_style_epochs: HashMap<String, u64>,
    /// The Field2D source id the scene-declared cloud overlay consumes
    /// (plan C2) — [`Self::ingest_field`] routes frames only for it.
    cloud_field_source: Option<String>,
    /// Custom-layer factories by scene `kind` (plan D4). A scene
    /// `Layer::Custom { kind }` binds to the registered factory; unknown
    /// kinds degrade to the unsupported report. The `flow-field` demo kind
    /// is registered by default.
    custom_layer_kinds: HashMap<String, CustomLayerFactory>,
}

/// Builds a [`CustomLayer`] against the map's MSAA-pass parameters.
pub type CustomLayerFactory = Box<dyn Fn(&CustomLayerInit) -> Box<dyn CustomLayer> + Send + Sync>;

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
        let map = Map::new(
            device,
            queue,
            surface_format,
            size,
            to_core_camera(camera),
            options,
        )?;
        let mut engine = Self {
            map,
            scene: Scene::new(),
            resolver,
            raster_sources: HashMap::new(),
            terrain_source: None,
            vector_sources: HashMap::new(),
            layer_colors: HashMap::new(),
            line_layers: Vec::new(),
            unsupported: Vec::new(),
            max_texture_size,
            pixel_ratio,
            decode_queue: crate::codec::DecodeQueue::new(),
            vector_style_epochs: HashMap::new(),
            cloud_field_source: None,
            custom_layer_kinds: HashMap::new(),
        };
        // The built-in demo/diagnostic custom layer — registered by default
        // so every host (desktop, Android, web) can declare
        // `Layer::Custom { kind: "flow-field" }` with zero host code, which
        // is the D4 portability gate.
        engine.register_custom_layer_kind("flow-field", |init| {
            Box::new(crate::custom_layers::FlowFieldLayer::new(init))
        });
        Ok(engine)
    }

    /// Register (or replace) a custom-layer `kind`: scenes declaring
    /// `Layer::Custom {{ kind }}` will bind to `factory`. Takes effect at
    /// the next `apply`.
    pub fn register_custom_layer_kind(
        &mut self,
        kind: impl Into<String>,
        factory: impl Fn(&CustomLayerInit) -> Box<dyn CustomLayer> + Send + Sync + 'static,
    ) {
        self.custom_layer_kinds
            .insert(kind.into(), Box::new(factory));
    }

    /// Pin the frame animation clock (haze drift, custom-layer time) for
    /// deterministic rendering; `None` returns to the wall clock.
    pub fn set_time_override(&mut self, secs: Option<f32>) {
        self.map.set_time_override(secs);
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
            && new_pos[prefix]
                .source()
                .map(|s| !dirty.contains(s))
                .unwrap_or(true)
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
            self.line_layers.retain(|l| l != id);
        }
        // Re-install the new tail in order (core appends).
        for layer in new_pos.iter().skip(prefix) {
            self.install_positional(layer, &new);
        }

        // --- route tubes: scene content, but each is an overlay MESH keyed
        // by id (not a core stack layer), so they diff as their own set.
        // An undeclared id clears (an empty polyline removes the mesh); a
        // new or changed declaration (or dirty source) re-installs — the
        // core's set_route_tube is idempotent per id.
        let old_tubes = tube_layers(old);
        let new_tubes = tube_layers(&new);
        for t in &old_tubes {
            if !new_tubes.iter().any(|n| n.id() == t.id()) {
                self.map
                    .set_route_tube(t.id(), &[], CoreColor::rgba(0, 0, 0, 0), 0.0);
            }
        }
        for t in &new_tubes {
            let unchanged = old_tubes.iter().any(|o| o == t)
                && t.source().map(|s| !dirty.contains(s)).unwrap_or(true);
            if !unchanged {
                self.install_positional(t, &new);
            }
        }
        // Terrain is global; if the new scene has no hillshade, drop it.
        if !new
            .layers
            .iter()
            .any(|l| matches!(l, Layer::Hillshade { .. }))
        {
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
            .filter(|l| !is_supportable(l, &new, &self.custom_layer_kinds))
            .map(|l| l.id().to_string())
            .collect();
    }

    /// Install one positional layer into the core map (append).
    fn install_positional(&mut self, layer: &Layer, scene: &Scene) {
        // Any (re)install of a vector layer invalidates tessellations built
        // against its previous style: bump the layer's style epoch so
        // in-flight decode results are dropped at apply. Entries are never
        // removed — epochs stay monotonic per id, so a layer that is
        // removed and later re-added can't collide with a stale result.
        match layer {
            Layer::Line { id, .. }
            | Layer::Fill { id, .. }
            | Layer::FillExtrusion { id, .. }
            | Layer::Symbol { id, .. } => {
                *self.vector_style_epochs.entry(id.clone()).or_insert(0) += 1;
            }
            _ => {}
        }
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
                height_only,
            } => {
                if let Some(ResolvedSource::Dem(dem)) = self.resolve(scene, source) {
                    self.map.set_terrain_source(
                        dem.clone(),
                        TerrainOptions {
                            exaggeration: *exaggeration,
                            ..Default::default()
                        },
                    );
                    // Height-only: the DEM just displaces the ground and
                    // the basemap lights itself from the sun. Otherwise
                    // also draw the classic relief-shading overlay.
                    if !*height_only {
                        self.map
                            .add_hillshade_layer(id.clone(), HillshadeStyle::default());
                    }
                    self.terrain_source = Some(dem);
                }
            }
            Layer::Custom { id, kind } => {
                // Bind to the registered factory; unknown kinds fall through
                // to the unsupported report (degrade, don't guess).
                if let Some(factory) = self.custom_layer_kinds.get(kind.as_str()) {
                    let layer = factory(&self.map.custom_layer_init());
                    self.map.add_custom_layer(id.clone(), kind.clone(), layer);
                }
            }
            Layer::Tube {
                id,
                source,
                color,
                radius_px,
            } => {
                // Scene-declared 3D route tube (plan P5.2). The polyline
                // comes straight from the GeoJSON source; the tube mesh is
                // not a stack layer, so removal is handled in `reconcile`'s
                // tube diff, not the positional prefix.
                if let Some(SourceDef::GeoJson { data }) = scene.sources.get(source) {
                    let points: Vec<CoreLatLng> = crate::geojson::parse_line(data)
                        .into_iter()
                        .map(|(lng, lat)| CoreLatLng::new(lat, lng))
                        .collect();
                    self.map.set_route_tube(
                        id,
                        &points,
                        CoreColor::rgba(color.r, color.g, color.b, color.a),
                        *radius_px,
                    );
                }
            }
            Layer::Line {
                id,
                source,
                source_layer,
                color,
                dash_array,
                ..
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = compile_vector_layer_style(layer, name, zoom, self.pixel_ratio)
                        .expect("Line compiles to a vector style");
                    self.map.add_vector_layer(id.clone(), vsrc.clone(), style);
                    // A `[dash, gap]` array makes the layer dashed (pixels).
                    if let Some((d, g)) = dash_to_pair(dash_array) {
                        let r = self.pixel_ratio;
                        self.map.set_vector_layer_dash(id, Some((d * r, g * r)));
                    }
                    self.vector_sources.insert(id.clone(), vsrc);
                    self.line_layers.push(id.clone());
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
                color,
                ..
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = compile_vector_layer_style(layer, name, zoom, self.pixel_ratio)
                        .expect("Fill compiles to a vector style");
                    self.map.add_vector_layer(id.clone(), vsrc.clone(), style);
                    self.vector_sources.insert(id.clone(), vsrc);
                    if !color.is_data_driven() {
                        self.layer_colors.insert(id.clone(), color.clone());
                    }
                }
            }
            Layer::FillExtrusion {
                id,
                source,
                source_layer,
                ..
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = compile_vector_layer_style(layer, name, zoom, self.pixel_ratio)
                        .expect("FillExtrusion compiles to a vector style");
                    self.map.add_vector_layer(id.clone(), vsrc.clone(), style);
                    self.vector_sources.insert(id.clone(), vsrc);
                }
            }
            Layer::Symbol {
                id,
                source,
                source_layer,
                ..
            } => {
                if let Some(ResolvedSource::Vector(vsrc)) = self.resolve(scene, source) {
                    let zoom = self.map.camera().zoom;
                    let name = geojson_or_declared(scene, source, source_layer);
                    let style = compile_vector_layer_style(layer, name, zoom, self.pixel_ratio)
                        .expect("Symbol compiles to a vector style");
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
                // Feature properties ride into the marker's data (P6.4) so a
                // hit test answers with the feature's domain attributes
                // (id, name), not just a position. `layer` is reserved.
                for ((lng, lat), props) in crate::geojson::parse_points_with_props(data) {
                    let mut data: HashMap<String, String> = props;
                    data.insert("layer".to_string(), id.clone());
                    self.map.add_marker(Marker {
                        id: MarkerId(0),
                        lng_lat: CoreLatLng { lng, lat },
                        radius_px: r,
                        color: CoreColor::rgba(c.r, c.g, c.b, c.a),
                        data,
                    });
                }
            }
        }
    }

    fn resolve(&self, scene: &Scene, source: &str) -> Option<ResolvedSource> {
        scene
            .sources
            .get(source)
            .map(|def| self.resolver.resolve(source, def))
    }

    /// Synchronously drain pending tiles against the resolved sources and
    /// ingest them. A convenience for headless rendering, inspection, and
    /// tests; an async host instead drives `pending`/ingest itself.
    pub fn pump_tiles(&mut self) -> DrainStats {
        let mut stats = DrainStats::default();
        loop {
            let before_round = stats.raster_tiles + stats.terrain_tiles + stats.vector_tiles;
            let pending = self.map.plan_start_preview();
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
                            if let Some(dem) = fetch_decode_dem(src.as_ref(), tile) {
                                self.map.ingest_terrain_tile(tile, &dem);
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

    /// Push one fetched raster tile (encoded PNG/JPEG/WebP bytes, exactly
    /// as served). The bytes are ACCEPTED, not decoded here (plan B4.1):
    /// decode runs off the render thread and the tile applies during a
    /// later `render()` under the per-frame budget — so the plan keeps the
    /// attempt `Fetching` until it becomes drawable. Undecodable bytes are
    /// dropped and the tile re-pends; the host's retry/backoff owns policy.
    /// Returns `true` when accepted (a duplicate already in flight is
    /// "accepted" too — the queue dedups).
    pub fn ingest_raster_encoded(&mut self, layer_id: &str, tile: TileId, bytes: &[u8]) -> bool {
        // A delivery that raced the accept→apply window must not re-upload a
        // now-resident tile — that restarts the fade and reads as
        // steady-state flicker.
        if self.map.is_raster_ingested(layer_id, tile) {
            return true;
        }
        self.decode_queue.enqueue(
            crate::codec::QueueKey::Raster {
                layer_id: layer_id.to_string(),
                tile,
            },
            bytes.to_vec(),
            None,
            None,
        );
        true
    }

    /// Push one fetched DEM tile (encoded Terrain-RGB/Terrarium image).
    /// Same accept-then-decode-off-thread contract as
    /// [`Self::ingest_raster_encoded`].
    pub fn ingest_terrain_encoded(&mut self, tile: TileId, bytes: &[u8]) -> bool {
        if self.map.is_terrain_ingested(tile) {
            return true;
        }
        // The DEM codec (plan D3) runs in the decode worker; hand it the
        // source's declared RGB→metres encoding.
        let enc = self.terrain_source.as_ref().map(|s| s.dem_encoding());
        self.decode_queue.enqueue(
            crate::codec::QueueKey::Terrain { tile },
            bytes.to_vec(),
            None,
            enc,
        );
        true
    }

    /// Apply decoded tiles to the GPU caches, bounded by
    /// the tiered apply budget ([`crate::codec::APPLY_BUDGET_MOVING`] while
    /// the camera animates, [`crate::codec::APPLY_BUDGET_SETTLED`] otherwise).
    /// Runs at the top of every `render()`;
    /// public so headless harnesses can drain deterministically without
    /// rendering.
    pub fn pump_decoded(&mut self) {
        use crate::codec::{DecodedKind, QueueKey};
        let budget = if self.map.is_camera_animating() {
            crate::codec::APPLY_BUDGET_MOVING
        } else {
            crate::codec::APPLY_BUDGET_SETTLED
        };
        let map = &mut self.map;
        let epochs = &self.vector_style_epochs;
        self.decode_queue.drain(budget, |d| match (d.key, d.kind) {
            (QueueKey::Raster { ref layer_id, tile }, DecodedKind::Image { rgba, w, h }) => {
                map.ingest_raster(layer_id, tile, &rgba, w, h);
            }
            (QueueKey::Terrain { tile }, DecodedKind::Dem { dem }) => {
                map.ingest_terrain_tile(tile, &dem);
            }
            (QueueKey::Vector { ref layer_id, tile }, DecodedKind::Vector { out, epoch }) => {
                // Stale-style guard: a repaint/rebuild bumped the epoch
                // while this tile tessellated — drop it; the tile is still
                // pending and refetches against the new style.
                if epochs.get(layer_id).copied().unwrap_or(0) == epoch {
                    map.ingest_vector_mesh(
                        layer_id,
                        tile,
                        &out.mesh,
                        out.labels,
                        out.icons,
                        out.interactive,
                    );
                }
            }
            // Key/kind disagreement cannot be constructed by `decode`.
            _ => {}
        });
    }

    /// Push one fetched vector tile (raw MVT protobuf bytes). Accepted,
    /// not decoded here (plan B4.2): MVT decode + lyon tessellation run off
    /// the render thread against the layer's CURRENT style, and the mesh
    /// applies during a later `render()` under the per-frame budget. A
    /// style change that races the decode is caught by an epoch check at
    /// apply — the stale mesh is dropped and the tile re-pends.
    pub fn ingest_mvt(&mut self, layer_id: &str, tile: TileId, bytes: &[u8]) -> bool {
        if self.map.is_vector_ingested(layer_id, tile) {
            return true;
        }
        let Some(style) = self.map.vector_layer_style(layer_id) else {
            // Layer gone (raced a scene edit): accepted-and-dropped.
            return true;
        };
        let epoch = self.vector_style_epochs.get(layer_id).copied().unwrap_or(0);
        self.decode_queue.enqueue(
            crate::codec::QueueKey::Vector {
                layer_id: layer_id.to_string(),
                tile,
            },
            bytes.to_vec(),
            Some((std::sync::Arc::new(style), epoch)),
            None,
        );
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

    /// Reserve `bottom_px` at the bottom of the viewport (e.g. a sheet over the
    /// map) — shifts projection + rendering up by `bottom_px/2`. 0 = none.
    pub fn set_viewport_inset(&mut self, bottom_px: f64) {
        self.map.set_viewport_inset(bottom_px);
    }

    /// Reserve `right_px` at the right of the viewport (desktop side panel) —
    /// shifts projection/unprojection/render left by `right_px/2`.
    pub fn set_viewport_inset_right(&mut self, right_px: f64) {
        self.map.set_viewport_inset_right(right_px);
    }

    /// Lock the camera's zoom so the user can't zoom past the map's
    /// accuracy. Pass an explicit `(min, max)` to override, or `None` to
    /// track the active tile sources automatically (the default — bounds
    /// follow each layer's declared zoom range). The current zoom is clamped
    /// into range immediately.
    pub fn set_zoom_bounds(&mut self, bounds: Option<(f64, f64)>) {
        self.map
            .set_zoom_bounds(bounds.map(|(min, max)| ZoomBounds::new(min, max)));
    }

    /// The zoom range the camera is currently locked to, as `(min, max)`.
    pub fn zoom_bounds(&self) -> (f64, f64) {
        let b = self.map.zoom_bounds();
        (b.min, b.max)
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

    // ---- Environment / content (SCENE-owned, plans P5.2 + P6.1) --------
    //
    // There are deliberately NO imperative content setters here — public
    // or hidden: lighting, shadows, haze, gain, clouds, and route tubes
    // are Scene state. Declare them in the IR (`environment`,
    // `Layer::Tube`) and re-apply; the engine diffs. Even the sim gates
    // author scenes (P6.1 deleted the last harness verbs), and the
    // `invariants` test forbids their return.

    /// Push one field frame for a scene-declared [`SourceDef::Field2D`]
    /// source (plan C2) — the field twin of the tile `ingest_*` methods.
    /// Frames for a source the current scene's cloud overlay doesn't
    /// consume are dropped with a warning (data is transport; the SCENE
    /// decides what renders). `slot` is 0 (current timestep) or 1 (next).
    pub fn ingest_field(
        &mut self,
        source: &str,
        slot: u32,
        grid_w: u32,
        grid_h: u32,
        precip: &[u8],
        coverage: &[u8],
    ) -> bool {
        if self.cloud_field_source.as_deref() != Some(source) {
            log::warn!(
                "ingest_field: {source:?} is not the scene's cloud field source; frame dropped"
            );
            return false;
        }
        self.ingest_radar_frame(slot, grid_w, grid_h, precip, coverage);
        true
    }

    /// Upload a radar frame into slot 0 (current timestep) or 1 (next),
    /// from two `grid_w * grid_h` byte planes: `precip` and `coverage`,
    /// each normalised to `0..=255`. This is TRANSPORT (data delivery,
    /// like tile ingest), not content — what renders is declared by the
    /// scene's `environment.clouds`. Prefer [`Self::ingest_field`], which
    /// checks the frame against the scene's declared field source.
    pub fn ingest_radar_frame(
        &mut self,
        slot: u32,
        grid_w: u32,
        grid_h: u32,
        precip: &[u8],
        coverage: &[u8],
    ) {
        let frame = RadarFrame::from_u8(grid_w, grid_h, precip, coverage);
        self.map.ingest_radar_frame(slot, &frame);
    }

    /// Set the cloud animation clock (`time`, seconds) and the slot-0→slot-1
    /// crossfade (`blend`, `0..=1`) — what a time slider scrubs.
    pub fn set_cloud_time(&mut self, time: f32, blend: f32) {
        self.map.set_cloud_time(time, blend);
    }

    /// The atmosphere subsystem's S7 DEBUG surface: replace the cloud
    /// overlay's renderer look-tuning parameters (feature scale, erosion,
    /// softness, intensity, extinction, debug-view selector, …) for the
    /// desktop debug panel. This is look-TUNING of the renderer, not scene
    /// content — WHAT renders (source, bounds, visibility, sim mode) stays
    /// declared by `environment.clouds`, and the camera/time/sun-driven
    /// fields of the struct are overwritten every frame from the
    /// Environment, so nothing authored here can contradict the Scene.
    /// Deliberately `debug_*`-named and hidden: not a host contract.
    #[doc(hidden)]
    pub fn debug_cloud_params(&mut self, params: turbomap_core::CloudParams) {
        self.map.set_cloud_params(params);
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
        self.map.tick(web_time::Instant::now())
    }

    /// Terrain-aware screen→ground hit: `(lat, lng, world_z, hit_terrain)`.
    /// Inherent (not part of the [`MapEngine`] contract) — only the wgpu engine
    /// has relief to raycast; the host uses this for exact marker placement/drag
    /// in 3D. See [`Map::screen_to_ground_lng_lat`].
    pub fn unproject_ground(&self, x: f64, y: f64) -> (f64, f64, f32, bool) {
        let hit = self.map.screen_to_ground_lng_lat((x, y));
        (
            hit.lng_lat.lat,
            hit.lng_lat.lng,
            hit.world_z,
            hit.hit_terrain,
        )
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
        // Road widths grow as you zoom past city-detail level — the Google
        // behaviour where streets swell into ribbons up close. Below the
        // reference the curve is flat (baked widths unchanged), so low-zoom
        // and pixel-constant-width behaviour is exactly as before.
        let scale = line_width_scale(zoom);
        for id in &self.line_layers {
            self.map.set_vector_layer_width_scale(id, scale);
        }
    }

    /// Record one frame into the host's encoder + target view.
    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        self.pump_decoded();
        self.update_dynamic_paint();
        self.map.render(encoder, target);
    }

    /// Finalize per-frame bookkeeping after the queue submit.
    pub fn after_submit(&mut self) {
        self.map.after_submit();
    }

    /// True while a camera animation, a tile fade-in, or a decode backlog
    /// is in progress — the host keeps rendering (render-on-demand) until
    /// this goes false. The backlog term is load-bearing: decoded tiles
    /// apply inside `render()`, so a sleeping host would strand them.
    pub fn is_animating(&self) -> bool {
        self.map.is_animating() || self.decode_queue.backlog() > 0
    }

    /// Metrics for the last rendered frame (cpu/gpu time, per-layer cache
    /// stats) — the inspection tooling reads these.
    pub fn last_frame_metrics(&self) -> &turbomap_core::map::FrameMetrics {
        self.map.last_frame_metrics()
    }

    /// The subsystem registry's combined live-state snapshot (slice D2):
    /// per-subsystem inspect JSON + budget reports, keyed by subsystem name.
    /// Hosts dump this verbatim into their debug surfaces.
    pub fn inspect_json(&self) -> String {
        self.map.inspect_json()
    }

    /// One streaming step for plan-driven hosts: fetches to start (each with
    /// a `RequestId`) and in-flight attempts to cancel. Deliveries complete
    /// through the existing `ingest_*`; failures/cancellations report back
    /// via [`Self::fetch_failed`] / [`Self::fetch_cancelled`]. (P5.1: the
    /// legacy pull-push shim is gone — every host consumes the plan.)
    pub fn streaming_plan(&mut self, max_start: usize) -> turbomap_core::map::StreamingPlan {
        self.map.streaming_plan(max_start)
    }

    /// Report a plan-issued fetch attempt as failed (re-pends if wanted).
    pub fn fetch_failed(&mut self, request: turbomap_world::RequestId) {
        self.map.fetch_failed(request);
    }

    /// Report a `cancel` entry as honoured.
    pub fn fetch_cancelled(&mut self, request: turbomap_world::RequestId) {
        self.map.fetch_cancelled(request);
    }

    /// [`Self::streaming_plan`] serialized for the bindings (wasm, uniffi):
    /// `{"start":[{"id",…,"kind","layer","z","x","y"}],"cancel":[ids]}`. One
    /// serializer so every host parses the same shape; see
    /// [`streaming_plan_to_json`].
    pub fn streaming_plan_json(&mut self, max_start: usize) -> String {
        streaming_plan_to_json(&self.map.streaming_plan(max_start))
    }
}

/// JSON for a [`StreamingPlan`](turbomap_core::map::StreamingPlan). `kind` is
/// `raster`/`hillshade`/`vector`/`terrain`; `layer` is `__terrain` for the
/// shared DEM.
/// `RequestId`s are session-scoped counters, far below 2^53, so they survive
/// a JS `number` exactly.
pub fn streaming_plan_to_json(plan: &turbomap_core::map::StreamingPlan) -> String {
    use turbomap_core::map::PendingTile;
    let start: Vec<String> = plan
        .start
        .iter()
        .map(|r| {
            let (kind, layer, t) = match &r.fetch {
                PendingTile::Raster { layer_id, tile } => ("raster", layer_id.as_str(), tile),
                PendingTile::Hillshade { layer_id, tile } => ("hillshade", layer_id.as_str(), tile),
                PendingTile::Vector { layer_id, tile } => ("vector", layer_id.as_str(), tile),
                PendingTile::Terrain { tile } => ("terrain", "__terrain", tile),
            };
            format!(
                "{{\"id\":{},\"kind\":\"{kind}\",\"layer\":\"{layer}\",\"z\":{},\"x\":{},\"y\":{}}}",
                r.id.0, t.z, t.x, t.y
            )
        })
        .collect();
    let cancel: Vec<String> = plan.cancel.iter().map(|id| id.0.to_string()).collect();
    format!(
        "{{\"start\":[{}],\"cancel\":[{}]}}",
        start.join(","),
        cancel.join(",")
    )
}

/// JSON for hit-test results (plan P6.4), top-most first:
/// `[{"layer":"...","feature_id":"..."|null,"properties":{"k":"v",...}}]`.
/// One wire shape for every string-typed binding (JNI, wasm); properties are
/// key-sorted so the output is deterministic.
pub fn hits_to_json(hits: &[turbomap_scene::Hit]) -> String {
    fn js(s: &str) -> String {
        serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
    }
    let items: Vec<String> = hits
        .iter()
        .map(|h| {
            let mut props: Vec<_> = h.properties.iter().collect();
            props.sort();
            let props: Vec<String> = props
                .into_iter()
                .map(|(k, v)| format!("{}:{}", js(k), js(v)))
                .collect();
            format!(
                "{{\"layer\":{},\"feature_id\":{},\"properties\":{{{}}}}}",
                js(&h.layer_id),
                h.feature_id
                    .as_deref()
                    .map(js)
                    .unwrap_or_else(|| "null".to_string()),
                props.join(",")
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

#[cfg(test)]
mod hit_json_tests {
    use super::hits_to_json;
    use turbomap_scene::Hit;

    #[test]
    fn hits_serialize_with_sorted_props_and_null_feature_ids() {
        let hits = vec![
            Hit {
                layer_id: "pins".into(),
                feature_id: Some("41".into()),
                properties: [("name", "Bergen"), ("id", "mk-1")]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            },
            Hit {
                layer_id: "water".into(),
                feature_id: None,
                properties: Default::default(),
            },
        ];
        assert_eq!(
            hits_to_json(&hits),
            r#"[{"layer":"pins","feature_id":"41","properties":{"id":"mk-1","name":"Bergen"}},{"layer":"water","feature_id":null,"properties":{}}]"#
        );
    }
}

#[cfg(test)]
mod plan_json_tests {
    use super::streaming_plan_to_json;
    use turbomap_core::map::{FetchRequest, PendingTile, StreamingPlan};
    use turbomap_core::TileId;
    use turbomap_world::RequestId;

    #[test]
    fn plan_json_carries_ids_kinds_and_cancels() {
        let plan = StreamingPlan {
            start: vec![
                FetchRequest {
                    id: RequestId(7),
                    fetch: PendingTile::Raster {
                        layer_id: "base".into(),
                        tile: TileId::new(11, 1058, 588),
                    },
                },
                FetchRequest {
                    id: RequestId(8),
                    fetch: PendingTile::Terrain {
                        tile: TileId::new(9, 264, 147),
                    },
                },
            ],
            cancel: vec![RequestId(3), RequestId(5)],
        };
        let json = streaming_plan_to_json(&plan);
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(v["start"][0]["id"], 7);
        assert_eq!(v["start"][0]["kind"], "raster");
        assert_eq!(v["start"][0]["layer"], "base");
        assert_eq!(v["start"][1]["kind"], "terrain");
        assert_eq!(v["start"][1]["layer"], "__terrain");
        assert_eq!(v["start"][1]["x"], 264);
        assert_eq!(v["cancel"], serde_json::json!([3, 5]));
        // Empty plan is valid JSON too.
        let empty = streaming_plan_to_json(&StreamingPlan::default());
        assert_eq!(empty, "{\"start\":[],\"cancel\":[]}");
    }
}

impl TurbomapEngine {
    /// Layer ids the backend skipped at the last apply.
    pub fn unsupported_layers(&self) -> &[String] {
        &self.unsupported
    }

    /// Delivered-but-not-yet-drawable tiles inside the decode queue — the
    /// streaming trace's backlog number now that hosts hand bytes straight
    /// through (plan B4.3). Non-zero also keeps `is_animating` true.
    pub fn decode_backlog(&self) -> usize {
        self.decode_queue.backlog()
    }

    /// Inspection escape hatch: the wrapped core map.
    /// Mutable access to the core map — debug/test hooks (pass isolation,
    /// clock pinning) that deliberately aren't part of the engine surface.
    /// Hidden since P6.2: no host reaches the map anymore (the invariants
    /// suite forbids `map_mut` outside this crate); only the engine's own
    /// GPU tests still use it.
    #[doc(hidden)]
    pub fn map_mut(&mut self) -> &mut Map {
        &mut self.map
    }

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
        // The scene-declared environment (plan C1): applied as one block —
        // the same core setters the imperative side-doors call, now driven
        // by the diff so environment state is declarative on every host.
        if let Some(env) = &delta.environment {
            use turbomap_scene::LightingDef;
            match env.lighting {
                LightingDef::Default => self.map.set_sun_time(None),
                LightingDef::TimeTracked { unix_seconds } => {
                    self.map.set_sun_time(Some(unix_seconds))
                }
                LightingDef::Fixed {
                    azimuth_deg,
                    altitude_deg,
                } => self.map.set_sun_position(Some(turbomap_core::SunPosition {
                    azimuth_deg,
                    altitude_deg,
                })),
            }
            self.map.set_terrain_shadows(env.terrain_shadows);
            self.map.set_terrain_lit(env.terrain_lit);
            self.map.set_aerial_haze(env.aerial_haze);
            self.map.set_sky_enabled(env.sky);
            self.map.set_basemap_gain(env.basemap_gain);
            // The weather-cloud overlay (plan C2): declared, not enabled by
            // side-door. The Field2D source anchors it geographically; frame
            // data arrives via `ingest_field` like tiles arrive via ingest.
            match &env.clouds {
                Some(clouds) => {
                    let bounds = match self.scene.sources.get(&clouds.source) {
                        Some(SourceDef::Field2D { bounds }) => Some(*bounds),
                        _ => {
                            log::warn!(
                                "environment.clouds source {:?} is not a field-2d source;                                  overlay disabled",
                                clouds.source
                            );
                            None
                        }
                    };
                    if let Some([west, south, east, north]) = bounds {
                        self.map.enable_clouds(clouds.grid[0], clouds.grid[1]);
                        self.map.set_cloud_geo_bounds(west, south, east, north);
                        self.map.set_clouds_visible(clouds.visible);
                        self.map.set_cloud_sim(clouds.animate);
                        self.cloud_field_source = Some(clouds.source.clone());
                    } else {
                        self.map.disable_clouds();
                        self.cloud_field_source = None;
                    }
                }
                None => {
                    self.map.disable_clouds();
                    self.cloud_field_source = None;
                }
            }
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
        // `None` for behind-camera / non-finite points so overlays hide them
        // instead of snapping to the centre fallback (pinned dot / radiating
        // route lines). `try_lng_lat_to_screen` does the cull.
        self.map
            .try_lng_lat_to_screen(CoreLatLng {
                lng: geo.lng,
                lat: geo.lat,
            })
            .map(|(x, y)| ScreenPoint { x, y })
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
            // Real since plan D4: `Layer::Custom` binds to a registered
            // factory and draws as a phase-bound node in the frame's MSAA
            // pass (`custom:<id>`). Unknown kinds still degrade to the
            // unsupported report rather than lying.
            custom_layers: true,
            terrain: true,
            // TRUE (plan C3): `Match` paints compile to per-feature style
            // rules and zoom curves evaluate on the GPU — this backend does
            // render data-driven paint, and has since the expression work
            // landed. The flag simply lied.
            data_driven_paint: true,
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
                    | Layer::FillExtrusion { .. }
                    | Layer::Symbol { .. }
                    | Layer::Hillshade { .. }
                    | Layer::Custom { .. }
            )
        })
        .cloned()
        .collect()
}

/// The scene's route-tube declarations. Tubes are scene content like any
/// layer, but each installs an overlay MESH keyed by id — not a core stack
/// layer — so they must not enter the positional prefix (whose removal
/// walks the core's `layer_ids`, which never contain a tube). They diff
/// as their own set in `reconcile`.
fn tube_layers(scene: &Scene) -> Vec<&Layer> {
    scene
        .layers
        .iter()
        .filter(|l| matches!(l, Layer::Tube { .. }))
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
/// Drives the inspect tool's `unsupported` report. A provider chain counts
/// as its providers' (validated-uniform) kind — probe the first.
fn is_supportable(
    layer: &Layer,
    scene: &Scene,
    custom_kinds: &HashMap<String, CustomLayerFactory>,
) -> bool {
    let source_is = |want: fn(&SourceDef) -> bool| {
        layer
            .source()
            .and_then(|s| scene.sources.get(s))
            .map(|d| match d {
                SourceDef::Chain { providers } => providers.first().map(want).unwrap_or(false),
                other => want(other),
            })
            .unwrap_or(false)
    };
    match layer {
        Layer::Raster { .. } => source_is(|d| {
            matches!(
                d,
                SourceDef::RasterXyz { .. } | SourceDef::PmtilesRaster { .. }
            )
        }),
        Layer::Hillshade { .. } => {
            source_is(|d| matches!(d, SourceDef::DemXyz { .. } | SourceDef::PmtilesDem { .. }))
        }
        Layer::Line { .. }
        | Layer::Fill { .. }
        | Layer::FillExtrusion { .. }
        | Layer::Symbol { .. } => source_is(|d| {
            matches!(
                d,
                SourceDef::GeoJson { .. }
                    | SourceDef::VectorXyz { .. }
                    | SourceDef::PmtilesVector { .. }
            )
        }),
        Layer::Circle { .. } => source_is(|d| matches!(d, SourceDef::GeoJson { .. })),
        Layer::Tube { .. } => source_is(|d| matches!(d, SourceDef::GeoJson { .. })),
        // Real since plan D4: supported iff a factory is registered for the
        // declared kind.
        Layer::Custom { kind, .. } => custom_kinds.contains_key(kind.as_str()),
    }
}

/// Compile one IR vector layer (`Fill`/`FillExtrusion`/`Line`/`Symbol`)
/// into the core `VectorStyle` the renderer tessellates against — exactly
/// the lowering `reconcile` performs when installing the layer.
/// `layer_name` is the resolved MVT source-layer name (GeoJSON sources use
/// the fixed synthetic layer name). Returns `None` for non-vector layers.
///
/// Hidden: this is the P6.2 fidelity-gate seam — style-translation tests
/// assert IR-compiled rules against hand-built / MapLibre-parsed
/// `VectorStyle`s without booting a GPU. It is not a host contract.
#[doc(hidden)]
pub fn compile_vector_layer_style(
    layer: &Layer,
    layer_name: String,
    zoom: f64,
    pixel_ratio: f32,
) -> Option<VectorStyle> {
    match layer {
        Layer::Fill {
            filter,
            color,
            opacity,
            ..
        } => Some(fill_style(layer_name, filter, color, opacity, zoom)),
        Layer::FillExtrusion {
            filter,
            color,
            height_m,
            height_property,
            min_height_property,
            ..
        } => Some(fill_extrusion_style(
            layer_name,
            filter,
            color,
            height_m,
            height_property,
            min_height_property,
            zoom,
        )),
        Layer::Line {
            filter,
            color,
            width,
            ..
        } => Some(line_style(
            layer_name,
            filter,
            color,
            width,
            zoom,
            pixel_ratio,
        )),
        Layer::Symbol {
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
            letter_spacing,
            font_weight,
            ..
        } => Some(symbol_style(
            layer_name,
            filter,
            text_field,
            text_size,
            color,
            halo_color,
            halo_width,
            sort_key,
            *placement,
            icon_image,
            icon_size,
            icon_color,
            zoom,
            pixel_ratio,
            *text_anchor,
            *letter_spacing,
            *font_weight,
        )),
        _ => None,
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
fn color_rules(
    layer_filter: &Filter,
    color: &Paint<Color>,
    zoom: f64,
) -> Vec<(CoreFilter, CoreColor)> {
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

/// Split a layer filter into its feature predicate and its tile-zoom
/// window. [`Filter::ZoomRange`] at the top level (or anywhere inside a
/// top-level [`Filter::All`], intersecting when repeated) lowers onto the
/// compiled rules' `min_zoom..=max_zoom` band; any other placement leaves
/// the range where it is, and [`map_filter`] rejects it loudly.
fn split_zoom_window(filter: &Filter) -> (Filter, u8, u8) {
    match filter {
        Filter::ZoomRange { min, max } => (Filter::Always, *min, *max),
        Filter::All(fs) => {
            let (mut min, mut max) = (0u8, 22u8);
            let mut rest: Vec<Filter> = Vec::new();
            for f in fs {
                if let Filter::ZoomRange { min: lo, max: hi } = f {
                    min = min.max(*lo);
                    max = max.min(*hi);
                } else {
                    rest.push(f.clone());
                }
            }
            let feature = match rest.len() {
                0 => Filter::Always,
                1 => rest.remove(0),
                _ => Filter::All(rest),
            };
            (feature, min, max)
        }
        other => (other.clone(), 0, 22),
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
    let (feature_filter, min_zoom, max_zoom) = split_zoom_window(filter);
    let op = opacity.at(zoom);
    let rules = color_rules(&feature_filter, color, zoom)
        .into_iter()
        .map(|(f, c)| {
            let a = (c.a as f32 * op).round().clamp(0.0, 255.0) as u8;
            CoreRule {
                source_layer: layer_name.clone(),
                filter: f,
                paint: CorePaint::Fill {
                    color: CoreColor::rgba(c.r, c.g, c.b, a),
                },
                min_zoom,
                max_zoom,
                interactive: false,
            }
        })
        .collect();
    VectorStyle {
        background: CoreColor::rgba(0, 0, 0, 0),
        rules,
    }
}

/// Build a core `VectorStyle` for a Scene `FillExtrusion` layer — extrude
/// matching polygons to 3D prisms. Colour can be data-driven (per class);
/// `height_m` is evaluated at the current zoom (height curves are rare, so
/// a single value per build is fine).
#[allow(clippy::too_many_arguments)]
fn fill_extrusion_style(
    layer_name: String,
    filter: &Filter,
    color: &Paint<Color>,
    height_m: &Paint<f32>,
    height_property: &Option<String>,
    min_height_property: &Option<String>,
    zoom: f64,
) -> VectorStyle {
    let (feature_filter, min_zoom, max_zoom) = split_zoom_window(filter);
    let h = height_m.at(zoom);
    let rules = color_rules(&feature_filter, color, zoom)
        .into_iter()
        .map(|(f, c)| CoreRule {
            source_layer: layer_name.clone(),
            filter: f,
            paint: CorePaint::FillExtrusion {
                color: c,
                height_m: h,
                height_property: height_property.clone(),
                min_height_property: min_height_property.clone(),
            },
            min_zoom,
            max_zoom,
            interactive: false,
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
    letter_spacing: f32,
    font_weight: f32,
) -> VectorStyle {
    let (feature_filter, min_zoom, max_zoom) = split_zoom_window(filter);
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
    let rules = color_rules(&feature_filter, color, zoom)
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
                letter_spacing,
                weight: font_weight,
            },
            min_zoom,
            max_zoom,
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
    rules.push((
        map_filter(layer_filter),
        resolve_color(None),
        resolve_width(None),
    ));
    rules
}

/// Per-frame multiplier on baked line widths as a function of camera zoom.
/// Flat at 1.0 up to [`LINE_WIDTH_REF_ZOOM`] — so low-zoom maps and the
/// pixel-constant-width contract are untouched — then grows ~12% per zoom
/// level as you push in, capped, giving roads the Google-style swell up
/// close without re-tessellating.
fn line_width_scale(zoom: f64) -> f32 {
    const LINE_WIDTH_REF_ZOOM: f64 = 15.0;
    const PER_ZOOM_GROWTH: f64 = 1.12;
    const MAX_SCALE: f64 = 2.0;
    if zoom <= LINE_WIDTH_REF_ZOOM {
        return 1.0;
    }
    PER_ZOOM_GROWTH
        .powf(zoom - LINE_WIDTH_REF_ZOOM)
        .min(MAX_SCALE) as f32
}

fn line_style(
    layer_name: String,
    filter: &Filter,
    color: &Paint<Color>,
    width: &Paint<f32>,
    zoom: f64,
    pixel_ratio: f32,
) -> VectorStyle {
    let (feature_filter, min_zoom, max_zoom) = split_zoom_window(filter);
    let rules = line_rules(&feature_filter, color, width, zoom, pixel_ratio)
        .into_iter()
        .map(|(f, c, w)| CoreRule {
            source_layer: layer_name.clone(),
            filter: f,
            paint: CorePaint::Line { color: c, width: w },
            min_zoom,
            max_zoom,
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
        Filter::In(key, values) => CoreFilter::In(
            key.clone(),
            values.iter().map(filter_value_to_string).collect(),
        ),
        Filter::Not(inner) => CoreFilter::Not(Box::new(map_filter(inner))),
        Filter::All(fs) => CoreFilter::All(fs.iter().map(map_filter).collect()),
        Filter::Any(fs) => CoreFilter::Any(fs.iter().map(map_filter).collect()),
        Filter::ZoomRange { min, max } => {
            // Only meaningful at the top of a layer filter (or inside a
            // top-level `All`), where `split_zoom_window` lowers it onto the
            // rule's zoom band before this mapping runs. Nested under
            // `Not`/`Any` it has no per-feature meaning — warn, match all.
            log::warn!(
                "Filter::ZoomRange({min}..={max}) nested under Not/Any cannot compile to a \
                 zoom band; treating it as match-all"
            );
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

/// As [`fetch_decode`], but runs the DEM codec too: image bytes → real
/// heights + coverage, per the source's declared encoding (plan D3).
fn fetch_decode_dem(src: &dyn TileSource, tile: TileId) -> Option<turbomap_core::dem::DecodedDem> {
    let (rgba, w, h) = fetch_decode(src, tile)?;
    turbomap_core::decode_dem_rgba(&rgba, w, h, src.dem_encoding())
}

#[cfg(test)]
mod tests {
    use super::{line_width_scale, split_zoom_window};
    use turbomap_scene::{Filter, FilterValue};

    #[test]
    fn zoom_windows_split_off_the_layer_filter() {
        let eq = || Filter::Eq("kind".into(), FilterValue::String("city".into()));

        // No window anywhere: full band, filter untouched.
        assert_eq!(split_zoom_window(&eq()), (eq(), 0, 22));
        assert_eq!(split_zoom_window(&Filter::Always), (Filter::Always, 0, 22));

        // Bare window: the whole filter is the band.
        assert_eq!(
            split_zoom_window(&Filter::ZoomRange { min: 6, max: 14 }),
            (Filter::Always, 6, 14)
        );

        // Window + predicate inside All: band extracted, single remaining
        // predicate unwrapped (compiled rules must equal a hand-built
        // `Filter::Eq`, not `All([Eq])` — the fidelity gate compares them).
        assert_eq!(split_zoom_window(&eq().within_zoom(9, 22)), (eq(), 9, 22));

        // Repeated windows intersect.
        assert_eq!(
            split_zoom_window(&Filter::All(vec![
                Filter::ZoomRange { min: 4, max: 14 },
                Filter::ZoomRange { min: 8, max: 22 },
                eq(),
            ])),
            (eq(), 8, 14)
        );

        // Nested under Not/Any the window is NOT extracted (map_filter
        // rejects it loudly instead).
        let nested = Filter::Not(Box::new(Filter::ZoomRange { min: 0, max: 5 }));
        assert_eq!(split_zoom_window(&nested), (nested.clone(), 0, 22));
    }

    #[test]
    fn line_width_scale_is_flat_below_reference_and_grows_above() {
        // At and below the reference zoom the curve is exactly 1.0, so the
        // baked widths — and the pixel-constant-width contract at low zoom —
        // are untouched.
        assert_eq!(line_width_scale(9.0), 1.0);
        assert_eq!(line_width_scale(9.6), 1.0, "flat region keeps z9 constant");
        assert_eq!(
            line_width_scale(15.0),
            1.0,
            "reference zoom is the baked width"
        );

        // Past the reference roads swell, monotonically, and the growth is
        // capped so it can never run away.
        assert!(line_width_scale(16.0) > 1.0);
        assert!(
            line_width_scale(18.0) > line_width_scale(16.0),
            "monotonic growth"
        );
        assert!(line_width_scale(30.0) <= 2.0, "capped");
    }
}
