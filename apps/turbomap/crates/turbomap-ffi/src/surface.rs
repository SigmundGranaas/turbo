//! On-screen render path for Android.
//!
//! uniffi carries the control plane, but a `wgpu::Surface` needs an
//! `ANativeWindow`, which can't cross uniffi. This module is the small
//! hand-written JNI glue that bridges a Java `Surface` to a presenting
//! [`TurbomapEngine`]: the host (a `SurfaceView`/`Choreographer` loop, or an
//! `ImageReader` in tests) calls these `native*` functions.
//!
//! Methods take a raw `jlong` handle to a boxed [`OnScreen`]; every entry point
//! catches unwinds so a Rust panic can never propagate across the FFI boundary.

// This module is, by nature, raw JNI + raw-handle wgpu surface creation — the
// workspace's `unsafe_code = "warn"` doesn't fit a hand-written FFI boundary.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jdouble, jfloat, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use turbomap_core::{Camera, LatLng as CoreLatLng, MapOptions, PendingTile, TileId};
use turbomap_engine::{CameraState, HostDrivenResolver, MapEngine, TurbomapEngine};
use turbomap_scene::{LatLng, Scene, ScreenPoint};

use crate::trace::{frame_trace, stats_json, FrameTrace};

/// Last failure reason (GPU/surface init or a caught panic), surfaced to the
/// host via `nativeLastError` so the engine **fails loudly** — there is no
/// MapLibre fallback by design, so a failure must be reported, never hidden
/// behind a silent blank.
static LAST_ERROR: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = Some(msg.into());
    }
}

/// Live surfaces keyed by an opaque integer handle. We hand the HOST an id, not
/// a raw `Box` pointer: when a surface is destroyed its entry is removed, so a
/// stale handle — a JNI call racing surface teardown (rotation / activity
/// recreate), which used to dereference freed memory and SIGSEGV — now resolves
/// to "not found" and safely no-ops. Each call clones the `Arc`, so an in-flight
/// call keeps the surface alive even if `nativeDestroy` removes it concurrently.
static SURFACES: LazyLock<Mutex<HashMap<u64, Arc<Surface>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// Route the `log` facade (ours + wgpu's) to logcat once, so GPU/pipeline
/// errors and render-path diagnostics are visible on device. No-op off Android.
fn init_logging() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("Turbomap"),
        );
        install_panic_hook();
        log::info!("turbomap logging initialised");
    });
}

/// Route Rust panics to logcat *with location + backtrace*, at the panic site.
///
/// The `catch_unwind` firewalls (`with_map`, `nativeCreate`) recover the
/// message so a caught panic doesn't wedge the map — but by the time they run,
/// the stack has unwound and the origin is gone. This hook fires first, while
/// the panicking frame is still live, and logs file:line + a forced backtrace
/// to logcat (tag `Turbomap`). That's the device-side backtrace the crash hunt
/// kept lacking: `adb logcat -s Turbomap:E` now shows exactly where a panic
/// originated, not just that one happened.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("<non-string panic payload>");
        // `force_capture` ignores RUST_BACKTRACE (unset on devices) so we always
        // get frames; symbolication depends on the shipped symbols, but the
        // address trace + location already pin most bugs.
        let backtrace = std::backtrace::Backtrace::force_capture();
        log::error!("RUST PANIC at {loc}: {msg}\nbacktrace:\n{backtrace}");
    }));
}

/// Best-effort string for a caught panic payload.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic (non-string payload)".to_string())
}

/// A presenting map: the wgpu surface + the engine that draws into it. The
/// `NativeWindow` is held to keep the `ANativeWindow` reference alive.
struct OnScreen {
    surface: wgpu::Surface<'static>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    config: wgpu::SurfaceConfiguration,
    engine: TurbomapEngine,
    /// Last viewport bottom-inset applied (px). Mirrored here so the published
    /// snapshot can reconstruct the projection for wait-free, always-valid
    /// project/unproject without touching the engine.
    inset: f64,
    /// Per-frame perf diagnostics (render-thread-owned, no contention).
    perf: FramePerf,
    _instance: wgpu::Instance,
    _window: NativeWindow,
}

/// Lightweight render-thread perf counters, logged to logcat (tag `Turbomap`)
/// so device-side stalls are visible without a profiler. A summary line every
/// ~1 s, plus an immediate line on any frame that blows the budget — so a
/// freeze is captured at the exact frame it happens.
#[derive(Default)]
struct FramePerf {
    frame_idx: u64,
    last_frame_at: Option<std::time::Instant>,
    last_log_at: Option<std::time::Instant>,
    // Accumulated since the last summary log.
    acc_frames: u32,
    acc_ingested: u32,
    acc_render_ms: f64,
    acc_ingest_ms: f64,
    max_gap_ms: f64,
    /// Wall-time gap since the previous frame (ms) — the inter-frame jitter the
    /// structured trace reports. Set every `record()`; read when building the
    /// per-frame trace published in the snapshot.
    last_gap_ms: f64,
}

