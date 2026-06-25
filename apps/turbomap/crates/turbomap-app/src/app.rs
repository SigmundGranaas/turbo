//! winit `ApplicationHandler` driving the vector map. The body
//! here is intentionally thin: anything substantive lives in
//! one of `gpu`, `surface`, `schedule`, `map_host`, or `ui`.

use std::sync::Arc;
use std::time::Instant;

use turbomap_core::{
    Camera, CloudParams, Color, Filter, HitResult, LatLng, Map, MapOptions, Marker, MarkerId,
    Paint, RadarFrame, RasterFormat, Rule, SunPosition, TileSource, VectorStyle, VectorTileSource,
};
use turbomap_clouds::{DebugView, SyntheticStorm};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

/// Vector-tile worker thread count. See `map_host` for the
/// raster + DEM pool sizes (4 + 3 respectively).
const FETCH_WORKERS: usize = 6;

pub struct TurbomapApp {
    raster_source: Arc<dyn TileSource>,
    /// Optional DEM source for the GPU hillshade layer. When `None`,
    /// the demo skips adding the hillshade layer.
    dem_source: Option<Arc<dyn TileSource>>,
    vector_source: Arc<dyn VectorTileSource>,
    style: VectorStyle,
    initial_camera: Camera,
    state: Option<RunningState>,
}

struct RunningState {
    window: Arc<Window>,
    /// Owns the wgpu surface + its configuration. Hides the
    /// macOS-specific Outdated/Lost recovery and the
    /// "configure invalidates the drawable pool" trap. See
    /// `surface.rs`.
    surface: crate::surface::RenderSurface,
    /// Decides *when* to render. Pure state machine. Hides
    /// the resize-burst quiet window and the dirty-bit
    /// bookkeeping that used to be scattered across half a
    /// dozen fields. See `schedule.rs`.
    scheduler: crate::schedule::RenderScheduler,
    /// All GPU-side handles in one place. The surface itself
    /// was already moved out at startup and lives in
    /// `surface` above; `gpu` still owns the device, queue,
    /// adapter, format, etc. See `gpu.rs`.
    gpu: crate::gpu::GpuContext,
    /// `Map` + three fetch pumps + the inflight / failed
    /// bookkeeping for tile loading. See `map_host.rs`.
    host: crate::map_host::MapHost,
    /// Pointer position, drag anchor, click-vs-pan
    /// disambiguation. See `input.rs`. The event handler
    /// translates winit events into `Gesture`s and forwards
    /// them to `host.map_mut()`.
    pointer: crate::input::PointerState,
    /// Egui panel + renderer wrapped into one object. The
    /// borrow-checker enforces that retired font-atlas
    /// textures are freed AFTER `queue.submit`, not before
    /// (the previous version of this code documented that
    /// invariant in a comment and got it wrong twice). See
    /// `ui.rs`.
    ui: crate::ui::UiOverlay,
    /// Mirror of the UI panel's slider values. Lives in
    /// `App` because the panel mutates `Map` via these and
    /// the closure that builds the egui frame can't own
    /// both. Updated each frame, persists across them.
    ui_state: UiState,
    /// True while the pointer is over an egui widget. Suppresses
    /// pan/click handling for that frame so dragging a slider doesn't
    /// also pan the map underneath.
    egui_wants_pointer: bool,
    /// Monotonic frame counter, used as the diagnostic
    /// framebuffer-dump filename when `TURBOMAP_DUMP_DIR` is
    /// set in the environment.
    frame_counter: u32,
    /// Synthetic radar sequence backing the cloud debug scene. Generated
    /// once at startup; the panel's frame-A/B sliders pick pairs out of it.
    cloud_frames: Vec<RadarFrame>,
    /// Which `(frame_a, frame_b)` pair is currently uploaded to the GPU,
    /// so `apply_clouds` only re-uploads textures when the selection moves.
    cloud_uploaded: Option<(usize, usize)>,
}

/// UI-bound state. Cached so the panel survives across frames; the
/// map's actual layer fade / visibility is the source of truth and
/// this struct just mirrors it for the slider widgets.
#[derive(Debug, Clone)]
struct UiState {
    raster_visible: bool,
    hillshade_visible: bool,
    /// Analytic sky pass (Map::set_sky_enabled). Off by default for water debug.
    sky_visible: bool,
    vector_visible: bool,
    fade_in_secs: f32,
    /// Smoothed + sampled frame-timing display. The raw
    /// per-frame numbers change every render — at 120 Hz
    /// vsync that's 120 text-width changes per second which
    /// rearranges the panel layout and makes the drop shadow
    /// jitter. We sample at ~5 Hz and pad the formatted
    /// string to a fixed width so the panel is layout-stable.
    metrics_display: MetricsDisplay,
    /// Procedural cloud-overlay debug controls. See [`CloudUiState`].
    clouds: CloudUiState,
}

#[derive(Debug, Clone)]
struct MetricsDisplay {
    text: String,
    last_update: Instant,
    /// Rolling sum of per-frame samples since the last
    /// display update, used to compute the mean shown to
    /// the user.
    cpu_sum_ms: f64,
    gpu_sum_ms: f64,
    sample_count: u32,
    gpu_sample_count: u32,
}

