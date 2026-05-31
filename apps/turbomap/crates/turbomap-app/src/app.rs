//! winit `ApplicationHandler` driving the vector map. The body
//! here is intentionally thin: anything substantive lives in
//! one of `gpu`, `surface`, `schedule`, `map_host`, or `ui`.

use std::sync::Arc;
use std::time::Instant;

use turbomap_core::{
    Camera, Color, Filter, HitResult, LatLng, Map, MapOptions, Marker, MarkerId, Paint, Rule,
    TileSource, VectorStyle, VectorTileSource,
};

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
}

/// UI-bound state. Cached so the panel survives across frames; the
/// map's actual layer fade / visibility is the source of truth and
/// this struct just mirrors it for the slider widgets.
#[derive(Debug, Clone)]
struct UiState {
    raster_visible: bool,
    hillshade_visible: bool,
    vector_visible: bool,
    fade_in_secs: f32,
    /// Smoothed + sampled frame-timing display. The raw
    /// per-frame numbers change every render — at 120 Hz
    /// vsync that's 120 text-width changes per second which
    /// rearranges the panel layout and makes the drop shadow
    /// jitter. We sample at ~5 Hz and pad the formatted
    /// string to a fixed width so the panel is layout-stable.
    metrics_display: MetricsDisplay,
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
            hillshade_visible: true,
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

        // Drop a couple of demo markers on Norwegian cities so the user can
        // see hit-testing fire for both features and markers.
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

        // 4. Render the map onto the drawable.
        self.host.render(&mut encoder, &frame.view);

        // 5. egui on top. The borrow-checker keeps `Map` /
        //    `ui_state` / the panel closure alive across the
        //    egui frame in one place; `ui.frame` returns a
        //    `PendingUi` that we must hand back to
        //    `ui.present` AFTER the queue submit so the GPU
        //    isn't still sampling textures we're freeing.
        let ui_state = &mut self.ui_state;
        let map_for_ui = self.host.map_mut();
        let pending = self.ui.frame(
            &self.gpu.device,
            &self.gpu.queue,
            &self.window,
            &mut encoder,
            &frame.view,
            frame.size,
            |ctx| build_ui(ctx, ui_state, map_for_ui),
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
fn build_ui(ctx: &egui::Context, ui: &mut UiState, map: &mut Map) {
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
    egui::Window::new("turbomap")
        .default_pos([12.0, 12.0])
        .resizable(false)
        .collapsible(true)
        .frame(frame)
        .show(ctx, |panel| {
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
            panel.checkbox(&mut ui.raster_visible, "basemap (Kartverket grey topo)");
            panel.checkbox(&mut ui.hillshade_visible, "hillshade (turbo DEM)");
            panel.checkbox(&mut ui.vector_visible, "vector (VersaTiles OSM)");
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
        wgpu::ImageCopyTexture {
            texture: src,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
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
        device.poll(wgpu::Maintain::Poll);
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

    // Vector overlay: VersaTiles OSM with disk caching.
    let vector_source: Arc<dyn VectorTileSource> = Arc::new(
        turbomap_tiles_http::HttpVectorTileSource::versatiles_osm()
            .expect("build VersaTiles source")
            .with_cache_dir(cache_root.join("versatiles-osm")),
    );

    // Raster basemap: Kartverket Norgeskart in the grey topo style — a
    // light, neutral basemap that lets the vector overlay's colours pop.
    let raster_source: Arc<dyn TileSource> = Arc::new(
        turbomap_tiles_http::HttpRasterSource::kartverket_topo_grey()
            .expect("build Kartverket grey source"),
    );

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

    let style = versatiles_demo_style();
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
