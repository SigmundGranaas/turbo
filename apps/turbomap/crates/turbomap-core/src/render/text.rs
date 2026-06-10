//! Text rendering pipeline. Reads `LabelRequest`s out of the visible
//! vector-tile cache, lays them out per frame against a single GPU atlas,
//! does AABB collision in screen space, and draws the surviving glyphs.

use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

/// Minimum screen-space spacing between two line labels carrying the *same*
/// text. A long road clipped across several tiles yields one along-line
/// label per piece; without this they'd stack up (the doubled-name bug).
/// It doubles as the repeat distance for genuinely long roads. MapLibre's
/// `symbol-spacing` default is 250px; we match it.
const LINE_LABEL_REPEAT_PX: f32 = 250.0;

use crate::{
    camera::Camera,
    scene::Scene,
    tessellate::LabelRequest,
    text::{Aabb, FontAtlas, LayoutCache, ATLAS_SIZE},
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
struct TextInstance {
    screen_origin: [f32; 2],
    screen_size: [f32; 2],
    atlas_origin: [f32; 2],
    atlas_size: [f32; 2],
    color: [u8; 4],
    halo_color: [u8; 4],
    /// Halo half-width as a normalized SDF threshold offset beyond the
    /// glyph contour (0 = no halo). px → this is converted at stage time.
    halo_width: f32,
    /// Rotation of the glyph quad about `pivot`, in radians (0 for
    /// axis-aligned point labels). Line labels set this to the local path
    /// tangent so glyphs follow the curve.
    angle: f32,
    /// Screen-space point the quad rotates about (the glyph's pen point on
    /// the path). Ignored when `angle == 0`.
    pivot: [f32; 2],
}

/// Output of one per-vector-layer [`TextPipeline::prepare`] call: the
/// half-open instance range this layer's surviving glyphs occupy in
/// the pipeline's shared instance buffer (uploaded by `finish_frame`).
pub(crate) struct PreparedText {
    start: u32,
    end: u32,
}

pub(crate) struct TextPipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u64,
    globals_buffer: wgpu::Buffer,
    atlas_texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    atlas: FontAtlas,
    /// Anchor-relative laid-out glyphs, keyed by (text, font_size). Avoids
    /// per-frame layout for the steady-state visible label set.
    layout_cache: LayoutCache,
    /// Frame-local staging: instances appended by per-layer `prepare`
    /// calls, uploaded once by `finish_frame`. One shared buffer (the
    /// pipeline is shared across vector layers) — per-layer ranges are
    /// carried in [`PreparedText`].
    staged: Vec<TextInstance>,
    /// Viewport recorded by the frame's `prepare` calls, written into
    /// the globals uniform by `finish_frame`.
    frame_viewport: [f32; 2],
}

