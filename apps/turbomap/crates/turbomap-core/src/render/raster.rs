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
            bind_group_layouts: &[Some(&camera_bgl), Some(&texture_bgl), Some(terrain_bgl)],
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
                indices.extend_from_slice(&[i, i_right, i_diag, i, i_diag, i_down]);
            }
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
            device,
            queue,
        }
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
            }),
        );

        // Build draw batches. For each visible tile we emit:
        //   • A backdrop (when self isn't fully faded in): preferred is a
        //     shallower ancestor sub-sampled to fit; if no ancestor is
        //     cached, we use any deeper descendant tiles that cover part of
        //     this region instead (this is what makes zoom-out smooth).
        //   • Self on top, at smoothstep-ramped alpha — but only when a
        //     backdrop exists. Fading in from a black clear colour reads as
        //     a "pop" rather than a blend, so without a backdrop we just
        //     show the tile instantly.
        let mut batches: Vec<DrawBatch> = Vec::new();
        for tile in scene.visible_tiles() {
            let (nw, _se) = tile.world_bounds();
            let world_size = 1.0 / (1u64 << tile.z) as f32;
            let nw_f32 = [(nw.x - origin.x) as f32, (nw.y - origin.y) as f32];

            let self_age = cache.age_secs(tile);
            let ancestor = cache.nearest_ancestor(tile);
            let descendants = if ancestor.is_none() {
                cache.covered_descendants(tile, 3)
            } else {
                Vec::new()
            };
            // Fade every freshly-ingested tile, whether it blends over a cached
            // ancestor/descendant backdrop OR straight over the empty-tile clear
            // colour. The clear is a light grey (not black), so fading up from it
            // reads as a soft reveal, not a pop — and crucially this makes
            // disk-cached tiles (which arrive all-at-once with no backdrop) fade in
            // too, instead of snapping. `fade_in_secs == 0` (goldens) still snaps.
            let self_alpha = match self_age {
                None => 0.0,
                Some(age) if fade_in_secs <= 0.0 || age >= fade_in_secs => 1.0,
                Some(age) => {
                    let t = (age / fade_in_secs).clamp(0.0, 1.0);
                    t * t * (3.0 - 2.0 * t) // smoothstep
                }
            };

            // Backdrop layer.
            if self_alpha < 1.0 {
                if let Some(ancestor_id) = ancestor {
                    let sub = tile
                        .sub_uv_in(ancestor_id)
                        .expect("nearest_ancestor must be a real ancestor");
                    let (dem_src, dem_uv_origin, dem_uv_size) =
                        resolve_dem_subuv(tile, terrain.as_deref_mut(), halo_uv);
                    push_instance(
                        &mut batches,
                        ancestor_id,
                        dem_src,
                        Instance {
                            world_origin: nw_f32,
                            world_size,
                            alpha: 1.0,
                            uv_origin: [sub.origin.x as f32, sub.origin.y as f32],
                            uv_size: sub.size as f32,
                            dem_uv_origin,
                            dem_uv_size,
                            _pad: 0.0,
                        },
                    );
                } else {
                    for desc in &descendants {
                        let (d_nw, _) = desc.world_bounds();
                        let d_size = 1.0 / (1u64 << desc.z) as f32;
                        let (dem_src, dem_uv_origin, dem_uv_size) =
                            resolve_dem_subuv(*desc, terrain.as_deref_mut(), halo_uv);
                        push_instance(
                            &mut batches,
                            *desc,
                            dem_src,
                            Instance {
                                world_origin: [(d_nw.x - origin.x) as f32, (d_nw.y - origin.y) as f32],
                                world_size: d_size,
                                alpha: 1.0,
                                uv_origin: [0.0, 0.0],
                                uv_size: 1.0,
                                dem_uv_origin,
                                dem_uv_size,
                                _pad: 0.0,
                            },
                        );
                    }
                }
            }

            // Foreground layer: self.
            if self_alpha > 0.0 {
                let (dem_src, dem_uv_origin, dem_uv_size) =
                    resolve_dem_subuv(tile, terrain.as_deref_mut(), halo_uv);
                push_instance(
                    &mut batches,
                    tile,
                    dem_src,
                    Instance {
                        world_origin: nw_f32,
                        world_size,
                        alpha: self_alpha,
                        uv_origin: [0.0, 0.0],
                        uv_size: 1.0,
                        dem_uv_origin,
                        dem_uv_size,
                        _pad: 0.0,
                    },
                );
            }
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
            let _ = cache
                .get(b.texture_id)
                .expect("batch references uncached texture");
            if let Some(src) = b.dem_source {
                let t = terrain
                    .as_deref_mut()
                    .expect("dem_source set implies terrain present");
                let _ = t
                    .get_entry(src)
                    .expect("dem_source was cached at resolve time");
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

        let mut start: u32 = 0;
        for b in &prepared.batches {
            // Basemap colour texture.
            let entry = cache
                .peek(b.texture_id)
                .expect("prepare touched this texture");
            pass.set_bind_group(1, &entry.bind_group, &[]);
            // DEM height texture. The batch already resolved which
            // DEM source tile to use (self or nearest ancestor); we
            // just rebind it. The per-instance dem_uv_origin/size
            // narrows the sample window to each drawn tile's slice
            // of that DEM source.
            match b.dem_source {
                Some(src) => {
                    let t = terrain.expect("dem_source set implies terrain present");
                    let entry = t.peek_entry(src).expect("prepare touched this DEM tile");
                    pass.set_bind_group(2, &entry.bind_group, &[]);
                }
                None => pass.set_bind_group(2, placeholder_dem, &[]),
            }
            let end = start + b.instance_count;
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
    let interior = 1.0 - 2.0 * halo_uv;
    let Some(t) = terrain else {
        return (None, [0.0, 0.0], 1.0);
    };
    let Some(binding) = t.bind_for(drawn_tile) else {
        return (None, [0.0, 0.0], 1.0);
    };
    let source = binding.source_tile;
    let sub = drawn_tile
        .sub_uv_in(source)
        .expect("TerrainCache::bind_for must return self or an ancestor");
    let origin = [
        halo_uv + sub.origin.x as f32 * interior,
        halo_uv + sub.origin.y as f32 * interior,
    ];
    let size = sub.size as f32 * interior;
    (Some(source), origin, size)
}
