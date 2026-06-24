//! Raster-tile render pipeline.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{scene::Scene, tile::TileId};

use super::{cache::TextureCache, terrain::TerrainCache};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CameraUniform {
    /// 4×4 view-projection matrix. The vertex shader applies it to a
    /// world-space `(x, y, 0, 1)` to land in clip space. Built by
    /// `Camera::view_projection_matrix` so tilt + bearing are
    /// supported transparently — at pitch=bearing=0 the matrix is
    /// equivalent to the legacy ortho centre+scale form.
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Globals {
    /// Fractional UV inset that maps the displayed tile to the
    /// texture's halo'd interior. Same idea as the hillshade
    /// pipeline's halo handling — the terrain DEM is served with a
    /// 1-pixel halo for crack-free vertex displacement.
    halo_uv: f32,
    /// `1 / metres_per_world` at the current camera latitude. Pre-
    /// multiplied with exaggeration so the shader does one mul.
    meters_to_world: f32,
    /// Vertical exaggeration from `TerrainOptions`. Folded with
    /// `meters_to_world` so a single shader uniform suffices.
    exaggeration: f32,
    /// 0 = Mapbox-Terrain-RGB, 1 = Terrarium. Matches `DemEncoding`.
    encoding: u32,
    // --- Sun shading + aerial perspective (3D terrain) ---
    // Layout is std140-compatible: every `vec3` starts on a 16-byte
    // boundary with the trailing scalar packed into its 4th lane.
    /// Unit direction towards the sun (world frame x=E, y=S, z=up).
    sun_dir: [f32; 3],
    /// Ambient floor in [0,1] — darkest a self-shadowed slope reaches.
    ambient: f32,
    /// Atmosphere colour distant terrain fades toward (aerial perspective).
    haze_color: [f32; 3],
    /// Pre-scaled haze density (folds 1/altitude + a pitch ramp).
    haze_density: f32,
    /// Sunlight colour (warm at golden hour, neutral midday).
    light_color: [f32; 3],
    /// 1 = sun-shade + haze the displaced terrain, 0 = flat texture.
    terrain_lit: f32,
    /// Cast-shadow heightfield transform (camera-relative world-xy → [0,1] UV)
    /// and strength. `shadow_strength == 0` disables the per-fragment march. See
    /// [`super::shadow`]. Packs to one std140 16-byte slot (vec2 + 2 scalars).
    shadow_origin: [f32; 2],
    shadow_inv_size: f32,
    shadow_strength: f32,
    /// Camera eye in the relative-to-centre (RTC) frame the vertex shader
    /// emits. The fragment/vertex stage measures `length(world - eye_world)`
    /// for physically based aerial perspective (haze by true eye distance,
    /// not distance from the look-at point — which whites out at grazing
    /// pitch). One std140 16-byte slot (vec3 + the curvature scalar).
    eye_world: [f32; 3],
    /// Earth-curvature drop coefficient (`π·cos³φ`, 0 = flat/no-DEM). The vertex
    /// shader lowers `world_z` by `curvature_coeff · dot(world_xy, world_xy)` so
    /// distant terrain bends away over the horizon (see `earth_curvature_coeff`).
    curvature_coeff: f32,
    /// World-xy size of one heightfield texel (the march step) + the penumbra
    /// softness band (world-z). One more 16-byte slot (2 scalars + pad).
    shadow_texel_world: f32,
    shadow_softness: f32,
    /// Seconds since renderer start. Slowly drifts the valley-fog field so it
    /// evolves over time. See the haze block in `shader.wgsl`.
    time: f32,
    _pad0: f32,
    /// Absolute world-xy of the camera centre (the RTC origin). Added to the
    /// camera-relative fragment world-xy to reconstruct an absolute world
    /// position, so the valley-fog field stays welded to the terrain instead of
    /// sliding with the screen.
    cam_origin: [f32; 2],
    _pad1: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    corner: [f32; 2],
    /// Skirt depth as a fraction of the tile's world size. 0 for the
    /// flat grid vertices; > 0 for the perimeter "curtain" verts, which
    /// share an edge vertex's xy + DEM/texture UV but hang straight down
    /// in world-z to cover mixed-LOD T-junction cracks (see mesh build).
    skirt: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Instance {
    world_origin: [f32; 2],
    world_size: f32,
    alpha: f32,
    uv_origin: [f32; 2],
    uv_size: f32,
    /// Sub-UV of this basemap tile within the bound DEM tile. When
    /// the DEM bind is the basemap's own tile, this is (halo_uv,
    /// halo_uv, 1 - 2*halo_uv). When the DEM is an *ancestor*, this
    /// narrows the sample window to the basemap's slice of the
    /// ancestor's coverage — otherwise the vertex shader samples
    /// the entire ancestor's heightmap across one child, producing
    /// the visible "facet patches" at zoom levels above the DEM
    /// pyramid's top.
    dem_uv_origin: [f32; 2],
    dem_uv_size: f32,
    _pad: f32,
}

/// One draw call per (basemap-texture, DEM-source-tile) pair. Multiple
/// basemap instances draw from the same basemap texture (ancestor
/// fallback) AND the same DEM source (so the DEM bind group needs to
/// be switched only when one or the other changes). Splitting by
/// both axes lets the inner draw loop bind once per batch instead of
/// once per instance.
struct DrawBatch {
    texture_id: TileId,
    /// Which DEM tile this batch's instances were resolved against.
    /// `None` means "use the Map-level placeholder" (no terrain
    /// registered, or no ancestor cached for the drawn area).
    dem_source: Option<TileId>,
    instances: Vec<Instance>,
}

/// One draw call recorded by `prepare`, replayed by `draw`. Holds only
/// tile ids + an instance count — no references into the caches, so the
/// prepared frame can outlive the mutable prepare borrows.
struct PreparedBatch {
    texture_id: TileId,
    dem_source: Option<TileId>,
    instance_count: u32,
}

/// Output of [`RasterPipeline::prepare`]: everything `draw` needs to
/// replay the frame's draw list inside the shared render pass. An empty
/// batch list means "draw nothing" (the frame-level clear covers the
/// old clear-only path).
pub(crate) struct PreparedRaster {
    batches: Vec<PreparedBatch>,
}

/// Per-frame terrain configuration the Map hands the raster pipeline.
/// Identical shape to what the hillshade pipeline computes; lifting
/// it to a shared struct keeps the two ground layers in lock-step.
#[derive(Debug, Copy, Clone)]
pub(crate) struct TerrainConfig {
    /// Pre-multiplied `1 / metres-per-world` at the current camera
    /// latitude. Zero → no displacement.
    pub meters_to_world: f32,
    pub exaggeration: f32,
    /// 0 = MapboxRgb, 1 = Terrarium. Matches `DemEncoding`.
    pub encoding: u32,
    /// Unit direction towards the sun (world frame x=E, y=S, z=up).
    pub sun_dir: [f32; 3],
    /// Ambient floor in [0,1] for self-shadowed slopes.
    pub ambient: f32,
    /// Atmosphere/horizon colour distant terrain fades toward.
    pub haze_color: [f32; 3],
    /// Pre-scaled aerial-perspective density (already folds 1/altitude
    /// and the pitch ramp, so it's zoom-stable and 0 when top-down).
    pub haze_density: f32,
    /// Sunlight colour for the time of day.
    pub light_color: [f32; 3],
    /// Cast-shadow grid origin in the camera-relative (RTC) world frame the
    /// vertex shader emits. With `shadow_inv_size`, maps a fragment's world-xy
    /// to the shadow texture's `[0,1]` UV.
    pub shadow_origin: [f32; 2],
    /// `1 / shadow_world_size` — the reciprocal of the world extent the shadow
    /// grid covers. 0 leaves the UV mapping degenerate (only used when
    /// `shadow_strength == 0`).
    pub shadow_inv_size: f32,
    /// 0 = no cast shadows (heightfield ignored); > 0 blends the per-fragment
    /// sun-march result into the direct light term by this factor. The Map sets
    /// this from `set_terrain_shadows`.
    pub shadow_strength: f32,
    /// World-xy size of one heightfield texel — the per-fragment march step.
    pub shadow_texel_world: f32,
    /// World-z penumbra band over which an occluder fades the shadow in.
    pub shadow_softness: f32,
    /// Seconds since renderer start — animates the procedural low-haze drift.
    pub time: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            meters_to_world: 0.0,
            exaggeration: 1.0,
            encoding: 0,
            sun_dir: [0.0, 0.0, 1.0],
            ambient: 0.35,
            haze_color: [0.74, 0.80, 0.88],
            haze_density: 0.0,
            light_color: [1.0, 1.0, 1.0],
            shadow_origin: [0.0, 0.0],
            shadow_inv_size: 0.0,
            shadow_strength: 0.0,
            shadow_texel_world: 0.0,
            shadow_softness: 1.0,
            time: 0.0,
        }
    }
}