fn build(
    window: NativeWindow,
    width: u32,
    height: u32,
    camera: CameraState,
) -> Result<OnScreen, String> {
    let instance = wgpu::Instance::new({
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        desc.backends = wgpu::Backends::PRIMARY | wgpu::Backends::GL;
        // Same crash-avoidance as the offscreen path: never request Vulkan
        // debug-utils (the emulator driver SIGSEGVs in vkSetDebugUtilsObjectName).
        desc.flags
            .remove(wgpu::InstanceFlags::DEBUG | wgpu::InstanceFlags::VALIDATION);
        desc
    });

    let window_handle = AndroidNdkWindowHandle::new(window.ptr().cast());
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::AndroidNdk(window_handle),
    };
    // Safety: `window` outlives `surface` (both owned by the returned struct),
    // and the handle points at a live ANativeWindow we hold a reference to.
    let surface = unsafe { instance.create_surface_unsafe(target) }
        .map_err(|e| format!("create_surface failed: {e}"))?;

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .map_err(|e| format!("no compatible GPU adapter: {e}"))?;

    let caps = surface.get_capabilities(&adapter);
    // Decision 3: prefer an sRGB surface format (colours are blended in linear,
    // re-encoded by the *Srgb target); fall back to whatever the surface offers.
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or_else(|| caps.formats[0]);

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("turbomap-surface-device"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        memory_hints: wgpu::MemoryHints::Performance,
        experimental_features: wgpu::ExperimentalFeatures::default(),
        trace: wgpu::Trace::Off,
    }))
    .map_err(|e| format!("request_device failed: {e}"))?;
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    // Fade newly-ingested tiles in over ~0.3 s instead of popping. The host keeps
    // rendering (render-on-demand) while `is_animating` is true so the fade
    // completes. Goldens keep the default 0 (deterministic, no time dependence).
    //
    // A MODERATE off-screen prefetch ring (~half a screen of warm tiles). Big enough that a
    // pan reveals ready tiles, small enough that first load doesn't flood the decoder with a
    // huge burst that all cross-fades at once ("tiles loading over each other" + frame drops).
    // 512 px proved too aggressive on cold first load; 256 keeps panning warm without the chaos.
    let options = MapOptions {
        fade_in_secs: 0.3,
        prefetch_margin_px: 256,
        ..MapOptions::default()
    };
    let engine = TurbomapEngine::new(
        device.clone(),
        queue.clone(),
        format,
        (config.width, config.height),
        camera,
        options,
        Box::new(HostDrivenResolver),
    )
    .map_err(|e| format!("engine init failed: {e}"))?;

    Ok(OnScreen {
        surface,
        device,
        queue,
        config,
        engine,
        inset: 0.0,
        perf: FramePerf::default(),
        _instance: instance,
        _window: window,
    })
}

impl OnScreen {
    fn render(&mut self) {
        // Advance any in-flight camera animation (fling / ease / zoom-fling) to
        // the current wall-clock before drawing — physics is time-based, so each
        // rendered frame samples it forward. No-op when nothing is animating.
        self.engine.tick_now();
        use wgpu::CurrentSurfaceTexture;
        let frame = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(t) | CurrentSurfaceTexture::Suboptimal(t) => t,
            CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
                // Lost/outdated (rotate, background, resize): reconfigure and try
                // once more; if it still isn't ready, skip this frame.
                self.surface.configure(&self.device, &self.config);
                match self.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(t) | CurrentSurfaceTexture::Suboptimal(t) => t,
                    _ => return,
                }
            }
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.engine.render(&mut encoder, &view);
        self.queue.submit([encoder.finish()]);
        frame.present();
        self.engine.after_submit();
        // Reclaim deferred GPU resources. When the tile caches evict under
        // budget (every pan/zoom drops textures + bind groups), wgpu queues the
        // backing GPU memory for destruction and only frees it on a device
        // maintain pass — `present()` does NOT guarantee one on Vulkan/GLES. A
        // long session of tile churn would otherwise pile up freed-but-not-
        // reclaimed textures until the device OOMs ("crashes after long use").
        // `Poll` is non-blocking: it runs cleanup for already-completed work
        // without stalling the frame.
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.engine.resize(self.config.width, self.config.height);
    }

    /// Apply one queued control mutation to the engine (render thread only).
    fn apply_cmd(&mut self, cmd: Cmd) {
        use std::time::Duration;
        match cmd {
            Cmd::SetCamera {
                lat,
                lng,
                zoom,
                bearing,
            } => {
                let mut c = self.engine.camera();
                c.center = LatLng::new(lat, lng);
                c.zoom = zoom;
                c.bearing_deg = bearing;
                self.engine.set_camera(c);
            }
            Cmd::PanByPixels { dx, dy } => {
                // Ground-plane pan against the LIVE camera on the render thread, so
                // sequential queued pans compose (each sees the previous result).
                // Must stay inset-neutral: the old inline `pixel_to_world` round-trip
                // baked the bottom viewport inset into the sample point, adding a
                // constant northward `inset/2` px drift PER MOVE EVENT — with the
                // record/follow sheet's inset active, a single swipe (dozens of
                // events) compounded that into the map flying away.
                self.engine.pan_by_pixels(dx, dy);
            }
            Cmd::Fling(vx, vy) => self.engine.fling((vx, vy)),
            Cmd::ZoomFling { v, fx, fy } => self.engine.zoom_fling(v, (fx, fy)),
            Cmd::EaseTo {
                lat,
                lng,
                zoom,
                bearing,
                dur_ms,
            } => {
                let mut target = self.engine.camera();
                target.center = LatLng::new(lat, lng);
                target.zoom = zoom;
                target.bearing_deg = bearing;
                self.engine.ease_to(target, Duration::from_millis(dur_ms));
            }
            Cmd::EasePitch { pitch, dur_ms } => {
                let mut target = self.engine.camera();
                target.pitch_deg = pitch;
                self.engine.ease_to(target, Duration::from_millis(dur_ms));
            }
            Cmd::ZoomAroundAnimated {
                factor,
                fx,
                fy,
                dur_ms,
            } => {
                self.engine
                    .zoom_around_animated(factor, (fx, fy), Duration::from_millis(dur_ms));
            }
            Cmd::ZoomAround { factor, fx, fy } => self.engine.zoom_around(factor, (fx, fy)),
            Cmd::OrbitAround { db, dp, fx, fy } => {
                self.engine.rotate_around(db, (fx, fy));
                self.engine.pitch_around(dp, (fx, fy));
            }
            Cmd::CancelAnimation => {
                let here = self.engine.camera();
                self.engine.set_camera(here);
            }
            Cmd::SetViewportInset(px) => {
                self.inset = px;
                self.engine.set_viewport_inset(px);
            }
            Cmd::IngestRadar {
                slot,
                w,
                h,
                precip,
                coverage,
            } => {
                self.engine
                    .ingest_radar_frame(slot, w, h, &precip, &coverage);
            }
            Cmd::SetCloudTime { time, blend } => self.engine.set_cloud_time(time, blend),
            Cmd::ApplyScene(scene) => {
                self.engine.apply(*scene);
            }
            Cmd::FetchFailed(id) => {
                self.engine.fetch_failed(turbomap_world::RequestId(id));
            }
            Cmd::FetchCancelled(id) => {
                self.engine.fetch_cancelled(turbomap_world::RequestId(id));
            }
            Cmd::PumpTiles => {
                self.engine.pump_tiles();
            }
            Cmd::Resize { w, h } => self.resize(w, h),
        }
    }

    /// Decode + upload one fetched tile (render thread only; rate-limited).
    fn apply_ingest(&mut self, ingest: Ingest) {
        match ingest {
            Ingest::Raster { layer, tile, bytes } => {
                self.engine.ingest_raster_encoded(&layer, tile, &bytes);
            }
            Ingest::Terrain { tile, bytes } => {
                self.engine.ingest_terrain_encoded(tile, &bytes);
            }
            Ingest::Vector { layer, tile, bytes } => {
                self.engine.ingest_mvt(&layer, tile, &bytes);
            }
        }
    }

    /// Build the immutable read model the UI loads wait-free. Tile work is
    /// NOT in here — it rides the plan outbox (consume-once), not a
    /// republished snapshot.
    fn build_snapshot(&self) -> Snapshot {
        Snapshot {
            cam: self.engine.camera(),
            viewport: (self.config.width as f64, self.config.height as f64),
            inset: self.inset,
            animating: self.engine.is_animating(),
            pending_count: 0,
            stats_json: stats_json(&self.engine),
        }
    }
}

