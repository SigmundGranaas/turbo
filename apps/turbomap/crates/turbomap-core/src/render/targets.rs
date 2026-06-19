//! The frame's render attachments — the multisampled colour target the frame
//! pass draws into and the depth buffer it tests against.
//!
//! Pulls the `Map`'s loose `depth_view` + `depth_size` + `msaa_color_view`
//! trio into one type that owns their creation and the resize rule (recreate
//! both, together, when the surface size changes — Metal asserts otherwise).
//! A step toward a self-contained `Renderer`: the targets are the renderer's
//! state, not the map's.

use super::{DEPTH_FORMAT, MSAA_SAMPLES};

/// Multisampled colour + depth attachments, sized to the surface.
pub(crate) struct FrameTargets {
    /// Multisampled colour target the frame pass renders into; resolved to the
    /// single-sampled surface at pass end.
    color_view: wgpu::TextureView,
    /// Depth attachment so the back of a 3D mountain doesn't overdraw the
    /// front. Shared by every pass in the frame.
    depth_view: wgpu::TextureView,
    /// The size both attachments were built for; resize is a no-op until the
    /// surface actually changes dimensions.
    size: (u32, u32),
}

impl FrameTargets {
    pub(crate) fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Self {
        Self {
            color_view: create_msaa_color_view(device, surface_format, size),
            depth_view: create_depth_view(device, size),
            size,
        }
    }

    /// Recreate both attachments for a new surface size. No-op when the size is
    /// unchanged or degenerate (0 in either dimension) — the depth + colour
    /// targets must always match the surface or Metal asserts on the next draw.
    pub(crate) fn resize(
        &mut self,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        size: (u32, u32),
    ) {
        if size == self.size || size.0 == 0 || size.1 == 0 {
            return;
        }
        self.color_view = create_msaa_color_view(device, surface_format, size);
        self.depth_view = create_depth_view(device, size);
        self.size = size;
    }

    pub(crate) fn color_view(&self) -> &wgpu::TextureView {
        &self.color_view
    }

    pub(crate) fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }
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

/// Build the multisampled colour attachment the frame pass renders into,
/// before resolving down to the (single-sample) surface. Re-created on resize;
/// its format matches the surface so the resolve is valid.
fn create_msaa_color_view(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    size: (u32, u32),
) -> wgpu::TextureView {
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
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
