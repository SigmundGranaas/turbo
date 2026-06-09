//! Vector-tile render pipeline. One pipeline draws both polygon fills and
//! line strokes — they're already triangulated by lyon at tessellation
//! time, so the GPU just sees position+color triangles.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::{scene::Scene, tessellate::VectorVertex, tile::TileId};

use super::vector_cache::VectorMeshCache;

/// wgpu's `min_uniform_buffer_offset_alignment` is 256 on every backend we
/// target. We pad each per-tile uniform out to this so they can be bound
/// with dynamic offsets without violating alignment.
const TILE_UNIFORM_STRIDE: u64 = 256;
/// Total bytes carved out for the per-tile uniform buffer. Allows up to
/// `MAX_TILES_PER_FRAME` tiles drawn per frame.
const MAX_TILES_PER_FRAME: u64 = 256;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniform {
    /// World → clip 4×4 — see raster `shader.wgsl` for the rationale.
    view_proj: [[f32; 4]; 4],
}

/// The only data the GPU reads per draw — 4 bytes. We bind 256-byte
/// slices (the dynamic-offset alignment requirement) but only fill the
/// first 4 bytes; the rest is left zero.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct TileUniform {
    /// 0.0 → tile is invisible, 1.0 → fully faded in.
    tile_alpha: f32,
}

pub(crate) struct VectorPipeline {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    tile_uniform_buffer: wgpu::Buffer,
    tile_bind_group: wgpu::BindGroup,
    queue: Arc<wgpu::Queue>,
}

impl VectorPipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-vector-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("vector_shader.wgsl").into()),
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-vector-camera-bgl"),
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

        // Per-tile bind group: one f32 alpha per draw, bound with a dynamic
        // offset so we can pack all visible-tile alphas into a single
        // buffer and switch between them with cheap offset changes.
        // WGSL aligns uniform structs to 16 bytes; declare that as the
        // minimum binding size even though our Rust `TileUniform` is just
        // 4 bytes of useful data. The buffer is sub-allocated at
        // TILE_UNIFORM_STRIDE (256) intervals to satisfy
        // `min_uniform_buffer_offset_alignment`, but the shader's binding
        // *view* is 16 bytes wide.
        // tile_alpha(4) + use_paint_color(4) + pad(8) + paint_color vec4(16).
        const WGSL_TILE_UNIFORM_BYTES: u64 = 32;
        let tile_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-vector-tile-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(WGSL_TILE_UNIFORM_BYTES),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-vector-layout"),
            bind_group_layouts: &[Some(&camera_bgl), Some(&tile_bgl)],
            immediate_size: 0,
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VectorVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: [f32; 2] @ offset 0
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // color: [u8; 4] Unorm @ offset 8
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 8,
                    shader_location: 1,
                },
                // edge_pos: [u8; 4] Unorm @ offset 12 — shader reads .x.
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 12,
                    shader_location: 2,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-vector-pipeline"),
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
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-vector-camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-vector-camera-bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let tile_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-vector-tile-uniforms"),
            size: TILE_UNIFORM_STRIDE * MAX_TILES_PER_FRAME,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tile_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-vector-tile-bg"),
            layout: &tile_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &tile_uniform_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(WGSL_TILE_UNIFORM_BYTES),
                }),
            }],
        });

        Self {
            pipeline,
            camera_buffer,
            camera_bind_group,
            tile_uniform_buffer,
            tile_bind_group,
            queue,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        &mut self,
        scene: &Scene,
        cache: &mut VectorMeshCache,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clear_color: [f32; 4],
        fade_in_secs: f32,
        is_first_layer: bool,
        // When set, the shader uses this colour for every fragment instead
        // of the baked vertex colour — the zoom/data-driven paint path.
        paint_override: Option<[f32; 4]>,
    ) {
        let camera = scene.camera();
        let (vw, vh) = scene.viewport_px();
        let uniform = CameraUniform {
            view_proj: camera.view_projection_matrix((vw, vh)),
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));

        // Collect tile IDs we'll actually draw, paired with their fade
        // alpha. The fade alpha is smoothstep'd against the tile's age so
        // freshly arrived tiles ramp up over `fade_in_secs`.
        //
        // When a visible child tile isn't loaded yet, we draw its
        // nearest cached ancestor instead — mirroring the raster
        // pipeline's pyramid fallback. The ancestor's mesh covers
        // the child's area (and its siblings'); deduping across
        // children of the same parent stops us drawing the ancestor
        // N times per frame. Ancestors render first so any child
        // that *is* loaded paints on top with its own fade-in.
        let mut to_draw: Vec<(TileId, f32)> = Vec::new();
        let mut fallback_ancestors: std::collections::HashSet<TileId> =
            std::collections::HashSet::new();
        for tile in scene.visible_tiles() {
            if let Some(age) = cache.age_secs(tile) {
                let alpha = fade_alpha(age, fade_in_secs);
                to_draw.push((tile, alpha));
            } else if let Some(ancestor) = cache.nearest_ancestor(tile) {
                fallback_ancestors.insert(ancestor);
            }
        }
        // Drain the HashSet into a Vec, then sort `(z, x, y)`.
        // `HashSet::into_iter` is randomised per frame, so
        // without this sort multiple fallback ancestors would
        // draw in a different order every frame — overlapping
        // ancestor tiles would visibly "fight about who is on
        // top", which is the flicker the user reported.
        let mut ancestors: Vec<TileId> = fallback_ancestors
            .into_iter()
            .filter(|a| cache.peek(*a).is_some())
            .collect();
        ancestors.sort_by(|a, b| (a.z, a.x, a.y).cmp(&(b.z, b.x, b.y)));
        let mut ordered: Vec<(TileId, f32)> = ancestors.into_iter().map(|a| (a, 1.0)).collect();
        ordered.extend(to_draw);
        let to_draw = ordered;
        let draw_count = to_draw.len().min(MAX_TILES_PER_FRAME as usize);

        // Pack per-tile uniforms into the shared buffer at aligned offsets.
        // Only the first 4 bytes of each 256-byte slot carry data (the
        // rest is just alignment padding); one big write covers them all.
        if draw_count > 0 {
            let (use_paint, paint) = match paint_override {
                Some(c) => (1.0f32, c),
                None => (0.0f32, [0.0; 4]),
            };
            let mut bytes = vec![0u8; draw_count * TILE_UNIFORM_STRIDE as usize];
            for (i, (_, alpha)) in to_draw.iter().take(draw_count).enumerate() {
                let off = i * TILE_UNIFORM_STRIDE as usize;
                bytes[off..off + 4].copy_from_slice(&alpha.to_le_bytes());
                bytes[off + 4..off + 8].copy_from_slice(&use_paint.to_le_bytes());
                // paint_color vec4 at the 16-byte-aligned slot.
                for (k, comp) in paint.iter().enumerate() {
                    let c = off + 16 + k * 4;
                    bytes[c..c + 4].copy_from_slice(&comp.to_le_bytes());
                }
            }
            self.queue
                .write_buffer(&self.tile_uniform_buffer, 0, &bytes);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("turbomap-vector-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: if is_first_layer {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0] as f64,
                            g: clear_color[1] as f64,
                            b: clear_color[2] as f64,
                            a: clear_color[3] as f64,
                        })
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);

        for (idx, (tile, _)) in to_draw.iter().take(draw_count).enumerate() {
            let entry = cache.get(*tile).expect("just verified above");
            if entry.index_count == 0 {
                continue;
            }
            let offset = (idx as u64 * TILE_UNIFORM_STRIDE) as u32;
            pass.set_bind_group(1, &self.tile_bind_group, &[offset]);
            pass.set_vertex_buffer(0, entry.vertex_buffer.slice(..));
            pass.set_index_buffer(entry.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..entry.index_count, 0, 0..1);
        }
    }
}

