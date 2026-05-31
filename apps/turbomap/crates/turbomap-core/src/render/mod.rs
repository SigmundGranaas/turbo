//! GPU rendering pipelines. Not unit-tested — correctness is observed
//! visually via the smoke test in `turbomap-app`.
//!
//! Each submodule owns one wgpu pipeline. The unified `Map` in
//! `crate::map` is the orchestrator: it owns the per-layer state, picks
//! which pipeline to dispatch per layer, runs a single text pass after
//! all geometry layers, and finally renders markers on top.

pub(crate) mod cache;
pub(crate) mod gpu_timestamps;
pub(crate) mod hillshade;
pub(crate) mod marker;
pub(crate) mod raster;
pub mod terrain;
pub(crate) mod text;
pub(crate) mod vector;
pub(crate) mod vector_cache;

pub(crate) use cache::TextureCache;

/// First-frame clear colour, expressed in *linear* light space because
/// wgpu interprets `wgpu::Color` as linear before the framebuffer's
/// sRGB conversion. Black reads as a render bug to users; pure white
/// reads as glare. We pick a muted slate that matches the Kartverket
/// grey topo basemap's empty-tile look — sRGB ≈ (170, 170, 165),
/// achieved by linear ≈ (0.41, 0.41, 0.39).
pub(crate) const BACKGROUND_CLEAR: wgpu::Color = wgpu::Color {
    r: 0.41,
    g: 0.41,
    b: 0.39,
    a: 1.0,
};

/// Depth attachment format. `Depth32Float` is supported on every
/// backend wgpu targets and gives plenty of precision for the height
/// ranges we deal with (≤ ~3 km of relief over Mercator world units
/// at zoom 6+).
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Depth-stencil for ground-plane pipelines (raster, hillshade,
/// vector). Writes z; later draws at the same pixel are rejected
/// when their depth is greater (i.e. behind a mountain face that's
/// already painted).
#[allow(dead_code)] // Held for Phase 4 (raster/vector displacement).
pub(crate) fn ground_depth_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Depth-stencil for screen-space overlays (text + markers). They
/// don't have meaningful depth; treat them as always-in-front of the
/// world without writing z (so subsequent overlays don't depth-cull
/// each other).
#[allow(dead_code)] // Held for Phase 4 (text/marker depth coexistence).
pub(crate) fn overlay_depth_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::Always,
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}
