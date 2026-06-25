//! The frame's render attachments — the multisampled HDR colour target the
//! frame pass draws into, the depth buffer it tests against, the single-sample
//! HDR texture it resolves to (sampled by the post-process), and the two
//! half-resolution HDR ping-pong textures the bloom blur bounces between.
//!
//! Pulls the `Map`'s loose `depth_view` + `depth_size` + `msaa_color_view`
//! trio into one type that owns their creation and the resize rule (recreate
//! all attachments, together, when the surface size changes — Metal asserts
//! otherwise). A step toward a self-contained `Renderer`: the targets are the
//! renderer's state, not the map's.
//!
//! The frame pass no longer resolves straight to the sRGB surface. It resolves
//! to [`HDR_FORMAT`], which the [`post`](super::post) pass then bloom-blurs and
//! filmic-tonemaps down to the surface. So highlights can exceed 1.0 through the
//! whole geometry pass and only get compressed at the very end.

use super::{DEPTH_FORMAT, HDR_FORMAT, MSAA_SAMPLES};

/// Multisampled HDR colour + depth + resolve + bloom attachments,
/// sized to the surface.
pub(crate) struct FrameTargets {
    /// Multisampled HDR colour target the frame pass renders into; resolved to
    /// [`Self::hdr_resolve_view`] at pass end. On the realistic-water path the
    /// opaque pass STORES it so the water pass can load + draw on top.
    color_view: wgpu::TextureView,
    /// Depth attachment so the back of a 3D mountain doesn't overdraw the
    /// front. Shared by every pass in the frame.
    depth_view: wgpu::TextureView,
    /// Single-sample HDR texture the frame pass resolves into; sampled by the
    /// post-process (bright-pass + final tonemap) AND, on the realistic-water
    /// path, by the water pass as the opaque Scene Colour.
    hdr_resolve_view: wgpu::TextureView,
    /// Half-resolution HDR ping/pong textures the separable bloom blur bounces
    /// between (bright-pass + downsample → `a`, blur_h → `b`, blur_v → `a`).
    bloom_a_view: wgpu::TextureView,
    bloom_b_view: wgpu::TextureView,
    /// The size all attachments were built for; resize is a no-op until the
    /// surface actually changes dimensions.
    size: (u32, u32),
}

impl FrameTargets {
    pub(crate) fn new(device: &wgpu::Device, size: (u32, u32)) -> Self {
        let (bw, bh) = bloom_size(size);
        Self {
            color_view: create_msaa_color_view(device, size),
            depth_view: create_depth_view(device, size),
            hdr_resolve_view: create_resolve_view(device, size, "turbomap-hdr-resolve"),
            bloom_a_view: create_bloom_view(device, (bw, bh), "turbomap-bloom-a"),
            bloom_b_view: create_bloom_view(device, (bw, bh), "turbomap-bloom-b"),
            size,
        }
    }

    /// Recreate every attachment for a new surface size. No-op when the size is
    /// unchanged or degenerate (0 in either dimension) — the depth + colour +
    /// resolve targets must always match the surface or Metal asserts on the
    /// next draw.
    pub(crate) fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if size == self.size || size.0 == 0 || size.1 == 0 {
            return;
        }
        let (bw, bh) = bloom_size(size);
        self.color_view = create_msaa_color_view(device, size);
        self.depth_view = create_depth_view(device, size);
        self.hdr_resolve_view = create_resolve_view(device, size, "turbomap-hdr-resolve");
        self.bloom_a_view = create_bloom_view(device, (bw, bh), "turbomap-bloom-a");
        self.bloom_b_view = create_bloom_view(device, (bw, bh), "turbomap-bloom-b");
        self.size = size;
    }

    pub(crate) fn color_view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    pub(crate) fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    pub(crate) fn hdr_resolve_view(&self) -> &wgpu::TextureView {
        &self.hdr_resolve_view
    }

    pub(crate) fn bloom_a_view(&self) -> &wgpu::TextureView {
        &self.bloom_a_view
    }

    pub(crate) fn bloom_b_view(&self) -> &wgpu::TextureView {
        &self.bloom_b_view
    }
}

/// Bloom textures run at half resolution — cheaper, and the blur is wide enough
/// that the lost detail never shows. Clamp to at least 1×1 so a degenerate
/// surface still builds a valid texture.
fn bloom_size(size: (u32, u32)) -> (u32, u32) {
    ((size.0 / 2).max(1), (size.1 / 2).max(1))
}

fn create_depth_view(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("turbomap-depth"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLES,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Build the multisampled HDR colour attachment the frame pass renders into,
/// before resolving down to the single-sample HDR resolve texture. Re-created
/// on resize; its format is [`HDR_FORMAT`] so the resolve is valid.
fn create_msaa_color_view(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("turbomap-msaa-color"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLES,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// A single-sample HDR resolve target (the opaque `hdr_resolve` and the
/// realistic-water `composite` are identical in shape). Both are resolved into
/// (`RENDER_ATTACHMENT`) and sampled afterwards (`TEXTURE_BINDING`).
fn create_resolve_view(device: &wgpu::Device, size: (u32, u32), label: &str) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// One half-res HDR bloom texture. Both rendered into (blur passes) and sampled
/// (the next blur pass / final composite), so it needs both usages.
fn create_bloom_view(
    device: &wgpu::Device,
    size: (u32, u32),
    label: &str,
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