impl FramePerf {
    /// Accumulate one frame; log a summary ~1×/s and an immediate line on any
    /// frame that exceeds the 16.7 ms vsync budget (the stalls the user feels).
    fn record(
        &mut self,
        render_ms: f64,
        ingest_ms: f64,
        ingested: u32,
        backlog: usize,
        pending: u32,
    ) {
        let now = std::time::Instant::now();
        let gap_ms = self
            .last_frame_at
            .map(|t| (now - t).as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        self.last_frame_at = Some(now);
        self.last_gap_ms = gap_ms;
        self.frame_idx += 1;
        self.acc_frames += 1;
        self.acc_ingested += ingested;
        self.acc_render_ms += render_ms;
        self.acc_ingest_ms += ingest_ms;
        if gap_ms > self.max_gap_ms {
            self.max_gap_ms = gap_ms;
        }

        let frame_ms = render_ms + ingest_ms;
        if frame_ms > 16.7 {
            log::warn!(
                "SLOW frame #{}: total={:.1}ms (render={:.1} ingest={:.1}, {} tiles), backlog={}, pending={}",
                self.frame_idx, frame_ms, render_ms, ingest_ms, ingested, backlog, pending
            );
        }

        let due = self
            .last_log_at
            .map(|t| (now - t).as_secs_f64() >= 1.0)
            .unwrap_or(true);
        if due {
            let n = self.acc_frames.max(1) as f64;
            log::info!(
                "PERF {:.0} fps | render avg {:.1}ms | ingest avg {:.1}ms ({} tiles/s) | maxgap {:.0}ms | backlog={} pending={}",
                self.acc_frames as f64
                    / self.last_log_at.map(|t| (now - t).as_secs_f64()).unwrap_or(1.0).max(0.001),
                self.acc_render_ms / n,
                self.acc_ingest_ms / n,
                self.acc_ingested,
                self.max_gap_ms,
                backlog,
                pending,
            );
            self.last_log_at = Some(now);
            self.acc_frames = 0;
            self.acc_ingested = 0;
            self.acc_render_ms = 0.0;
            self.acc_ingest_ms = 0.0;
            self.max_gap_ms = 0.0;
        }
    }
}

// The structured per-frame trace (`FrameTrace`, `frame_trace`, `stats_json`)
// lives in the ungated `crate::trace` module so its pure serialization is
// host-compiled + unit-tested (this `surface` module is Android-only).

// ──────────────────────────────────────────────────────────────────────────
// Thread model — eliminate the ANR bug class by *design*, not by patching.
//
// The map runs on two threads: the dedicated render/frame loop, and the UI
// thread (gestures, overlay projection, the tile reconciler). The old design
// shared `Mutex<OnScreen>` between them, so any UI call could block for a whole
// frame while the render thread held the lock (e.g. during the CPU shadow
// march) → input dispatch timeout → ANR ("froze then crashed").
//
// Here the UI thread NEVER touches the engine and NEVER waits on a render frame:
//   • Mutations are wait-free *commands* pushed onto lock-free channels and
//     applied by the render thread at the top of the next frame.
//   • Reads load an immutable [`Snapshot`] the render thread republishes after
//     each frame (the brief `Mutex<Arc<Snapshot>>` only ever guards a pointer
//     swap — zero work under it, so it cannot accumulate to a stall).
// The engine (`OnScreen`) is owned solely by the render thread + rare lifecycle
// calls (`render`/`resize`), serialised by `render: Mutex`. The UI hot path
// touches none of it, so a slow frame degrades to dropped frames (jank), never
// a frozen UI. The lock the UI used to wait on is gone — the class is gone.
// ──────────────────────────────────────────────────────────────────────────

/// A control-plane mutation, applied on the render thread. Cheap; the whole
/// batch drains every frame so camera/sun/scene changes land within one frame.
enum Cmd {
    /// Centre/zoom/bearing set; pitch is preserved from the live camera (the
    /// 2D gesture path doesn't touch tilt).
    SetCamera {
        lat: f64,
        lng: f64,
        zoom: f64,
        bearing: f64,
    },
    /// Translate the camera by a screen-space finger delta (px). Applied on the
    /// render thread against the LIVE camera, so successive pan events within one
    /// frame ACCUMULATE instead of each recomputing from a stale snapshot and
    /// overwriting (the dropped-intermediate-motion "throttle"/jitter). Ground-
    /// plane unproject → consistent under pitch (no terrain-hit skitter).
    PanByPixels {
        dx: f64,
        dy: f64,
    },
    Fling(f64, f64),
    ZoomFling {
        v: f64,
        fx: f64,
        fy: f64,
    },
    EaseTo {
        lat: f64,
        lng: f64,
        zoom: f64,
        bearing: f64,
        dur_ms: u64,
    },
    EasePitch {
        pitch: f64,
        dur_ms: u64,
    },
    ZoomAroundAnimated {
        factor: f64,
        fx: f64,
        fy: f64,
        dur_ms: u64,
    },
    ZoomAround {
        factor: f64,
        fx: f64,
        fy: f64,
    },
    OrbitAround {
        db: f64,
        dp: f64,
        fx: f64,
        fy: f64,
    },
    CancelAnimation,
    SetViewportInset(f64),
    // Environment/content mutations (sun, shadows, clouds config, tubes) have
    // no Cmds: they are SCENE state (plan P5.2), carried by Cmd::ApplyScene.
    IngestRadar {
        slot: u32,
        w: u32,
        h: u32,
        precip: Vec<u8>,
        coverage: Vec<u8>,
    },
    SetCloudTime {
        time: f32,
        blend: f32,
    },
    ApplyScene(Box<Scene>),
    /// Report a plan-issued fetch as failed (the engine re-pends + backs off
    /// — retry policy has exactly one owner, the lifecycle table).
    FetchFailed(u64),
    /// Report a plan `cancel` as honoured, or a `start` the host declined.
    FetchCancelled(u64),
    PumpTiles,
    Resize {
        w: u32,
        h: u32,
    },
}

/// A fetched tile to upload. Separate from [`Cmd`] so tile bursts are rate-
/// limited per frame (decode + GPU upload is the expensive part) without ever
/// delaying a control command — control fully drains, ingest is capped.
enum Ingest {
    Raster {
        layer: String,
        tile: TileId,
        bytes: Vec<u8>,
    },
    Terrain {
        tile: TileId,
        bytes: Vec<u8>,
    },
    Vector {
        layer: String,
        tile: TileId,
        bytes: Vec<u8>,
    },
}

/// Cheap, immutable read model republished after every frame. UI reads load it
/// wait-free, so they never touch the engine.
#[derive(Clone)]
struct Snapshot {
    cam: CameraState,
    /// Viewport size + bottom inset at publish time, so project/unproject can
    /// reconstruct the projection without the engine (always-valid, wait-free).
    viewport: (f64, f64),
    inset: f64,
    animating: bool,
    /// Desired-but-not-resident count from the last minted plan — perf
    /// telemetry only (the actionable list rides the plan outbox).
    pending_count: u32,
    stats_json: String,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            cam: CameraState {
                center: LatLng::new(0.0, 0.0),
                zoom: 0.0,
                pitch_deg: 0.0,
                bearing_deg: 0.0,
            },
            viewport: (1.0, 1.0),
            inset: 0.0,
            animating: false,
            pending_count: 0,
            stats_json: "{}".to_string(),
        }
    }
}

