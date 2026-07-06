//! Built-in custom layers (plan D4).
//!
//! The demo/diagnostic `flow-field` kind: an animated, world-anchored field
//! of tapered streaks whose direction comes from a procedural hash of the
//! ABSOLUTE world cell (so arrows stay pinned to the ground through pans and
//! zooms) and whose sway animates with the frame clock. Everything is
//! GPU-procedural — `prepare` uploads one uniform, the vertex shader
//! synthesises every streak from its instance index — so the layer is a
//! faithful minimal template for real custom layers: one Rust impl + one
//! WGSL file, portable to every host (plan gate: desktop + web from the
//! same code).

use turbomap_core::{CustomFrameCtx, CustomLayer, CustomLayerInit, CustomPhase};

/// On-screen spacing between streaks, in logical pixels.
const CELL_PX: f64 = 48.0;
/// Grid dimension cap per axis (64² streaks max — a bounded, trivial draw).
const MAX_GRID: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    view_proj: [[f32; 4]; 4],
    /// xy = RTC world position of the grid's first cell centre;
    /// z = world cell size; w = time (s).
    grid: [f32; 4],
    /// x,y = grid dims (as f32); z,w = absolute cell index of the first
    /// cell (as f32 — the world-stable hash seed).
    dims: [f32; 4],
    /// Streak colour (premultipliable straight alpha).
    color: [f32; 4],
}

pub struct FlowFieldLayer {
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    queue: std::sync::Arc<wgpu::Queue>,
    instances: u32,
}

impl FlowFieldLayer {
    pub fn new(init: &CustomLayerInit) -> Self {
        let device = &init.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-flow-field"),
            source: wgpu::ShaderSource::Wgsl(include_str!("flow_field.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-flow-field-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-flow-field-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-flow-field-pipeline"),
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
                    format: init.color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // Overlay contribution: the MSAA pass has a depth attachment, so
            // the pipeline must declare one — but streaks float above the
            // scene: never test, never write.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: init.depth_format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: init.sample_count,
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-flow-field-uniform"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-flow-field-bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            uniform_buf,
            bind_group,
            queue: init.queue.clone(),
            instances: 0,
        }
    }
}

impl CustomLayer for FlowFieldLayer {
    fn phase(&self) -> CustomPhase {
        CustomPhase::Overlay
    }

    fn prepare(&mut self, ctx: &CustomFrameCtx) {
        // World cell size that reads as ~CELL_PX on screen; the grid covers
        // the viewport with margin (pitch pulls in far ground, so overshoot).
        let cell = CELL_PX / ctx.pixels_per_world_unit.max(1e-9);
        let (vw, vh) = ctx.viewport_px;
        let span_px = (vw.max(vh) as f64) * 1.6;
        let n = ((span_px / CELL_PX).ceil() as u32 + 2).min(MAX_GRID);
        // Snap the grid to the ABSOLUTE world cell lattice so streaks are
        // pinned to the ground: the first cell's index seeds the hash, and
        // its RTC offset places it — full precision because the subtraction
        // happens in f64 before the cast.
        let half = 0.5 * n as f64 * cell;
        let first_cell_x = ((ctx.origin.0 - half) / cell).floor();
        let first_cell_y = ((ctx.origin.1 - half) / cell).floor();
        let rtc_x = (first_cell_x * cell + 0.5 * cell) - ctx.origin.0;
        let rtc_y = (first_cell_y * cell + 0.5 * cell) - ctx.origin.1;
        self.instances = n * n;
        let u = Uniform {
            view_proj: ctx.view_proj,
            grid: [rtc_x as f32, rtc_y as f32, cell as f32, ctx.time_s],
            // Hash seeds wrap at 2^20 cells — f32-exact and world-stable
            // within any real viewing session.
            dims: [
                n as f32,
                n as f32,
                (first_cell_x.rem_euclid(1_048_576.0)) as f32,
                (first_cell_y.rem_euclid(1_048_576.0)) as f32,
            ],
            // A readable teal, translucent so the basemap shows through.
            color: [0.05, 0.45, 0.5, 0.55],
        };
        self.queue
            .write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&u));
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.instances == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        // 6 vertices per streak (two triangles), synthesised in the shader.
        pass.draw(0..6, 0..self.instances);
    }
}
