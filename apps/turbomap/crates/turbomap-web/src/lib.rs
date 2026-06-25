//! wasm-bindgen web host for the turbomap engine.
//!
//! The third host after Android (JNI `surface.rs`) and the desktop winit app
//! (`turbomap-app`). All three drive the SAME [`TurbomapEngine`] over the same
//! control plane — scene in, host-driven tile IO, `render()` per frame — so the
//! browser runs the device's exact render code paths, just behind a `<canvas>`
//! + WebGPU instead of an `ANativeWindow` + Vulkan/GL.
//!
//! Design mirrors the Android surface glue, minus the threading: the browser is
//! single-threaded, so there is no command queue / snapshot — JS calls these
//! methods directly on the (one) main thread and drives the rAF render loop
//! itself. Tile IO is host-driven: JS reads [`TurboMap::pending_tiles`], fetches
//! via `fetch()` (it owns auth/caching/offline), and pushes bytes back through
//! the `ingest_*` methods, exactly like the Kotlin/Swift hosts.
//!
//! The whole crate is `wasm32`-only (browser GPU + DOM); on native it is an
//! empty lib so the workspace build stays green.

#![cfg(target_arch = "wasm32")]

use std::sync::Arc;

use wasm_bindgen::prelude::*;

use turbomap_core::{Camera as CoreCamera, LatLng as CoreLatLng, MapOptions, PendingTile, TileId};
use turbomap_engine::{CameraState, HostDrivenResolver, MapEngine, TurbomapEngine};
use turbomap_scene::{LatLng, Scene, ScreenPoint};

/// One-time process init: route Rust panics to the browser console with a
/// readable message + stack instead of an opaque `unreachable executed` trap.
/// Idempotent — safe to call from JS before constructing a map.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    // `console_log` would be nicer, but `log` → console isn't wired here yet;
    // wgpu's own errors already surface via the panic hook + GPU validation.
}

/// A presenting turbomap bound to a browser `<canvas>`. Constructed async
/// (WebGPU adapter/device acquisition are promises), then driven frame-by-frame
/// from a JS `requestAnimationFrame` loop.
#[wasm_bindgen]
pub struct TurboMap {
    surface: wgpu::Surface<'static>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,
    /// sRGB view format the engine renders to (surface itself may be non-sRGB).
    render_format: wgpu::TextureFormat,
    engine: TurbomapEngine,
    width: u32,
    height: u32,
    /// Bottom viewport inset (px) — reserved for a UI sheet, same as Android.
    inset: f64,
    /// Right viewport inset (px) — reserved for a desktop side panel.
    inset_right: f64,
}

