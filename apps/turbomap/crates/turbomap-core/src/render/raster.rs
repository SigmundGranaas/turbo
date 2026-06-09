//! Raster-tile render pipeline.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{scene::Scene, tile::TileId};

use super::{
    cache::TextureCache,
    terrain::TerrainCache,
    BACKGROUND_CLEAR, DEPTH_FORMAT,
};

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
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            meters_to_world: 0.0,
            exaggeration: 1.0,
            encoding: 0,
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
                    visibility: wgpu::ShaderStages::VERTEX,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        &mut self,
        scene: &Scene,
        cache: &mut TextureCache,
        mut terrain: Option<&mut TerrainCache>,
        placeholder_dem: &wgpu::BindGroup,
        terrain_options: TerrainConfig,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        fade_in_secs: f32,
        is_first_layer: bool,
    ) {
        let camera = scene.camera();
        let (vw, vh) = scene.viewport_px();
        let uniform = CameraUniform {
            view_proj: camera.view_projection_matrix((vw, vh)),
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
            let nw_f32 = [nw.x as f32, nw.y as f32];

            let self_age = cache.age_secs(tile);
            let ancestor = cache.nearest_ancestor(tile);
            let descendants = if ancestor.is_none() {
                cache.covered_descendants(tile, 3)
            } else {
                Vec::new()
            };
            let has_backdrop = ancestor.is_some() || !descendants.is_empty();

            let self_alpha = match self_age {
                None => 0.0,
                Some(_) if !has_backdrop => 1.0, // no blend possible — show now
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
                                world_origin: [d_nw.x as f32, d_nw.y as f32],
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
            if is_first_layer {
                // Clear the target so the previous frame doesn't linger.
                let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("turbomap-raster-clear-only"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        resolve_target: None,
                depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(BACKGROUND_CLEAR),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
            multiview_mask: None,
                });
            }
            return;
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

        // No pre-collection of bind-group refs: `wgpu::BindGroup`
        // isn't Clone in wgpu 22 and the basemap + terrain caches are
        // independent fields on Map, so we can mutably re-borrow each
        // inside the per-batch loop below — `pass.set_bind_group`
        // internally Arc-counts the binding, so we don't need to
        // hold the &BindGroup across iterations.

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("turbomap-raster-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: if is_first_layer {
                        wgpu::LoadOp::Clear(BACKGROUND_CLEAR)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: if is_first_layer {
                        wgpu::LoadOp::Clear(1.0)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);

        let mut start: u32 = 0;
        for b in &batches {
            // Basemap colour texture.
            {
                let entry = cache
                    .get(b.texture_id)
                    .expect("batch references uncached texture");
                pass.set_bind_group(1, &entry.bind_group, &[]);
            }
            // DEM height texture. The batch already resolved which
            // DEM source tile to use (self or nearest ancestor); we
            // just rebind it. The per-instance dem_uv_origin/size
            // narrows the sample window to each drawn tile's slice
            // of that DEM source.
            match b.dem_source {
                Some(src) => {
                    let t = terrain
                        .as_deref_mut()
                        .expect("dem_source set implies terrain present");
                    let entry = t
                        .get_entry(src)
                        .expect("dem_source was cached at resolve time");
                    pass.set_bind_group(2, &entry.bind_group, &[]);
                }
                None => pass.set_bind_group(2, placeholder_dem, &[]),
            }
            let end = start + b.instances.len() as u32;
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