/// Reconstruct a [`Camera`] from a published snapshot for wait-free, always-valid
/// projection (the elevation-aware engine path needs the render lock; this flat
/// fallback never blocks and never drops a gesture — exact in 2D, a hair off only
/// for elevation under 3D tilt, which is invisible at gesture scale).
fn snapshot_camera(s: &Snapshot) -> Camera {
    Camera::new(
        CoreLatLng::new(s.cam.center.lat, s.cam.center.lng),
        s.cam.zoom,
    )
    .with_pitch(s.cam.pitch_deg)
    .with_bearing(s.cam.bearing_deg)
    .with_viewport_inset(s.inset)
}

/// Hard cap on a single ingested tile payload (16 MiB). A 256–512px encoded
/// raster/DEM tile is well under 1 MB; a larger array is a misrouted endpoint,
/// an error page, or a corrupt cache entry. Reject it *before* `convert_byte_array`
/// copies it into a `Vec` — an unbounded payload there aborts the process in
/// scudo (`internal map failure: Out of memory`). Defense-in-depth behind the
/// host-side guard in `TurbomapMapView.launchTileFetch`.
const MAX_INGEST_TILE_BYTES: i32 = 16 * 1024 * 1024;

/// The shared handle. UI-facing fields (`cmd*`, `ingest*`, `snapshot`,
/// `plan_*`) are wait-free or brief-lock; `render` is held only by the
/// render thread + lifecycle.
///
/// TILE TRANSPORT (plan P5.1): the STREAMING PLAN, not the legacy
/// pending-tiles poll. The host grants fetch lanes
/// (`nativeTakeStreamingPlanJson(handle, freeLanes)`); the render thread
/// mints at most that many `start`s from the engine's lifecycle table each
/// frame (so in-flight fetches are never re-issued — the table's `Fetching`
/// state IS the dedup that the old host-side `queued` set approximated) and
/// appends the plan to an outbox the host drains consume-once. Deliveries
/// complete through the ordinary `ingest*`; failures/cancels report by
/// request id, and the ENGINE owns retry/backoff policy.
struct Surface {
    cmd_tx: crossbeam_channel::Sender<Cmd>,
    cmd_rx: crossbeam_channel::Receiver<Cmd>,
    ingest_tx: crossbeam_channel::Sender<Ingest>,
    ingest_rx: crossbeam_channel::Receiver<Ingest>,
    /// Fetch lanes granted by the host and not yet spent by the render
    /// thread. Swapped to 0 when a plan is minted, so the host's stated
    /// capacity bounds exactly one plan's `start` list.
    plan_lanes: std::sync::atomic::AtomicU32,
    /// Plans minted but not yet taken by the host (JSON objects, in mint
    /// order). Drained whole by `nativeTakeStreamingPlanJson` — consume-once,
    /// so a start can never be lost to a snapshot overwrite.
    plan_outbox: Mutex<Vec<String>>,
    snapshot: Mutex<Arc<Snapshot>>,
    render: Mutex<OnScreen>,
}

