//! Ground "floor" backstop pipeline.
//!
//! A full-screen pass (after the sky, before the terrain) that ray-casts each
//! pixel onto a virtual ground plane just below sea level and fills it with a
//! neutral sea-grey. Where terrain tiles haven't streamed in yet — or a
//! mixed-LOD seam gapes — the hole shows the floor instead of see-through
//! sky/clear. It writes depth, so real terrain (which sits higher) overdraws it
//! wherever it exists. See `floor_shader.wgsl`.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct FloorGlobals {
    /// RTC view-projection (origin = camera centre).
    pub view_proj: [[f32; 4]; 4],
    /// Inverse of `view_proj` — reconstructs world-space ray dirs per pixel.
    pub inv_view_proj: [[f32; 4]; 4],
    /// Fill colour (linear RGBA).
    pub color: [f32; 4],
    /// Plane height in world-z (slightly below sea level).
    pub floor_z: f32,
    /// Earth-curvature drop coefficient (π·cos³φ) — matches the terrain shader.
    pub curvature_coeff: f32,
    /// 0 = don't draw (flat 2D map); 1 = draw.
    pub enabled: f32,
    pub _pad: f32,
}

pub(crate) struct FloorPipeline {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    queue: Arc<wgpu::Queue>,
}

impl FloorPipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-floor-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("floor_shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-floor-bgl"),
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
            label: Some("turbomap-floor-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-floor-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            // Writes depth (so the terrain, drawn after and higher, overdraws it).
            depth_stencil: Some(super::ground_depth_state()),
            multisample: super::multisample_state(),
            multiview_mask: None,
            cache: None,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-floor-globals"),
            size: std::mem::size_of::<FloorGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-floor-bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            globals_buffer,
            bind_group,
            queue,
        }
    }

    /// Upload this frame's floor params and draw the full-screen triangle. Call
    /// after the sky and before the terrain. No-op visually when `enabled == 0`.
    pub(crate) fn draw(&self, globals: &FloorGlobals, pass: &mut wgpu::RenderPass<'_>) {
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(globals));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
