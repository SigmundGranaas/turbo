//! Terrain cast shadows via a per-fragment GPU horizon-march.
//!
//! The Lambertian `N·L` term in `shader.wgsl` is *self*-shading only: a slope
//! facing away from the sun darkens, but a peak does not throw a shadow across
//! the valley behind it (each terrain draw binds only its own DEM tile at
//! `@group(2)` and can't see the neighbouring caster).
//!
//! Cast shadows therefore need a *frame-global* view of the relief. We assemble
//! the visible terrain into one camera-centred square **heightfield** (world-z,
//! sampled across tiles from the shared [`TerrainCache`](super::terrain::TerrainCache))
//! and upload it as a texture; the terrain fragment shader marches it toward the
//! sun **per-pixel, every frame** (see `shader.wgsl`).
//!
//! Per-fragment + every-frame is what makes the shadow **sharp** (no precomputed
//! low-res visibility grid to blur/upscale) and **stable under pan** (the
//! heightfield is ground-pinned, so marching world coordinates yields the same
//! shadow as the camera moves — shadows stay welded to the terrain). The grid is
//! re-assembled only when the camera settles in a new region, or the sun /
//! resident DEM change — so its CPU cost stays off the pan path.

use std::sync::Arc;

/// Per-side resolution of the heightfield grid. Higher = finer occluders (and a
/// larger CPU re-assembly when it re-centres). The per-fragment march keeps the
/// shadow EDGE sharp regardless of this — `HEIGHT_DIM` sets occluder fidelity,
/// not edge crispness. 192² re-assembles in a few ms, and only on settle (never
/// mid-pan), so it doesn't hitch panning.
pub(crate) const HEIGHT_DIM: usize = 192;

/// GPU side of the cast-shadow feature: a single `HEIGHT_DIM²` `R32Float`
/// heightfield texture (world-z elevation per cell) plus its bind group, bound
/// at `@group(3)` of the raster pipeline. Map-level (one per renderer), uploaded
/// whenever the assembled region changes. The fragment shader marches it toward
/// the sun per-pixel; this texture is just the relief, not a precomputed result.
///
/// `R32Float` is sampled *unfiltered* (nearest) so it needs no `FLOAT32_FILTERABLE`
/// device feature — the per-pixel march gives crisp edges without bilinear relief.
pub(crate) struct ShadowMap {
    pub(crate) layout: Arc<wgpu::BindGroupLayout>,
    texture: wgpu::Texture,
    pub(crate) bind_group: wgpu::BindGroup,
    queue: Arc<wgpu::Queue>,
}

impl ShadowMap {
    pub(crate) fn new(device: &wgpu::Device, queue: &Arc<wgpu::Queue>) -> Self {
        let layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("turbomap-shadow-bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            // Unfiltered float — no FLOAT32_FILTERABLE needed.
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
            },
        ));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("turbomap-shadow-heightfield"),
            size: wgpu::Extent3d {
                width: HEIGHT_DIM as u32,
                height: HEIGHT_DIM as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Initialise to sea level (0 world-z) so a frame before the first upload
        // (or with shadows off) marches a flat field → no spurious darkening.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&vec![0.0f32; HEIGHT_DIM * HEIGHT_DIM]),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((HEIGHT_DIM * 4) as u32),
                rows_per_image: Some(HEIGHT_DIM as u32),
            },
            wgpu::Extent3d {
                width: HEIGHT_DIM as u32,
                height: HEIGHT_DIM as u32,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-shadow-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-shadow-bg"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            layout,
            texture,
            bind_group,
            queue: queue.clone(),
        }
    }

    /// Upload the assembled heightfield (world-z elevations, row-major, `HEIGHT_DIM²`).
    /// A size mismatch is ignored rather than panicking, leaving prior contents.
    pub(crate) fn upload_heights(&self, heights: &[f32]) {
        if heights.len() != HEIGHT_DIM * HEIGHT_DIM {
            log::warn!(
                "turbomap: shadow heightfield len {} != {}, skipping upload",
                heights.len(),
                HEIGHT_DIM * HEIGHT_DIM,
            );
            return;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(heights),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((HEIGHT_DIM * 4) as u32),
                rows_per_image: Some(HEIGHT_DIM as u32),
            },
            wgpu::Extent3d {
                width: HEIGHT_DIM as u32,
                height: HEIGHT_DIM as u32,
                depth_or_array_layers: 1,
            },
        );
    }
}
