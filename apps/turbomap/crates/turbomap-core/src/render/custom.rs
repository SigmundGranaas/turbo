//! Custom render layers — host-supplied contributions to the frame graph
//! (plan slice D4, architecture §III.3).
//!
//! A custom layer is exactly a registered [`CustomLayer`] bound to a
//! declared MSAA phase: write-once Rust + WGSL, portable across every host
//! (desktop, Android, web) because it lives below the platform boundary.
//! The layer joins the frame's single MSAA pass as an ordinary named graph
//! node (`custom:<id>`), so it shows up in the per-pass report and can be
//! masked off for isolation like any built-in pass. The engine binds scene
//! `Layer::Custom { id, kind }` declarations to registered factories by
//! `kind`; the `Map` API below is the direct (non-IR) route.

use std::sync::Arc;

/// Everything a custom layer needs at construction time to build pipelines
/// compatible with the frame's single MSAA pass: render into
/// `color_format` at `sample_count` samples, and — for `Ground` phase
/// contributions — against `depth_format`.
pub struct CustomLayerInit {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    /// The frame target format the MSAA pass resolves to.
    pub color_format: wgpu::TextureFormat,
    /// Depth attachment format of the MSAA pass.
    pub depth_format: wgpu::TextureFormat,
    /// Sample count of the MSAA pass (pipelines must match it).
    pub sample_count: u32,
}

/// Per-frame context handed to [`CustomLayer::prepare`]. Everything is in
/// the frame's relative-to-centre (RTC) world frame: `view_proj` maps
/// `(world - origin, z)` to clip space, exactly like the built-in ground
/// pipelines, so custom geometry stays f32-precise at deep zoom.
pub struct CustomFrameCtx {
    /// RTC view-projection matrix (column-major, `Camera` convention).
    pub view_proj: [[f32; 4]; 4],
    /// Absolute world-xy of the RTC origin (the camera centre).
    pub origin: (f64, f64),
    pub viewport_px: (u32, u32),
    /// Screen pixels per world unit at the current zoom.
    pub pixels_per_world_unit: f64,
    pub zoom: f64,
    pub pitch_deg: f64,
    pub bearing_deg: f64,
    /// Seconds since renderer start (or the test override —
    /// `Map::set_time_override` pins it for deterministic goldens).
    /// Drives animation.
    pub time_s: f32,
}

/// Which of the shared MSAA pass's phases the contribution joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomPhase {
    /// With the ground layers (tile stack order applies; depth attachment
    /// available, so geometry can sit in the 3D scene).
    Ground,
    /// Above the ground and route tubes, below icons/text/markers.
    Overlay,
}

/// `Send` where threads exist. wgpu's WebGPU backend types are not `Send`
/// (wasm is single-threaded), so the bound would make every custom layer
/// unimplementable on web — the exact platform D4's portability gate names.
/// Same conditional-bound pattern wgpu itself uses (`WasmNotSend`).
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> MaybeSend for T {}
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T> MaybeSend for T {}

/// A host-supplied render contribution. Implementations own their entire
/// GPU state (pipeline, buffers), created from a [`CustomLayerInit`];
/// `prepare` runs in the frame's CPU/upload phase (mutable), `draw` is
/// replayed inside the single MSAA render pass (immutable — the prepared
/// state must be self-contained by then).
pub trait CustomLayer: MaybeSend {
    /// The phase this layer's draw joins. Declared once — sampled at every
    /// frame's registration, but expected to be constant.
    fn phase(&self) -> CustomPhase {
        CustomPhase::Overlay
    }

    /// Per-frame CPU work: uniform/vertex uploads, animation state.
    fn prepare(&mut self, ctx: &CustomFrameCtx);

    /// Record draws into the shared MSAA pass.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>);
}
