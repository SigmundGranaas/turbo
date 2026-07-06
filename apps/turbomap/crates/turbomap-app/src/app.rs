//! winit `ApplicationHandler` driving the map as an ordinary **Scene host**
//! on `TurbomapEngine` (plan P6.2 — the last imperative host is gone). The
//! app owns a [`SceneState`] document and a fetch pipeline; every content
//! mutation edits the document, rebuilds the [`Scene`], and re-applies —
//! the engine diffs. Camera moves go through the engine's control-plane
//! verbs. The body here is intentionally thin: anything substantive lives
//! in one of `gpu`, `surface`, `schedule`, `map_host`, `styles`, or `ui`.

use std::sync::Arc;
use std::time::Instant;

use turbomap_clouds::{DebugView, RadarFrame, SyntheticStorm};
use turbomap_core::{CloudParams, MapOptions, TileSource, VectorTileSource};
use turbomap_engine::{CameraState, HostDrivenResolver, MapEngine, ScreenPoint, TurbomapEngine};
use turbomap_scene::{
    CloudsDef, Color, Filter, LatLng, Layer, LightingDef, Paint, Scene, SourceDef,
};
use turbomap_tiles_http::{HttpRasterSource, HttpVectorTileSource};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

/// Vector-tile worker thread count (raster: 4, DEM: 3 — see `resumed`).
const FETCH_WORKERS: usize = 6;

// Scene ids — the stable contract between the scene builder, the fetch
// pipeline's plan handling, and the panel's checkbox state.
const RASTER_LAYER_ID: &str = "basemap";
const HILLSHADE_LAYER_ID: &str = "hillshade";
const RASTER_SOURCE: &str = "basemap";
const DEM_SOURCE: &str = "dem";
const VECTOR_SOURCE: &str = "vector";
const RADAR_SOURCE: &str = "radar";
/// Prefix shared by every marker circle layer (`marker-rings-N` /
/// `marker-dots-N`) — `dispatch_click` recognises marker hits by it.
const MARKER_LAYER_PREFIX: &str = "marker-";

/// Cyan — visually distinct from the demo Norwegian-city markers so
/// user-added points are easy to spot.
const USER_MARKER_COLOR: Color = Color::rgba(0x00, 0xB8, 0xD4, 0xFF);

/// The app's tile endpoints: the concrete HTTP fetchers the pumps run
/// (with their disk caches), paired with the [`SourceDef`]s that declare
/// the same endpoints to the Scene. `run()` builds both from one URL
/// template so they can't drift.
pub struct SourceSet {
    pub raster: Arc<HttpRasterSource>,
    pub raster_def: SourceDef,
    /// Optional DEM endpoint for terrain + the hillshade layer. `None`
    /// (no `TURBO_API_URL`) means the scene declares no hillshade at all.
    pub dem: Option<Arc<HttpRasterSource>>,
    pub dem_def: Option<SourceDef>,
    pub vector: Arc<HttpVectorTileSource>,
    pub vector_def: SourceDef,
}

pub struct TurbomapApp {
    sources: SourceSet,
    /// The vector overlay as Scene IR layers (styles.rs lists, or the
    /// served MapLibre style lowered via `parse_style_layers`).
    vector_layers: Vec<Layer>,
    initial_camera: CameraState,
    state: Option<RunningState>,
}

struct RunningState {
    window: Arc<Window>,
    /// Owns the wgpu surface + its configuration. See `surface.rs`.
    surface: crate::surface::RenderSurface,
    /// Decides *when* to render. Pure state machine. See `schedule.rs`.
    scheduler: crate::schedule::RenderScheduler,
    /// All GPU-side handles in one place. See `gpu.rs`.
    gpu: crate::gpu::GpuContext,
    /// The renderer behind the `MapEngine` contract — owns the core map,
    /// the codec (decode off the render thread), and the streaming plan.
    engine: TurbomapEngine,
    /// The plan-driven raw-bytes fetch loop. See `map_host.rs`.
    fetch: crate::map_host::FetchPipeline,
    /// The scene DOCUMENT — the app's single content-authoring surface.
    /// Every mutation rebuilds `scene_state.scene()` and re-applies.
    scene_state: SceneState,
    /// Pointer position, drag anchor, click-vs-pan disambiguation. See
    /// `input.rs`.
    pointer: crate::input::PointerState,
    /// Egui panel + renderer wrapped into one object. See `ui.rs`.
    ui: crate::ui::UiOverlay,
    /// Mirror of the UI panel's widget values. `build_ui` mutates ONLY
    /// this (plus camera via engine verbs); `sync_scene_from_ui` folds the
    /// content-shaped bits into `scene_state` each frame.
    ui_state: UiState,
    /// True while the pointer is over an egui widget. Suppresses pan/click
    /// handling for that frame so dragging a slider doesn't also pan the
    /// map underneath.
    egui_wants_pointer: bool,
    /// Monotonic frame counter, used as the diagnostic framebuffer-dump
    /// filename when `TURBOMAP_DUMP_DIR` is set in the environment.
    frame_counter: u32,
    /// `TURBOMAP_DUMP_DIR`, read once at startup. `Some` = dump every
    /// rendered frame to `<dir>/frame_<n>.png` (slow; diagnostics only).
    dump_dir: Option<String>,
    /// Synthetic radar sequence backing the cloud debug scene. Generated
    /// once at startup; the panel's frame-A/B sliders pick pairs out of it.
    cloud_frames: Vec<RadarFrame>,
    /// Which `(frame_a, frame_b)` pair is currently uploaded to the GPU,
    /// so `sync_clouds` only re-uploads textures when the selection moves.
    cloud_uploaded: Option<(usize, usize)>,
}

/// One marker: a named point with a colour. Serialised into the scene's
/// GeoJSON marker source; the name rides into hit-test properties so a
/// click can identify (and remove) the marker it landed on.
#[derive(Debug, Clone)]
struct MarkerSpec {
    name: String,
    pos: LatLng,
    color: Color,
}

/// The scene-declared cloud overlay's document state (TURBO_CLOUDS debug
/// scene). Frame DATA is transport (`engine.ingest_field`), not here.
#[derive(Debug, Clone)]
struct CloudSceneState {
    /// `[west, south, east, north]` — anchors the radar field.
    bounds: [f64; 4],
    grid: [u32; 2],
    visible: bool,
    animate: bool,
}

