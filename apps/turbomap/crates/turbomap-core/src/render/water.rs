//! Realistic-water render pipeline.
//!
//! Water-body fills are split out of the vector mesh at tessellation time (see
//! [`crate::tessellate`]) and drawn here instead of as a flat fill: an animated
//! wave-perturbed normal drives a Fresnel blend between a deep-water tint and a
//! reflection of the analytic sky, plus an HDR sun glitter that feeds the bloom
//! pass.
//!
//! This pipeline deliberately reuses the [`VectorPipeline`]'s per-frame camera
//! (group 0) and per-tile (group 1) bind groups and its [`PreparedVector`] draw
//! list, so each water tile drapes onto the terrain with the *same* placement
//! and DEM sub-UV as the vector fills it replaces — a single source of truth for
//! tile geometry. The only water-specific GPU state is the group-3 lighting
//! uniform and the (separate) water vertex/index buffers in each cache entry.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::tessellate::VectorVertex;

use super::terrain::TerrainCache;
use super::vector::{PreparedVector, VectorPipeline, TILE_UNIFORM_STRIDE};
use super::vector_cache::VectorMeshCache;

/// WGSL `min_binding_size` for the per-tile uniform — must match the value the
/// [`VectorPipeline`] declares so the shared bind group is layout-compatible.
const WGSL_TILE_UNIFORM_BYTES: u64 = 64;

/// Per-frame water lighting + animation. Mirrors `WaterGlobals` in
/// `water_shader.wgsl` byte-for-byte (16-byte-aligned `vec3 + f32` rows).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct WaterGlobals {
    /// Direction toward the sun (world frame) — shared with the sky/terrain.
    pub sun_dir: [f32; 3],
    pub sun_intensity: f32,
    pub zenith_color: [f32; 3],
    /// Renderer wall-clock seconds (wave animation).
    pub time: f32,
    pub horizon_color: [f32; 3],
    /// 1 / metres-per-world at the camera latitude (physical wave scale).
    pub meters_to_world: f32,
    pub sun_color: [f32; 3],
    /// 1.0 ⇒ march the reflected ray against the terrain heightfield (group 4)
    /// to mirror the surrounding mountains; 0.0 ⇒ reflect the analytic sky only
    /// (no terrain / heightfield not yet assembled).
    pub ssr_enabled: f32,
    /// Camera eye in the RTC frame (for the per-fragment view vector).
    pub eye: [f32; 3],
    /// Shoreline-foam intensity (0 = none). Driven up a little by sea state; the
    /// foam itself is found from the heightfield (land rising next to water).
    pub foam: f32,
    /// Heightfield placement (shared with the cast-shadow/AO field): UV =
    /// `(world.xy - hf_origin) * hf_inv_size`. `hf_origin` is rebased into this
    /// frame's RTC frame; `hf_inv_size = 1 / world_size`.
    pub hf_origin: [f32; 2],
    pub hf_inv_size: f32,
    /// Converts a heightfield texel (world-z in the field's steeper vertical
    /// scale) into the mesh's world-z so the march compares against the drawn
    /// surface: `mesh_z = texel * cos²lat`.
    pub hf_to_mesh_z: f32,
    /// Dominant wave **propagation** direction in the world frame (unit; x=E,
    /// y=S). Derived from MET wave-from / wind-from bearing. The wave octaves fan
    /// around this so the swell runs the way the forecast says.
    pub wave_dir: [f32; 2],
    /// Sea-state ferocity: a multiplier on wave amplitude/steepness from MET wave
    /// height (and wind). ~0.5 = calm, ~1 = moderate, up to ~4 = storm.
    pub wave_amp: f32,
    /// Whitecap amount (0..1): white breaking crests, ramped in when the sea
    /// state turns extreme (big waves / strong wind).
    pub whitecap: f32,
    /// 1.0 ⇒ the realistic AAA path (Gerstner vertex displacement + wave normals
    /// + Fresnel reflection + sun glitter); 0.0 ⇒ a flat matte body-colour fill
    /// (the rail toggle off — matches the pre-AAA flat water look).
    pub realistic: f32,
    /// Pads the struct to a 16-byte multiple (128 B) so the Rust `size_of` matches
    /// the WGSL uniform's rounded stride.
    pub _pad: [f32; 3],
}