/// Mirror of the procedural cloud overlay's debug-scene controls. The Map
/// owns the real `CloudParams`; this struct holds what the panel edits and
/// `RunningState::apply_clouds` pushes into the Map each frame. The
/// camera-driven fields (world-lock affine, inv-view-proj, slab altitude)
/// are recomputed inside `Map::render` from the live camera, so the look
/// knobs we set here are never clobbered.
#[derive(Debug, Clone)]
struct CloudUiState {
    /// Master on/off (drives `Map::set_clouds_visible`).
    enabled: bool,
    /// Auto-advance the drift/boil clock each frame (`time`).
    animate: bool,
    /// Multiplier on the per-frame `time` advance when `animate`.
    speed: f32,
    /// Drift/boil clock in seconds (scrubbable; auto-advanced when animating).
    time: f32,
    /// Crossfade `0..1` between synthetic radar frame A (slot 0) and B (slot 1).
    blend: f32,
    /// Index into the synthetic storm sequence uploaded to slot 0.
    frame_a: usize,
    /// Index into the synthetic storm sequence uploaded to slot 1.
    frame_b: usize,
    /// Look knobs + debug-view selector. Camera/affine fields are ignored
    /// (overwritten per frame by `Map::render`).
    params: CloudParams,
}

impl Default for CloudUiState {
    fn default() -> Self {
        Self {
            // Off by default: the cloud overlay rendered as a grid of white blobs
            // over open water and confounded water debugging. Re-enable in the
            // "weather clouds" panel section.
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

impl MetricsDisplay {
    /// One-second sample window. At 120 Hz that's ~120
    /// per-frame samples averaged into one displayed value;
    /// the value updates exactly once per second so the
    /// panel doesn't visibly twitch.
    const SAMPLE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

    fn new() -> Self {
        Self {
            text: "frame: cpu  -.-- ms · gpu  -.-- ms".into(),
            last_update: Instant::now() - Self::SAMPLE_INTERVAL,
            cpu_sum_ms: 0.0,
            gpu_sum_ms: 0.0,
            sample_count: 0,
            gpu_sample_count: 0,
        }
    }

    /// Add one frame's sample. Updates the displayed string
    /// at most once per `SAMPLE_INTERVAL` from the mean of
    /// all samples accumulated in the window. The mean
    /// dampens per-frame variance so the value doesn't
    /// twitch between adjacent digits at 120 Hz, which is
    /// what produced the visible flicker.
    fn refresh(&mut self, cpu_ms: f64, gpu_ms: Option<f64>) {
        self.cpu_sum_ms += cpu_ms;
        self.sample_count += 1;
        if let Some(g) = gpu_ms {
            self.gpu_sum_ms += g;
            self.gpu_sample_count += 1;
        }
        let now = Instant::now();
        if now.duration_since(self.last_update) < Self::SAMPLE_INTERVAL {
            return;
        }
        self.last_update = now;
        let cpu_mean = self.cpu_sum_ms / self.sample_count.max(1) as f64;
        let gpu_mean = if self.gpu_sample_count > 0 {
            format!("{:>5.2}", self.gpu_sum_ms / self.gpu_sample_count as f64)
        } else {
            " n/a ".into()
        };
        self.text = format!("frame: cpu {:>5.2} ms · gpu {} ms", cpu_mean, gpu_mean);
        self.cpu_sum_ms = 0.0;
        self.gpu_sum_ms = 0.0;
        self.sample_count = 0;
        self.gpu_sample_count = 0;
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            raster_visible: true,
            // Off by default — water debug isolates the surface; the DEM source
            // stays registered (drives the water terrain-reflection) but the grey
            // relief overlay is hidden. Re-enable via the layers checkbox.
            hillshade_visible: false,
            sky_visible: false,
            vector_visible: true,
            // DIAG: was 0.4. With fade-in active, each newly
            // loaded tile's alpha animates from 0→1 over this
            // window — every frame during the fade has a
            // slightly different alpha, so pixels change per
            // frame whether the camera moved or not. Setting
            // to 0 makes new tiles snap to fully opaque
            // immediately, eliminating per-tile animation
            // as a flicker source.
            fade_in_secs: 0.0,
            metrics_display: MetricsDisplay::new(),
            clouds: CloudUiState::default(),
        }
    }
}

impl TurbomapApp {
    pub fn new(
        raster_source: Arc<dyn TileSource>,
        dem_source: Option<Arc<dyn TileSource>>,
        vector_source: Arc<dyn VectorTileSource>,
        style: VectorStyle,
        initial_camera: Camera,
    ) -> Self {
        Self {
            raster_source,
            dem_source,
            vector_source,
            style,
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

        let mut map = Map::new(
            gpu.device.clone(),
            gpu.queue.clone(),
            gpu.surface_format,
            render_surface.size(),
            self.initial_camera,
            MapOptions::default(),
        )
        .expect("create map");
        // Layer stack (back → front):
        //   1. Kartverket grey topo raster basemap
        //   2. (optional) GPU hillshade from our own DEM endpoint
        //   3. VersaTiles OSM vector overlay (roads + labels)
        map.add_raster_layer(crate::map_host::RASTER_LAYER_ID, self.raster_source.clone());
        if let Some(dem) = &self.dem_source {
            // Register the DEM at Map level. From here, the hillshade
            // overlay just describes the *look* (sun direction,
            // shadow/highlight colours); the shared TerrainCache owns
            // the data and any future displacement-aware pipeline
            // will see the same tiles.
            map.set_terrain_source(dem.clone(), turbomap_core::TerrainOptions::default());
            map.add_hillshade_layer(
                crate::map_host::HILLSHADE_LAYER_ID,
                turbomap_core::HillshadeStyle::default(),
            );
        }
        map.add_vector_layer(
            crate::map_host::VECTOR_LAYER_ID,
            self.vector_source.clone(),
            self.style.clone(),
        );

        // A fixed low sun so terrain lighting is in frame from the first render.
        map.set_sun_position(Some(SunPosition {
            azimuth_deg: 145.0,
            altitude_deg: 18.0,
        }));
        // Minimal scene by default: no sky, no hillshade overlay (DEM stays).
        map.set_sky_enabled(false);
        if self.dem_source.is_some() {
            map.set_layer_visibility(crate::map_host::HILLSHADE_LAYER_ID, false);
        }

        // Demo city markers — off by default; set
        // TURBO_MARKERS=1 to drop them for hit-test testing.
        if std::env::var("TURBO_MARKERS").is_ok() {
            for (name, lat, lng, color) in [
                ("Bergen", 60.39, 5.32, Color::rgb(0xE5, 0x39, 0x35)),
                ("Oslo", 59.91, 10.75, Color::rgb(0x1E, 0x88, 0xE5)),
                ("Trondheim", 63.43, 10.39, Color::rgb(0x43, 0xA0, 0x47)),
                ("Tromsø", 69.65, 18.96, Color::rgb(0xFD, 0xD8, 0x35)),
            ] {
                let mut data = std::collections::HashMap::new();
                data.insert("name".to_owned(), name.to_owned());
                map.add_marker(Marker {
                    id: MarkerId(0),
                    lng_lat: LatLng::new(lat, lng),
                    radius_px: 10.0,
                    color,
                    data,
                });
            }
        }

        // Procedural cloud overlay — OFF by default for the clean water-debug
        // base (it rendered as a confounding blob grid over the sea). Set
        // TURBO_CLOUDS=1 to set up the synthetic-storm debug scene again.
        let cloud_frames: Vec<RadarFrame> = if std::env::var("TURBO_CLOUDS").is_ok() {
            let storm = SyntheticStorm::default();
            let frames = storm.generate();
            map.enable_clouds(storm.width, storm.height);
            let last = frames.len().saturating_sub(1);
            map.ingest_radar_frame(0, &frames[0]);
            map.ingest_radar_frame(1, &frames[1.min(last)]);
            let c = self.initial_camera.center;
            map.set_cloud_geo_bounds(c.lng - 4.0, c.lat - 2.0, c.lng + 4.0, c.lat + 2.0);
            frames
        } else {
            Vec::new()
        };

        let host = crate::map_host::build(
            map,
            self.vector_source.clone(),
            self.raster_source.clone(),
            self.dem_source.clone(),
            self.style.clone(),
            FETCH_WORKERS,
            4,
            3,
        );

        let ui = crate::ui::UiOverlay::new(&gpu.device, gpu.surface_format, &window);

        self.state = Some(RunningState {
            window: window.clone(),
            surface: render_surface,
            scheduler: crate::schedule::RenderScheduler::new(),
            gpu,
            host,
            pointer: crate::input::PointerState::default(),
            ui,
            ui_state: UiState::default(),
            egui_wants_pointer: false,
            frame_counter: 0,
            cloud_uploaded: if cloud_frames.is_empty() {
                None
            } else {
                Some((0, 1.min(cloud_frames.len().saturating_sub(1))))
            },
            cloud_frames,
        });
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        // egui sees every event first so it can update its own state
        // (cursor position, key state, viewport size). We never
        // short-circuit on `consumed` — window-state events like
        // Resized or CloseRequested MUST still reach our match below.
        // An earlier version returned early when egui said "consumed",
        // which silently dropped Resized events during live resize on
        // macOS and froze the surface. Instead we let the match run,
        // and gate the input-specific branches (pointer / keyboard)
        // on `egui_wants_pointer` / `egui_wants_keyboard` so dragging
        // a slider doesn't also pan the map underneath.
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
                // No configure here. The scheduler will hold
                // the size for ~30 ms; on_redraw applies it
                // when the burst settles. See `schedule.rs`.
                state.scheduler.notice_resize(size.width, size.height);
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let gesture = state.pointer.on_cursor_moved((position.x, position.y));
                if state.egui_wants_pointer {
                    return;
                }
                if let Some(crate::input::Gesture::Pan { dx, dy }) = gesture {
                    state.host.map_mut().pan_by_pixels(dx, dy);
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
                        state.host.map_mut().zoom_around(factor, focus);
                        state.window.request_redraw();
                    }
                    MouseScrollDelta::PixelDelta(p) => {
                        state.host.map_mut().pan_by_pixels(p.x, p.y);
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
                    state.host.map_mut().zoom_around(factor, focus);
                    state.window.request_redraw();
                }
            }
            WindowEvent::PanGesture { delta, .. } => {
                if state.egui_wants_pointer {
                    return;
                }
                state.host.map_mut().pan_by_pixels(delta.x as f64, delta.y as f64);
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
                // Camera tilt + bearing controls — exercise the new
                // perspective camera. W/S nudge pitch ±5°, A/D rotate
                // bearing ±15°, R resets to top-down north-up.
                let mut camera = state.host.map().camera();
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
                    state.host.map_mut().set_camera(camera);
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                state.on_redraw();
            }
            _ => {}
        }
    }

    /// winit calls this once per loop tick (Poll mode) or before
    /// each sleep (Wait mode). We use it as the *guaranteed* tick
    /// that drives rendering whenever something has happened on a
    /// worker thread — tile fetches, animation, anything that the
    /// event-driven `RedrawRequested` path might miss because
    /// macOS isn't delivering the event yet (e.g. the window
    /// hasn't been made key, the tab is in the background, etc).
    ///
    /// Each tick we hand the scheduler the current workload
    /// snapshot and act on its decision. All the "when should
    /// we render?" logic lives in `schedule.rs`; here we just
    /// translate that decision into winit calls. See the
    /// module-level docs of `schedule` for the rules.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        use winit::event_loop::ControlFlow;
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let workload = state.host.workload();
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

    /// A click was detected at `pos`. If it lands on a marker, delete it;
    /// otherwise place a new user marker there. Vector features under the
    /// click are still logged for visibility.
    fn dispatch_click(&mut self, pos: (f64, f64)) {
        let hits = self.host.map().hit_test(pos, 6.0);

        // hit_test returns markers first, top-down. The topmost one wins
        // on a click — match how the user sees the stack.
        let marker_hit = hits.iter().find_map(|h| match h {
            HitResult::Marker(m) => Some(m.id),
            _ => None,
        });

        if let Some(id) = marker_hit {
            self.host.map_mut().remove_marker(id);
            log::info!("removed marker {:?}", id);
        } else {
            let lng_lat = self.host.map().screen_to_lng_lat(pos);
            let id = self.host.map_mut().add_marker(Marker {
                id: MarkerId(0),
                lng_lat,
                radius_px: 8.0,
                // Cyan — visually distinct from the demo Norwegian-city
                // markers so user-added points are easy to spot.
                color: Color::rgb(0x00, 0xB8, 0xD4),
                data: std::collections::HashMap::new(),
            });
            log::info!("added marker {:?} at lat={:.4}, lng={:.4}", id, lng_lat.lat, lng_lat.lng);
        }

        // For visibility: anything *else* we hit (other markers stacked,
        // features) gets logged but doesn't affect the add/delete action.
        for hit in &hits {
            match hit {
                HitResult::Marker(m) if Some(m.id) != marker_hit => {
                }
                _ => {}
            }
        }
        // Marker set changed → redraw needed (we won't get a redraw
        // request otherwise since the camera didn't move).
        self.window.request_redraw();
    }

    fn on_redraw(&mut self) {
        let now = Instant::now();
        // Bail out if we're still in a resize-event burst. The
        // scheduler tracks the timeline; on_redraw just asks.
        // Skipping here is the foundational architectural rule
        // that prevents drawable-pool exhaustion (see
        // `surface.rs` and `schedule.rs` headers).
        if self.scheduler.in_resize_burst(now) {
            return;
        }
        // If a resize just settled, apply it BEFORE acquiring a
        // drawable. The scheduler hands us the latest stored
        // size; we reconfigure both the surface and the map.
        if let Some((width, height)) = self.scheduler.take_settled_resize(now) {
            let (w, h) = self.surface.size();
            self.host.resize(w, h);
        }

        // 0–2. All tile-pipeline work — animation tick, channel
        //      drains, recently-failed GC, fetch dispatch — lives
        //      in `MapHost`. See `map_host.rs`.
        self.host.tick(now);
        self.host.drain_workers();
        self.host.dispatch_fetches();

        // 3. Acquire the drawable for this frame.
        let actual = self.window.inner_size();
        let window_size = (actual.width.max(1), actual.height.max(1));
        let Some(frame) = self.surface.acquire(window_size) else {
            return;
        };
        let mut encoder =
            self.gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("turbomap-frame"),
                });

