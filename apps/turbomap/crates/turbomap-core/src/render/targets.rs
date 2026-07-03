//! The frame's render attachments — the multisampled colour target the frame
//! pass draws into (matching the surface format, resolved straight to the
//! frame's target view) and the depth buffer it tests against.
//!
//! Pulls the `Map`'s loose `depth_view` + `depth_size` + `msaa_color_view`
//! trio into one type that owns their creation and the resize rule (recreate
//! all attachments, together, when the surface size changes — Metal asserts
//! otherwise). A step toward a self-contained `Renderer`: the targets are the
//! renderer's state, not the map's.

use super::{DEPTH_FORMAT, MSAA_SAMPLES};

/// Multisampled colour + depth attachments, sized to the surface.
pub(crate) struct FrameTargets {
    /// Multisampled colour target the frame pass renders into; resolved to the
    /// frame's target view at pass end. Same format as the surface so the
    /// resolve is valid.
    color_view: wgpu::TextureView,
    /// Depth attachment so the back of a 3D mountain doesn't overdraw the
    /// front. Shared by every draw in the frame pass.
    depth_view: wgpu::TextureView,
    /// The surface format the colour attachment was built with.
    format: wgpu::TextureFormat,
    /// The size all attachments were built for; resize is a no-op until the
    /// surface actually changes dimensions.
    size: (u32, u32),
}

impl FrameTargets {
    pub(crate) fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            color_view: create_msaa_color_view(device, size, format),
            depth_view: create_depth_view(device, size),
            format,
            size,
        }
    }

    /// Recreate every attachment for a new surface size. No-op when the size is
    /// unchanged or degenerate (0 in either dimension) — the depth + colour
    /// targets must always match the surface or Metal asserts on the next draw.
    pub(crate) fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if size == self.size || size.0 == 0 || size.1 == 0 {
            return;
        }
        self.color_view = create_msaa_color_view(device, size, self.format);
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
/// before resolving to the frame's (surface-format) target view. Re-created on
/// resize; its format must match the surface so the resolve is valid.
fn create_msaa_color_view(
    device: &wgpu::Device,
    size: (u32, u32),
    format: wgpu::TextureFormat,
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