/// The complete content description the app owns — `scene()` lowers it to
/// the IR document the engine consumes. Layer-visibility toggles are
/// expressed as layer inclusion (removal-and-refetch; the disk caches make
/// re-adding cheap).
struct SceneState {
    raster_def: SourceDef,
    dem_def: Option<SourceDef>,
    vector_def: SourceDef,
    vector_layers: Vec<Layer>,
    raster_visible: bool,
    hillshade_visible: bool,
    vector_visible: bool,
    /// Analytic sky pass — scene environment state (IR `environment.sky`).
    sky: bool,
    markers: Vec<MarkerSpec>,
    /// Monotonic counter naming click-added markers (`user-N`).
    next_user_marker: u32,
    clouds: Option<CloudSceneState>,
}

impl SceneState {
    /// Lower the document to the Scene IR. Pure — called on every content
    /// mutation; the engine's diff keeps the GPU work minimal.
    fn scene(&self) -> Scene {
        let mut scene = Scene::new();
        scene
            .sources
            .insert(RASTER_SOURCE.to_string(), self.raster_def.clone());
        if let Some(dem) = &self.dem_def {
            scene.sources.insert(DEM_SOURCE.to_string(), dem.clone());
        }
        scene
            .sources
            .insert(VECTOR_SOURCE.to_string(), self.vector_def.clone());

        // Layer stack (back → front): raster basemap, terrain/hillshade,
        // vector overlay, marker circles (overlay track).
        if self.raster_visible {
            scene.layers.push(Layer::Raster {
                id: RASTER_LAYER_ID.to_string(),
                source: RASTER_SOURCE.to_string(),
                opacity: Paint::Const(1.0),
            });
        }
        if self.dem_def.is_some() {
            // The DEM stays registered even with the relief overlay off
            // (it drives terrain displacement / water reflection):
            // `height_only` expresses the overlay toggle without dropping
            // the terrain source.
            scene.layers.push(Layer::Hillshade {
                id: HILLSHADE_LAYER_ID.to_string(),
                source: DEM_SOURCE.to_string(),
                exaggeration: 1.5,
                height_only: !self.hillshade_visible,
            });
        }
        if self.vector_visible {
            scene.layers.extend(self.vector_layers.iter().cloned());
        }
        self.push_marker_layers(&mut scene);

        if let Some(c) = &self.clouds {
            scene.sources.insert(
                RADAR_SOURCE.to_string(),
                SourceDef::Field2D { bounds: c.bounds },
            );
            scene.environment.clouds = Some(CloudsDef {
                source: RADAR_SOURCE.to_string(),
                grid: c.grid,
                visible: c.visible,
                animate: c.animate,
            });
        }
        // A fixed low sun so terrain lighting is in frame from the first
        // render (the same 145°/18° the imperative host pinned).
        scene.environment.lighting = LightingDef::Fixed {
            azimuth_deg: 145.0,
            altitude_deg: 18.0,
        };
        scene.environment.sky = self.sky;
        scene
    }

    /// Markers → GeoJSON circle layers: one source + white-ring/dot layer
    /// pair per marker colour (the engine's circle layers paint one colour
    /// each — `Paint::Match` has no per-feature path in the marker track).
    /// All rings go under all dots so overlapping markers stack cleanly.
    fn push_marker_layers(&self, scene: &mut Scene) {
        if self.markers.is_empty() {
            return;
        }
        let mut groups: Vec<(Color, Vec<&MarkerSpec>)> = Vec::new();
        for m in &self.markers {
            match groups.iter_mut().find(|(c, _)| *c == m.color) {
                Some((_, v)) => v.push(m),
                None => groups.push((m.color, vec![m])),
            }
        }
        for (i, (_, group)) in groups.iter().enumerate() {
            scene.sources.insert(
                format!("markers-{i}"),
                SourceDef::GeoJson {
                    data: markers_geojson(group),
                },
            );
        }
        for (i, _) in groups.iter().enumerate() {
            scene.layers.push(Layer::Circle {
                id: format!("marker-rings-{i}"),
                source: format!("markers-{i}"),
                source_layer: None,
                filter: Filter::Always,
                color: Paint::Const(Color::rgb(0xFF, 0xFF, 0xFF)),
                radius: Paint::Const(11.0),
            });
        }
        for (i, (color, _)) in groups.iter().enumerate() {
            scene.layers.push(Layer::Circle {
                id: format!("marker-dots-{i}"),
                source: format!("markers-{i}"),
                source_layer: None,
                filter: Filter::Always,
                color: Paint::Const(*color),
                radius: Paint::Const(8.0),
            });
        }
    }
}

/// A FeatureCollection of point features with a `name` property each —
/// what the engine's circle layers consume and what hit tests report back.
fn markers_geojson(markers: &[&MarkerSpec]) -> String {
    let features: Vec<serde_json::Value> = markers
        .iter()
        .map(|m| {
            serde_json::json!({
                "type": "Feature",
                "properties": { "name": m.name },
                "geometry": { "type": "Point", "coordinates": [m.pos.lng, m.pos.lat] }
            })
        })
        .collect();
    serde_json::json!({ "type": "FeatureCollection", "features": features }).to_string()
}

/// UI-bound state. `build_ui` edits this and nothing else (camera moves go
/// straight through engine verbs); `sync_scene_from_ui` diffs it against
/// the scene document once per frame.
#[derive(Debug, Clone)]
struct UiState {
    raster_visible: bool,
    hillshade_visible: bool,
    /// Analytic sky pass. Off by default for water debug.
    sky_visible: bool,
    vector_visible: bool,
    /// Procedural cloud-overlay debug controls. See [`CloudUiState`].
    clouds: CloudUiState,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            raster_visible: true,
            // Off by default — water debug isolates the surface; the DEM
            // source stays registered (height_only terrain) but the grey
            // relief overlay is hidden. Re-enable via the layers checkbox.
            hillshade_visible: false,
            sky_visible: false,
            vector_visible: true,
            clouds: CloudUiState::default(),
        }
    }
}

