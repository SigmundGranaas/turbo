//! Icon/sprite rendering — instanced textured quads sampling the built-in
//! sprite atlas. Mirrors the text pipeline's single-pass shape: one shared
//! instance buffer staged by per-vector-layer `prepare` calls and uploaded
//! once by `finish_frame`. Icons draw after geometry and *before* text, so
//! a label centred on a shield sprite sits on top of it.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::{
    camera::Camera,
    scene::Scene,
    sprite::{SpriteAtlas, SPRITE_ATLAS_H, SPRITE_ATLAS_W},
};

use super::vector_cache::VectorMeshCache;

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
struct IconInstance {
    screen_centre: [f32; 2],
    size_px: [f32; 2],
    atlas_origin: [f32; 2],
    atlas_size: [f32; 2],
    /// Linear-RGBA tint the monochrome SDF shape is coloured with.
    tint: [u8; 4],
    /// NDC depth of the icon's ground anchor, so a building in front of it
    /// occludes the marker (3D coherence). At pitch 0 this equals the ground
    /// depth, so flat maps composite the icon on top exactly as before.
    depth: f32,
}

/// Half-open instance range one layer's icons occupy in the shared buffer.
pub(crate) struct PreparedIcons {
    start: u32,
    end: u32,
}

pub(crate) struct IconPipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u64,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    atlas: SpriteAtlas,
    staged: Vec<IconInstance>,
    frame_viewport: [f32; 2],
    /// Screen boxes of icons placed this frame — shared across all vector
    /// layers (reset in `begin_frame`), so POI dots don't pile on top of
    /// each other in dense areas. Earlier layers win the space.
    frame_placed: Vec<crate::text::Aabb>,
}

impl IconPipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-icon-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("icon_shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-icon-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-icon-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
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
            array_stride: std::mem::size_of::<IconInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 32,
                    shader_location: 5,
                },
                // depth: f32 @ 36 — NDC depth of the ground anchor.
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 36,
                    shader_location: 6,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-icon-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout, instance_layout],
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
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(super::overlay_occluded_depth_state()),
            multisample: super::multisample_state(),
            multiview_mask: None,
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
            label: Some("turbomap-icon-quad-vb"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-icon-quad-ib"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_capacity = 512u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-icon-instance"),
            size: instance_capacity * std::mem::size_of::<IconInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-icon-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // The procedural atlas: built once, uploaded once. Authored sRGB →
        // sampled as sRGB so the read decodes to linear like every other
        // colour the renderer touches.
        let atlas = SpriteAtlas::new();
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("turbomap-icon-atlas"),
            size: wgpu::Extent3d {
                width: SPRITE_ATLAS_W,
                height: SPRITE_ATLAS_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            atlas.bitmap(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SPRITE_ATLAS_W),
                rows_per_image: Some(SPRITE_ATLAS_H),
            },
            wgpu::Extent3d {
                width: SPRITE_ATLAS_W,
                height: SPRITE_ATLAS_H,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-icon-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-icon-bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
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
            atlas,
            staged: Vec::new(),
            frame_viewport: [0.0; 2],
            frame_placed: Vec::new(),
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.staged.clear();
        self.frame_placed.clear();
    }

    /// Stage one vector layer's icons: project each request to screen, look
    /// up its sprite, size it (height = requested px, width by aspect), and
    /// append an instance. Returns the layer's instance range for `draw`.
    pub(crate) fn prepare(
        &mut self,
        scene: &Scene,
        cache: &mut VectorMeshCache,
        marker_anchors: &std::collections::HashSet<(u32, u32)>,
    ) -> PreparedIcons {
        let camera = scene.camera();
        let (vw, vh) = scene.viewport_px();
        let viewport_px = (vw as f32, vh as f32);
        self.frame_viewport = [viewport_px.0, viewport_px.1];

        let start = self.staged.len() as u32;
        for tile in scene.visible_tiles() {
            let Some(entry) = cache.get(tile) else { continue };
            for icon in &entry.icons {
                // A POI marker's dot only draws if its label survived — dot
                // and label cull as one unit (no orphan dots).
                if icon.requires_label
                    && !marker_anchors.contains(&crate::text::anchor_key(icon.world_pos))
                {
                    continue;
                }
                let Some(info) = self.atlas.get(&icon.sprite) else {
                    continue;
                };
                let (sx, sy) = world_to_screen(&camera, icon.world_pos, viewport_px);
                let h = icon.size_px;
                let w = icon.size_px * info.width as f32 / info.height.max(1) as f32;
                if sx < -w || sx > viewport_px.0 + w || sy < -h || sy > viewport_px.1 + h {
                    continue;
                }
                // Frame-wide de-cluttering: drop an icon that would crowd an
                // already-placed one. The positive pad enforces a gap so
                // dense POI areas read as spaced markers, not a smear.
                let bbox = crate::text::Aabb {
                    min_x: sx - w * 0.5,
                    min_y: sy - h * 0.5,
                    max_x: sx + w * 0.5,
                    max_y: sy + h * 0.5,
                }
                .pad(3.0);
                if self.frame_placed.iter().any(|p| p.overlaps(bbox)) {
                    continue;
                }
                self.frame_placed.push(bbox);
                let depth = camera.world_ndc_depth(
                    crate::geo::WorldPoint::new(icon.world_pos.0 as f64, icon.world_pos.1 as f64),
                    (viewport_px.0 as f64, viewport_px.1 as f64),
                );
                self.staged.push(IconInstance {
                    screen_centre: [sx, sy],
                    size_px: [w, h],
                    atlas_origin: [
                        info.atlas_x as f32 / SPRITE_ATLAS_W as f32,
                        info.atlas_y as f32 / SPRITE_ATLAS_H as f32,
                    ],
                    atlas_size: [
                        info.width as f32 / SPRITE_ATLAS_W as f32,
                        info.height as f32 / SPRITE_ATLAS_H as f32,
                    ],
                    // sRGB-authored tint → linear (the target re-encodes).
                    tint: icon.color.to_linear_bytes(),
                    depth,
                });
            }
        }
        PreparedIcons {
            start,
            end: self.staged.len() as u32,
        }
    }

    /// Upload the frame's globals + staged instances (grow as needed).
    pub(crate) fn finish_frame(&mut self) {
        if self.staged.is_empty() {
            return;
        }
        let globals = Globals {
            viewport: self.frame_viewport,
            _pad: [0.0; 2],
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        if (self.staged.len() as u64) > self.instance_capacity {
            let mut new_cap = self.instance_capacity.max(1);
            while new_cap < self.staged.len() as u64 {
                new_cap *= 2;
            }
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("turbomap-icon-instance"),
                size: new_cap * std::mem::size_of::<IconInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&self.staged));
    }

    pub(crate) fn draw(&self, prepared: &PreparedIcons, pass: &mut wgpu::RenderPass<'_>) {
        if prepared.start == prepared.end {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..6, 0, prepared.start..prepared.end);
    }
}

fn world_to_screen(camera: &Camera, world: (f32, f32), viewport_px: (f32, f32)) -> (f32, f32) {
    let p = camera.world_to_screen(
        crate::WorldPoint::new(world.0 as f64, world.1 as f64),
        (viewport_px.0 as f64, viewport_px.1 as f64),
    );
    match p {
        Some((x, y)) => (x as f32, y as f32),
        None => (f32::NEG_INFINITY, f32::NEG_INFINITY),
    }
}