        // 3b. Sync the cloud debug-scene panel state into the Map (frame
        //     upload on selection change, look knobs, animation clock)
        //     BEFORE rendering, so this frame draws with the current panel
        //     values. The camera-driven cloud fields are recomputed inside
        //     `Map::render`, so the order is safe.
        self.apply_clouds();

        // 4. Render the map onto the drawable.
        self.host.render(&mut encoder, &frame.view);

        // 5. egui on top. The borrow-checker keeps `Map` /
        //    `ui_state` / the panel closure alive across the
        //    egui frame in one place; `ui.frame` returns a
        //    `PendingUi` that we must hand back to
        //    `ui.present` AFTER the queue submit so the GPU
        //    isn't still sampling textures we're freeing.
        let cloud_frame_count = self.cloud_frames.len();
        let ui_state = &mut self.ui_state;
        let map_for_ui = self.host.map_mut();
        let pending = self.ui.frame(
            &self.gpu.device,
            &self.gpu.queue,
            &self.window,
            &mut encoder,
            &frame.view,
            frame.size,
            |ctx| build_ui(ctx, ui_state, map_for_ui, cloud_frame_count),
        );

        // 6. Submit, present, then let UI free retired
        //    textures. Map's GPU timestamp readback is armed
        //    here for next frame's metrics.
        //
        //    DIAG: if `TURBOMAP_DUMP_DIR` is set, dump the
        //    just-rendered framebuffer to a PNG before
        //    present. This is the GPU's actual output —
        //    diffs across these PNGs are ground truth for
        //    "the GPU rendered different pixels", separate
        //    from anything macOS does at composite time.
        self.frame_counter = self.frame_counter.wrapping_add(1);
        self.gpu.queue.submit([encoder.finish()]);
        self.host.after_submit();
        frame.present();
        if self.ui.present(pending) {
            self.scheduler.notice_egui_repaint();
        }