/// Mirror of the procedural cloud overlay's debug-scene controls. The
/// scene document owns visible/animate (rebuild + re-apply on change);
/// frame selection re-uploads via `ingest_field`; time/blend scrub through
/// `set_cloud_time`; the LOOK knobs ride the engine's S7 debug surface
/// (`debug_cloud_params`). The camera-driven fields (world-lock affine,
/// inv-view-proj, slab altitude) are recomputed inside the engine's render
/// from the live camera, so the look knobs we set here are never clobbered.
#[derive(Debug, Clone)]
struct CloudUiState {
    /// Master on/off (scene `CloudsDef::visible`).
    enabled: bool,
    /// Engine cloud sim drives the clock (scene `CloudsDef::animate`).
    animate: bool,
    /// Multiplier on the per-frame `time` advance when `animate`.
    speed: f32,
    /// Drift/boil clock in seconds (scrubbable when not animating).
    time: f32,
    /// Crossfade `0..1` between synthetic radar frame A (slot 0) and B (slot 1).
    blend: f32,
    /// Index into the synthetic storm sequence uploaded to slot 0.
    frame_a: usize,
    /// Index into the synthetic storm sequence uploaded to slot 1.
    frame_b: usize,
    /// Look knobs + debug-view selector. Camera/affine fields are ignored
    /// (overwritten per frame by the renderer).
    params: CloudParams,
}

impl Default for CloudUiState {
    fn default() -> Self {
        Self {
            // Off by default: the cloud overlay rendered as a grid of white
            // blobs over open water and confounded water debugging.
            // Re-enable in the "weather clouds" panel section.
            enabled: false,
            animate: true,
            speed: 1.0,
            time: 0.0,
            blend: 0.0,
            frame_a: 0,
            frame_b: 1,
            params: CloudParams::default(),
        }
    }
}

impl TurbomapApp {
    pub fn new(sources: SourceSet, vector_layers: Vec<Layer>, initial_camera: CameraState) -> Self {
        Self {
            sources,
            vector_layers,
            initial_camera,
            state: None,
        }
    }
}

