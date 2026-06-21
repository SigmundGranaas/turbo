//! Route/track rendering as a raised 3D **tube** — a single lit mesh, not a
//! per-tile-clipped flat line. Building it once (CPU) from the full polyline
//! kills the tile-seam "spiderweb" the draped vector line produced, gives the
//! path real volume (so it reads as 3D, not painted-on), and is freely
//! thickenable. The tube floats a touch above the terrain surface and is
//! depth-tested, so hills in front occlude it like a real object.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    origin_delta: [f32; 2],
    _pad0: [f32; 2],
    /// .x = pixels per world unit, .y = radius px, .z = lift (in radii).
    extrude: [f32; 4],
    sun_dir: [f32; 3],
    ambient: f32,
    light_color: [f32; 3],
    _pad1: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct RouteVertex {
    /// Route-local xy (world − route origin) + absolute world z.
    pub(crate) pos: [f32; 3],
    /// Outward tube normal, for lighting.
    pub(crate) normal: [f32; 3],
    pub(crate) color: [u8; 4],
    _pad: [u8; 4],
}

pub(crate) struct RoutePipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    index_count: u32,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl RoutePipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-route-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("route_shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-route-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-route-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RouteVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 24,
                    shader_location: 2,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-route-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            // Cull back faces so the tube's far wall doesn't overdraw the near one.
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            // A real 3D object: depth-tested + written, so terrain in front
            // occludes it and it self-occludes correctly.
            depth_stencil: Some(super::ground_depth_state()),
            multisample: super::multisample_state(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-route-uniform"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-route-bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let vertex_capacity = 1024u64;
        let index_capacity = 2048u64;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-route-vb"),
            size: vertex_capacity * std::mem::size_of::<RouteVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-route-ib"),
            size: index_capacity * std::mem::size_of::<u32>() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            vertex_buffer,
            index_buffer,
            vertex_capacity,
            index_capacity,
            index_count: 0,
            device,
            queue,
        }
    }

    /// Replace the mesh (grows the GPU buffers when needed). Empty clears it.
    pub(crate) fn upload(&mut self, vertices: &[RouteVertex], indices: &[u32]) {
        self.index_count = indices.len() as u32;
        if vertices.is_empty() || indices.is_empty() {
            self.index_count = 0;
            return;
        }
        if vertices.len() as u64 > self.vertex_capacity {
            self.vertex_capacity = (vertices.len() as u64).next_power_of_two();
            self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("turbomap-route-vb"),
                size: self.vertex_capacity * std::mem::size_of::<RouteVertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if indices.len() as u64 > self.index_capacity {
            self.index_capacity = (indices.len() as u64).next_power_of_two();
            self.index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("turbomap-route-ib"),
                size: self.index_capacity * std::mem::size_of::<u32>() as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
        self.queue
            .write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(indices));
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw(
        &self,
        view_proj: [[f32; 4]; 4],
        origin_delta: [f32; 2],
        ppw: f32,
        radius_px: f32,
        lift: f32,
        sun_dir: [f32; 3],
        ambient: f32,
        light_color: [f32; 3],
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if self.index_count == 0 {
            return;
        }
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&Uniforms {
                view_proj,
                origin_delta,
                _pad0: [0.0; 2],
                extrude: [ppw, radius_px, lift, 0.0],
                sun_dir,
                ambient,
                light_color,
                _pad1: 0.0,
            }),
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

/// Build a closed-ring tube mesh around a polyline. `points` are world xy
/// (mercator-normalised) and `world_z[i]` is the absolute world-space height of
/// each centerline vertex (terrain elevation + lift). `origin` re-bases xy for
/// f32 precision; `radius` is in world units; `segments` is the ring resolution.
/// Returns route-local vertices + triangle indices (empty for < 2 points).
pub(crate) fn build_tube(
    points: &[(f64, f64)],
    world_z: &[f32],
    origin: (f64, f64),
    segments: usize,
    color: [u8; 4],
) -> (Vec<RouteVertex>, Vec<u32>) {
    let n = points.len();
    if n < 2 || world_z.len() != n {
        return (Vec::new(), Vec::new());
    }
    let seg = segments.max(3);
    let mut verts: Vec<RouteVertex> = Vec::with_capacity(n * seg);
    let mut indices: Vec<u32> = Vec::with_capacity((n - 1) * seg * 6);

    for i in 0..n {
        // Tangent from neighbours (central difference), in xy.
        let prev = points[i.saturating_sub(1)];
        let next = points[(i + 1).min(n - 1)];
        let mut tx = next.0 - prev.0;
        let mut ty = next.1 - prev.1;
        let tlen = (tx * tx + ty * ty).sqrt();
        if tlen > 1e-12 {
            tx /= tlen;
            ty /= tlen;
        } else {
            tx = 1.0;
            ty = 0.0;
        }
        // Horizontal side (perpendicular to tangent) and world-up.
        let sx = -ty;
        let sy = tx;
        let cx = (points[i].0 - origin.0) as f32;
        let cy = (points[i].1 - origin.1) as f32;
        let cz = world_z[i];
        for k in 0..seg {
            let theta = std::f64::consts::TAU * (k as f64) / (seg as f64);
            let (st, ct) = theta.sin_cos();
            // radial = cosθ·side + sinθ·up  (unit; side & up orthonormal). The
            // shader extrudes the centerline along this to a screen-px radius.
            let nx = ct * sx;
            let ny = ct * sy;
            let nz = st;
            verts.push(RouteVertex {
                // Centerline (same for every ring vertex) + radial; extruded GPU-side.
                pos: [cx, cy, cz],
                normal: [nx as f32, ny as f32, nz as f32],
                color,
                _pad: [0; 4],
            });
        }
    }

    for i in 0..(n - 1) {
        let a = (i * seg) as u32;
        let b = ((i + 1) * seg) as u32;
        for k in 0..seg {
            let k1 = ((k + 1) % seg) as u32;
            let a0 = a + k as u32;
            let a1 = a + k1;
            let b0 = b + k as u32;
            let b1 = b + k1;
            // Two CCW triangles per quad (so back-face culling keeps the
            // outward faces).
            indices.extend_from_slice(&[a0, b0, a1, a1, b0, b1]);
        }
    }
    (verts, indices)
}
