//! Progressive, world-locked ambient occlusion.
//!
//! The cast-shadow heightfield ([`super::shadow`]) already gives us a
//! camera-centred, ground-pinned relief field. This module bakes a horizon-based
//! ambient-occlusion field from it — the fraction of the sky hemisphere each
//! ground cell can see — into a world-locked texture the terrain shader samples.
//!
//! The bake is **progressive**: each frame the [`AoField`] runs one batch of
//! azimuth directions with additive blending (`ao_shader.wgsl`), so a cheap
//! first pass lands immediately and the quality climbs over the next few frames.
//! Once all directions are in, the field is **cached** — AO is sun-independent,
//! so it's reused across pans (within the region), tilts, orbits and the whole
//! day/night cycle, recomputed only when the terrain region or resident DEM
//! changes. That's the "save the info" half: the texture *is* the saved result.

use bytemuck::{Pod, Zeroable};

/// Total azimuth directions in a full bake. More = smoother AO, more frames to
/// converge. 16 reads as proper directional occlusion without banding.
pub(crate) const TOTAL_DIRS: u32 = 16;
/// Directions baked per frame. Set to the full count so the field is baked in
/// ONE atomic pass: the earlier multi-frame accumulation exposed intermediate
/// (too-bright, partial) states every time the region re-keyed, which read as
/// flicker. A full bake only runs on a settle and then caches, so it's cheap
/// enough to do at once, and the AO is correct the instant it lands. The
/// accumulation structure is kept so the cadence can be re-tuned later.
pub(crate) const BATCH: u32 = TOTAL_DIRS;
/// Heightfield texels marched per direction — the AO reach. Scaled with
/// HEIGHT_DIM (256) so the world reach is a fixed fraction of the visible extent.
const STEPS: u32 = 43;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct AoParams {
    inv_dim: f32,
    texel_world: f32,
    dir_start: f32,
    dir_count: f32,
    total_dirs: f32,
    steps: f32,
    _pad: [f32; 2],
}

/// Identity of the region an AO field was baked for. Deliberately **excludes the
/// sun** (AO is ambient, sun-independent) so time-of-day changes never trigger a
/// rebake — only a settle in a new region or freshly-streamed DEM does.
#[derive(PartialEq, Eq, Clone)]
pub(crate) struct AoKey {
    /// Absolute world origin of the field (bit patterns so the key is `Eq`).
    pub origin: [u32; 2],
    /// World extent of the field.
    pub size: u32,
    /// Monotonic DEM-insert count — a freshly-filled region rebakes.
    pub dem_inserts: u64,
}

pub(crate) struct AoField {
    pipeline: wgpu::RenderPipeline,
    params_buffer: wgpu::Buffer,
    params_bind_group: wgpu::BindGroup,
    queue: std::sync::Arc<wgpu::Queue>,
    /// How many directions of the current key are baked so far (`>= TOTAL_DIRS`
    /// means converged → no more passes until the key changes).
    pub(crate) done: u32,
    /// The region the in-progress / converged field is keyed to.
    pub(crate) key: Option<AoKey>,
}

impl AoField {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: std::sync::Arc<wgpu::Queue>,
        height_tex_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-ao-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ao_shader.wgsl").into()),
        });

        let params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-ao-params-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-ao-layout"),
            bind_group_layouts: &[Some(height_tex_layout), Some(&params_bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-ao-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ao"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ao"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: super::AO_FORMAT,
                    // Additive: each frame's direction batch adds its normalised
                    // contribution, so the field accumulates toward the mean.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-ao-params"),
            size: std::mem::size_of::<AoParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-ao-params-bg"),
            layout: &params_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            params_buffer,
            params_bind_group,
            queue,
            done: 0,
            key: None,
        }
    }

    /// Run the next direction batch for `key` into `ao_view`, reading the
    /// heightfield via `height_bg`. `world_size` is the field's world extent (to
    /// size the march step). On a new region this clears the field first; once
    /// `done >= TOTAL_DIRS` it's a no-op (the cached field stands). Returns
    /// whether a pass was issued (for metrics/debug).
    pub(crate) fn accumulate(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        height_bg: &wgpu::BindGroup,
        ao_view: &wgpu::TextureView,
        key: AoKey,
        world_size: f32,
    ) -> bool {
        let new_region = self.key.as_ref() != Some(&key);
        if new_region {
            self.key = Some(key);
            self.done = 0;
        }
        if self.done >= TOTAL_DIRS {
            return false;
        }
        let clear = self.done == 0;
        let batch = BATCH.min(TOTAL_DIRS - self.done);

        let params = AoParams {
            inv_dim: 1.0 / super::shadow::HEIGHT_DIM as f32,
            texel_world: world_size / super::shadow::HEIGHT_DIM as f32,
            dir_start: self.done as f32,
            dir_count: batch as f32,
            total_dirs: TOTAL_DIRS as f32,
            steps: STEPS as f32,
            _pad: [0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        let load = if clear {
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
        } else {
            wgpu::LoadOp::Load
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("turbomap-ao-accumulate"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: ao_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, height_bg, &[]);
            pass.set_bind_group(1, &self.params_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.done += batch;
        true
    }
}