impl ApplicationHandler for TurbomapApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let window_attrs = Window::default_attributes()
            .with_title("turbomap (vector)")
            .with_inner_size(PhysicalSize::new(1024, 768));
        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("create window"),
        );

        let size = window.inner_size();
        let mut gpu = crate::gpu::GpuContext::new(window.clone());
        let render_surface = crate::surface::RenderSurface::new(
            gpu.device.clone(),
            gpu.take_surface(),
            gpu.surface_format,
            gpu.alpha_mode,
            (size.width.max(1), size.height.max(1)),
            gpu.max_texture_dimension_2d(),
        );

        // The engine host boundary: URL sources resolve to stubs
        // (`HostDrivenResolver`); THIS app fetches the bytes (see
        // `map_host.rs`) and pushes them back via `ingest_*`.
        let mut engine = TurbomapEngine::new(
            gpu.device.clone(),
            gpu.queue.clone(),
            gpu.surface_format,
            render_surface.size(),
            self.initial_camera,
            MapOptions::default(),
            Box::new(HostDrivenResolver),
        )
        .expect("create engine");

        // Demo city markers — off by default; set TURBO_MARKERS=1 to drop
        // them for hit-test testing.
        let markers: Vec<MarkerSpec> = if std::env::var("TURBO_MARKERS").is_ok() {
            [
                ("Bergen", 60.39, 5.32, Color::rgb(0xE5, 0x39, 0x35)),
                ("Oslo", 59.91, 10.75, Color::rgb(0x1E, 0x88, 0xE5)),
                ("Trondheim", 63.43, 10.39, Color::rgb(0x43, 0xA0, 0x47)),
                ("Tromsø", 69.65, 18.96, Color::rgb(0xFD, 0xD8, 0x35)),
            ]
            .into_iter()
            .map(|(name, lat, lng, color)| MarkerSpec {
                name: name.to_string(),
                pos: LatLng::new(lat, lng),
                color,
            })
            .collect()
        } else {
            Vec::new()
        };

        // Procedural cloud overlay — OFF by default for the clean
        // water-debug base. TURBO_CLOUDS=1 declares the synthetic-storm
        // debug scene (Field2D source + CloudsDef; frames upload below).
        let (cloud_frames, clouds): (Vec<RadarFrame>, Option<CloudSceneState>) =
            if std::env::var("TURBO_CLOUDS").is_ok() {
                let storm = SyntheticStorm::default();
                let frames = storm.generate();
                let c = self.initial_camera.center;
                let clouds = CloudSceneState {
                    bounds: [c.lng - 4.0, c.lat - 2.0, c.lng + 4.0, c.lat + 2.0],
                    grid: [storm.width, storm.height],
                    visible: false,
                    animate: true,
                };
                (frames, Some(clouds))
            } else {
                (Vec::new(), None)
            };

        let ui_state = UiState::default();
        let scene_state = SceneState {
            raster_def: self.sources.raster_def.clone(),
            dem_def: self.sources.dem_def.clone(),
            vector_def: self.sources.vector_def.clone(),
            vector_layers: self.vector_layers.clone(),
            raster_visible: ui_state.raster_visible,
            hillshade_visible: ui_state.hillshade_visible,
            vector_visible: ui_state.vector_visible,
            sky: ui_state.sky_visible,
            markers,
            next_user_marker: 1,
            clouds,
        };
        let scene = scene_state.scene();
        if let Err(e) = scene.validate() {
            log::warn!("scene failed validation: {e}");
        }
        engine.apply(scene);
        let unsupported = engine.unsupported_layers();
        if !unsupported.is_empty() {
            log::warn!("engine skipped unsupported layers: {unsupported:?}");
        }

        // Initial radar frame pair for the declared cloud field (data is
        // transport — pushed like tiles, not part of the document).
        let cloud_uploaded = if cloud_frames.is_empty() {
            None
        } else {
            let b = 1.min(cloud_frames.len() - 1);
            upload_radar_frame(&mut engine, 0, &cloud_frames[0]);
            upload_radar_frame(&mut engine, 1, &cloud_frames[b]);
            Some((0, b))
        };

        // Raw-bytes fetch pumps over the app's HTTP sources (disk caches
        // included). The engine's codec owns every decode.
        let vector = self.sources.vector.clone();
        let raster = self.sources.raster.clone();
        let fetch = crate::map_host::FetchPipeline::new(
            crate::runtime::BytesPump::new("vector", FETCH_WORKERS, move |tile| {
                vector.request_bytes(tile).map_err(|e| e.to_string())
            }),
            crate::runtime::BytesPump::new("raster", 4, move |tile| {
                raster
                    .request(tile)
                    .map(|t| t.bytes)
                    .map_err(|e| e.to_string())
            }),
            self.sources.dem.clone().map(|dem| {
                crate::runtime::BytesPump::new("dem", 3, move |tile| {
                    dem.request(tile)
                        .map(|t| t.bytes)
                        .map_err(|e| e.to_string())
                })
            }),
        );

        let ui = crate::ui::UiOverlay::new(&gpu.device, gpu.surface_format, &window);

        self.state = Some(RunningState {
            window: window.clone(),
            surface: render_surface,
            scheduler: crate::schedule::RenderScheduler::new(),
            gpu,
            engine,
            fetch,
            scene_state,
            pointer: crate::input::PointerState::default(),
            ui,
            ui_state,
            egui_wants_pointer: false,
            frame_counter: 0,
            dump_dir: std::env::var("TURBOMAP_DUMP_DIR").ok(),
            cloud_frames,
            cloud_uploaded,
        });
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        // egui sees every event first so it can update its own state
        // (cursor position, key state, viewport size). We never
        // short-circuit on `consumed` — window-state events like Resized or
        // CloseRequested MUST still reach our match below. Instead we let
        // the match run and gate the input-specific branches on
        // `egui_wants_pointer` / `egui_wants_keyboard` so dragging a slider
        // doesn't also pan the map underneath.
        let overlay_resp = state.ui.on_window_event(&state.window, &event);
        state.egui_wants_pointer = overlay_resp.egui_used_pointer;
        let egui_wants_keyboard = state.ui.wants_keyboard();
        if overlay_resp.repaint_requested {
            state.scheduler.notice_egui_repaint();
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }
                // No configure here. The scheduler will hold the size for
                // ~30 ms; on_redraw applies it when the burst settles.
                state.scheduler.notice_resize(size.width, size.height);
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let gesture = state.pointer.on_cursor_moved((position.x, position.y));
                if state.egui_wants_pointer {
                    return;
                }
                if let Some(crate::input::Gesture::Pan { dx, dy }) = gesture {
                    state.engine.pan_by_pixels(dx, dy);
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: bstate,
                button,
                ..
            } => {
                if state.egui_wants_pointer && bstate == ElementState::Pressed {
                    return;
                }
                if button == MouseButton::Left {
                    match bstate {
                        ElementState::Pressed => state.pointer.on_left_press(),
                        ElementState::Released => {
                            if let Some(crate::input::Gesture::Click { pos }) =
                                state.pointer.on_left_release()
                            {
                                state.dispatch_click(pos);
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if state.egui_wants_pointer {
                    return;
                }
                match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        let factor = 2.0_f64.powf(y as f64 * 0.25);
                        let focus = state.focus_or_centre();
                        state.engine.zoom_around(factor, focus);
                        state.window.request_redraw();
                    }
                    MouseScrollDelta::PixelDelta(p) => {
                        state.engine.pan_by_pixels(p.x, p.y);
                        state.window.request_redraw();
                    }
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                if state.egui_wants_pointer {
                    return;
                }
                let factor = 1.0 + delta;
                if factor > 0.0 {
                    let focus = state.focus_or_centre();
                    state.engine.zoom_around(factor, focus);
                    state.window.request_redraw();
                }
            }
            WindowEvent::PanGesture { delta, .. } => {
                if state.egui_wants_pointer {
                    return;
                }
                state.engine.pan_by_pixels(delta.x as f64, delta.y as f64);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        repeat: _,
                        ..
                    },
                ..
            } if !egui_wants_keyboard => {
                // Camera tilt + bearing controls — W/S nudge pitch ±5°, A/D
                // rotate bearing ±15°, R resets to top-down north-up.
                let mut camera = state.engine.camera();
                let mut changed = false;
                match code {
                    KeyCode::KeyW => {
                        camera.pitch_deg = (camera.pitch_deg + 5.0).min(60.0);
                        changed = true;
                    }
                    KeyCode::KeyS => {
                        camera.pitch_deg = (camera.pitch_deg - 5.0).max(0.0);
                        changed = true;
                    }
                    KeyCode::KeyA => {
                        camera.bearing_deg = (camera.bearing_deg - 15.0).rem_euclid(360.0);
                        changed = true;
                    }
                    KeyCode::KeyD => {
                        camera.bearing_deg = (camera.bearing_deg + 15.0).rem_euclid(360.0);
                        changed = true;
                    }
                    KeyCode::KeyR => {
                        camera.pitch_deg = 0.0;
                        camera.bearing_deg = 0.0;
                        changed = true;
                    }
                    _ => {}
                }
                if changed {
                    state.engine.set_camera(camera);
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                state.on_redraw();
            }
            _ => {}
        }
    }

    /// winit calls this once per loop tick. Each tick we hand the scheduler
    /// the current workload snapshot and act on its decision; see the
    /// module-level docs of `schedule` for the rules.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        use winit::event_loop::ControlFlow;
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let workload = state.fetch.workload(&state.engine);
        match state.scheduler.schedule(Instant::now(), workload) {
            crate::schedule::Schedule::Render => {
                state.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            crate::schedule::Schedule::WakeAt(t) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(t));
            }
            crate::schedule::Schedule::Idle => {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}

impl RunningState {
    fn focus_or_centre(&self) -> (f64, f64) {
        let (w, h) = self.surface.size();
        self.pointer
            .last_pos()
            .unwrap_or((w as f64 * 0.5, h as f64 * 0.5))
    }

    /// A click was detected at `pos`. If it lands on a marker circle,
    /// delete that marker from the scene document; otherwise unproject and
    /// add a new user marker there. Everything under the click is logged
    /// for visibility.
    fn dispatch_click(&mut self, pos: (f64, f64)) {
        let point = ScreenPoint { x: pos.0, y: pos.1 };
        let hits = self.engine.hit_test(point, 6.0);
        for h in &hits {
            log::info!(
                "hit: layer={} feature={:?} props={:?}",
                h.layer_id,
                h.feature_id,
                h.properties
            );
        }

        // hit_test returns markers first, top-down. The topmost one wins on
        // a click — match how the user sees the stack. Markers identify by
        // their `name` property (unique: cities + `user-N`).
        let marker_name = hits
            .iter()
            .find(|h| h.layer_id.starts_with(MARKER_LAYER_PREFIX))
            .and_then(|h| h.properties.get("name").cloned());

        if let Some(name) = marker_name {
            self.scene_state.markers.retain(|m| m.name != name);
            log::info!("removed marker {name:?}");
        } else {
            let Some(pos) = self.engine.unproject(point) else {
                return;
            };
            let name = format!("user-{}", self.scene_state.next_user_marker);
            self.scene_state.next_user_marker += 1;
            log::info!(
                "added marker {name:?} at lat={:.4}, lng={:.4}",
                pos.lat,
                pos.lng
            );
            self.scene_state.markers.push(MarkerSpec {
                name,
                pos,
                color: USER_MARKER_COLOR,
            });
        }
        // Content changed → rebuild + re-apply (the engine diffs; only the
        // marker circles rebuild).
        self.engine.apply(self.scene_state.scene());
        // Marker set changed → redraw needed (we won't get a redraw request
        // otherwise since the camera didn't move).
        self.window.request_redraw();
    }

    /// Fold the panel's content-shaped edits into the scene document and
    /// re-apply if anything moved. Camera edits already went through engine
    /// verbs inside `build_ui`; this handles everything the SCENE owns.
    fn sync_scene_from_ui(&mut self) {
        let ui = &self.ui_state;
        let s = &mut self.scene_state;
        let mut changed = false;
        if s.raster_visible != ui.raster_visible {
            s.raster_visible = ui.raster_visible;
            changed = true;
        }
        if s.hillshade_visible != ui.hillshade_visible {
            s.hillshade_visible = ui.hillshade_visible;
            changed = true;
        }
        if s.vector_visible != ui.vector_visible {
            s.vector_visible = ui.vector_visible;
            changed = true;
        }
        if s.sky != ui.sky_visible {
            s.sky = ui.sky_visible;
            changed = true;
        }
        if let Some(c) = s.clouds.as_mut() {
            if c.visible != ui.clouds.enabled {
                c.visible = ui.clouds.enabled;
                changed = true;
            }
            if c.animate != ui.clouds.animate {
                c.animate = ui.clouds.animate;
                changed = true;
            }
        }
        if changed {
            self.engine.apply(self.scene_state.scene());
        }
    }

    /// Push the cloud debug-panel's DATA and DEBUG state into the engine:
    /// re-upload the radar texture pair only when the frame-A/B selection
    /// changes, scrub the clock when not animating (the engine's sim owns
    /// it otherwise, plan E2), and forward the look knobs through the S7
    /// debug surface. Visible/animate are scene state — `sync_scene_from_ui`
    /// already folded them into the document.
    fn sync_clouds(&mut self) {
        let ui = &self.ui_state.clouds;
        if self.scene_state.clouds.is_none() || self.cloud_frames.is_empty() || !ui.enabled {
            return;
        }
        let last = self.cloud_frames.len() - 1;
        let a = ui.frame_a.min(last);
        let b = ui.frame_b.min(last);
        if self.cloud_uploaded != Some((a, b)) {
            upload_radar_frame(&mut self.engine, 0, &self.cloud_frames[a]);
            upload_radar_frame(&mut self.engine, 1, &self.cloud_frames[b]);
            self.cloud_uploaded = Some((a, b));
        }
        self.engine.debug_cloud_params(ui.params);
        if !ui.animate {
            // Host-scrubbed: the panel's sliders own time + crossfade. (In
            // animate mode the engine's cloud sim owns the clock and these
            // sliders are inert — the Environment is authoritative.)
            self.engine.set_cloud_time(ui.time, ui.blend);
        }
    }

    fn on_redraw(&mut self) {
        let now = Instant::now();
        // Bail out if we're still in a resize-event burst — the
        // foundational rule that prevents drawable-pool exhaustion (see
        // `surface.rs` and `schedule.rs` headers).
        if self.scheduler.in_resize_burst(now) {
            return;
        }
        // If a resize just settled, apply it BEFORE acquiring a drawable.
        if let Some((width, height)) = self.scheduler.take_settled_resize(now) {
            self.surface.resize_to(width, height);
            let (w, h) = self.surface.size();
            self.engine.resize(w, h);
        }

        // 0. Content sync (panel → scene document → engine.apply) BEFORE
        //    rendering, so this frame draws with the current panel values.
        self.sync_scene_from_ui();
        self.sync_clouds();

        // 1. Camera animation tick + the tile transport loop: GC retry
        //    backoff, drain worker bytes into ingest, take a plan.
        self.engine.tick_now();
        self.fetch.tick(now);
        self.fetch.drain(&mut self.engine);
        self.fetch.dispatch(&mut self.engine);

        // 2. Acquire the drawable for this frame.
        let actual = self.window.inner_size();
        let window_size = (actual.width.max(1), actual.height.max(1));
        let Some(frame) = self.surface.acquire(window_size) else {
            return;
        };
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("turbomap-frame"),
            });

        // 3. Render the map onto the drawable (decoded tiles apply inside
        //    under the per-frame budget).
        self.engine.render(&mut encoder, &frame.view);

        // 4. egui on top. `ui.frame` returns a `PendingUi` that we must
        //    hand back to `ui.present` AFTER the queue submit so the GPU
        //    isn't still sampling textures we're freeing.
        let cloud_frame_count = self.cloud_frames.len();
        let ui_state = &mut self.ui_state;
        let engine = &mut self.engine;
        let pending = self.ui.frame(
            &self.gpu.device,
            &self.gpu.queue,
            &self.window,
            &mut encoder,
            &frame.view,
            frame.size,
            |ctx| build_ui(ctx, ui_state, engine, cloud_frame_count),
        );

        // 5. DIAG: if `TURBOMAP_DUMP_DIR` is set, copy the just-rendered
        //    framebuffer into a readback buffer inside this submission and
        //    save it after the submit. This is the GPU's actual output —
        //    ground truth for "what did the app render".
        let dump = self.dump_dir.is_some().then(|| {
            encode_frame_dump(
                &self.gpu.device,
                &frame.texture.texture,
                frame.size,
                &mut encoder,
            )
        });
        let dump_size = frame.size;
        let dump_format = self.surface.format();

        // 6. Submit, present, then let UI free retired textures. The
        //    engine's GPU timestamp readback is armed here for next frame's
        //    metrics.
        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.gpu.queue.submit([encoder.finish()]);
        self.engine.after_submit();
        frame.present();
        if self.ui.present(pending) {
            self.scheduler.notice_egui_repaint();
        }
        if let (Some(dir), Some((buffer, padded))) = (self.dump_dir.clone(), dump) {
            save_dump_to_png(
                &self.gpu.device,
                buffer,
                padded,
                dump_size,
                dump_format,
                &dir,
                self.frame_counter,
            );
        }

        // Cloud animation needs no manual redraw nudge: with the scene's
        // animate flag on, the ENGINE's cloud sim drives the clock (plan
        // E2) and an active sim counts as `is_animating`, which keeps the
        // scheduler pumping frames through the normal workload path.
    }
}