impl TextPipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        use wgpu::util::DeviceExt;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-text-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("text_shader.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-text-bgl"),
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
            label: Some("turbomap-text-layout"),
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
            array_stride: std::mem::size_of::<TextInstance>() as u64,
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
                    offset: 36,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 40,
                    shader_location: 7,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 32,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 44,
                    shader_location: 8,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 48,
                    shader_location: 9,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-text-pipeline"),
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
            // The Map's single frame pass always carries a depth
            // attachment. Text is a screen-space overlay — always in
            // front, never writing z.
            depth_stencil: Some(super::overlay_depth_state()),
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
            label: Some("turbomap-text-quad-vb"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-text-quad-ib"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_capacity = 4096u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-text-instance"),
            size: instance_capacity * std::mem::size_of::<TextInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-text-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("turbomap-text-atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-text-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-text-bg"),
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
            atlas_texture,
            bind_group,
            device,
            queue,
            atlas: FontAtlas::new(),
            layout_cache: LayoutCache::new(),
            staged: Vec::new(),
            frame_viewport: [0.0; 2],
        }
    }

    /// Register a fallback font face for scripts the bundled default
    /// doesn't cover (CJK, Arabic, …). New glyphs pack into the existing
    /// atlas and are re-uploaded on the next dirty frame. Returns `false`
    /// if the bytes don't parse as a font.
    pub(crate) fn add_fallback_face(&mut self, bytes: Vec<u8>) -> bool {
        self.atlas.add_fallback_face(bytes)
    }

    /// Reset the frame-local staging list. Call once per `Map::render`
    /// before the per-layer `prepare` calls.
    pub(crate) fn begin_frame(&mut self) {
        self.staged.clear();
    }

    /// Lay out labels for one vector layer's currently visible tiles
    /// (per-layer collision set, like the old per-layer text pass) and
    /// stage the surviving glyph instances. The returned range is drawn
    /// by `draw` after `finish_frame` uploads the shared buffer.
    pub(crate) fn prepare(
        &mut self,
        scene: &Scene,
        cache: &mut VectorMeshCache,
    ) -> PreparedText {
        let camera = scene.camera();
        let (vw, vh) = scene.viewport_px();
        let viewport_px = (vw as f32, vh as f32);

        // 1. Collect all labels from visible tiles, sort by distance from
        // camera centre (closest wins on collision), and reject any that
        // overlap an already-placed label.
        // Pull (anchor, font_size, color, text) tuples out of the cache —
        // strings are reasonably small and labels per tile rarely exceed
        // ~50, so the per-frame allocation is cheap. Avoiding it would
        // require interior-mutability shenanigans on the cache.
        let world_centre = camera.center.to_world();
        let mut candidates: Vec<LabelRequest> = Vec::new();
        for tile in scene.visible_tiles() {
            if let Some(entry) = cache.get(tile) {
                candidates.extend(entry.labels.iter().cloned());
            }
        }
        // Placement priority: higher rank first (important labels win
        // collisions), then nearest-to-centre as the tiebreak among equals.
        candidates.sort_by(|a, b| {
            b.rank
                .partial_cmp(&a.rank)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    let da = sq_dist(a.world_pos, (world_centre.x as f32, world_centre.y as f32));
                    let db = sq_dist(b.world_pos, (world_centre.x as f32, world_centre.y as f32));
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        let start = self.staged.len() as u32;
        let mut placed: Vec<Aabb> = Vec::new();
        // Screen anchors of already-placed line labels, keyed by text — the
        // cross-tile dedup / repeat-spacing index for road names.
        let mut placed_line_text: HashMap<String, Vec<(f32, f32)>> = HashMap::new();
        for label in candidates {
            // Line-placed labels (road names) follow a projected centerline.
            if let Some(path) = &label.path {
                self.stage_line_label(
                    &camera,
                    viewport_px,
                    &label,
                    path,
                    &mut placed,
                    &mut placed_line_text,
                );
                continue;
            }
            // Project world → screen.
            let screen = world_to_screen(&camera, label.world_pos, viewport_px);
            // Skip labels off-screen with a small margin so we don't pay to
            // lay out distant text.
            if screen.0 < -200.0
                || screen.0 > viewport_px.0 + 200.0
                || screen.1 < -50.0
                || screen.1 > viewport_px.1 + 50.0
            {
                continue;
            }
            // Cache hit: cached glyphs are anchor-relative (centred around
            // origin). Translate by `screen` to get this frame's positions.
            let cached: Vec<crate::text::LayoutGlyph> = self
                .layout_cache
                .get_or_compute(&label.text, label.font_size_px, &mut self.atlas)
                .to_vec();
            let mut aabb_xy = (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            );
            let translated: Vec<crate::text::LayoutGlyph> = cached
                .into_iter()
                .map(|g| {
                    let x = g.screen_x + screen.0;
                    let y = g.screen_y + screen.1;
                    aabb_xy.0 = aabb_xy.0.min(x);
                    aabb_xy.1 = aabb_xy.1.min(y);
                    aabb_xy.2 = aabb_xy.2.max(x + g.width);
                    aabb_xy.3 = aabb_xy.3.max(y + g.height);
                    crate::text::LayoutGlyph {
                        screen_x: x,
                        screen_y: y,
                        ..g
                    }
                })
                .collect();
            if translated.is_empty() {
                continue;
            }
            let aabb = Aabb {
                min_x: aabb_xy.0,
                min_y: aabb_xy.1,
                max_x: aabb_xy.2,
                max_y: aabb_xy.3,
            };
            let padded = aabb.pad(2.0);
            if placed.iter().any(|p| p.overlaps(padded)) {
                continue;
            }
            placed.push(padded);
            // sRGB-authored → linear, since the target re-encodes on write.
            let color = label.color.to_linear_bytes();
            let halo_color = label.halo_color.to_linear_bytes();
            // px → normalized SDF units. The atlas encodes 128/SDF_PAD u8
            // units per glyph pixel (see text::generate_sdf); /255 puts it
            // in the [0,1] frame the shader samples.
            let halo_width = label.halo_width
                * (128.0 / crate::text::SDF_PAD as f32)
                / 255.0;
            for g in translated {
                self.staged.push(TextInstance {
                    screen_origin: [g.screen_x, g.screen_y],
                    screen_size: [g.width, g.height],
                    atlas_origin: [g.atlas_x / ATLAS_SIZE as f32, g.atlas_y / ATLAS_SIZE as f32],
                    atlas_size: [g.atlas_w / ATLAS_SIZE as f32, g.atlas_h / ATLAS_SIZE as f32],
                    color,
                    halo_color,
                    halo_width,
                    // Point labels are axis-aligned: zero rotation makes the
                    // pivot irrelevant, so the shader path is unchanged.
                    angle: 0.0,
                    pivot: [g.screen_x, g.screen_y],
                });
            }
        }

        // Record the viewport so `finish_frame` can fill the shared
        // globals uniform (every layer's scene syncs from the same Map
        // camera + viewport, so last-writer-wins is exact).
        self.frame_viewport = [viewport_px.0, viewport_px.1];

        PreparedText {
            start,
            end: self.staged.len() as u32,
        }
    }

    /// Stage one line-placed label: project its world centerline to screen,
    /// run the glyphs along the curve, collide as a single AABB, and emit
    /// the rotated glyph instances.
    fn stage_line_label(
        &mut self,
        camera: &Camera,
        viewport_px: (f32, f32),
        label: &LabelRequest,
        path: &[(f32, f32)],
        placed: &mut Vec<Aabb>,
        placed_line_text: &mut HashMap<String, Vec<(f32, f32)>>,
    ) {
        // Cheap cull on the anchor (path midpoint) before any layout.
        let anchor = world_to_screen(camera, label.world_pos, viewport_px);
        if anchor.0 < -300.0
            || anchor.0 > viewport_px.0 + 300.0
            || anchor.1 < -120.0
            || anchor.1 > viewport_px.1 + 120.0
        {
            return;
        }
        // Cross-tile dedup / repeat spacing: drop this label if the same
        // road name was already placed within the repeat distance. Sorting
        // put the piece nearest the camera centre first, so the survivor is
        // the best-placed one.
        if let Some(prev) = placed_line_text.get(&label.text) {
            let min_sq = LINE_LABEL_REPEAT_PX * LINE_LABEL_REPEAT_PX;
            if prev.iter().any(|&p| sq_dist(p, anchor) < min_sq) {
                return;
            }
        }
        let screen_path: Vec<(f32, f32)> = path
            .iter()
            .map(|&p| world_to_screen(camera, p, viewport_px))
            .collect();
        // Behind-camera / non-finite projection → skip the whole label.
        if screen_path
            .iter()
            .any(|p| !p.0.is_finite() || !p.1.is_finite())
        {
            return;
        }
        let Some(glyphs) = crate::text::layout_along_path(
            &label.text,
            label.font_size_px,
            &screen_path,
            &mut self.atlas,
        ) else {
            return;
        };

        // Collision AABB: the axis-aligned bound of every rotated glyph quad.
        let mut bb = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        for g in &glyphs {
            for (cx, cy) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let (rx, ry) = rotate_about(
                    (g.screen_x + cx * g.width, g.screen_y + cy * g.height),
                    g.pivot,
                    g.angle,
                );
                bb.0 = bb.0.min(rx);
                bb.1 = bb.1.min(ry);
                bb.2 = bb.2.max(rx);
                bb.3 = bb.3.max(ry);
            }
        }
        let padded = Aabb {
            min_x: bb.0,
            min_y: bb.1,
            max_x: bb.2,
            max_y: bb.3,
        }
        .pad(2.0);
        if placed.iter().any(|p| p.overlaps(padded)) {
            return;
        }
        placed.push(padded);
        // Record this placement so later same-name pieces honour the repeat
        // distance (cross-tile dedup).
        placed_line_text
            .entry(label.text.clone())
            .or_default()
            .push(anchor);

        let color = label.color.to_linear_bytes();
        let halo_color = label.halo_color.to_linear_bytes();
        let halo_width = label.halo_width * (128.0 / crate::text::SDF_PAD as f32) / 255.0;
        for g in glyphs {
            self.staged.push(TextInstance {
                screen_origin: [g.screen_x, g.screen_y],
                screen_size: [g.width, g.height],
                atlas_origin: [g.atlas_x / ATLAS_SIZE as f32, g.atlas_y / ATLAS_SIZE as f32],
                atlas_size: [g.atlas_w / ATLAS_SIZE as f32, g.atlas_h / ATLAS_SIZE as f32],
                color,
                halo_color,
                halo_width,
                angle: g.angle,
                pivot: [g.pivot.0, g.pivot.1],
            });
        }
    }

    /// Upload everything the frame's `prepare` calls staged: the glyph
    /// atlas (if dirty), the globals uniform, and the shared instance
    /// buffer (grown as needed). Call once, after all `prepare`s and
    /// before the render pass begins.
    pub(crate) fn finish_frame(&mut self) {
        if self.staged.is_empty() {
            return;
        }

        // 2. Re-upload the atlas if it's been touched since the last frame.
        if self.atlas.take_dirty() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                self.atlas.bitmap(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ATLAS_SIZE),
                    rows_per_image: Some(ATLAS_SIZE),
                },
                wgpu::Extent3d {
                    width: ATLAS_SIZE,
                    height: ATLAS_SIZE,
                    depth_or_array_layers: 1,
                },
            );
        }

        // 3. Upload globals + instances.
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
                label: Some("turbomap-text-instance"),
                size: new_cap * std::mem::size_of::<TextInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&self.staged));
    }

    /// Draw one layer's prepared glyph range inside the Map's single
    /// render pass, on top of whatever geometry drew before it.
    pub(crate) fn draw(&self, prepared: &PreparedText, pass: &mut wgpu::RenderPass<'_>) {
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
    // Use the camera's matrix-based projection so labels follow tilt
    // + bearing correctly. Off-screen / behind-camera projections fall
    // back to a far-offscreen sentinel that the cull filter rejects.
    let p = camera.world_to_screen(
        crate::WorldPoint::new(world.0 as f64, world.1 as f64),
        (viewport_px.0 as f64, viewport_px.1 as f64),
    );
    match p {
        Some((x, y)) => (x as f32, y as f32),
        None => (f32::NEG_INFINITY, f32::NEG_INFINITY),
    }
}

/// Rotate `p` about `pivot` by `angle` radians (screen space). Mirrors the
/// vertex shader so CPU-side collision boxes match what the GPU draws.
fn rotate_about(p: (f32, f32), pivot: (f32, f32), angle: f32) -> (f32, f32) {
    let (s, c) = angle.sin_cos();
    let (rx, ry) = (p.0 - pivot.0, p.1 - pivot.1);
    (pivot.0 + rx * c - ry * s, pivot.1 + rx * s + ry * c)
}

fn sq_dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}