        // Cloud drift/boil animation isn't part of `Map::is_animating`
        // (which tracks camera + tile fades), so it won't keep the
        // scheduler awake on its own. Drive a fresh frame directly while
        // the panel's animate toggle is on.
        if self.ui_state.clouds.enabled && self.ui_state.clouds.animate {
            self.window.request_redraw();
        }
    }

    /// Push the cloud debug-panel state into the Map for this frame.
    ///
    /// Re-uploads the radar texture pair only when the frame-A/B selection
    /// changes (texture upload is the one non-trivial cost here), advances
    /// the drift clock when animating, and forwards the look knobs +
    /// debug-view selector. The world-lock affine, inverse-view-projection
    /// and slab altitude are deliberately *not* set here — `Map::render`
    /// recomputes them from the live camera every frame.
    fn apply_clouds(&mut self) {
        let map = self.host.map_mut();
        map.set_clouds_visible(self.ui_state.clouds.enabled);
        if !self.ui_state.clouds.enabled || self.cloud_frames.is_empty() {
            return;
        }

        let last = self.cloud_frames.len() - 1;
        let a = self.ui_state.clouds.frame_a.min(last);
        let b = self.ui_state.clouds.frame_b.min(last);
        if self.cloud_uploaded != Some((a, b)) {
            map.ingest_radar_frame(0, &self.cloud_frames[a]);
            map.ingest_radar_frame(1, &self.cloud_frames[b]);
            self.cloud_uploaded = Some((a, b));
        }

        if self.ui_state.clouds.animate {
            // Fixed per-frame step (panel renders at vsync while animating).
            // Exact rate is irrelevant for a look-debug tool; `speed` scales it.
            self.ui_state.clouds.time += 0.016 * self.ui_state.clouds.speed;
        }

        map.set_cloud_params(self.ui_state.clouds.params);
        map.set_cloud_time(self.ui_state.clouds.time, self.ui_state.clouds.blend);
    }
}