/// Convert one synthetic [`RadarFrame`] into the two byte planes the
/// engine's field-ingest transport takes, and push it into `slot`.
fn upload_radar_frame(engine: &mut TurbomapEngine, slot: u32, frame: &RadarFrame) {
    let n = (frame.width * frame.height) as usize;
    let mut precip = Vec::with_capacity(n);
    let mut coverage = Vec::with_capacity(n);
    for cell in &frame.cells {
        precip.push((cell.precip.clamp(0.0, 1.0) * 255.0).round() as u8);
        coverage.push((cell.coverage.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    engine.ingest_field(
        RADAR_SOURCE,
        slot,
        frame.width,
        frame.height,
        &precip,
        &coverage,
    );
}

/// Builds the egui side panel. Called inside `Context::run` so the widget
/// tree is rebuilt each frame from the current state. Mutates ONLY
/// `UiState` (content — folded into the scene document by
/// `sync_scene_from_ui`) and the camera (control plane — engine verbs).
fn build_ui(
    ctx: &egui::Context,
    ui: &mut UiState,
    engine: &mut TurbomapEngine,
    cloud_frame_count: usize,
) {
    // Custom frame with NO shadow + fully opaque background. egui's default
    // drop-shadow alpha gradient blended against the map produced visible
    // per-pixel flicker (verified by frame-diff analysis).
    let frame = egui::Frame::window(&ctx.global_style())
        .shadow(egui::epaint::Shadow::NONE)
        .fill(egui::Color32::from_rgb(28, 28, 30));
    // Bound the panel to a FIXED width and scroll its contents within the
    // window height so the geometry is stable frame-to-frame.
    let max_h = (ctx.content_rect().height() - 48.0).max(200.0);
    egui::Window::new("turbomap")
        .default_pos([12.0, 12.0])
        .resizable(false)
        .collapsible(true)
        .default_width(270.0)
        .max_width(270.0)
        .frame(frame)
        .show(ctx, |panel| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(max_h)
                .show(panel, |panel| {
                    // Frame-metric label removed: updating it per frame made
                    // the panel visibly twitch. Diagnostics via RUST_LOG.
                    panel.separator();

                    let mut camera = engine.camera();
                    let mut camera_changed = false;
                    // Capped at 65° because the basemap + vector layers are
                    // still flat (z=0) and visibly separate from the
                    // hillshade above that angle.
                    panel.horizontal(|row| {
                        row.label("pitch");
                        let mut p = camera.pitch_deg as f32;
                        if row
                            .add(egui::Slider::new(&mut p, 0.0..=65.0).suffix("°"))
                            .changed()
                        {
                            camera.pitch_deg = p as f64;
                            camera_changed = true;
                        }
                    });
                    panel.horizontal(|row| {
                        row.label("bearing");
                        let mut b = camera.bearing_deg as f32;
                        if row
                            .add(egui::Slider::new(&mut b, 0.0..=360.0).suffix("°"))
                            .changed()
                        {
                            camera.bearing_deg = b as f64;
                            camera_changed = true;
                        }
                        if row.button("⟲").on_hover_text("north-up").clicked() {
                            camera.bearing_deg = 0.0;
                            camera_changed = true;
                        }
                    });
                    panel.horizontal(|row| {
                        row.label("zoom");
                        let mut z = camera.zoom as f32;
                        if row.add(egui::Slider::new(&mut z, 4.0..=18.0)).changed() {
                            camera.zoom = z as f64;
                            camera_changed = true;
                        }
                    });
                    if camera_changed {
                        engine.set_camera(camera);
                    }

                    if panel.button("reset camera (top-down, north-up)").clicked() {
                        let mut c = engine.camera();
                        c.pitch_deg = 0.0;
                        c.bearing_deg = 0.0;
                        engine.set_camera(c);
                    }

                    panel.separator();
                    panel.label("layers");
                    // Checkboxes edit UiState only; `sync_scene_from_ui`
                    // rebuilds + re-applies the scene next frame. (The old
                    // fade-in slider is gone: layer fade-in is a Map-level
                    // render option, not scene content — nothing in the IR
                    // declares it, so the panel no longer fakes ownership.)
                    panel.checkbox(&mut ui.raster_visible, "basemap (raster)");
                    panel.checkbox(&mut ui.hillshade_visible, "hillshade (turbo DEM)");
                    panel.checkbox(&mut ui.vector_visible, "vector (roads/water)");
                    panel.checkbox(&mut ui.sky_visible, "sky (atmosphere)");

                    panel.separator();
                    build_cloud_controls(panel, &mut ui.clouds, cloud_frame_count);
                });
        });
}

/// The procedural-cloud debug-scene controls. Edits `CloudUiState` only;
/// `RunningState::sync_scene_from_ui` / `sync_clouds` push the values into
/// the scene document / the engine each frame.
fn build_cloud_controls(panel: &mut egui::Ui, c: &mut CloudUiState, frame_count: usize) {
    egui::CollapsingHeader::new("weather clouds")
        .default_open(false)
        .show(panel, |ui| {
            ui.checkbox(&mut c.enabled, "enabled");
            if !c.enabled {
                return;
            }

            ui.label(
                egui::RichText::new(
                    "tilt (pitch slider / W,S) → 3D parallax · pan + zoom → world-lock",
                )
                .weak()
                .small(),
            );

            // --- time / radar sequence ---
            ui.separator();
            ui.horizontal(|row| {
                row.checkbox(&mut c.animate, "animate");
                row.add(
                    egui::Slider::new(&mut c.speed, 0.0..=4.0)
                        .text("speed")
                        .step_by(0.05),
                );
            });
            ui.add(egui::Slider::new(&mut c.time, 0.0..=120.0).text("time (drift/boil)"));
            ui.add(egui::Slider::new(&mut c.blend, 0.0..=1.0).text("crossfade A→B"));
            if frame_count > 1 {
                let last = frame_count - 1;
                ui.horizontal(|row| {
                    row.label("radar frame");
                    row.add(egui::Slider::new(&mut c.frame_a, 0..=last).text("A"));
                    row.add(egui::Slider::new(&mut c.frame_b, 0..=last).text("B"));
                });
            }

            // --- debug view: isolate any pipeline stage ---
            ui.separator();
            const VIEWS: [DebugView; 9] = [
                DebugView::Final,
                DebugView::RadarPrecip,
                DebugView::RadarCoverage,
                DebugView::CloudField,
                DebugView::Density,
                DebugView::Light,
                DebugView::Alpha,
                DebugView::Albedo,
                DebugView::Parallax,
            ];
            egui::ComboBox::from_label("debug view")
                .selected_text(c.params.debug_view.label())
                .show_ui(ui, |combo| {
                    for v in VIEWS {
                        combo.selectable_value(&mut c.params.debug_view, v, v.label());
                    }
                });

            // --- look knobs (mirror CloudParams) ---
            ui.separator();
            let p = &mut c.params;
            ui.add(egui::Slider::new(&mut p.intensity, 0.0..=1.5).text("intensity (opacity)"));
            ui.add(egui::Slider::new(&mut p.map_scale, 1.0..=24.0).text("feature scale"));
            ui.add(egui::Slider::new(&mut p.softness, 0.0..=1.0).text("edge softness"));
            ui.add(egui::Slider::new(&mut p.erosion, 0.0..=1.0).text("edge erosion"));
            ui.add(egui::Slider::new(&mut p.sun_elevation, 0.0..=1.0).text("sun elevation"));
            ui.add(egui::Slider::new(&mut p.extinction, 1.0..=40.0).text("view extinction"));
            ui.add(egui::Slider::new(&mut p.light_extinction, 1.0..=40.0).text("light extinction"));
            ui.horizontal(|row| {
                row.label("wind");
                row.add(egui::Slider::new(&mut p.wind[0], -3.0..=3.0).text("x"));
                row.add(egui::Slider::new(&mut p.wind[1], -3.0..=3.0).text("y"));
            });
            ui.horizontal(|row| {
                row.label("sun dir");
                row.add(egui::Slider::new(&mut p.sun_dir[0], -1.0..=1.0).text("x"));
                row.add(egui::Slider::new(&mut p.sun_dir[1], -1.0..=1.0).text("y"));
            });

            if ui.button("reset look").clicked() {
                let keep_view = c.params.debug_view;
                c.params = CloudParams::default();
                c.params.debug_view = keep_view;
            }
        });
}

/// Dump support, part 1: encode a `copy_texture_to_buffer` of the frame
/// into the caller's encoder. Returns the readback buffer + padded stride.
///
/// This is a diagnostic (gated on `TURBOMAP_DUMP_DIR`): it shows the GPU's
/// actual output, byte-exact, on disk — separate from anything the OS
/// compositor does at present time.
fn encode_frame_dump(
    device: &wgpu::Device,
    src: &wgpu::Texture,
    size: (u32, u32),
    encoder: &mut wgpu::CommandEncoder,
) -> (wgpu::Buffer, u32) {
    let (width, height) = size;
    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(256) * 256;
    let buffer_size = (padded_bytes_per_row * height) as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dump-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: src,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    (buffer, padded_bytes_per_row)
}

/// Dump support, part 2: after the submit, map the readback buffer and
/// write `<dir>/frame_<id>.png`. Sync readback with `Poll` spinning — slow,
/// diagnostics only.
#[allow(clippy::too_many_arguments)]
fn save_dump_to_png(
    device: &wgpu::Device,
    buffer: wgpu::Buffer,
    padded_bytes_per_row: u32,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    dir: &str,
    frame_id: u32,
) {
    let (width, height) = size;
    let unpadded_bytes_per_row = width * 4;
    let buffer_slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    // Spin polling so the surface keeps advancing. `Maintain::Wait` hangs
    // because it blocks on ALL future submissions, including ones the
    // render loop hasn't sent yet.
    let started = std::time::Instant::now();
    loop {
        let _ = device.poll(wgpu::PollType::Poll);
        if let Ok(Ok(())) = rx.recv_timeout(std::time::Duration::from_millis(10)) {
            break;
        }
        if started.elapsed() > std::time::Duration::from_secs(2) {
            log::warn!("dump map_async timed out for frame {}", frame_id);
            return;
        }
    }
    let data = buffer_slice.get_mapped_range();
    // Strip the row padding into a contiguous RGBA buffer.
    let mut rgba = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        rgba.extend_from_slice(&data[start..end]);
    }
    drop(data);
    buffer.unmap();
    // BGRA surfaces need a swizzle to RGBA for PNG.
    if matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
    }
    let _ = std::fs::create_dir_all(dir);
    let path = format!("{}/frame_{:05}.png", dir, frame_id);
    if let Err(e) = image::save_buffer(&path, &rgba, width, height, image::ColorType::Rgba8) {
        log::warn!("failed to dump frame {}: {}", frame_id, e);
    }
}

