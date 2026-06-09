//! Marker rendering — instanced anti-aliased discs in screen space.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::{camera::Camera, map::Marker, scene::Scene};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    corner: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct MarkerInstance {
    screen_centre: [f32; 2],
    radius_px: f32,
    _pad: f32,
    color: [u8; 4],
    _pad2: [u8; 12],
}

pub(crate) struct MarkerPipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u64,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl MarkerPipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-marker-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("marker_shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-marker-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-marker-layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MarkerInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 8,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 12,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 16,
                    shader_location: 4,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-marker-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_layout, instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertices = [
            Vertex { corner: [0.0, 0.0] },
            Vertex { corner: [1.0, 0.0] },
            Vertex { corner: [1.0, 1.0] },
            Vertex { corner: [0.0, 1.0] },
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-marker-quad-vb"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-marker-quad-ib"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_capacity = 256u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-marker-instance"),
            size: instance_capacity * std::mem::size_of::<MarkerInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-marker-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-marker-bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_capacity,
            globals_buffer,
            bind_group,
            device,
            queue,
        }
    }

    pub(crate) fn render(
        &mut self,
        scene: &Scene,
        markers: &[Marker],
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
    ) {
        if markers.is_empty() {
            return;
        }
        let camera = scene.camera();
        let (vw, vh) = scene.viewport_px();
        let viewport_px = (vw as f32, vh as f32);

        let mut instances: Vec<MarkerInstance> = Vec::with_capacity(markers.len());
        for m in markers {
            let world = m.lng_lat.to_world();
            let (sx, sy) = world_to_screen(&camera, (world.x as f32, world.y as f32), viewport_px);
            // Cheap on-screen cull with a generous margin.
            if sx < -m.radius_px
                || sx > viewport_px.0 + m.radius_px
                || sy < -m.radius_px
                || sy > viewport_px.1 + m.radius_px
            {
                continue;
            }
            instances.push(MarkerInstance {
                screen_centre: [sx, sy],
                radius_px: m.radius_px,
                _pad: 0.0,
                // sRGB-authored → linear, since the target re-encodes.
                color: m.color.to_linear_bytes(),
                _pad2: [0; 12],
            });
        }
        if instances.is_empty() {
            return;
        }

        let globals = Globals {
            viewport: [viewport_px.0, viewport_px.1],
            _pad: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        if (instances.len() as u64) > self.instance_capacity {
            let mut new_cap = self.instance_capacity.max(1);
            while new_cap < instances.len() as u64 {
                new_cap *= 2;
            }
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("turbomap-marker-instance"),
                size: new_cap * std::mem::size_of::<MarkerInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("turbomap-marker-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..6, 0, 0..instances.len() as u32);
    }
}

fn world_to_screen(camera: &Camera, world: (f32, f32), viewport_px: (f32, f32)) -> (f32, f32) {
    // Project through the camera's matrix so markers stay anchored to
    // their world position under tilt + bearing. Off-screen falls back
    // to a sentinel the caller's cull filter rejects.
    let p = camera.world_to_screen(
        crate::WorldPoint::new(world.0 as f64, world.1 as f64),
        (viewport_px.0 as f64, viewport_px.1 as f64),
    );
    match p {
        Some((x, y)) => (x as f32, y as f32),
        None => (f32::NEG_INFINITY, f32::NEG_INFINITY),
    }
}