/// Smoothstep ramp `[0, fade_in_secs)` → `[0, 1)`; `>= fade_in_secs` ⇒ 1.
/// `fade_in_secs == 0` disables fading (everything full alpha).
pub(crate) fn fade_alpha(age_secs: f32, fade_in_secs: f32) -> f32 {
    if fade_in_secs <= 0.0 || age_secs >= fade_in_secs {
        return 1.0;
    }
    let t = (age_secs / fade_in_secs).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    //! Value boundary: the fade_alpha curve drives the per-tile uniform.
    //! Hosts care that (a) tiles ramp from 0 → 1, (b) the curve is
    //! monotone, (c) `fade_in_secs == 0` is an instant-show no-op.
    use super::fade_alpha;

    #[test]
    fn fade_alpha_starts_at_zero_and_ends_at_one() {
        assert!((fade_alpha(0.0, 0.2) - 0.0).abs() < 1e-6);
        assert!((fade_alpha(0.2, 0.2) - 1.0).abs() < 1e-6);
        assert!((fade_alpha(1.0, 0.2) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fade_alpha_is_monotone_over_the_fade_window() {
        let dur = 0.3;
        let mut last = -1.0;
        for i in 0..=30 {
            let t = i as f32 / 30.0 * dur;
            let a = fade_alpha(t, dur);
            assert!(a >= last, "non-monotone at t={t}: {a} < {last}");
            last = a;
        }
    }

    #[test]
    fn fade_alpha_disabled_when_duration_is_zero_or_negative() {
        assert_eq!(fade_alpha(0.0, 0.0), 1.0);
        assert_eq!(fade_alpha(0.5, -1.0), 1.0);
    }
}