/// Scene-IR encoding for a core DEM source's declared wire encoding.
fn scene_dem_encoding(e: turbomap_core::DemEncoding) -> turbomap_scene::DemEncoding {
    match e {
        turbomap_core::DemEncoding::MapboxRgb => turbomap_scene::DemEncoding::MapboxRgb,
        turbomap_core::DemEncoding::Terrarium => turbomap_scene::DemEncoding::Terrarium,
    }
}

/// One [`SourceDef`] declaring the same endpoint an [`HttpRasterSource`]
/// fetches from — template + zoom bounds read back off the source so the
/// scene document and the fetcher can't drift.
fn raster_source_def(s: &HttpRasterSource) -> SourceDef {
    SourceDef::RasterXyz {
        tiles: vec![s.url_template().to_string()],
        tile_size: 256,
        min_zoom: TileSource::min_zoom(s),
        max_zoom: TileSource::max_zoom(s),
        attribution: s.attribution().map(str::to_string),
    }
}

fn dem_source_def(s: &HttpRasterSource) -> SourceDef {
    SourceDef::DemXyz {
        tiles: vec![s.url_template().to_string()],
        encoding: scene_dem_encoding(s.dem_encoding()),
        min_zoom: TileSource::min_zoom(s),
        max_zoom: TileSource::max_zoom(s),
        halo: s.dem_halo_px(),
    }
}