// Re-export the layer ids from `map_host` so `build_ui`'s
// callers can refer to them without depending on the host
// module directly. Layer IDs are stable contract between
// `MapHost::dispatch_fetches` and the panel's checkbox state.
const RASTER_LAYER_ID_PUB: &str = crate::map_host::RASTER_LAYER_ID;
const HILLSHADE_LAYER_ID_PUB: &str = crate::map_host::HILLSHADE_LAYER_ID;
const VECTOR_LAYER_ID_PUB: &str = crate::map_host::VECTOR_LAYER_ID;

/// Builds the egui side panel. Called inside `Context::run` so the
/// widget tree is rebuilt each frame from the current state. Mutates
/// the Map directly on slider/checkbox change — no diff step.
fn build_ui(ctx: &egui::Context, ui: &mut UiState, map: &mut Map, cloud_frame_count: usize) {
    // Custom frame with NO shadow + fully opaque background.
    // egui's default Window has a soft drop-shadow rendered as
    // an alpha gradient that, when blended against the map
    // underneath each frame, produces 1-5/255 per-pixel
    // variance (verified by frame-diff analysis of a recorded
    // screen video). That variance is the visible flicker the
    // user reported — text edges + shadow gradient changing
    // subtly per frame as egui's sub-pixel positioning
    // rounds differently.
    let frame = egui::Frame::window(&ctx.style())
        .shadow(egui::epaint::Shadow::NONE)
        .fill(egui::Color32::from_rgb(28, 28, 30));
    // Bound the panel to a FIXED width and scroll its contents within the
    // window height. Without this the window auto-sized to its (now tall)
    // content — overflowing the screen and reflowing per frame, which read as
    // heavy flicker. A fixed width + capped scroll area keeps the geometry
    // stable frame-to-frame.
    let max_h = (ctx.screen_rect().height() - 48.0).max(200.0);
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
            // Frame-metric label removed. Updating the label
            // every frame (or even every second) made the
            // panel — and the map blended underneath through
            // its semi-transparent background — visibly
            // twitch. Diagnostics moved to `RUST_LOG=info`.
            let _ = map.last_frame_metrics();

            panel.separator();

            let mut camera = map.camera();
            let mut camera_changed = false;
            // 3D terrain is now wired up — the hillshade layer renders
            // as a subdivided heightmap mesh, so tilt shows real
            // mountains rising off the basemap. Capped at 65° because
            // the basemap + vector layers are still flat (z=0) and
            // visibly separate from the hillshade above that angle.
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
                if row
                    .add(egui::Slider::new(&mut z, 4.0..=18.0))
                    .changed()
                {
                    camera.zoom = z as f64;
                    camera_changed = true;
                }
            });
            if camera_changed {
                map.set_camera(camera);
            }

            if panel
                .button("reset camera (top-down, north-up)")
                .clicked()
            {
                let mut c = map.camera();
                c.pitch_deg = 0.0;
                c.bearing_deg = 0.0;
                map.set_camera(c);
            }

            panel.separator();
            panel.label("layers");
            let prev_raster = ui.raster_visible;
            let prev_hill = ui.hillshade_visible;
            let prev_vec = ui.vector_visible;
            panel.checkbox(&mut ui.raster_visible, "basemap (raster)");
            panel.checkbox(&mut ui.hillshade_visible, "hillshade (turbo DEM)");
            panel.checkbox(&mut ui.vector_visible, "vector (roads/water)");
            if panel.checkbox(&mut ui.sky_visible, "sky (atmosphere)").changed() {
                map.set_sky_enabled(ui.sky_visible);
            }
            if ui.raster_visible != prev_raster {
                map.set_layer_visibility(RASTER_LAYER_ID_PUB, ui.raster_visible);
            }
            if ui.hillshade_visible != prev_hill {
                map.set_layer_visibility(HILLSHADE_LAYER_ID_PUB, ui.hillshade_visible);
            }
            if ui.vector_visible != prev_vec {
                map.set_layer_visibility(VECTOR_LAYER_ID_PUB, ui.vector_visible);
            }

            panel.separator();
            panel.horizontal(|row| {
                row.label("fade-in");
                if row
                    .add(
                        egui::Slider::new(&mut ui.fade_in_secs, 0.0..=1.5)
                            .suffix(" s")
                            .step_by(0.05),
                    )
                    .changed()
                {
                    map.set_layer_fade_in(RASTER_LAYER_ID_PUB, ui.fade_in_secs);
                    map.set_layer_fade_in(HILLSHADE_LAYER_ID_PUB, ui.fade_in_secs);
                    map.set_layer_fade_in(VECTOR_LAYER_ID_PUB, ui.fade_in_secs);
                }
            });

            panel.separator();
            build_cloud_controls(panel, &mut ui.clouds, cloud_frame_count);
            });
        });
}

