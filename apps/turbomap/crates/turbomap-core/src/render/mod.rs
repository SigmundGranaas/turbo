//! GPU rendering pipelines. Not unit-tested — correctness is observed
//! visually via the smoke test in `turbomap-app`.
//!
//! Each submodule owns one wgpu pipeline. The unified `Map` in
//! `crate::map` is the orchestrator: it owns the per-layer state, picks
//! which pipeline to dispatch per layer, runs a single text pass after
//! all geometry layers, and finally renders markers on top.

pub(crate) mod ao;
pub(crate) mod cache;
pub(crate) mod floor;
pub(crate) mod frame;
pub(crate) mod gpu_timestamps;
pub mod graph;
pub(crate) mod hillshade;
pub(crate) mod icon;
pub(crate) mod marker;
pub(crate) mod raster;
pub(crate) mod route;
pub(crate) mod shadow;
pub(crate) mod sky;
pub(crate) mod targets;
pub mod terrain;
pub(crate) mod text;
pub(crate) mod vector;
pub(crate) mod vector_cache;

pub(crate) use cache::TextureCache;

/// True iff every element of a 4×4 matrix is finite (no `NaN`/`Inf`).
///
/// The single safety predicate behind the renderer's finite gate: a `NaN`
/// matrix is valid Rust and uploads fine, but a mobile Vulkan/GLES driver
/// hangs the moment it's used in a draw (desktop Metal tolerates it, which is
/// why the crash only shows on device). Every matrix bound for the GPU is
/// checked with this before upload; a frame that fails is dropped rather than
/// fed to the driver. See `Map::render`'s master gate and the sky/cloud
/// inverse-projection guards.
pub(crate) fn mat4_is_finite(m: &[[f32; 4]; 4]) -> bool {
    m.iter().flatten().all(|v| v.is_finite())
}

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

/// Format of the world-locked ambient-occlusion field ([`ao`]). One channel of
/// accumulated sky occlusion in [0,1]; `R16Float` is renderable, blendable (for
/// progressive accumulation) and filterable (so the terrain samples it smoothly)
/// without any extra device feature.
pub(crate) const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// Multisample count for the single frame pass. 4× is supported on every
/// backend we target and smooths the geometry edges that don't carry their
/// own shader AA (polygon fills especially). All pipelines that draw into
/// the frame pass must match this, and the pass resolves into the surface.
/// (Mobile-bandwidth tuning — drop to 1, or swap for a post-process AA — is
/// a device-time decision; this constant is the single switch.)
pub(crate) const MSAA_SAMPLES: u32 = 4;

/// The `MultisampleState` every frame-pass pipeline uses, so they all match
/// the multisampled attachment.
pub(crate) fn multisample_state() -> wgpu::MultisampleState {
    wgpu::MultisampleState {
        count: MSAA_SAMPLES,
        ..Default::default()
    }
}

/// Depth-stencil for ground-plane pipelines that displace by the
/// terrain DEM (raster). Writes z; later draws at the same pixel are
/// rejected when their depth is greater (i.e. behind a mountain face
/// that's already painted).
pub(crate) fn ground_depth_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Depth-stencil for screen-space overlays (text, markers, draped vector
/// geometry). They don't have
/// meaningful depth; treat them as always-in-front of the world without
/// writing z (so subsequent overlays don't depth-cull each other).
pub(crate) fn overlay_depth_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::Always),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::mat4_is_finite;

    #[test]
    fn finite_gate_accepts_real_matrices_and_rejects_nan_inf() {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert!(mat4_is_finite(&identity));

        let mut with_nan = identity;
        with_nan[2][3] = f32::NAN;
        assert!(
            !mat4_is_finite(&with_nan),
            "a single NaN must fail the gate"
        );

        let mut with_inf = identity;
        with_inf[0][0] = f32::INFINITY;
        assert!(!mat4_is_finite(&with_inf), "an Inf must fail the gate");
    }
}
