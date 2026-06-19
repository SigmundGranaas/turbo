//! Analytic atmosphere sky pipeline.
//!
//! Draws one full-screen triangle at the *start* of the frame pass with
//! depth disabled, so the terrain (and everything else) paints over it.
//! The sky is therefore visible only where no geometry covers the
//! screen — the horizon band that appears as the map tilts. Colour comes
//! from [`crate::sun::atmosphere`], the same time-of-day palette the
//! terrain shading and aerial-perspective haze use.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct SkyGlobals {
    /// Inverse RTC view-projection — reconstructs world-space ray dirs.
    pub inv_view_proj: [[f32; 4]; 4],
    pub sun_dir: [f32; 3],
    pub sun_intensity: f32,
    pub zenith_color: [f32; 3],
    pub _p0: f32,
    pub horizon_color: [f32; 3],
    pub _p1: f32,
    pub sun_color: [f32; 3],
    pub _p2: f32,
}

pub(crate) struct SkyPipeline {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    queue: Arc<wgpu::Queue>,
}

impl SkyPipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-sky-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sky_shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-sky-bgl"),
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
            label: Some("turbomap-sky-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-sky-pipeline"),
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
            // Depth disabled (compare Always, no write): the sky is a
            // backdrop the terrain overdraws, and must not occlude or be
            // occluded by the cleared depth buffer.
            depth_stencil: Some(super::overlay_depth_state()),
            multisample: super::multisample_state(),
            multiview_mask: None,
            cache: None,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-sky-globals"),
            size: std::mem::size_of::<SkyGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-sky-bg"),
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

    /// Upload this frame's sky params and draw the full-screen triangle.
    /// Call first inside the frame pass, before any geometry.
    pub(crate) fn draw(&self, globals: &SkyGlobals, pass: &mut wgpu::RenderPass<'_>) {
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(globals));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