/// Sea state for the water surface, derived from the MET wave/wind forecast.
/// Held on the `Map` and patched into [`WaterGlobals`] each frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WaterConditions {
    /// Wave propagation direction (unit, world x=E/y=S).
    pub wave_dir: [f32; 2],
    /// Amplitude/steepness multiplier (~0.5 calm … ~4 storm).
    pub wave_amp: f32,
    /// Whitecap amount (0..1).
    pub whitecap: f32,
    /// Shoreline-foam intensity (0..~1.6).
    pub foam: f32,
}

impl Default for WaterConditions {
    fn default() -> Self {
        // Calm: gentle swell running east, no whitecaps, ordinary shore foam.
        Self {
            wave_dir: [1.0, 0.0],
            wave_amp: 1.0,
            whitecap: 0.0,
            foam: 1.0,
        }
    }
}

impl WaterConditions {
    /// Derive the sea state from a MET forecast. All inputs are optional (MET
    /// drops fields inland / at the series tail); missing values fall back to a
    /// calm default. Bearings are degrees the wave/wind comes *from* (compass).
    pub(crate) fn from_forecast(
        wave_from_deg: Option<f32>,
        wave_height_m: Option<f32>,
        wind_speed_ms: Option<f32>,
        wind_from_deg: Option<f32>,
    ) -> Self {
        let smoothstep = |e0: f32, e1: f32, x: f32| {
            let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        // Direction: waves travel *toward* (from + 180°). Compass bearing β maps
        // to world (x=E, y=S) as (sin β, -cos β), so the propagation dir for a
        // "from" bearing is (-sin, +cos). Prefer the wave bearing, else wind.
        let wave_dir = match wave_from_deg.or(wind_from_deg) {
            Some(d) => {
                let r = d.to_radians();
                let v = [-r.sin(), r.cos()];
                let len = (v[0] * v[0] + v[1] * v[1]).sqrt().max(1e-6);
                [v[0] / len, v[1] / len]
            }
            None => [1.0, 0.0],
        };
        let h = wave_height_m.unwrap_or(0.0).max(0.0);
        let w = wind_speed_ms.unwrap_or(0.0).max(0.0);
        // Ferocity from wave height (the dominant cue), with a wind floor so a
        // blow with little reported swell still roughens the surface.
        let wave_amp = (0.5 + h * 0.7).max(0.4 + w * 0.06).clamp(0.5, 4.0);
        // Whitecaps appear ~2 m / fresh breeze and saturate in a gale/storm.
        let whitecap = smoothstep(2.0, 5.0, h).max(smoothstep(8.0, 16.0, w));
        // Shore foam always present; a bit more in a big sea.
        let foam = (0.7 + h * 0.12).clamp(0.7, 1.6);
        Self {
            wave_dir,
            wave_amp,
            whitecap,
            foam,
        }
    }
}

pub(crate) struct WaterPipeline {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    queue: Arc<wgpu::Queue>,
}

impl WaterPipeline {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        // Shared terrain DEM bind-group layout (group 2) for draping.
        terrain_bgl: &wgpu::BindGroupLayout,
        // A view of the cast-shadow/AO heightfield texture — baked into group 3
        // (binding 1) so the reflection march can sample it. The view stays valid
        // across `ShadowMap::upload_heights` (contents-only rewrites). Bound here
        // rather than as a 5th group because devices cap `max_bind_groups` at 4.
        height_view: &wgpu::TextureView,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-water-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("water_shader.wgsl").into()),
        });

        // Groups 0 (camera) and 1 (per-tile) are described identically to the
        // VectorPipeline so the water render pipeline's layout is compatible
        // with the vector pipeline's bind groups, which we reuse at draw time.
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-water-camera-bgl"),
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
        let tile_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-water-tile-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(WGSL_TILE_UNIFORM_BYTES),
                },
                count: None,
            }],
        });
        // Group 3: the water lighting/animation uniform (binding 0) plus the
        // terrain heightfield texture (binding 1) the reflection march samples.
        // Both fragment-only. (Folded into one group because devices cap
        // `max_bind_groups` at 4 — camera/tile/terrain take 0..2.)
        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-water-globals-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // VERTEX too: the Gerstner swell displaces the grid vertices,
                    // so the vertex stage reads time/wave_dir/amp/eye/meters.
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<WaterGlobals>() as u64,
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        // R32Float heightfield, read with `textureLoad` (no filtering).
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-water-layout"),
            bind_group_layouts: &[
                Some(&camera_bgl),
                Some(&tile_bgl),
                Some(terrain_bgl),
                Some(&globals_bgl),
            ],
            immediate_size: 0,
        });

        // Same vertex layout as the vector pipeline — water meshes share the
        // `VectorVertex` format (we only read base/z/color, but the buffer
        // stride must match).
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VectorVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 16,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 20,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Unorm8x4,
                    offset: 24,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 28,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 32,
                    shader_location: 6,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-water-pipeline"),
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
            // Draped, depth-test-no-write like the vector pipeline (avoids
            // z-fighting the terrain mesh the water rests on). Drawn before the
            // vector mesh so roads/buildings paint over the water.
            depth_stencil: Some(super::overlay_depth_state()),
            multisample: super::multisample_state(),
            multiview_mask: None,
            cache: None,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-water-globals"),
            size: std::mem::size_of::<WaterGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-water-globals-bg"),
            layout: &globals_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(height_view),
                },
            ],
        });

        Self {
            pipeline,
            globals_buffer,
            globals_bind_group,
            queue,
        }
    }

    /// Upload this frame's water lighting + animation globals. Called once per
    /// frame before the draw(s).
    pub(crate) fn prepare(&self, globals: &WaterGlobals) {
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(globals));
    }

    /// Draw a layer's water meshes, replaying `prepared`'s tile list and
    /// reusing `vector`'s camera (group 0) + per-tile (group 1) bind groups, so
    /// placement/draping match the vector fills exactly. Binds each tile's DEM
    /// (group 2) and the shared water globals (group 3).
    pub(crate) fn draw(
        &self,
        prepared: &PreparedVector,
        cache: &VectorMeshCache,
        terrain: Option<&TerrainCache>,
        placeholder_dem: &wgpu::BindGroup,
        vector: &VectorPipeline,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        let tiles = prepared.tiles();
        if tiles.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, vector.camera_bind_group(), &[]);
        // Group 3 carries both the water globals and the heightfield texture.
        pass.set_bind_group(3, &self.globals_bind_group, &[]);

        for (idx, (tile, dem_src)) in tiles.iter().enumerate() {
            let Some(entry) = cache.peek(*tile) else {
                continue;
            };
            // Skip tiles with no water (4-byte placeholder buffers).
            if entry.water_index_count == 0 {
                continue;
            }
            let offset = (idx as u64 * TILE_UNIFORM_STRIDE) as u32;
            pass.set_bind_group(1, vector.tile_bind_group(), &[offset]);
            let dem = match (*dem_src, terrain) {
                (Some(s), Some(t)) => t.exact_bind_group(s).unwrap_or(placeholder_dem),
                _ => placeholder_dem,
            };
            pass.set_bind_group(2, dem, &[]);
            pass.set_vertex_buffer(0, entry.water_vertex_buffer.slice(..));
            pass.set_index_buffer(
                entry.water_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..entry.water_index_count, 0, 0..1);
        }
    }
}
