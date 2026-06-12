//! Hillshade pipeline. One textured quad per visible DEM tile; the
//! fragment shader computes slope + aspect from the gradient of the
//! decoded elevation field and shades accordingly.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{dem::DemEncoding, scene::Scene, style::HillshadeStyle, tile::TileId};

use super::{terrain::TerrainCache, vector::fade_alpha, DEPTH_FORMAT};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniform {
    /// World → clip-space 4×4 view-projection from
    /// `Camera::view_projection_matrix`. See `shader.wgsl` for the
    /// rationale; same uniform shape as raster + vector.
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    sun_dir: [f32; 3],
    exaggeration: f32,
    shadow_color: [f32; 4],
    highlight_color: [f32; 4],
    opacity: f32,
    encoding: u32,
    /// Fractional UV inset on each side that maps to the halo ring of
    /// the DEM texture. When the source serves a 258×258 PNG with
    /// 1 px halo, the displayed 256×256 of geography corresponds to
    /// texture UV [1/258, 257/258]. `halo_uv = 1/258`. Default 0
    /// (no halo).
    halo_uv: f32,
    /// Metres-per-world-unit conversion. `1 / (256 * cos(lat) *
    /// circumference)` for Mercator. The vertex shader multiplies
    /// this by the sampled elevation to get world-space z.
    meters_to_world: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    corner: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Instance {
    world_origin: [f32; 2],
    world_size: f32,
    /// Per-tile alpha for fade-in. Each tile fades independently from
    /// its own ingest age — late-arriving tiles ease in over their
    /// own `fade_in_secs`, not a global layer timer.
    alpha: f32,
}

/// Output of [`HillshadePipeline::prepare`]: the tile draw list, index-
/// aligned with the instances uploaded by `prepare`. No cache
/// references — `draw` re-looks tiles up immutably via `peek_entry`.
pub(crate) struct PreparedHillshade {
    tiles: Vec<TileId>,
}

pub(crate) struct HillshadePipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    /// Number of indices in the subdivided grid — `16 * 16 * 6` for
    /// the current `GRID=16` setting. Stored so the draw call uses
    /// the right range.
    index_count: u32,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u64,
    camera_buffer: wgpu::Buffer,
    globals_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    /// DEM tile halo in source pixels. Used to compute `halo_uv` so
    /// the vertex shader can map the displayed 256² geography to the
    /// interior of the (256 + 2*halo)² texture, leaving the halo
    /// ring available for gradient sampling without ClampToEdge
    /// seams.
    halo_px: u32,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl HillshadePipeline {
    /// Build the hillshade pipeline. The DEM bind-group layout is
    /// borrowed from `TerrainCache` — this pipeline does NOT own its
    /// own DEM texture cache. The Map orchestrates per-tile DEM bind
    /// group binding at render time.
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        terrain_bgl: &wgpu::BindGroupLayout,
        halo_px: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-hillshade-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hillshade_shader.wgsl").into()),
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-hillshade-camera-bgl"),
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
                    // VERTEX too — the vertex shader reads halo_uv
                    // out of Globals to inset the displayed quad's
                    // UV into the DEM texture's non-halo interior.
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-hillshade-layout"),
            bind_group_layouts: &[Some(&camera_bgl), Some(terrain_bgl)],
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
            array_stride: std::mem::size_of::<Instance>() as u64,
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
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-hillshade-pipeline"),
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
            // Hillshade is the SECOND displaced layer drawn at the
            // same DEM positions as the raster basemap, but the two
            // displace by *different* exaggerations (the basemap uses
            // `TerrainOptions::exaggeration`, hillshade uses its
            // style's) and through slightly different shader paths,
            // so their depths don't agree. In the per-pass era the
            // hillshade pass cleared depth before testing, making its
            // LessEqual test a guaranteed pass; with the whole frame
            // in ONE pass the cleared-buffer trick is gone, so encode
            // the same observable semantics directly: always pass,
            // never write. The basemap owns the depth buffer; the
            // hillshade colour just blends on top.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: super::multisample_state(),
            multiview_mask: None,
            cache: None,
        });

        // 17×17 vertex grid → 16×16 quads → 512 triangles per tile.
        // Enough resolution to express terrain detail down to ~16 m
        // at zoom 9 (one cell ≈ 256 m / 16 vertices). Tile-edge
        // vertices fall on integer divisions of the tile so adjacent
        // tiles agree on their shared edge and the mesh doesn't
        // crack — provided the DEM tiles have halo>=1, which our
        // turbo_terrain_rgb preset enforces.
        const GRID: u32 = 16;
        let mut vertices: Vec<Vertex> = Vec::with_capacity(((GRID + 1) * (GRID + 1)) as usize);
        for vy in 0..=GRID {
            for vx in 0..=GRID {
                vertices.push(Vertex {
                    corner: [vx as f32 / GRID as f32, vy as f32 / GRID as f32],
                });
            }
        }
        let mut indices: Vec<u16> = Vec::with_capacity((GRID * GRID * 6) as usize);
        for vy in 0..GRID {
            for vx in 0..GRID {
                let i = (vy * (GRID + 1) + vx) as u16;
                let i_right = i + 1;
                let i_down = i + (GRID + 1) as u16;
                let i_diag = i_down + 1;
                // Two triangles per cell, CCW from top-left.
                indices.extend_from_slice(&[i, i_right, i_diag, i, i_diag, i_down]);
            }
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-hillshade-quad-vb"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-hillshade-quad-ib"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_capacity = 256u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-hillshade-instance"),
            size: instance_capacity * std::mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-hillshade-camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-hillshade-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-hillshade-camera-bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: globals_buffer.as_entire_binding(),
                },
            ],
        });

        let index_count = indices.len() as u32;
        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            instance_buffer,
            instance_capacity,
            camera_buffer,
            globals_buffer,
            camera_bind_group,
            halo_px,
            device,
            queue,
        }
    }

    /// CPU half of a frame: camera/globals/instance uploads plus LRU
    /// touches for every DEM tile the draw list references.
    pub(crate) fn prepare(
        &mut self,
        scene: &Scene,
        terrain: &mut TerrainCache,
        style: HillshadeStyle,
        fade_in_secs: f32,
        meters_to_world: f32,
    ) -> PreparedHillshade {
        let camera = scene.camera();
        let (vw, vh) = scene.viewport_px();

        // Upload camera + globals.
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&CameraUniform {
                view_proj: camera.view_projection_matrix((vw, vh)),
            }),
        );
        // Halo UV is `halo / (256 + 2*halo)` — the fraction of texture
        // width that the halo ring occupies on each side. The vertex
        // shader interpolates the displayed quad's UV in
        // `[halo_uv, 1 - halo_uv]`, so corner samples land on the
        // first non-halo texel and the gradient kernel can step into
        // the halo without ClampToEdge.
        let halo_uv = if self.halo_px == 0 {
            0.0
        } else {
            self.halo_px as f32 / (256.0 + 2.0 * self.halo_px as f32)
        };
        self.queue.write_buffer(
            &self.globals_buffer,
            0,
            bytemuck::bytes_of(&Globals {
                sun_dir: sun_direction(style.sun_azimuth_deg, style.sun_altitude_deg),
                exaggeration: style.exaggeration,
                shadow_color: linearise(style.shadow_color),
                highlight_color: linearise(style.highlight_color),
                opacity: style.opacity,
                encoding: match style.encoding {
                    DemEncoding::MapboxRgb => 0,
                    DemEncoding::Terrarium => 1,
                },
                halo_uv,
                meters_to_world,
            }),
        );

        // One draw per visible tile (no ancestor-fallback for DEM yet —
        // missing tiles are just skipped). Build instance buffer flat.
        // Per-tile alpha is a smoothstep from the tile's own ingest age
        // so late arrivals fade in independently rather than snapping.
        let mut instances: Vec<Instance> = Vec::new();
        let mut tiles: Vec<TileId> = Vec::new();
        for tile in scene.visible_tiles() {
            if let Some(age) = terrain.age_secs(tile) {
                let _ = terrain.get_entry(tile); // bump LRU
                let (nw, _) = tile.world_bounds();
                let world_size = 1.0 / (1u64 << tile.z) as f32;
                instances.push(Instance {
                    world_origin: [nw.x as f32, nw.y as f32],
                    world_size,
                    alpha: fade_alpha(age, fade_in_secs),
                });
                tiles.push(tile);
            }
        }

        if instances.is_empty() {
            // Nothing to draw — the Map-level frame clear replaces the
            // old clear-only pass.
            return PreparedHillshade { tiles: Vec::new() };
        }

        if (instances.len() as u64) > self.instance_capacity {
            let mut new_cap = self.instance_capacity.max(1);
            while new_cap < instances.len() as u64 {
                new_cap *= 2;
            }
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("turbomap-hillshade-instance"),
                size: new_cap * std::mem::size_of::<Instance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));

        PreparedHillshade { tiles }
    }

    /// GPU half of the frame: replay the prepared tile list inside the
    /// Map's single render pass. `prepare` already touched every tile,
    /// so the read-only `peek_entry` lookups can't miss within one
    /// `Map::render`.
    pub(crate) fn draw(
        &self,
        prepared: &PreparedHillshade,
        terrain: &TerrainCache,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if prepared.tiles.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);

        for (i, tile) in prepared.tiles.iter().enumerate() {
            let entry = terrain
                .peek_entry(*tile)
                .expect("prepare touched this DEM tile");
            pass.set_bind_group(1, &entry.bind_group, &[]);
            pass.draw_indexed(0..self.index_count, 0, i as u32..i as u32 + 1);
        }
    }
}