/// The procedural-cloud debug-scene controls. Edits `CloudUiState` only;
/// `RunningState::apply_clouds` pushes the values into the Map each frame.
/// Lives in its own fn so the giant `build_ui` closure stays readable.
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
            ui.add(
                egui::Slider::new(&mut p.light_extinction, 1.0..=40.0).text("light extinction"),
            );
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

/// Dump the just-rendered surface texture to `<dir>/frame_<id>.png`.
///
/// This is a diagnostic. When the user reports flicker, we need
/// to know whether the GPU is producing different pixels each
/// frame, or whether the GPU output is stable but the macOS
/// compositor is choosing between stale swap-chain images. This
/// function shows the GPU's actual output, byte-exact, on disk.
///
/// Sync readback: encodes a `copy_texture_to_buffer`, submits a
/// separate command buffer (since the caller's main encoder is
/// still being used), maps the readback buffer with `Wait`
/// polling. Slow — only intended for diagnostic runs, gated on
/// `TURBOMAP_DUMP_DIR`.
#[allow(dead_code)] // kept for future render-output diagnostics
fn encode_frame_dump(
    device: &wgpu::Device,
    src: &wgpu::Texture,
    size: (u32, u32),
    encoder: &mut wgpu::CommandEncoder,
) -> (wgpu::Buffer, u32) {
    let (width, height) = size;
    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let padded_bytes_per_row = ((unpadded_bytes_per_row + 255) / 256) * 256;
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

#[allow(clippy::too_many_arguments, dead_code)] // kept for future render-output diagnostics
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
    // Spin polling so the surface keeps advancing. `Maintain::Wait`
    // hangs because it blocks on ALL future submissions, including
    // ones the render loop hasn't sent yet.
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
    // DIAG: dump first 16 bytes to see what we actually copied
    let head: Vec<u8> = data.iter().take(16).copied().collect();
    log::info!("dump frame {} first 16 bytes: {:?}", frame_id, head);
    // Strip the row padding into a contiguous RGBA buffer.
    let mut rgba = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        rgba.extend_from_slice(&data[start..end]);
    }
    drop(data);
    buffer.unmap();
    // The surface format is BGRA8UnormSrgb — swap to RGBA for PNG.
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
    if let Err(e) = image::save_buffer(
        &path,
        &rgba,
        width,
        height,
        image::ColorType::Rgba8,
    ) {
        log::warn!("failed to dump frame {}: {}", frame_id, e);
    }
}

