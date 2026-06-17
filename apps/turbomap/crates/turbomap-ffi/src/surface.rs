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

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, LazyLock, Mutex};

use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jdouble, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use turbomap_core::{MapOptions, PendingTile, TileId};
use turbomap_engine::{CameraState, HostDrivenResolver, MapEngine, TurbomapEngine};
use turbomap_scene::{LatLng, Scene, ScreenPoint};

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
    _instance: wgpu::Instance,
    _window: NativeWindow,
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
    // A tighter off-screen prefetch ring than the 256px default: the reconciler
    // fetches nearest-first, so visible tiles still come in first, but a smaller
    // margin roughly halves the per-view tile count on a slow connection — the
    // visible area fills noticeably faster instead of competing with a wide ring.
    let options = MapOptions {
        fade_in_secs: 0.3,
        prefetch_margin_px: 128,
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
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.engine.resize(self.config.width, self.config.height);
    }
}

/// `handle` is a `Box<Mutex<OnScreen>>` pointer. Every native entry point goes
/// through here, so the lock serialises access from the dedicated render thread
/// (the frame loop) and the UI thread (gestures, projection, the tile
/// reconciler) — the engine itself stays single-owner, no internal locking.
/// A panic while the lock is held poisons it; we recover the inner value so one
/// caught panic doesn't wedge the map forever.
unsafe fn with_map<R>(handle: jlong, f: impl FnOnce(&mut OnScreen) -> R) -> Option<R> {
    let ptr = handle as *const Mutex<OnScreen>;
    if ptr.is_null() {
        return None;
    }
    let mtx = &*ptr;
    match catch_unwind(AssertUnwindSafe(|| {
        let mut guard = mtx.lock().unwrap_or_else(|poison| poison.into_inner());
        f(&mut guard)
    })) {
        Ok(r) => Some(r),
        Err(payload) => {
            set_error(format!("panic: {}", panic_message(&*payload)));
            None
        }
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
        Ok(Ok(map)) => Box::into_raw(Box::new(Mutex::new(map))) as jlong,
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
    let ok = unsafe {
        with_map(handle, |map| {
            let Ok(scene) = serde_json::from_str::<Scene>(&json) else {
                return false;
            };
            if scene.validate().is_err() {
                return false;
            }
            map.engine.apply(scene);
            true
        })
    };
    if ok == Some(true) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativePumpLocal(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    unsafe {
        with_map(handle, |map| {
            map.engine.pump_tiles();
        });
    }
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeRender(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    unsafe {
        with_map(handle, |map| map.render());
    }
}

/// True while a camera animation or tile fade-in is running — the host keeps
/// drawing until it goes false, then parks (render-on-demand).
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeIsAnimating(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jboolean {
    let animating = unsafe { with_map(handle, |map| map.engine.is_animating()) };
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
    unsafe {
        with_map(handle, |map| map.engine.fling((vx, vy)));
    }
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
        with_map(handle, |map| map.engine.zoom_fling(zoom_velocity, (fx, fy)));
    }
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
        with_map(handle, |map| {
            let mut target = map.engine.camera();
            target.center = LatLng::new(lat, lng);
            target.zoom = zoom;
            target.bearing_deg = bearing_deg;
            map.engine
                .ease_to(target, std::time::Duration::from_millis(duration_ms.max(0) as u64));
        });
    }
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
        with_map(handle, |map| {
            map.engine.zoom_around_animated(
                factor,
                (fx, fy),
                std::time::Duration::from_millis(duration_ms.max(0) as u64),
            );
        });
    }
}

/// Catch any in-flight camera animation, freezing the camera exactly where it
/// is (finger-down stops the motion). `set_camera(current)` clears `active`.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeCancelAnimation(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    unsafe {
        with_map(handle, |map| {
            let here = map.engine.camera();
            map.engine.set_camera(here);
        });
    }
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
    let json = unsafe {
        with_map(handle, |map| {
            let m = map.engine.last_frame_metrics();
            let tiles: usize = m.layers.iter().map(|l| l.cache.entries).sum();
            let bytes: usize = m.layers.iter().map(|l| l.cache.bytes_used).sum();
            let budget: usize = m.layers.iter().map(|l| l.cache.budget_bytes).max().unwrap_or(0);
            let evictions: u64 = m.layers.iter().map(|l| l.cache.evictions).sum();
            let hits: u64 = m.layers.iter().map(|l| l.cache.hits).sum();
            let misses: u64 = m.layers.iter().map(|l| l.cache.misses).sum();
            format!(
                "{{\"tiles\":{tiles},\"bytes\":{bytes},\"budget\":{budget},\"evictions\":{evictions},\"hits\":{hits},\"misses\":{misses}}}"
            )
        })
    }
    .unwrap_or_else(|| "{}".to_string());
    env.new_string(json).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
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
    unsafe {
        with_map(handle, |map| map.engine.set_viewport_inset(bottom_px.max(0.0)));
    }
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
        with_map(handle, |map| map.resize(width.max(0) as u32, height.max(0) as u32));
    }
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
        with_map(handle, |map| {
            let mut cam = map.engine.camera();
            cam.center = LatLng::new(lat, lng);
            cam.zoom = zoom;
            cam.bearing_deg = bearing_deg;
            map.engine.set_camera(cam);
        });
    }
}