#[wasm_bindgen]
impl TurboMap {
    /// Build a map presenting into `canvas`, sized `width`×`height` px, centred
    /// at `lat`/`lng`/`zoom` (flat, north-up). Returns a rejected promise (JS
    /// error) if WebGPU is unavailable or the surface can't be created — the
    /// host shows an "unsupported browser" notice rather than a blank canvas.
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
        width: u32,
        height: u32,
        lat: f64,
        lng: f64,
        zoom: f64,
    ) -> Result<TurboMap, JsValue> {
        // WebGPU first; WebGL2 as a degraded fallback (advanced terrain/water/
        // cloud shaders may not run there — WebGPU is the supported target).
        let instance = wgpu::Instance::new({
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
            desc.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
            desc
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| js_err(format!("create_surface failed: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| js_err(format!("no compatible GPU adapter (WebGPU unavailable?): {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("turbomap-web-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| js_err(format!("request_device failed: {e}")))?;
        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let caps = surface.get_capabilities(&adapter);
        // The renderer blends in linear and relies on an **sRGB target** to
        // gamma-encode its result. Browsers expose the canvas surface as a
        // NON-sRGB format (Chrome WebGPU offers only `Bgra8Unorm`), so we
        // configure the surface with that base format but render through an
        // sRGB **view** (a permitted format reinterpretation). Without this the
        // frame is presented un-encoded (linear) and looks much darker than the
        // native (Vulkan/Metal) sRGB surfaces — the "darker than mobile" bug.
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f.remove_srgb_suffix(),
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .unwrap_or(caps.formats[0]);
        // The view format we actually render to (always sRGB so the encode runs).
        let render_format = surface_format.add_srgb_suffix();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            // Allow creating the sRGB view of the (possibly non-sRGB) surface.
            view_formats: vec![render_format],
        };
        surface.configure(&device, &config);

        // Same feel knobs as Android: fade tiles in instead of popping, and warm
        // a modest off-screen prefetch ring so a pan lands on ready tiles.
        let options = MapOptions {
            fade_in_secs: 0.3,
            prefetch_margin_px: 256,
            ..MapOptions::default()
        };
        let camera = CameraState {
            center: LatLng::new(lat, lng),
            zoom,
            pitch_deg: 0.0,
            bearing_deg: 0.0,
        };
        let engine = TurbomapEngine::new(
            device.clone(),
            queue.clone(),
            render_format,
            (config.width, config.height),
            camera,
            options,
            Box::new(HostDrivenResolver),
        )
        .map_err(|e| js_err(format!("engine init failed: {e}")))?;

        Ok(TurboMap {
            surface,
            device,
            queue,
            render_format,
            engine,
            width: config.width,
            height: config.height,
            config,
            inset: 0.0,
            inset_right: 0.0,
        })
    }

    /// Replace the whole map state with a Scene-IR JSON document. The engine
    /// diffs against the previous scene and does minimal GPU work. Throws on
    /// invalid JSON / scene.
    pub fn apply_scene(&mut self, scene_json: &str) -> Result<(), JsValue> {
        let scene: Scene =
            serde_json::from_str(scene_json).map_err(|e| js_err(format!("invalid scene: {e}")))?;
        scene
            .validate()
            .map_err(|e| js_err(format!("invalid scene: {e}")))?;
        self.engine.apply(scene);
        Ok(())
    }

    /// Drain sources that need no IO (inline GeoJSON) in-process. Remote tiles
    /// are untouched — they stay in [`pending_tiles`](Self::pending_tiles).
    pub fn pump_local_tiles(&mut self) {
        self.engine.pump_tiles();
    }

    /// Tiles the engine is waiting on, as a JSON array of
    /// `{"kind","layer","z","x","y"}` — the host fetches each and pushes the
    /// bytes back via the matching `ingest_*`. `kind` is raster/hillshade/
    /// vector/terrain; `layer` is `__terrain` for the shared DEM.
    pub fn pending_tiles(&self) -> String {
        let items: Vec<String> = self
            .engine
            .pending_tiles()
            .into_iter()
            .map(|p| {
                let (kind, layer, t) = match p {
                    PendingTile::Raster { layer_id, tile } => ("raster", layer_id, tile),
                    PendingTile::Hillshade { layer_id, tile } => ("hillshade", layer_id, tile),
                    PendingTile::Vector { layer_id, tile } => ("vector", layer_id, tile),
                    PendingTile::Terrain { tile } => ("terrain", "__terrain".to_string(), tile),
                };
                format!(
                    "{{\"kind\":\"{kind}\",\"layer\":\"{layer}\",\"z\":{},\"x\":{},\"y\":{}}}",
                    t.z, t.x, t.y
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Push a fetched raster tile (encoded PNG/JPEG/WebP, exactly as served).
    /// Returns `false` if the bytes don't decode.
    pub fn ingest_raster_tile(&mut self, layer: &str, z: u8, x: u32, y: u32, bytes: &[u8]) -> bool {
        self.engine
            .ingest_raster_encoded(layer, TileId::new(z, x, y), bytes)
    }

    /// Push a fetched DEM tile (encoded Terrain-RGB / Terrarium image).
    pub fn ingest_terrain_tile(&mut self, z: u8, x: u32, y: u32, bytes: &[u8]) -> bool {
        self.engine.ingest_terrain_encoded(TileId::new(z, x, y), bytes)
    }

    /// Push a fetched vector tile (raw MVT protobuf bytes).
    pub fn ingest_vector_tile(&mut self, layer: &str, z: u8, x: u32, y: u32, bytes: &[u8]) -> bool {
        self.engine
            .ingest_mvt(layer, TileId::new(z, x, y), bytes)
    }

    /// Render one frame to the canvas. Advances any in-flight camera animation
    /// first (physics is wall-clock based). Call this from a `requestAnimation
    /// Frame` loop; pair with [`is_animating`](Self::is_animating) to park the
    /// loop when nothing is moving (render-on-demand).
    pub fn render(&mut self) {
        self.engine.tick_now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                    _ => return,
                }
            }
            _ => return,
        };
        // Render through an sRGB view so the engine's linear output is
        // gamma-encoded on write (the surface itself may be non-sRGB on web).
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.render_format),
            ..Default::default()
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.engine.render(&mut encoder, &view);
        self.queue.submit([encoder.finish()]);
        frame.present();
        self.engine.after_submit();
    }

    /// True while a camera animation or tile fade-in is running — keep drawing
    /// until it goes false, then park the rAF loop (render-on-demand).
    pub fn is_animating(&self) -> bool {
        self.engine.is_animating()
    }

    /// Advance camera animation without drawing. `true` = still animating.
    pub fn tick(&mut self) -> bool {
        self.engine.tick_now()
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.config.width = self.width;
        self.config.height = self.height;
        self.surface.configure(&self.device, &self.config);
        self.engine.resize(self.config.width, self.config.height);
    }

    /// Reserve `bottom_px` at the bottom of the viewport (e.g. a UI sheet) so
    /// the projection + rendered frame shift up into the visible band.
    pub fn set_viewport_inset(&mut self, bottom_px: f64) {
        self.inset = bottom_px.max(0.0);
        self.engine.set_viewport_inset(self.inset);
    }

    /// Reserve `right_px` at the right of the viewport (a desktop side panel) so
    /// the projection + rendered frame shift left into the visible band — keeps
    /// the focus from hiding behind the panel.
    pub fn set_viewport_inset_right(&mut self, right_px: f64) {
        self.inset_right = right_px.max(0.0);
        self.engine.set_viewport_inset_right(self.inset_right);
    }

    pub fn set_camera(&mut self, lat: f64, lng: f64, zoom: f64, pitch_deg: f64, bearing_deg: f64) {
        self.engine.set_camera(CameraState {
            center: LatLng::new(lat, lng),
            zoom,
            pitch_deg,
            bearing_deg,
        });
    }

    /// Current camera as JSON `{"lat","lng","zoom","pitch","bearing"}`.
    pub fn camera_json(&self) -> String {
        let c = self.engine.camera();
        format!(
            "{{\"lat\":{},\"lng\":{},\"zoom\":{},\"pitch\":{},\"bearing\":{}}}",
            c.center.lat, c.center.lng, c.zoom, c.pitch_deg, c.bearing_deg
        )
    }

    /// One-finger pan step: translate the camera by a screen-space delta (px).
    /// Ground-plane unproject, consistent under pitch — same math as Android's
    /// `PanByPixels` so 2D/3D panning feels identical across hosts.
    pub fn pan_by_pixels(&mut self, dx: f64, dy: f64) {
        let cs = self.engine.camera();
        let cam = CoreCamera::new(CoreLatLng::new(cs.center.lat, cs.center.lng), cs.zoom)
            .with_pitch(cs.pitch_deg)
            .with_bearing(cs.bearing_deg)
            .with_viewport_inset(self.inset)
            .with_viewport_inset_right(self.inset_right);
        let vp = (self.config.width as f64, self.config.height as f64);
        // Pan relative to where the camera centre actually projects (the inset
        // shifts the principal point), so the inset cancels and a drag is a pure
        // screen-space delta regardless of any open panel/sheet.
        let target = cam
            .pixel_to_world(
                (vp.0 / 2.0 - self.inset_right / 2.0 - dx, vp.1 / 2.0 - self.inset / 2.0 - dy),
                vp,
            )
            .to_lat_lng();
        let mut c = cs;
        c.center = LatLng::new(target.lat, target.lng);
        self.engine.set_camera(c);
    }

    /// One focus-invariant zoom step by `factor` about screen pixel `(fx, fy)`
    /// — the wheel/pinch zoom. The world point under the focus stays put.
    pub fn zoom_around(&mut self, factor: f64, fx: f64, fy: f64) {
        self.engine.zoom_around(factor, (fx, fy));
    }

    /// Animated focus-invariant zoom by `factor` about `(fx, fy)` over
    /// `duration_ms` (eased). The smooth wheel / +/- button zoom — driven by
    /// `render`/`tick`; the focus world-point stays put, like `zoom_around`.
    pub fn zoom_around_animated(&mut self, factor: f64, fx: f64, fy: f64, duration_ms: u32) {
        self.engine.zoom_around_animated(
            factor,
            (fx, fy),
            std::time::Duration::from_millis(duration_ms as u64),
        );
    }

    /// Start an inertial pan fling at drag-release velocity `(vx, vy)` in
    /// screen px/s (same sign convention as [`pan_by_pixels`](Self::pan_by_pixels)).
    /// Driven by `render`/`tick`; a subsequent pan/zoom cancels it. This is the
    /// physics-swipe momentum, matching Android's tuned fling.
    pub fn fling(&mut self, vx: f64, vy: f64) {
        self.engine.fling((vx, vy));
    }

    /// Start a zoom fling (pinch-release momentum) at `zoom_velocity`
    /// (zoom-levels/s) about screen pixel `(fx, fy)`. Driven by `render`/`tick`.
    pub fn zoom_fling(&mut self, zoom_velocity: f64, fx: f64, fy: f64) {
        self.engine.zoom_fling(zoom_velocity, (fx, fy));
    }

    /// One 3D-mode orbit step: rotate bearing by `d_bearing_deg` + tilt by
    /// `d_pitch_deg`, both pivoting about focus pixel `(fx, fy)`.
    pub fn orbit_around(&mut self, d_bearing_deg: f64, d_pitch_deg: f64, fx: f64, fy: f64) {
        self.engine.rotate_around(d_bearing_deg, (fx, fy));
        self.engine.pitch_around(d_pitch_deg, (fx, fy));
    }

    /// Ease the camera to a target pose over `duration_ms` (accel/decel). Keep
    /// rendering while [`is_animating`](Self::is_animating) is true.
    pub fn ease_to(&mut self, lat: f64, lng: f64, zoom: f64, bearing_deg: f64, duration_ms: u32) {
        let mut target = self.engine.camera();
        target.center = LatLng::new(lat, lng);
        target.zoom = zoom;
        target.bearing_deg = bearing_deg;
        self.engine
            .ease_to(target, std::time::Duration::from_millis(duration_ms as u64));
    }

    /// Geo → screen. Returns `[x, y]` px, or `null` if the point is off-globe.
    pub fn project(&self, lat: f64, lng: f64) -> Option<Vec<f64>> {
        self.engine
            .project(LatLng::new(lat, lng))
            .map(|p| vec![p.x, p.y])
    }

    /// Screen → geo. Returns `[lat, lng]`, or `null` if the pixel hits no ground.
    pub fn unproject(&self, x: f64, y: f64) -> Option<Vec<f64>> {
        self.engine
            .unproject(ScreenPoint::new(x, y))
            .map(|g| vec![g.lat, g.lng])
    }

    /// Enable terrain cast shadows at `strength` in `[0,1]` (0 = off).
    pub fn set_terrain_shadows(&mut self, strength: f32) {
        self.engine.set_terrain_shadows(strength);
    }

    /// Basemap brightness gain for the 3D sun-lit terrain (1.0 = unchanged).
    /// The web host raises it for dark imagery (satellite) so it reads under the
    /// same lighting that suits bright topo. No effect on the flat 2D map.
    pub fn set_basemap_gain(&mut self, gain: f32) {
        self.engine.set_basemap_gain(gain);
    }

    /// Drive sun lighting from a unix timestamp (seconds), or `null` for the
    /// default fixed sun.
    pub fn set_sun_time(&mut self, unix_secs: Option<f64>) {
        self.engine.set_sun_time(unix_secs);
    }
}

fn js_err(msg: String) -> JsValue {
    JsValue::from_str(&msg)
}