impl Surface {
    fn new(on: OnScreen) -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
        let (ingest_tx, ingest_rx) = crossbeam_channel::unbounded();
        let snap = Arc::new(on.build_snapshot());
        Surface {
            cmd_tx,
            cmd_rx,
            ingest_tx,
            ingest_rx,
            plan_lanes: std::sync::atomic::AtomicU32::new(0),
            plan_outbox: Mutex::new(Vec::new()),
            snapshot: Mutex::new(snap),
            render: Mutex::new(on),
        }
    }

    /// Drain queued commands, render one frame, republish the snapshot — the
    /// only place the engine is mutated. Runs on the render thread.
    fn render_frame(&self) {
        let mut on = self.render.lock().unwrap_or_else(|p| p.into_inner());
        // Control commands fully drain — they must land this frame.
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            on.apply_cmd(cmd);
        }
        // Tile ingest ALSO drains fully (plan B4.3): `apply_ingest` is an
        // O(µs) hand-off into the engine's decode queue — workers decode
        // off this thread, and `render()` applies results under the
        // engine's tiered budget (tight while the camera moves, generous
        // when settled). The adaptive 8/6 ms time-slice that used to bound
        // decode-on-the-render-thread is retired: pacing has exactly one
        // owner now, inside the engine, identical on every host.
        let ingest_start = std::time::Instant::now();
        let mut ingested = 0u32;
        while let Ok(i) = self.ingest_rx.try_recv() {
            on.apply_ingest(i);
            ingested += 1;
        }
        let ingest_ms = ingest_start.elapsed().as_secs_f64() * 1000.0;
        // The backlog that matters is delivered-but-not-yet-drawable, which
        // now lives in the engine's decode queue (the channel is empty).
        let ingest_backlog = on.engine.decode_backlog();
        let render_start = std::time::Instant::now();
        on.render();
        let render_ms = render_start.elapsed().as_secs_f64() * 1000.0;
        // Mint the frame's streaming plan against the lanes the host granted
        // since the last mint. Zero lanes still surfaces cancels (aborting
        // in-flight fetches the camera moved away from must not wait on
        // capacity).
        let lanes = self.plan_lanes.swap(0, std::sync::atomic::Ordering::AcqRel);
        let plan = on.engine.streaming_plan(lanes as usize);
        let plan_pending = plan.start.len() as u32;
        if !plan.start.is_empty() || !plan.cancel.is_empty() {
            let json = turbomap_engine::engine::streaming_plan_to_json(&plan);
            self.plan_outbox
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(json);
        }
        let mut snap = on.build_snapshot();
        snap.pending_count = plan_pending;
        // A pending ingest backlog must keep the render-on-demand host awake, or the
        // remaining tiles would only trickle in on the next unrelated invalidation.
        snap.animating = snap.animating || ingest_backlog > 0;
        let pending = snap.pending_count;
        on.perf
            .record(render_ms, ingest_ms, ingested, ingest_backlog, pending);
        // Publish the full structured per-frame trace (Slice 1): the engine's
        // always-on FrameMetrics plus the streaming timings/counts only the FFI
        // render loop knows. Overwrites the cheap cache-only stats built above.
        snap.stats_json = frame_trace(
            &on.engine,
            FrameTrace {
                frame: on.perf.frame_idx,
                gap_ms: on.perf.last_gap_ms,
                ingest_ms,
                render_ms,
                pending,
                backlog: ingest_backlog,
                ingested,
                ..FrameTrace::default()
            },
        )
        .to_json();
        *self.snapshot.lock().unwrap_or_else(|p| p.into_inner()) = Arc::new(snap);
    }

    fn latest(&self) -> Arc<Snapshot> {
        self.snapshot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

/// Resolve the handle to its live `Surface` (or `None` if destroyed), then run
/// `f`. The `Arc` is cloned under a brief registry lock and released before `f`
/// runs, so a long render frame never holds the registry lock — and a call that
/// races `nativeDestroy` either sees the surface (kept alive by this clone) or
/// safely gets `None`. No raw-pointer deref, so a stale handle cannot fault.
unsafe fn with_surface<R>(handle: jlong, f: impl FnOnce(&Surface) -> R) -> Option<R> {
    let surface = {
        let reg = SURFACES.lock().unwrap_or_else(|p| p.into_inner());
        reg.get(&(handle as u64)).cloned()
    };
    let s = surface?;
    match catch_unwind(AssertUnwindSafe(|| f(&s))) {
        Ok(r) => Some(r),
        Err(payload) => {
            set_error(format!("panic: {}", panic_message(&*payload)));
            None
        }
    }
}

/// Enqueue a mutation — wait-free; returns before the render thread applies it.
unsafe fn enqueue(handle: jlong, cmd: Cmd) {
    unsafe {
        with_surface(handle, |s| {
            let _ = s.cmd_tx.send(cmd);
        });
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeCreate(
    env: JNIEnv,
    _class: JClass,
    surface: JObject,
    width: jint,
    height: jint,
    lat: jdouble,
    lng: jdouble,
    zoom: jdouble,
) -> jlong {
    init_logging();
    let raw_env = env.get_raw();
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<OnScreen, String> {
        // Safety: `surface` is a live android.view.Surface for this call.
        let window = unsafe { NativeWindow::from_surface(raw_env, surface.as_raw()) }
            .ok_or_else(|| "ANativeWindow from Surface was null".to_string())?;
        let camera = CameraState {
            center: LatLng::new(lat, lng),
            zoom,
            pitch_deg: 0.0,
            bearing_deg: 0.0,
        };
        build(window, width.max(0) as u32, height.max(0) as u32, camera)
    }));
    match result {
        Ok(Ok(map)) => {
            let id = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
            SURFACES
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(id, Arc::new(Surface::new(map)));
            id as jlong
        }
        Ok(Err(reason)) => {
            set_error(reason);
            0
        }
        Err(payload) => {
            set_error(format!("panic during create: {}", panic_message(&*payload)));
            0
        }
    }
}

/// The last failure reason (and clears it), or null if none — the host shows
/// this to the user instead of falling back. See [`LAST_ERROR`].
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeLastError(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let msg = LAST_ERROR.lock().ok().and_then(|mut slot| slot.take());
    match msg {
        Some(s) => env
            .new_string(s)
            .map(|js| js.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeApplyScene(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    scene_json: JString,
) -> jboolean {
    let json: String = match env.get_string(&scene_json) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    // Parse + validate on the calling thread (no engine needed), then enqueue —
    // so the caller still gets a synchronous valid/invalid result, but the
    // actual apply happens on the render thread without blocking.
    let Ok(scene) = serde_json::from_str::<Scene>(&json) else {
        return JNI_FALSE;
    };
    if scene.validate().is_err() {
        return JNI_FALSE;
    }
    unsafe { enqueue(handle, Cmd::ApplyScene(Box::new(scene))) };
    JNI_TRUE
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativePumpLocal(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    unsafe { enqueue(handle, Cmd::PumpTiles) };
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeRender(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    unsafe {
        with_surface(handle, |s| s.render_frame());
    }
}

/// True while a camera animation or tile fade-in is running — the host keeps
/// drawing until it goes false, then parks (render-on-demand). Read from the
/// last published snapshot (wait-free; never blocks on a render frame).
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeIsAnimating(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jboolean {
    let animating = unsafe { with_surface(handle, |s| s.latest().animating) };
    if animating == Some(true) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Start an inertial pan fling from the current camera at screen-pixel velocity
/// (drag-release velocity). The map glides + decelerates as `render` ticks.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeFling(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    vx: jdouble,
    vy: jdouble,
) {
    unsafe { enqueue(handle, Cmd::Fling(vx, vy)) };
}

/// Start a momentum **zoom** from a pinch release: `zoom_velocity` is in
/// zoom-levels/second (positive = zooming in), gliding about the pinch focus
/// `(fx, fy)`. Locked to the zoom axis — the focus pixel stays fixed, so the
/// map doesn't pan/drift sideways while the zoom coasts to rest.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeZoomFling(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    zoom_velocity: jdouble,
    fx: jdouble,
    fy: jdouble,
) {
    unsafe {
        enqueue(
            handle,
            Cmd::ZoomFling {
                v: zoom_velocity,
                fx,
                fy,
            },
        )
    };
}

/// Ease the camera to a target pose over `duration_ms` (accel/decel). Pitch is
/// kept from the current camera; pass the desired centre/zoom/bearing.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeEaseTo(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    lat: jdouble,
    lng: jdouble,
    zoom: jdouble,
    bearing_deg: jdouble,
    duration_ms: jint,
) {
    unsafe {
        enqueue(
            handle,
            Cmd::EaseTo {
                lat,
                lng,
                zoom,
                bearing: bearing_deg,
                dur_ms: duration_ms.max(0) as u64,
            },
        )
    };
}

/// Ease only the pitch (tilt) to `pitch_deg` over `duration_ms`, keeping the
/// current centre/zoom/bearing. Used for the 2D↔3D mode transition (ease into
/// a tilt on entering 3D, back to flat on leaving). Pitch is clamped to the
/// engine limit inside the camera.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeEasePitch(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    pitch_deg: jdouble,
    duration_ms: jint,
) {
    unsafe {
        enqueue(
            handle,
            Cmd::EasePitch {
                pitch: pitch_deg,
                dur_ms: duration_ms.max(0) as u64,
            },
        )
    };
}

/// Animate a focus-invariant zoom by `factor` about `(fx, fy)` over `duration_ms`.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeZoomAroundAnimated(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    factor: jdouble,
    fx: jdouble,
    fy: jdouble,
    duration_ms: jint,
) {
    unsafe {
        enqueue(
            handle,
            Cmd::ZoomAroundAnimated {
                factor,
                fx,
                fy,
                dur_ms: duration_ms.max(0) as u64,
            },
        )
    };
}

/// One immediate focus-invariant zoom step by `factor` about `(fx, fy)` — the live
/// pinch-zoom. The world point under the fingers stays under the fingers (unlike a
/// centre-anchored zoom). The animated variant above is for double-tap / scroll-wheel.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeZoomAround(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    factor: jdouble,
    fx: jdouble,
    fy: jdouble,
) {
    unsafe { enqueue(handle, Cmd::ZoomAround { factor, fx, fy }) };
}

/// One 3D-mode orbit step: rotate the bearing by `d_bearing_deg` and tilt by
/// `d_pitch_deg`, both pivoting about the pinned focus pixel `(fx, fy)` so that
/// pixel stays over the same world point — the location the user is orbiting
/// stays glued to its screen spot while the world spins/tilts around it. Pitch
/// is clamped to the engine's limit. Driven per move-event by the 1-finger drag
/// in 3D mode (see docs/architecture/2026-06-2d-3d-map-mode-gestures.md).
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeOrbitAround(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    d_bearing_deg: jdouble,
    d_pitch_deg: jdouble,
    fx: jdouble,
    fy: jdouble,
) {
    unsafe {
        enqueue(
            handle,
            Cmd::OrbitAround {
                db: d_bearing_deg,
                dp: d_pitch_deg,
                fx,
                fy,
            },
        )
    };
}

/// Catch any in-flight camera animation, freezing the camera exactly where it
/// is (finger-down stops the motion). `set_camera(current)` clears `active`.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeCancelAnimation(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    unsafe { enqueue(handle, Cmd::CancelAnimation) };
}

/// Compact JSON of the last frame's cache telemetry, summed across layers:
/// `{"tiles":N,"bytes":N,"budget":N,"evictions":N,"hits":N,"misses":N}`. The
/// host logs this to watch GPU-texture memory + eviction pressure.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeStats(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let json = unsafe { with_surface(handle, |s| s.latest().stats_json.clone()) }
        .unwrap_or_else(|| "{}".to_string());
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Reserve `bottom_px` at the bottom of the viewport (e.g. the live sheet) so the
/// projection + rendered frame shift up into the visible band.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeSetViewportInset(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    bottom_px: jdouble,
) {
    unsafe { enqueue(handle, Cmd::SetViewportInset(bottom_px.max(0.0))) };
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeResize(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    width: jint,
    height: jint,
) {
    unsafe {
        enqueue(
            handle,
            Cmd::Resize {
                w: width.max(0) as u32,
                h: height.max(0) as u32,
            },
        )
    };
}

/// Build a `double[]` JNI return; empty array on allocation failure.
fn double_array(env: &mut JNIEnv, values: &[f64]) -> jni::sys::jarray {
    match env.new_double_array(values.len() as i32) {
        Ok(arr) => {
            let _ = env.set_double_array_region(&arr, 0, values);
            arr.into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeSetCamera(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    lat: jdouble,
    lng: jdouble,
    zoom: jdouble,
    bearing_deg: jdouble,
) {
    unsafe {
        enqueue(
            handle,
            Cmd::SetCamera {
                lat,
                lng,
                zoom,
                bearing: bearing_deg,
            },
        )
    };
}

/// One-finger pan step: translate the camera by a screen-space finger delta (px).
/// Wait-free enqueue; the render thread applies it against the live camera so
/// successive pans within a frame accumulate (no stale-snapshot recompute, no
/// overwrite) — smooth, jitter-free panning in 2D and 3D alike.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativePanBy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    dx: jdouble,
    dy: jdouble,
) {
    unsafe { enqueue(handle, Cmd::PanByPixels { dx, dy }) };
}

/// `[lat, lng, zoom, bearingDeg]`.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeCamera(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jni::sys::jarray {
    let cam = unsafe { with_surface(handle, |s| s.latest().cam.clone()) };
    match cam {
        Some(c) => double_array(
            &mut env,
            &[c.center.lat, c.center.lng, c.zoom, c.bearing_deg],
        ),
        None => double_array(&mut env, &[]),
    }
}

/// `[x, y, valid]` — `valid` is 1.0 if the point projects, else 0.0.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeProject(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    lat: jdouble,
    lng: jdouble,
) -> jni::sys::jarray {
    // ALWAYS the elevation-aware projection, so a marker / waypoint / my-position pin sits ON
    // the 3D terrain. We BLOCK on the render lock rather than the old try_lock→flat-snapshot
    // fallback: that fallback projected onto the z=0 plane whenever a frame was mid-flight —
    // i.e. on most frames while the map was moving — so pins jittered between the ground and
    // the relief every frame (the "flicker between ground and 3D level" bug). Blocking is
    // deadlock-free here (`with_surface` already released the registry lock, and the render
    // thread never waits on the main thread while holding `render`); it only waits out the
    // current frame, which is imperceptible for a per-frame reprojection and far better than
    // the flicker. Unproject (the drag hot path) keeps its wait-free fallback; project does not.
    let p = unsafe {
        with_surface(handle, |s| {
            let on = s
                .render
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            on.engine
                .project(LatLng::new(lat, lng))
                .map(|sp| (sp.x, sp.y))
        })
    }
    .flatten();
    match p {
        Some((x, y)) => double_array(&mut env, &[x, y, 1.0]),
        None => double_array(&mut env, &[0.0, 0.0, 0.0]),
    }
}

/// `[lat, lng, valid]`.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeUnproject(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    x: jdouble,
    y: jdouble,
) -> jni::sys::jarray {
    // Exact when the render lock is free; otherwise a wait-free flat unprojection
    // from the snapshot (pixel_to_world always yields a point) — so a 1-finger
    // pan, which unprojects every move, never drops a frame to lock contention.
    let g = unsafe {
        with_surface(handle, |s| {
            if let Ok(on) = s.render.try_lock() {
                if let Some(ll) = on.engine.unproject(ScreenPoint::new(x, y)) {
                    return (ll.lat, ll.lng);
                }
            }
            let snap = s.latest();
            let ll = snapshot_camera(&snap)
                .pixel_to_world((x, y), snap.viewport)
                .to_lat_lng();
            (ll.lat, ll.lng)
        })
    };
    match g {
        Some((lat, lng)) => double_array(&mut env, &[lat, lng, 1.0]),
        None => double_array(&mut env, &[0.0, 0.0, 0.0]),
    }
}

