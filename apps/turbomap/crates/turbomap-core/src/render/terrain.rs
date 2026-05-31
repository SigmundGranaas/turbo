//! Shared DEM heightmap accessible to every ground-plane pipeline.
//!
//! The hillshade layer used to own its own `TextureCache` of DEM tiles.
//! For 3D terrain we need the *same* DEM available to the basemap and
//! vector pipelines too — otherwise the layers would all displace by
//! whatever they each fetched and visibly separate vertically. We
//! lift the cache to `Map`-level state, expose a `bind_group_for`
//! lookup that pipelines call per draw, and route DEM ingest through
//! one path.
//!
//! The cache stores 258×258 PNG tiles in `Rgba8Unorm` (linear, raw
//! Terrain-RGB bytes) — same shape the hillshade pipeline has been
//! consuming. Halo is required: without it, vertex displacement at
//! adjacent tile edges samples different DEM cells and the mesh
//! cracks.

use std::sync::Arc;

use crate::dem::DemEncoding;
use crate::tile::TileId;

use super::cache::TextureCache;

#[derive(Debug, Clone)]
pub struct TerrainOptions {
    /// Vertical exaggeration. 1.0 = realistic relief. Map renderers
    /// commonly run at 1.3–2.0 because realistic relief is hard to
    /// see at small map scales.
    pub exaggeration: f32,
    /// Encoding of the DEM source. Defaults to Mapbox Terrain-RGB.
    pub encoding: DemEncoding,
    /// Maximum source elevation, used to size the depth range. The
    /// camera near/far planes are derived from this so we have
    /// reasonable depth precision over the heightmap. Norway's
    /// highest point is 2 469 m — round up to 3 000 m for slack.
    pub max_elevation_m: f32,
}

impl Default for TerrainOptions {
    fn default() -> Self {
        Self {
            exaggeration: 1.5,
            encoding: DemEncoding::MapboxRgb,
            max_elevation_m: 3_000.0,
        }
    }
}

/// Map-level shared resources: a bind-group layout + sampler + a
/// placeholder 1×1 zero-elevation bind group. Created once at
/// `Map::new` and lent to pipelines that may sample the DEM. Keeping
/// these out of `TerrainCache` lets pipelines be constructed before
/// any terrain source is registered — they just bind the placeholder
/// every draw and render flat geometry.
pub(crate) struct TerrainShared {
    pub(crate) bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub(crate) sampler: Arc<wgpu::Sampler>,
    pub(crate) placeholder_bind_group: wgpu::BindGroup,
}

impl TerrainShared {
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("turbomap-terrain-bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            },
        ));
        let sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-terrain-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        }));
        let placeholder_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("turbomap-terrain-placeholder"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Mapbox Terrain-RGB encoding of 0 m elevation: (1, 134, 160).
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &placeholder_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[1, 134, 160, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let placeholder_view = placeholder_tex.create_view(&Default::default());
        let placeholder_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-terrain-placeholder-bg"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&placeholder_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            bind_group_layout: layout,
            sampler,
            placeholder_bind_group,
        }
    }
}

#[allow(dead_code)] // Phase 4+ — held in place for raster/vector displacement.
pub(crate) struct TerrainCache {
    cache: TextureCache,
    pub(crate) bind_group_layout: Arc<wgpu::BindGroupLayout>,
    pub(crate) sampler: Arc<wgpu::Sampler>,
    halo_px: u32,
}

impl TerrainCache {
    /// Build the per-source cache, sharing the Map-level layout +
    /// sampler held by `TerrainShared`. Created in `set_terrain_source`
    /// — the shared resources outlive any individual cache.
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        shared: &TerrainShared,
        budget_bytes: usize,
        halo_px: u32,
    ) -> Self {
        let cache = TextureCache::new(
            device,
            queue,
            shared.bind_group_layout.clone(),
            shared.sampler.clone(),
            budget_bytes,
            // Raw byte values — gamma curve must not be applied.
            wgpu::TextureFormat::Rgba8Unorm,
            // No mipmaps. Vertex displacement needs the base-level
            // height; mipping would smooth peaks at distance.
            false,
        );
        Self {
            cache,
            bind_group_layout: shared.bind_group_layout.clone(),
            sampler: shared.sampler.clone(),
            halo_px,
        }
    }

    pub(crate) fn halo_px(&self) -> u32 {
        self.halo_px
    }

    pub(crate) fn ingest(&mut self, tile: TileId, rgba: &[u8], width: u32, height: u32) {
        self.cache.insert(tile, rgba, width, height);
    }

    /// Age of the cached tile in seconds, or `None` if not present.
    /// Mirrors `TextureCache::age_secs` so the hillshade pipeline can
    /// compute per-tile fade-in directly off the shared cache.
    pub(crate) fn age_secs(&self, id: TileId) -> Option<f32> {
        self.cache.age_secs(id)
    }

    /// Touch + return the cache entry for `id`. Bumps the LRU.
    pub(crate) fn get_entry(
        &mut self,
        id: TileId,
    ) -> Option<&crate::render::cache::CacheEntry> {
        self.cache.get(id)
    }

    pub(crate) fn stats(&self) -> crate::render::cache::CacheStats {
        self.cache.stats()
    }

    /// Bind group for the DEM tile that should be used when drawing
    /// `tile`. Returns the exact tile if cached, otherwise walks up
    /// the pyramid for the nearest cached ancestor. The caller's
    /// vertex shader uses `source_tile` to remap its tile-local UVs
    /// into a sub-region of the ancestor's texture (when source_tile
    /// != requested).
    pub(crate) fn bind_for(&mut self, tile: TileId) -> Option<TerrainBinding<'_>> {
        // Two-step lookup to satisfy the borrow checker: first decide
        // which tile to use, then borrow that one immutably.
        let source = if self.cache.get(tile).is_some() {
            tile
        } else {
            self.cache.nearest_ancestor(tile)?
        };
        let entry = self.cache.get(source)?;
        Some(TerrainBinding {
            bind_group: &entry.bind_group,
            source_tile: source,
            halo_px: self.halo_px,
        })
    }

    /// Decoded elevation at world-space `(x, y)` on the ground plane,
    /// using whatever DEM tile currently covers that point. Used by
    /// the CPU side for hit-testing + label anchoring.
    pub(crate) fn elevation_at_world(
        &self,
        _world: (f64, f64),
        _encoding: DemEncoding,
    ) -> Option<f32> {
        // Stub for Phase 6. The cache holds compressed bind-group
        // entries; we'd need to keep a parallel CPU-side height map
        // to answer this cheaply. Hit-test ray-marching will be
        // wired here.
        None
    }
}

/// What a pipeline binds when drawing one tile. `source_tile` may
/// differ from the requested tile when an ancestor is being used as
/// fallback — the shader uses that to map vertex UVs to the right
/// sub-region of the ancestor's texture.
#[allow(dead_code)]
pub(crate) struct TerrainBinding<'a> {
    pub bind_group: &'a wgpu::BindGroup,
    pub source_tile: TileId,
    pub halo_px: u32,
}