/// `[lat, lng, zoom, bearingDeg]`.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeCamera(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jni::sys::jarray {
    let cam = unsafe { with_map(handle, |map| map.engine.camera()) };
    match cam {
        Some(c) => double_array(&mut env, &[c.center.lat, c.center.lng, c.zoom, c.bearing_deg]),
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
    let p = unsafe { with_map(handle, |map| map.engine.project(LatLng::new(lat, lng))) }.flatten();
    match p {
        Some(s) => double_array(&mut env, &[s.x, s.y, 1.0]),
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
    let g = unsafe {
        with_map(handle, |map| {
            map.engine.unproject(ScreenPoint::new(x, y))
        })
    }
    .flatten();
    match g {
        Some(ll) => double_array(&mut env, &[ll.lat, ll.lng, 1.0]),
        None => double_array(&mut env, &[0.0, 0.0, 0.0]),
    }
}

/// Tiles the engine is waiting on, as a JSON array
/// `[{"kind":"raster","layer":"basemap","z":..,"x":..,"y":..}, ...]` — the host
/// fetches each (it owns the URL templates + offline) and pushes bytes back.
#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativePendingTilesJson(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jni::sys::jstring {
    let json = unsafe {
        with_map(handle, |map| {
            let items: Vec<String> = map
                .engine
                .pending_tiles()
                .into_iter()
                .filter_map(|p| {
                    let (kind, layer, t) = match p {
                        PendingTile::Raster { layer_id, tile } => ("raster", layer_id, tile),
                        PendingTile::Hillshade { layer_id, tile } => ("hillshade", layer_id, tile),
                        PendingTile::Vector { layer_id, tile } => ("vector", layer_id, tile),
                        PendingTile::Terrain { .. } => return None,
                    };
                    Some(format!(
                        "{{\"kind\":\"{kind}\",\"layer\":\"{layer}\",\"z\":{},\"x\":{},\"y\":{}}}",
                        t.z, t.x, t.y
                    ))
                })
                .collect();
            format!("[{}]", items.join(","))
        })
    }
    .unwrap_or_else(|| "[]".to_string());
    env.new_string(json).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
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
    let data = match env.convert_byte_array(&bytes) {
        Ok(d) => d,
        Err(_) => return JNI_FALSE,
    };
    let ok = unsafe {
        with_map(handle, |map| {
            map.engine
                .ingest_raster_encoded(&layer, TileId::new(z.max(0) as u8, x.max(0) as u32, y.max(0) as u32), &data)
        })
    };
    if ok == Some(true) { JNI_TRUE } else { JNI_FALSE }
}

#[no_mangle]
pub extern "system" fn Java_com_sigmundgranaas_turbo_expressive_core_turbomap_android_NativeSurfaceMap_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    let ptr = handle as *mut Mutex<OnScreen>;
    if !ptr.is_null() {
        // Drop the boxed mutex+map (surface, device, engine, native window).
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe { drop(Box::from_raw(ptr)) }));
    }
}