pub(crate) struct RasterPipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    /// Index count of the subdivided-grid mesh. With `GRID=16` →
    /// 16×16 quads → 1 536 indices. Stored so the draw call uses
    /// the right range.
    index_count: u32,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u64,
    camera_buffer: wgpu::Buffer,
    globals_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    pub(crate) texture_bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub(crate) sampler: Arc<wgpu::Sampler>,
    /// When each currently-drawn tile FIRST appeared on screen (not when its
    /// bytes were ingested). The fade-in ramps from this, so a cache-served
    /// tile and a freshly-fetched one transition identically — killing the
    /// "fresh fades, cached snaps" inconsistency. Pruned each frame to the
    /// drawn set, so it stays bounded by viewport coverage.
    first_seen: std::collections::HashMap<TileId, web_time::Instant>,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl RasterPipeline {
    /// Build the raster pipeline. The `terrain_bgl` is the
    /// `TerrainCache`'s bind group layout — every draw binds one of
    /// its tiles (a real DEM or the 1×1 placeholder) at group 2 so
    /// the vertex shader can sample heights and displace the
    /// subdivided tile mesh.
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        terrain_bgl: &wgpu::BindGroupLayout,
        shadow_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-raster-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-camera-bgl"),
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
                    // The fragment stage reads the lighting/haze fields
                    // (sun, ambient, atmosphere) to shade the displaced
                    // terrain; the vertex stage reads the displacement
                    // fields. Visible to both.
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let texture_bgl = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("turbomap-tile-bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            },
        ));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-raster-layout"),
            bind_group_layouts: &[
                Some(&camera_bgl),
                Some(&texture_bgl),
                Some(terrain_bgl),
                Some(shadow_bgl),
            ],
            immediate_size: 0,
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 8,
                    shader_location: 8,
                },
            ],
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
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 24,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 28,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 36,
                    shader_location: 7,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-raster-pipeline"),
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
            // Raster basemap displaces by terrain DEM in its vertex
            // shader → 3D geometry → needs depth so back faces of
            // mountains don't paint over front faces.
            depth_stencil: Some(super::ground_depth_state()),
            multisample: super::multisample_state(),
            multiview_mask: None,
            cache: None,
        });

        // 17×17 vertex grid → 16×16 quads → 512 triangles per tile.
        // Same resolution as the hillshade mesh so the two layers
        // line up exactly across shared world positions. Tile-edge
        // vertices land on `corner ∈ {0, 1}` (and the halo'd DEM
        // sample at those UVs comes from the neighbouring tile via
        // the server's overscan), so adjacent rendered tiles agree
        // on their shared edge heights — no cracks.
        const GRID: u32 = 16;
        // Skirt depth as a fraction of tile world-size. A mixed-LOD seam can
        // expose a crack as tall as the local relief over half a coarse cell;
        // a half-tile-deep curtain comfortably covers the worst realistic case
        // (slopes rarely exceed ~100%) while staying hidden behind the surface
        // from any non-grazing angle. Tuned on-device in Phase 5.
        const SKIRT_FRAC: f32 = 0.5;
        let mut vertices: Vec<Vertex> = Vec::with_capacity(((GRID + 1) * (GRID + 1)) as usize);
        for vy in 0..=GRID {
            for vx in 0..=GRID {
                vertices.push(Vertex {
                    corner: [vx as f32 / GRID as f32, vy as f32 / GRID as f32],
                    skirt: 0.0,
                });
            }
        }
        let mut indices: Vec<u16> = Vec::with_capacity((GRID * GRID * 6 + GRID * 4 * 6) as usize);
        for vy in 0..GRID {
            for vx in 0..GRID {
                let i = (vy * (GRID + 1) + vx) as u16;
                let i_right = i + 1;
                let i_down = i + (GRID + 1) as u16;
                let i_diag = i_down + 1;
                indices.extend_from_slice(&[i, i_right, i_diag, i, i_diag, i_down]);
            }
        }

        // Skirt: a vertical curtain hanging straight down from every tile-edge
        // vertex. Where a finer neighbour subdivides this (coarser) tile's
        // edge, its interpolated heights dip below ours, opening a see-through
        // crack at the T-junction; the curtain backs that gap with terrain
        // colour. Cull mode is None, so the quad winding is irrelevant.
        let edge_vert = |vx: u32, vy: u32| (vy * (GRID + 1) + vx) as u16;
        let mut perimeter: Vec<(u32, u32)> = Vec::with_capacity((GRID * 4) as usize);
        for vx in 0..GRID {
            perimeter.push((vx, 0)); // top edge, L→R
        }
        for vy in 0..GRID {
            perimeter.push((GRID, vy)); // right edge, T→B
        }
        for vx in (1..=GRID).rev() {
            perimeter.push((vx, GRID)); // bottom edge, R→L
        }
        for vy in (1..=GRID).rev() {
            perimeter.push((0, vy)); // left edge, B→T
        }
        let skirt_base = vertices.len() as u16;
        for &(vx, vy) in &perimeter {
            vertices.push(Vertex {
                corner: [vx as f32 / GRID as f32, vy as f32 / GRID as f32],
                skirt: SKIRT_FRAC,
            });
        }
        let pn = perimeter.len();
        for k in 0..pn {
            let (vx, vy) = perimeter[k];
            let (nvx, nvy) = perimeter[(k + 1) % pn];
            let top_a = edge_vert(vx, vy);
            let top_b = edge_vert(nvx, nvy);
            let sk_a = skirt_base + k as u16;
            let sk_b = skirt_base + ((k + 1) % pn) as u16;
            indices.extend_from_slice(&[top_a, top_b, sk_b, top_a, sk_b, sk_a]);
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-quad-vertex"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("turbomap-quad-index"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_capacity = 256u64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-instance"),
            size: instance_capacity * std::mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-raster-globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-camera-bg"),
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

        let sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-tile-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // Trilinear: lerp between adjacent mip levels. The cache
            // uploads a full mip chain for raster tiles, so the GPU
            // picks the right LOD automatically. Without this, far-
            // zoomed tiles alias and shimmer when panning.
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        }));

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
            texture_bind_group_layout: texture_bgl,
            sampler,
            first_seen: std::collections::HashMap::new(),
            device,
            queue,
        }
    }

    /// True if any currently-drawn tile is still inside its fade-in window —
    /// drives `Map::is_animating` so render-on-demand keeps drawing until the
    /// crossfade settles. Keyed on first-on-screen time (see `first_seen`), so
    /// it stays true for a fading cache tile exactly as for a network one.
    pub(crate) fn has_active_fade(&self, window_secs: f32) -> bool {
        if window_secs <= 0.0 {
            return false;
        }
        let now = web_time::Instant::now();
        self.first_seen
            .values()
            .any(|t| now.duration_since(*t).as_secs_f32() < window_secs)
    }

    /// CPU half of a frame: uniform/instance uploads, batch building,
    /// and LRU touches for every tile the draw list will reference.
    /// Returns the draw list `draw` replays inside the shared pass.
    pub(crate) fn prepare(
        &mut self,
        scene: &Scene,
        cache: &mut TextureCache,
        mut terrain: Option<&mut TerrainCache>,
        terrain_options: TerrainConfig,
        fade_in_secs: f32,
    ) -> PreparedRaster {
        let camera = scene.camera();
        let (vw, vh) = scene.viewport_px();
        // Relative-to-centre frame: tile world coords go to the GPU as
        // `(world - origin)` so f32 keeps full precision at deep zoom.
        let origin = camera.center.to_world();
        let uniform = CameraUniform {
            view_proj: camera.view_projection_matrix_rtc(origin, (vw, vh)),
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));

        // Upload Globals — halo_uv comes from the terrain cache when
        // present, otherwise 0 (placeholder is 1×1 so halo is
        // irrelevant). `meters_to_world == 0` collapses the
        // displacement to zero (legacy flat behaviour).
        let (halo_uv, dem_present) = match terrain.as_deref() {
            Some(t) if terrain_options.meters_to_world > 0.0 => {
                let halo = t.halo_px();
                let uv = if halo == 0 {
                    0.0
                } else {
                    halo as f32 / (256.0 + 2.0 * halo as f32)
                };
                (uv, true)
            }
            _ => (0.0, false),
        };
        self.queue.write_buffer(
            &self.globals_buffer,
            0,
            bytemuck::bytes_of(&Globals {
                halo_uv,
                meters_to_world: if dem_present {
                    terrain_options.meters_to_world
                } else {
                    0.0
                },
                exaggeration: terrain_options.exaggeration,
                encoding: terrain_options.encoding,
                sun_dir: terrain_options.sun_dir,
                ambient: terrain_options.ambient,
                haze_color: terrain_options.haze_color,
                // No DEM → no relief to shade or fade; collapse to the
                // flat-texture path so the 2D map is untouched.
                haze_density: if dem_present {
                    terrain_options.haze_density
                } else {
                    0.0
                },
                light_color: terrain_options.light_color,
                terrain_lit: if dem_present { 1.0 } else { 0.0 },
                shadow_origin: terrain_options.shadow_origin,
                shadow_inv_size: terrain_options.shadow_inv_size,
                // No DEM → no relief to occlude; force shadows off so the flat
                // 2D map never samples the (stale) shadow texture.
                shadow_strength: if dem_present {
                    terrain_options.shadow_strength
                } else {
                    0.0
                },
                eye_world: camera.eye_offset_world((vw, vh)),
                // Earth-curvature droop — only with a DEM (flat 2D map stays a
                // disc). `π·cos³φ` at the camera latitude; see camera.rs.
                curvature_coeff: if dem_present {
                    let coslat = (camera.center.lat.to_radians().cos() as f32).abs();
                    crate::camera::earth_curvature_coeff(coslat)
                } else {
                    0.0
                },
                shadow_texel_world: terrain_options.shadow_texel_world,
                shadow_softness: terrain_options.shadow_softness,
                time: terrain_options.time,
                _pad0: 0.0,
                cam_origin: [origin.x as f32, origin.y as f32],
                _pad1: [0.0, 0.0],
            }),
        );

        // Build draw batches via best-available LOD resolution. For each ideal
        // (target-zoom) cell, `resolve_cell` returns the FINEST resident coverage
        // — exact tile, or retained finer descendants on zoom-out, or a coarse
        // ancestor backdrop where nothing finer is resident. We then:
        //   • draw the coarse ancestor backdrop first (opaque) — it fills gaps in
        //     partial coverage, and backstops any still-fading finer tile so the
        //     fade blends over real content, never the clear colour;
        //   • draw the finer/exact tiles on top, each crossfading in from when it
        //     FIRST appeared on screen (`first_seen`) — so a cache-served tile and
        //     a freshly-fetched one fade identically (no instant cache "snap").
        let now = web_time::Instant::now();
        let mut batches: Vec<DrawBatch> = Vec::new();
        {
            let resident = |t: TileId| cache.peek(t).is_some();
            let first_seen = &mut self.first_seen;

            let push_tile = |batches: &mut Vec<DrawBatch>,
                             terrain: &mut Option<&mut TerrainCache>,
                             texture: TileId,
                             target: TileId,
                             uv_origin: [f32; 2],
                             uv_size: f32,
                             alpha: f32| {
                let (nw, _) = target.world_bounds();
                let world_size = 1.0 / (1u64 << target.z) as f32;
                let (dem_src, dem_uv_origin, dem_uv_size) =
                    resolve_dem_subuv(target, terrain.as_deref_mut(), halo_uv);
                push_instance(
                    batches,
                    texture,
                    dem_src,
                    Instance {
                        world_origin: [(nw.x - origin.x) as f32, (nw.y - origin.y) as f32],
                        world_size,
                        alpha,
                        uv_origin,
                        uv_size,
                        dem_uv_origin,
                        dem_uv_size,
                        _pad: 0.0,
                    },
                );
            };

            for ideal in scene.visible_tiles() {
                let sources = resolve_cell(ideal, MAX_DOWN, &resident);
                // Separate the (optional) coarse backdrop from the finer tiles.
                let mut backdrop: Option<(TileId, TileId)> = None; // (ancestor, target)
                let mut wholes: Vec<TileId> = Vec::new();
                for s in &sources {
                    match *s {
                        CellSource::AncestorPatch { ancestor, target } => {
                            backdrop = Some((ancestor, target))
                        }
                        CellSource::Whole(t) => wholes.push(t),
                    }
                }

                // Crossfade alpha per finer tile, ramped from its first-on-screen
                // time (not its ingest age). `fade_in_secs == 0` (goldens) snaps.
                let mut whole_alpha: Vec<(TileId, f32)> = Vec::with_capacity(wholes.len());
                for t in &wholes {
                    let a = if fade_in_secs <= 0.0 {
                        1.0
                    } else {
                        let s0 = *first_seen.entry(*t).or_insert(now);
                        let age = now.duration_since(s0).as_secs_f32();
                        if age >= fade_in_secs {
                            1.0
                        } else {
                            let s = (age / fade_in_secs).clamp(0.0, 1.0);
                            s * s * (3.0 - 2.0 * s)
                        }
                    };
                    whole_alpha.push((*t, a));
                }

                // A backdrop is needed when coverage is partial (resolver already
                // gave one) OR when the finer coverage is fully resident but still
                // fading in — then backstop with the nearest resident ancestor.
                let any_fading =
                    whole_alpha.is_empty() || whole_alpha.iter().any(|(_, a)| *a < 1.0);
                let backdrop = backdrop.or_else(|| {
                    if any_fading {
                        nearest_resident_ancestor(ideal, &resident).map(|a| (a, ideal))
                    } else {
                        None
                    }
                });
                if let Some((ancestor, target)) = backdrop {
                    if let Some(sub) = target.sub_uv_in(ancestor) {
                        push_tile(
                            &mut batches,
                            &mut terrain,
                            ancestor,
                            target,
                            [sub.origin.x as f32, sub.origin.y as f32],
                            sub.size as f32,
                            1.0,
                        );
                    }
                }

                // Finer/exact tiles on top, each at its crossfade alpha, full UV.
                for (t, a) in whole_alpha {
                    if a <= 0.0 {
                        continue;
                    }
                    push_tile(&mut batches, &mut terrain, t, t, [0.0, 0.0], 1.0, a);
                }
            }

            // Key `first_seen` to RESIDENT tiles, not just on-screen ones. A tile
            // that scrolls/zooms out of view but stays cached keeps its
            // fade-complete timestamp, so when the camera pans back — or Following
            // mode nudges it from GPS — it stays crisp instead of re-fading from
            // its coarse ancestor (the "low-quality tile flashes over my good tile
            // then disappears" bug). Only genuinely new (newly-resident) tiles
            // fade; eviction drops the entry so a re-fetched tile fades afresh.
            first_seen.retain(|k, _| resident(*k));
        }

        let total_instances: u64 = batches.iter().map(|b| b.instances.len() as u64).sum();
        if total_instances == 0 {
            // Nothing to draw. The Map-level frame clear replaces the old
            // clear-only pass.
            return PreparedRaster { batches: Vec::new() };
        }

        if total_instances > self.instance_capacity {
            // Grow the instance buffer.
            let mut new_cap = self.instance_capacity.max(1);
            while new_cap < total_instances {
                new_cap *= 2;
            }
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("turbomap-instance"),
                size: new_cap * std::mem::size_of::<Instance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_cap;
        }

        // Flatten and upload.
        let mut flat: Vec<Instance> = Vec::with_capacity(total_instances as usize);
        for b in &batches {
            flat.extend_from_slice(&b.instances);
        }
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&flat));

        // Touch every tile the draw will reference so the LRU can't
        // evict it before (or, with budget pressure, shortly after)
        // the frame draws. `draw` then uses read-only `peek` lookups.
        let mut prepared = Vec::with_capacity(batches.len());
        for b in &batches {
            // Touch the colour tile so the LRU keeps it through the draw. If
            // it's somehow already gone (it was just resolved, so this is a
            // belt-and-braces guard), drop the batch rather than panic — `draw`
            // also skips a missing tile, so this just avoids the wasted bind.
            if cache.get(b.texture_id).is_none() {
                continue;
            }
            // Touch the DEM tile too (best-effort). A miss here means the draw
            // falls back to the flat placeholder for this batch — no panic.
            if let (Some(src), Some(t)) = (b.dem_source, terrain.as_deref_mut()) {
                let _ = t.get_entry(src);
            }
            prepared.push(PreparedBatch {
                texture_id: b.texture_id,
                dem_source: b.dem_source,
                instance_count: b.instances.len() as u32,
            });
        }
        PreparedRaster { batches: prepared }
    }

    /// GPU half of the frame: replay the prepared draw list inside the
    /// Map's single render pass. Only `pass.set_*`/`draw*` calls plus
    /// immutable cache lookups — `prepare` already touched every tile
    /// referenced here, so the `expect`s can't fire within one
    /// `Map::render`.
    pub(crate) fn draw(
        &self,
        prepared: &PreparedRaster,
        cache: &TextureCache,
        terrain: Option<&TerrainCache>,
        placeholder_dem: &wgpu::BindGroup,
        shadow_bg: &wgpu::BindGroup,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if prepared.batches.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        // Frame-global cast-shadow grid (group 3). Constant for the whole draw
        // list — bound once. When shadows are off, `shadow_strength` in Globals
        // is 0 so the shader never samples it (the texture is the fully-lit
        // placeholder anyway).
        pass.set_bind_group(3, shadow_bg, &[]);

        let mut start: u32 = 0;
        for b in &prepared.batches {
            let end = start + b.instance_count;
            // Basemap colour texture. `prepare` touched it, but under cache
            // budget pressure the LRU can still evict between prepare and draw.
            // Skip this batch's instances rather than panic — a missing tile is
            // one grey quad for a frame; an `expect` here is a crash.
            let Some(entry) = cache.peek(b.texture_id) else {
                start = end;
                continue;
            };
            pass.set_bind_group(1, &entry.bind_group, &[]);
            // DEM height texture. The batch already resolved which DEM source
            // tile to use; rebind it, or fall back to the flat placeholder if
            // terrain is absent or the DEM tile was evicted (renders flat for
            // this frame instead of crashing).
            match b.dem_source.and_then(|src| terrain.and_then(|t| t.peek_entry(src))) {
                Some(entry) => pass.set_bind_group(2, &entry.bind_group, &[]),
                None => pass.set_bind_group(2, placeholder_dem, &[]),
            }
            pass.draw_indexed(0..self.index_count, 0, start..end);
            start = end;
        }
    }
}