/// Convert (azimuth, altitude) in degrees to a unit-vector sun direction
/// in shader space. Azimuth 0° is north (-y in world space); altitude 90°
/// is straight up. We compose the standard map-cartography convention.
fn sun_direction(azimuth_deg: f32, altitude_deg: f32) -> [f32; 3] {
    let az = azimuth_deg.to_radians();
    let al = altitude_deg.to_radians();
    let cos_al = al.cos();
    [cos_al * az.sin(), -cos_al * az.cos(), al.sin()]
}

fn linearise(c: crate::style::Color) -> [f32; 4] {
    fn to_linear(u: u8) -> f32 {
        let s = u as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    [
        to_linear(c.r),
        to_linear(c.g),
        to_linear(c.b),
        c.a as f32 / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    //! Value boundary: a host configures sun direction in degrees and
    //! expects the shader to receive a unit-vector pointing in the
    //! conventional direction.
    use super::*;

    fn approx_vec3(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps && (a[2] - b[2]).abs() < eps
    }

    #[test]
    fn sun_directly_overhead_points_straight_up() {
        let v = sun_direction(0.0, 90.0);
        assert!(approx_vec3(v, [0.0, 0.0, 1.0], 1e-6), "got {v:?}");
    }

    #[test]
    fn sun_in_the_north_at_horizon_points_minus_y() {
        let v = sun_direction(0.0, 0.0);
        assert!(approx_vec3(v, [0.0, -1.0, 0.0], 1e-6), "got {v:?}");
    }

    #[test]
    fn sun_in_the_east_at_horizon_points_plus_x() {
        let v = sun_direction(90.0, 0.0);
        assert!(approx_vec3(v, [1.0, 0.0, 0.0], 1e-6), "got {v:?}");
    }
}