/// A minimal but readable style for VersaTiles' OpenMapTiles schema. Tuned
/// for "looks like a map" — water blue, parks green, roads grey with
/// motorways slightly emphasised, buildings light grey, boundaries dark.
/// Water-only style for debugging the realistic-water surface in isolation:
/// just the water-body fills (which feed the water pipeline), nothing else.
/// Covers both schemas — OMT/kart-api "water" and VersaTiles "ocean" +
/// "water_polygons" (see `is_water_source_layer`).
fn water_only_style() -> VectorStyle {
    let water = |layer: &str| Rule {
        source_layer: layer.into(),
        filter: Filter::Always,
        paint: Paint::Fill {
            color: Color::rgb(0x9E, 0xC2, 0xDF),
        },
        min_zoom: 0,
        max_zoom: 22,
        interactive: false,
    };
    VectorStyle {
        background: Color::rgba(0, 0, 0, 0),
        rules: vec![
            water("water"),
            water("ocean"),
            water("water_polygons"),
        ],
    }
}

fn versatiles_demo_style() -> VectorStyle {
    // VersaTiles uses the "Shortbread" schema, not OpenMapTiles. Layer
    // names are pluralised (streets, buildings, boundaries, place_labels),
    // and properties use `kind` instead of `class`.
    VectorStyle {
        // Transparent background — the raster basemap underneath shows
        // through unless a vector rule paints a pixel.
        background: Color::rgba(0, 0, 0, 0),
        rules: vec![
            // Broad-area fills (land / sites / ocean / water_polygons) are
            // intentionally omitted: the Kartverket grey topo basemap
            // already covers them. The vector overlay's job here is to
            // contribute roads, boundaries, and labels on top.
            // Rivers / streams.
            Rule {
                source_layer: "water_lines".into(),
                filter: Filter::Always,
                paint: Paint::Line {
                    color: Color::rgb(0x9E, 0xC2, 0xDF),
                    width: 20.0,
                },
                min_zoom: 8,
                max_zoom: 22,
                interactive: false,
            },
            // Buildings (high zoom only).
            Rule {
                source_layer: "buildings".into(),
                filter: Filter::Always,
                paint: Paint::Fill {
                    color: Color::rgb(0xDC, 0xD2, 0xC1),
                },
                min_zoom: 14,
                max_zoom: 22,
                interactive: true,
            },
            // Streets: motorways/trunks emphasised, primary tier middle,
            // residential/service everything-else at high zoom.
            Rule {
                source_layer: "streets".into(),
                filter: Filter::In("kind".into(), vec!["motorway".into(), "trunk".into()]),
                paint: Paint::Line {
                    color: Color::rgb(0xE8, 0x9C, 0x4C),
                    width: 35.0,
                },
                min_zoom: 6,
                max_zoom: 22,
                interactive: true,
            },
            Rule {
                source_layer: "streets".into(),
                filter: Filter::In(
                    "kind".into(),
                    vec!["primary".into(), "secondary".into(), "tertiary".into()],
                ),
                paint: Paint::Line {
                    color: Color::rgb(0xCE, 0xB9, 0x8B),
                    width: 22.0,
                },
                min_zoom: 8,
                max_zoom: 22,
                interactive: true,
            },
            Rule {
                source_layer: "streets".into(),
                filter: Filter::Always,
                paint: Paint::Line {
                    color: Color::rgb(0xBD, 0xB3, 0xA1),
                    width: 12.0,
                },
                min_zoom: 11,
                max_zoom: 22,
                interactive: true,
            },
            // Country / state boundaries.
            Rule {
                source_layer: "boundaries".into(),
                filter: Filter::Always,
                paint: Paint::Line {
                    color: Color::rgb(0x70, 0x60, 0x60),
                    width: 10.0,
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            },
            // Place labels — Shortbread uses `kind` and pluralises the
            // layer name. country/state at low zoom, city/town/village at
            // higher zoom.
            Rule {
                source_layer: "place_labels".into(),
                filter: Filter::Eq("kind".into(), "country".into()),
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 18.0,
                    color: Color::rgb(0x33, 0x33, 0x33),
                    halo_color: Color::rgb(0xff, 0xff, 0xff),
                    halo_width: 1.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 1.3,
                },
                min_zoom: 2,
                max_zoom: 6,
                interactive: true,
            },
            Rule {
                source_layer: "place_labels".into(),
                filter: Filter::In("kind".into(), vec!["state".into(), "province".into()]),
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 14.0,
                    color: Color::rgb(0x44, 0x44, 0x44),
                    halo_color: Color::rgb(0xff, 0xff, 0xff),
                    halo_width: 1.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 1.3,
                },
                min_zoom: 4,
                max_zoom: 8,
                interactive: true,
            },
            Rule {
                source_layer: "place_labels".into(),
                filter: Filter::Eq("kind".into(), "city".into()),
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 15.0,
                    color: Color::rgb(0x22, 0x22, 0x22),
                    halo_color: Color::rgb(0xff, 0xff, 0xff),
                    halo_width: 1.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 1.3,
                },
                min_zoom: 6,
                max_zoom: 14,
                interactive: true,
            },
            Rule {
                source_layer: "place_labels".into(),
                filter: Filter::Eq("kind".into(), "town".into()),
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 12.0,
                    color: Color::rgb(0x33, 0x33, 0x33),
                    halo_color: Color::rgb(0xff, 0xff, 0xff),
                    halo_width: 1.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 1.3,
                },
                min_zoom: 9,
                max_zoom: 14,
                interactive: true,
            },
            Rule {
                source_layer: "place_labels".into(),
                filter: Filter::In(
                    "kind".into(),
                    vec!["village".into(), "suburb".into(), "neighbourhood".into()],
                ),
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 11.0,
                    color: Color::rgb(0x44, 0x44, 0x44),
                    halo_color: Color::rgb(0xff, 0xff, 0xff),
                    halo_width: 1.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 1.3,
                },
                min_zoom: 12,
                max_zoom: 14,
                interactive: true,
            },
            // Street labels — Shortbread has a separate layer for street
            // names, with `kind`=primary/secondary/etc.
            Rule {
                source_layer: "street_labels".into(),
                filter: Filter::Always,
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 10.0,
                    color: Color::rgb(0x55, 0x55, 0x55),
                    halo_color: Color::rgb(0xff, 0xff, 0xff),
                    halo_width: 1.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 0.7,
                },
                min_zoom: 14,
                max_zoom: 22,
                interactive: true,
            },
        ],
    }
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cache_root = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("turbomap");
    log::info!("tile caches under {}", cache_root.display());

    // Vector layer + style. With TURBO_BASEMAP_URL set (e.g.
    // http://localhost:8090), the demo renders OUR OWN N50 basemap with the
    // tileserver's served MapLibre style — the self-hosted path. Without it,
    // the VersaTiles OSM overlay + built-in demo style (the original MVP).
    let basemap_base = std::env::var("TURBO_BASEMAP_URL").ok();
    // Default to a WATER-ONLY style so the realistic-water surface can be debugged
    // in isolation (no roads/labels/buildings). Set TURBO_FULL_STYLE=1 for the
    // full road/label overlay.
    let full_style = std::env::var("TURBO_FULL_STYLE").is_ok();
    let (vector_source, style): (Arc<dyn VectorTileSource>, VectorStyle) = match &basemap_base {
        Some(base) => {
            log::info!("vector basemap from {base}/v1/basemap (MVT)");
            let src = turbomap_tiles_http::HttpVectorTileSource::turbo_basemap(base)
                .expect("build turbo basemap source")
                .with_cache_dir(cache_root.join("turbo-basemap"));
            let style = if full_style {
                let style_json =
                    turbomap_tiles_http::fetch_text(&format!("{base}/v1/basemap/style.json"))
                        .expect("fetch /v1/basemap/style.json");
                turbomap_style_maplibre::parse_style(&style_json)
                    .expect("parse served MapLibre style")
            } else {
                water_only_style()
            };
            (Arc::new(src), style)
        }
        None => (
            Arc::new(
                turbomap_tiles_http::HttpVectorTileSource::versatiles_osm()
                    .expect("build VersaTiles source")
                    .with_cache_dir(cache_root.join("versatiles-osm")),
            ),
            if full_style {
                versatiles_demo_style()
            } else {
                water_only_style()
            },
        ),
    };

    // Raster basemap: Kartverket grey topo by default (neutral, lets the vector
    // colours pop). Override with TURBO_RASTER_URL=<{z}/{y}/{x} template> for a
    // satellite/aerial bed under the water — e.g. Esri World Imagery:
    //   https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}
    let raster_source: Arc<dyn TileSource> = match std::env::var("TURBO_RASTER_URL").ok() {
        Some(tmpl) => {
            let lower = tmpl.to_lowercase();
            let fmt = if lower.contains("arcgis")
                || lower.contains("imagery")
                || lower.ends_with(".jpg")
                || lower.ends_with(".jpeg")
            {
                RasterFormat::Jpeg
            } else {
                RasterFormat::Png
            };
            log::info!("raster basemap from TURBO_RASTER_URL ({fmt:?}): {tmpl}");
            Arc::new(
                turbomap_tiles_http::HttpRasterSource::new(tmpl, "turbomap-app/0.1", 0, 19, fmt)
                    .expect("build raster source from TURBO_RASTER_URL"),
            )
        }
        None => Arc::new(
            turbomap_tiles_http::HttpRasterSource::kartverket_topo_grey()
                .expect("build Kartverket grey source"),
        ),
    };

    // Optional GPU hillshade from our own tileserver's
    // `/v1/dem/rgb/{z}/{x}/{y}.png` endpoint. Set TURBO_API_URL to
    // enable (e.g. http://localhost:8080).
    let dem_source: Option<Arc<dyn TileSource>> =
        std::env::var("TURBO_API_URL").ok().and_then(|base| {
            log::info!("enabling hillshade layer from {base}/v1/dem/rgb/{{z}}/{{x}}/{{y}}.png");
            turbomap_tiles_http::HttpRasterSource::turbo_terrain_rgb(&base)
                .map(|s| Arc::new(s) as Arc<dyn TileSource>)
                .map_err(|e| log::warn!("DEM source build failed: {e}"))
                .ok()
        });

    let initial_camera = Camera::new(LatLng::new(60.39, 5.32), 11.0);
    let mut app = TurbomapApp::new(
        raster_source,
        dem_source,
        vector_source,
        style,
        initial_camera,
    );

    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    // `Poll` keeps the runloop ticking even when no OS events are
    // queued, so tile-fetch worker completions get drained promptly
    // even before the user moves the mouse. `Wait` (the previous
    // setting) starved the redraw cycle on macOS during cold startup
    // — workers completed in ~400 ms but the main thread never saw
    // their channel messages until something else woke the loop.
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).expect("run_app");
}