/// `[lat, lng, worldZ, hitTerrain, valid]` — TERRAIN-AWARE screen→ground.
///
/// Unlike `nativeUnproject` (flat z=0 plane, for pan/freehand), this marches the
/// view ray against the relief so a dragged marker lands on the exact ground
/// point under the finger in 3D. Exact when the render lock is free; otherwise a
/// wait-free FLAT unproject from the snapshot (`hitTerrain = 0`) so a drag never
/// drops a move event to lock contention — the exact answer only has to land on
/// the settled drop, when the lock is almost always free.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeUnprojectGround(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    x: jdouble,
    y: jdouble,
) -> jni::sys::jarray {
    let g = unsafe {
        with_surface(handle, |s| {
            if let Ok(on) = s.render.try_lock() {
                let (lat, lng, wz, hit) = on.engine.unproject_ground(x, y);
                return (lat, lng, wz as f64, if hit { 1.0 } else { 0.0 });
            }
            let snap = s.latest();
            let ll = snapshot_camera(&snap)
                .pixel_to_world((x, y), snap.viewport)
                .to_lat_lng();
            (ll.lat, ll.lng, 0.0, 0.0)
        })
    };
    match g {
        Some((lat, lng, wz, hit)) => double_array(&mut env, &[lat, lng, wz, hit, 1.0]),
        None => double_array(&mut env, &[0.0, 0.0, 0.0, 0.0, 0.0]),
    }
}