fn vector_source_def(s: &HttpVectorTileSource) -> SourceDef {
    SourceDef::VectorXyz {
        tiles: vec![s.url_template().to_string()],
        min_zoom: VectorTileSource::min_zoom(s),
        max_zoom: VectorTileSource::max_zoom(s),
    }
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cache_root = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("turbomap");
    log::info!("tile caches under {}", cache_root.display());

    // Vector layer + style, authored as Scene IR. With TURBO_BASEMAP_URL
    // set (e.g. http://localhost:8090), the demo renders OUR OWN N50
    // basemap with the tileserver's served MapLibre style lowered to IR
    // layers — the self-hosted path. Without it, the VersaTiles OSM overlay
    // + built-in demo layers (see `styles.rs`).
    let basemap_base = std::env::var("TURBO_BASEMAP_URL").ok();
    // Full road/label overlay by default. TURBO_WATER_ONLY=1 switches to
    // the water-fills-only debug style (isolates water rendering).
    let full_style = std::env::var("TURBO_WATER_ONLY").is_err();
    let (vector, vector_layers): (Arc<HttpVectorTileSource>, Vec<Layer>) = match &basemap_base {
        Some(base) => {
            log::info!("vector basemap from {base}/v1/basemap (MVT)");
            let src = turbomap_tiles_http::HttpVectorTileSource::turbo_basemap(base)
                .expect("build turbo basemap source")
                .with_cache_dir(cache_root.join("turbo-basemap"));
            let layers = if full_style {
                let style_json =
                    turbomap_tiles_http::fetch_text(&format!("{base}/v1/basemap/style.json"))
                        .expect("fetch /v1/basemap/style.json");
                // Style decision for this raster-hybrid host: the Kartverket
                // raster underneath shows the water — drop the served
                // style's flat water fills (keeps lines/labels). The style's
                // background colour is ignored for the same reason (the
                // raster bed IS the background; `parse_style_background`
                // exists for pure-vector hosts).
                turbomap_style_maplibre::without_water_fill_layers(
                    turbomap_style_maplibre::parse_style_layers(&style_json, VECTOR_SOURCE)
                        .expect("parse served MapLibre style"),
                )
            } else {
                crate::styles::water_only_layers(VECTOR_SOURCE)
            };
            (Arc::new(src), layers)
        }
        None => (
            Arc::new(
                turbomap_tiles_http::HttpVectorTileSource::versatiles_osm()
                    .expect("build VersaTiles source")
                    .with_cache_dir(cache_root.join("versatiles-osm")),
            ),
            if full_style {
                crate::styles::versatiles_demo_layers(VECTOR_SOURCE)
            } else {
                crate::styles::water_only_layers(VECTOR_SOURCE)
            },
        ),
    };

    // Raster basemap: Kartverket grey topo by default (neutral, lets the
    // vector colours pop). Override with TURBO_RASTER_URL=<{z}/{y}/{x}
    // template> for a satellite/aerial bed — e.g. Esri World Imagery.
    // Cached on disk so scene rebuilds (layer toggles) refetch from disk.
    let raster: Arc<HttpRasterSource> = match std::env::var("TURBO_RASTER_URL").ok() {
        Some(tmpl) => {
            let lower = tmpl.to_lowercase();
            let fmt = if lower.contains("arcgis")
                || lower.contains("imagery")
                || lower.ends_with(".jpg")
                || lower.ends_with(".jpeg")
            {
                turbomap_core::RasterFormat::Jpeg
            } else {
                turbomap_core::RasterFormat::Png
            };
            log::info!("raster basemap from TURBO_RASTER_URL ({fmt:?}): {tmpl}");
            Arc::new(
                turbomap_tiles_http::HttpRasterSource::new(tmpl, "turbomap-app/0.1", 0, 19, fmt)
                    .expect("build raster source from TURBO_RASTER_URL")
                    .with_cache_dir(cache_root.join("raster-custom")),
            )
        }
        None => Arc::new(
            turbomap_tiles_http::HttpRasterSource::kartverket_topo_grey()
                .expect("build Kartverket grey source")
                .with_cache_dir(cache_root.join("kartverket-topo-grey")),
        ),
    };

    // Optional DEM from our own tileserver's `/v1/dem/rgb/{z}/{x}/{y}.png`
    // endpoint (Mapbox Terrain-RGB, halo=1). Set TURBO_API_URL to enable.
    let dem: Option<Arc<HttpRasterSource>> = std::env::var("TURBO_API_URL").ok().and_then(|base| {
        log::info!("enabling hillshade layer from {base}/v1/dem/rgb/{{z}}/{{x}}/{{y}}.png");
        turbomap_tiles_http::HttpRasterSource::turbo_terrain_rgb(&base)
            .map(|s| Arc::new(s.with_cache_dir(cache_root.join("turbo-dem"))))
            .map_err(|e| log::warn!("DEM source build failed: {e}"))
            .ok()
    });

    let sources = SourceSet {
        raster_def: raster_source_def(&raster),
        dem_def: dem.as_deref().map(dem_source_def),
        vector_def: vector_source_def(&vector),
        raster,
        dem,
        vector,
    };

    let initial_camera = CameraState::new(LatLng::new(60.39, 5.32), 11.0);
    let mut app = TurbomapApp::new(sources, vector_layers, initial_camera);

    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    // `Poll` keeps the runloop ticking even when no OS events are queued,
    // so tile-fetch worker completions get drained promptly even before the
    // user moves the mouse.
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).expect("run_app");
}