/// Append `inst` to the trailing batch if it matches the
/// `(texture_id, dem_source)` key, else open a new batch. Keeping the
/// per-tile draws contiguous preserves the backdrop-then-child
/// ordering needed for the fade blend.
fn push_instance(
    batches: &mut Vec<DrawBatch>,
    texture_id: TileId,
    dem_source: Option<TileId>,
    inst: Instance,
) {
    match batches.last_mut() {
        Some(b) if b.texture_id == texture_id && b.dem_source == dem_source => {
            b.instances.push(inst);
        }
        _ => batches.push(DrawBatch {
            texture_id,
            dem_source,
            instances: vec![inst],
        }),
    }
}

/// Resolve which DEM tile to bind when drawing the world quad at
/// `drawn_tile`, and the sub-UV inside that DEM that maps to the
/// drawn_tile's footprint. The 0/0/1 fallback matches the 1×1
/// zero-elevation placeholder so omitting terrain has no effect.
fn resolve_dem_subuv(
    drawn_tile: TileId,
    terrain: Option<&mut TerrainCache>,
    halo_uv: f32,
) -> (Option<TileId>, [f32; 2], f32) {
    // The ancestor-walk + sub-UV remap lives on `TerrainCache` so the vector
    // pipeline drapes lines/fills the same way; this is a thin adapter for the
    // `Option<&mut>` the raster prepare threads through.
    match terrain {
        Some(t) => t.resolve_subuv(drawn_tile, halo_uv),
        None => (None, [0.0, 0.0], 1.0),
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Best-available coverage resolver (LOD retention).
//
// The old draw model iterated `scene.visible_tiles()` — a SINGLE target LOD —
// and used ancestor/descendant tiles only as a transient backdrop while a tile
// faded in. That made zoom-out *downgrade*: the deeper, higher-res tiles we
// already hold left the visible set and stopped drawing, replaced by a coarser
// tile (instantly, if it was cached). It also left the far field as a grey
// hole (nothing beyond the ring was ever drawn).
//
// This resolver instead answers, per ideal cell: "what is the best-resolution
// RESIDENT coverage I can draw right now?" — preferring FINER tiles (children
// before the cell itself) so retained detail wins over a coarse ideal, and
// falling to a coarse ancestor backdrop only where nothing finer is resident.
// It is a pure function of an `is_resident` predicate + tile arithmetic, so it
// is unit-tested exhaustively below with a plain `HashSet` (no GPU/cache).
// ──────────────────────────────────────────────────────────────────────────

/// How many zoom levels of already-resident *finer* detail to retain below the
/// ideal cell on zoom-out. Bounds transient overdraw (≤ `4^MAX_DOWN` sub-tiles
/// per ideal cell) before the LRU evicts the deep tiles; each recursion branch
/// also stops at the first resident level, so the typical count is far lower.
pub(crate) const MAX_DOWN: u8 = 2;

/// One unit of resident basemap coverage for an ideal cell, in draw order
/// (coarse backdrop first, finer detail on top).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CellSource {
    /// Draw `0` at its own world rect with full UV. It is the ideal cell or a
    /// resident descendant of it (finer detail retained across a zoom-out).
    Whole(TileId),
    /// No exact-or-finer tile covers (part of) the cell: draw resident
    /// `ancestor` stretched across `target`'s footprint (sub-UV sampled) as a
    /// coarse backdrop beneath any partial finer tiles.
    AncestorPatch { ancestor: TileId, target: TileId },
}

/// Best-available resident coverage for `ideal`. See module note above.
pub(crate) fn resolve_cell(
    ideal: TileId,
    max_down: u8,
    resident: &impl Fn(TileId) -> bool,
) -> Vec<CellSource> {
    // Fully covered by exact-or-finer resident tiles → draw just those (finest
    // wins; zero backdrop, zero overdraw beyond the retained detail).
    if let Some(tiles) = cover_finest(ideal, max_down, resident) {
        return tiles.into_iter().map(CellSource::Whole).collect();
    }
    // Otherwise: a coarse ancestor backdrop fills the whole cell, with whatever
    // partial finer tiles exist drawn on top. Either may be absent (→ Empty).
    let mut out = Vec::new();
    if let Some(anc) = nearest_resident_ancestor(ideal, resident) {
        out.push(CellSource::AncestorPatch { ancestor: anc, target: ideal });
    }
    collect_resident_descendants(ideal, max_down, resident, &mut out);
    out
}

/// `Some(finest resident tiles fully covering `cell`)`, or `None` if some
/// sub-region has no resident tile within `cell.z ..= cell.z + depth`. Tries
/// children FIRST so finer detail is preferred over a resident-but-coarse cell.
fn cover_finest(
    cell: TileId,
    depth: u8,
    resident: &impl Fn(TileId) -> bool,
) -> Option<Vec<TileId>> {
    if depth > 0 {
        if let Some(children) = cell.children() {
            let mut acc = Vec::new();
            let mut all = true;
            for c in children {
                match cover_finest(c, depth - 1, resident) {
                    Some(v) => acc.extend(v),
                    None => {
                        all = false;
                        break;
                    }
                }
            }
            if all {
                return Some(acc);
            }
        }
    }
    if resident(cell) {
        Some(vec![cell])
    } else {
        None
    }
}

/// Push the finest resident tiles found within `cell` (partial coverage ok),
/// stopping each branch at the first resident level. Used to overlay whatever
/// detail exists on top of a coarse ancestor backdrop.
fn collect_resident_descendants(
    cell: TileId,
    depth: u8,
    resident: &impl Fn(TileId) -> bool,
    out: &mut Vec<CellSource>,
) {
    if resident(cell) {
        out.push(CellSource::Whole(cell));
        return;
    }
    if depth == 0 {
        return;
    }
    if let Some(children) = cell.children() {
        for c in children {
            collect_resident_descendants(c, depth - 1, resident, out);
        }
    }
}

/// Nearest resident ancestor of `ideal` (walking up), or `None`.
fn nearest_resident_ancestor(ideal: TileId, resident: &impl Fn(TileId) -> bool) -> Option<TileId> {
    for k in 1..=ideal.z {
        let a = ideal.ancestor(k)?;
        if resident(a) {
            return Some(a);
        }
    }
    None
}

#[cfg(test)]
mod lod_resolver_tests {
    //! Stage-0 gate for the LOD-retention redesign. These lock the *correct*
    //! behaviour the old `prepare` (single target LOD + transient fallback)
    //! violated: it would draw a coarse cached tile over finer detail we still
    //! hold (the "zoom-out replaces the good tile with a worse one" report).

    use super::{resolve_cell, CellSource, MAX_DOWN};
    use crate::tile::TileId;
    use std::collections::HashSet;

    fn resident_set(tiles: &[TileId]) -> impl Fn(TileId) -> bool {
        let set: HashSet<TileId> = tiles.iter().copied().collect();
        move |t| set.contains(&t)
    }

    #[test]
    fn exact_resident_tile_draws_itself_only() {
        let ideal = TileId::new(14, 100, 200);
        let r = resident_set(&[ideal]);
        assert_eq!(resolve_cell(ideal, MAX_DOWN, &r), vec![CellSource::Whole(ideal)]);
    }

    #[test]
    fn zoom_out_retains_finer_children_instead_of_coarse_ideal() {
        // The headline bug: the ideal (coarse) tile IS resident, but we also
        // still hold all four finer children. The resolver must draw the FINER
        // children, never downgrade to the coarse ideal.
        let ideal = TileId::new(14, 100, 200);
        let children = ideal.children().unwrap();
        let mut held = vec![ideal];
        held.extend_from_slice(&children);
        let r = resident_set(&held);

        let got = resolve_cell(ideal, MAX_DOWN, &r);
        let expected: Vec<CellSource> = children.iter().copied().map(CellSource::Whole).collect();
        assert_eq!(got, expected, "must retain finer children, not draw coarse ideal");
    }

    #[test]
    fn zoom_out_uses_children_when_ideal_not_yet_loaded() {
        // Coarse ideal not fetched yet, but its four children are resident
        // (we were just zoomed in). Draw the children — no grey, no flat.
        let ideal = TileId::new(13, 50, 60);
        let children = ideal.children().unwrap();
        let r = resident_set(&children);
        let got = resolve_cell(ideal, MAX_DOWN, &r);
        assert_eq!(got.len(), 4);
        assert!(got.iter().all(|s| matches!(s, CellSource::Whole(t) if children.contains(t))));
    }

    #[test]
    fn coarse_ancestor_is_backdrop_when_nothing_finer_resident() {
        let ideal = TileId::new(15, 1000, 2000);
        let anc = ideal.ancestor(2).unwrap();
        let r = resident_set(&[anc]);
        assert_eq!(
            resolve_cell(ideal, MAX_DOWN, &r),
            vec![CellSource::AncestorPatch { ancestor: anc, target: ideal }],
        );
    }

    #[test]
    fn partial_finer_detail_draws_over_ancestor_backdrop() {
        // Ideal absent; ancestor resident; ONE of four children resident.
        // Expect the ancestor backdrop FIRST, then the partial finer tile.
        let ideal = TileId::new(14, 8, 8);
        let anc = ideal.ancestor(1).unwrap();
        let one_child = ideal.children().unwrap()[0];
        let r = resident_set(&[anc, one_child]);

        let got = resolve_cell(ideal, MAX_DOWN, &r);
        assert_eq!(got.first(), Some(&CellSource::AncestorPatch { ancestor: anc, target: ideal }));
        assert!(got.contains(&CellSource::Whole(one_child)));
    }

    #[test]
    fn nothing_resident_anywhere_is_empty() {
        let ideal = TileId::new(14, 100, 200);
        let r = resident_set(&[]);
        assert!(resolve_cell(ideal, MAX_DOWN, &r).is_empty());
    }

    #[test]
    fn steady_state_has_no_overdraw() {
        // No deeper tiles resident → exactly the ideal, no children, no backdrop.
        let ideal = TileId::new(16, 30000, 20000);
        let r = resident_set(&[ideal]);
        assert_eq!(resolve_cell(ideal, MAX_DOWN, &r), vec![CellSource::Whole(ideal)]);
    }
}