/// Features under a screen point (device px) within `tolPx`, top-most first,
/// as JSON `[{"layer":..,"feature_id":..,"properties":{..}}, ...]` (plan
/// P6.4). Scene circles answer with their geo-json feature properties, so a
/// tapped pin resolves to its domain id. Wait-free: exact when the render
/// lock is free, `[]` under contention — a tap is a settled gesture, so the
/// lock is almost always free; the host may simply re-ask.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeHitTest(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    x: jdouble,
    y: jdouble,
    tol_px: jdouble,
) -> jni::sys::jstring {
    let json = unsafe {
        with_surface(handle, |s| {
            if let Ok(on) = s.render.try_lock() {
                let hits = MapEngine::hit_test(&on.engine, ScreenPoint::new(x, y), tol_px);
                turbomap_engine::hits_to_json(&hits)
            } else {
                "[]".to_string()
            }
        })
    }
    .unwrap_or_else(|| "[]".to_string());
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Tiles the engine is waiting on, as a JSON array
/// `[{"kind":"raster","layer":"basemap","z":..,"x":..,"y":..}, ...]` — the host
/// fetches each (it owns the URL templates + offline) and pushes bytes back.
#[no_mangle]
/// Grant `free_lanes` fetch lanes and drain every plan minted since the last
/// call, as a JSON ARRAY of plan objects
/// (`[{"start":[{"id","kind","layer","z","x","y"}],"cancel":[ids]}, …]`).
/// Consume-once: each start appears in exactly one take. The first call after
/// a grant returns plans minted on the NEXT frame — poll on the host's pump
/// cadence. Honour every `cancel` (abort transport, then
/// `nativeReportFetchCancelled`); report failures via
/// `nativeReportFetchFailed`; deliveries complete through the ordinary
/// `nativeIngest*`.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeTakeStreamingPlanJson(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    free_lanes: jint,
) -> jni::sys::jstring {
    let json = unsafe {
        with_surface(handle, |s| {
            s.plan_lanes.fetch_max(
                free_lanes.max(0) as u32,
                std::sync::atomic::Ordering::AcqRel,
            );
            let plans: Vec<String> =
                std::mem::take(&mut *s.plan_outbox.lock().unwrap_or_else(|p| p.into_inner()));
            format!("[{}]", plans.join(","))
        })
    }
    .unwrap_or_else(|| "[]".to_string());
    env.new_string(json)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeReportFetchFailed(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    id: jlong,
) {
    unsafe { enqueue(handle, Cmd::FetchFailed(id.max(0) as u64)) }
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeReportFetchCancelled(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    id: jlong,
) {
    unsafe { enqueue(handle, Cmd::FetchCancelled(id.max(0) as u64)) }
}

/// Push a fetched raster tile (encoded PNG/JPEG/WebP). Returns false if it
/// doesn't decode or the handle is gone.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeIngestRaster(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    layer_id: JString,
    z: jint,
    x: jint,
    y: jint,
    bytes: jni::objects::JByteArray,
) -> jboolean {
    let layer: String = match env.get_string(&layer_id) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    // Reject an oversized payload before copying it out of the JVM array —
    // `convert_byte_array` would otherwise calloc the whole thing and abort
    // on a corrupt/misrouted tile. (Belt-and-suspenders; the host caps too.)
    if env.get_array_length(&bytes).unwrap_or(0) > MAX_INGEST_TILE_BYTES {
        return JNI_FALSE;
    }
    let data = match env.convert_byte_array(&bytes) {
        Ok(d) => d,
        Err(_) => return JNI_FALSE,
    };
    let tile = TileId::new(z.max(0) as u8, x.max(0) as u32, y.max(0) as u32);
    // The lifecycle table's `Fetching` state dedups re-requests now — the
    // plan never re-issues an in-flight tile, so the old host-side queued-set
    // guard is gone.
    let sent = unsafe {
        with_surface(handle, |s| {
            s.ingest_tx
                .send(Ingest::Raster {
                    layer,
                    tile,
                    bytes: data,
                })
                .is_ok()
        })
    };
    // Optimistic: the upload happens on the render thread next frame. A decode
    // failure just leaves the tile un-ingested; the reconciler re-requests it.
    if sent == Some(true) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Push a fetched vector tile (raw MVT/protobuf bytes) into the named vector
/// layer, so the engine tessellates it — for the realistic-water layer, its
/// `water` polygons feed the water pipeline. Returns false if oversized, the
/// handle is gone, or the ingest queue is closed. Mirrors [`nativeIngestRaster`]
/// (same queue); decode happens on the render thread.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeIngestVector(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    layer_id: JString,
    z: jint,
    x: jint,
    y: jint,
    bytes: jni::objects::JByteArray,
) -> jboolean {
    let layer: String = match env.get_string(&layer_id) {
        Ok(s) => s.into(),
        Err(_) => return JNI_FALSE,
    };
    if env.get_array_length(&bytes).unwrap_or(0) > MAX_INGEST_TILE_BYTES {
        return JNI_FALSE;
    }
    let data = match env.convert_byte_array(&bytes) {
        Ok(d) => d,
        Err(_) => return JNI_FALSE,
    };
    let tile = TileId::new(z.max(0) as u8, x.max(0) as u32, y.max(0) as u32);
    let sent = unsafe {
        with_surface(handle, |s| {
            s.ingest_tx
                .send(Ingest::Vector {
                    layer,
                    tile,
                    bytes: data,
                })
                .is_ok()
        })
    };
    if sent == Some(true) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Push a fetched DEM tile (encoded Mapbox-Terrain-RGB PNG) into the shared
/// heightmap, so the ground-plane pipelines displace by elevation (3D terrain).
/// Returns false if it doesn't decode or the handle is gone.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeIngestTerrain(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    z: jint,
    x: jint,
    y: jint,
    bytes: jni::objects::JByteArray,
) -> jboolean {
    if env.get_array_length(&bytes).unwrap_or(0) > MAX_INGEST_TILE_BYTES {
        return JNI_FALSE;
    }
    let data = match env.convert_byte_array(&bytes) {
        Ok(d) => d,
        Err(_) => return JNI_FALSE,
    };
    let tile = TileId::new(z.max(0) as u8, x.max(0) as u32, y.max(0) as u32);
    let sent = unsafe {
        with_surface(handle, |s| {
            s.ingest_tx
                .send(Ingest::Terrain { tile, bytes: data })
                .is_ok()
        })
    };
    if sent == Some(true) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

// ---- weather-cloud overlay: transport + clock only (plan P5.2) -----------
//
// WHAT renders (grid, geo bounds, visibility, lighting, shadows) is Scene
// state — declared in the IR's `environment` and applied via
// `nativeApplyScene`. Only the frame DATA push (transport, like tiles) and
// the playback clock (a control verb, like the camera) cross here.

/// Upload a radar frame into `slot` (0 = current, 1 = next) from two
/// `gridW * gridH` byte planes — `precip` and `coverage`, each 0..=255.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeIngestRadarFrame(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    slot: jint,
    grid_w: jint,
    grid_h: jint,
    precip: jni::objects::JByteArray,
    coverage: jni::objects::JByteArray,
) {
    let precip = match env.convert_byte_array(&precip) {
        Ok(d) => d,
        Err(_) => return,
    };
    let coverage = match env.convert_byte_array(&coverage) {
        Ok(d) => d,
        Err(_) => return,
    };
    unsafe {
        enqueue(
            handle,
            Cmd::IngestRadar {
                slot: slot.max(0) as u32,
                w: grid_w.max(0) as u32,
                h: grid_h.max(0) as u32,
                precip,
                coverage,
            },
        )
    };
}

/// Set the cloud animation clock (`time`, seconds) and the slot-0→slot-1
/// crossfade (`blend`, 0..=1) — what the time slider scrubs.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeSetCloudTime(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    time: jfloat,
    blend: jfloat,
) {
    unsafe { enqueue(handle, Cmd::SetCloudTime { time, blend }) };
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    // Remove the registry entry, then drop the Arc OUTSIDE the lock (the Surface
    // drop frees GPU resources — surface, device, engine, window). If an
    // in-flight call still holds a clone, the real teardown happens when it
    // returns; a later stale call just misses the registry and no-ops.
    let removed = catch_unwind(AssertUnwindSafe(|| {
        SURFACES
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&(handle as u64))
    }))
    .ok()
    .flatten();
    drop(removed);
}
