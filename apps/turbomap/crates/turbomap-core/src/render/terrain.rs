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

use std::collections::HashMap;
use std::sync::Arc;

use crate::dem::{decode_elevation, DemEncoding};
use crate::geo::WorldPoint;
use crate::scene::Scene;
use crate::source::TileSource;
use crate::tile::TileId;

use super::cache::TextureCache;

/// The active terrain subsystem: the DEM tile source plus the GPU + CPU caches
/// and the visibility `Scene` that track it. `Map` holds an `Option<Terrain>`
/// (None = flat 2D); this bundles the four pieces that move together and the
/// terrain-aware queries (surface height) that used to live as loose helpers
/// on the `Map` god-object.
pub(crate) struct Terrain {
    /// DEM source the host drains via `PendingTile::Terrain`. Drives both the
    /// tile cache ([`TerrainCache`]) and visibility tracking ([`Scene`]).
    pub(crate) source: Arc<dyn TileSource>,
    pub(crate) cache: TerrainCache,
    pub(crate) scene: Scene,
    pub(crate) options: TerrainOptions,
}

impl Terrain {
    pub(crate) fn new(
        source: Arc<dyn TileSource>,
        cache: TerrainCache,
        scene: Scene,
        options: TerrainOptions,
    ) -> Self {
        Self {
            source,
            cache,
            scene,
            options,
        }
    }

    /// World-space displaced height (z) of the ground at `world`, matching the
    /// terrain mesh: `elevation · meters_to_world · exaggeration`. 0 when no
    /// covering DEM tile is resident yet. `meters_to_world` is supplied by the
    /// caller (it depends on the camera-centre latitude — the same value the
    /// per-frame mesh displacement uses), so overlays align with the surface.
    pub(crate) fn ground_world_z(&self, world: WorldPoint, meters_to_world: f32) -> f32 {
        // Stable sampler: markers anchored here must not flicker as DEM tiles
        // load/evict (the per-frame churn snapped their screen position).
        match self.cache.elevation_at_world_stable((world.x, world.y)) {
            Some(elev) => elev * meters_to_world * self.options.exaggeration,
            None => 0.0,
        }
    }
}

/// Resolution (per side) of the CPU-side elevation grid kept per DEM
/// tile. The GPU keeps the full 256² texture for crisp displacement; the
/// CPU needs enough to anchor markers, drape paths, and — the finest
/// consumer — drive the cast-shadow horizon march ([`super::shadow`]),
/// which wants relief detail comparable to the rendered mesh or shadows
/// blur out. 128² = 64 KiB/tile (≈ 37 m/sample at z13) is a good balance.
const CPU_HEIGHT_DIM: usize = 128;

/// A downsampled, halo-trimmed elevation grid (metres) for one DEM tile,
/// retained CPU-side so the host can ask "how high is the ground here?"
/// without a GPU read-back. `CPU_HEIGHT_DIM × CPU_HEIGHT_DIM`, row-major,
/// covering the tile's geographic interior in [0,1]² tile-local UV.
struct HeightTile {
    grid: Vec<f32>,
}

impl HeightTile {
    /// Bilinearly sample the grid at tile-local `(u, v)` in [0,1].
    fn sample(&self, u: f32, v: f32) -> f32 {
        let n = CPU_HEIGHT_DIM;
        let fx = (u.clamp(0.0, 1.0) * (n - 1) as f32).clamp(0.0, (n - 1) as f32);
        let fy = (v.clamp(0.0, 1.0) * (n - 1) as f32).clamp(0.0, (n - 1) as f32);
        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1).min(n - 1);
        let y1 = (y0 + 1).min(n - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let at = |x: usize, y: usize| self.grid[y * n + x];
        let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
        let bot = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
        top * (1.0 - ty) + bot * ty
    }
}

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
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
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
            wgpu::TexelCopyTextureInfo {
                texture: &placeholder_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[1, 134, 160, 255],
            wgpu::TexelCopyBufferLayout {
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

pub(crate) struct TerrainCache {
    cache: TextureCache,
    // Held for symmetry with the Map-level shared resources; pipelines
    // bind the `TerrainShared` copies, so these are currently unread.
    #[allow(dead_code)]
    pub(crate) bind_group_layout: Arc<wgpu::BindGroupLayout>,
    #[allow(dead_code)]
    pub(crate) sampler: Arc<wgpu::Sampler>,
    halo_px: u32,
    /// DEM encoding, needed to decode raw bytes into the CPU height grid.
    encoding: DemEncoding,
    /// CPU-side elevations, kept in lock-step with the GPU cache (evicted
    /// tiles are dropped here too). Lets the host anchor markers + drape
    /// paths on the surface without a GPU read-back.
    heights: HashMap<TileId, HeightTile>,
    /// Sticky last-known elevation per quantised world cell, with the zoom it
    /// came from. Marker/overlay projection samples through this so a fine DEM
    /// tile evicting under load (→ a coarser sample, metres off at 6× exaggeration)
    /// doesn't snap the marker's screen position — the cause of the "A/B/C + flag
    /// markers flicker like crazy when tiles load". Never regresses to a coarser
    /// zoom; refines when a finer tile arrives. Interior-mutable so the read-only
    /// projection path (`&self`, under the render lock) can update it.
    sticky_elev: std::sync::Mutex<HashMap<(i64, i64), (u8, f32)>>,
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
        encoding: DemEncoding,
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
            encoding,
            heights: HashMap::new(),
            sticky_elev: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn halo_px(&self) -> u32 {
        self.halo_px
    }

    /// Ingest a DEM tile. Returns the ids evicted to stay within budget,
    /// so the caller drops them from the terrain scene's "ingested" set
    /// and they get re-requested when next desired.
    pub(crate) fn ingest(
        &mut self,
        tile: TileId,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Vec<TileId> {
        let evicted = self.cache.insert(tile, rgba, width, height);
        // Keep a CPU-side elevation grid in lock-step with the GPU cache.
        if let Some(ht) = self.decode_height_tile(rgba, width, height) {
            self.heights.insert(tile, ht);
        }
        for e in &evicted {
            self.heights.remove(e);
        }
        evicted
    }

    /// Decode raw DEM bytes into a downsampled, halo-trimmed elevation
    /// grid. `width`/`height` include the halo ring (e.g. 258 for a
    /// 256-tile with 1 px halo); we sample the geographic interior only,
    /// so adjacent tiles' grids line up at their shared edge.
    fn decode_height_tile(&self, rgba: &[u8], width: u32, height: u32) -> Option<HeightTile> {
        decode_height_grid(rgba, width, height, self.halo_px, self.encoding)
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

    /// Read-only lookup — no LRU bump. Draw-time counterpart of
    /// [`TerrainCache::get_entry`]; the prepare phase already touched
    /// every tile a draw will reference.
    pub(crate) fn peek_entry(&self, id: TileId) -> Option<&crate::render::cache::CacheEntry> {
        self.cache.peek(id)
    }

    /// Bind group for the EXACT DEM tile `id` if it's resident (no
    /// ancestor fallback), read-only. The vector pipeline binds this to
    /// drape lines/fills, falling back to the zero-elevation placeholder
    /// when the precise tile isn't loaded — keeping the binding simple
    /// (no sub-UV remap) at the cost of leaving a not-yet-loaded tile's
    /// features briefly flat.
    pub(crate) fn exact_bind_group(&self, id: TileId) -> Option<&wgpu::BindGroup> {
        self.cache.peek(id).map(|e| &e.bind_group)
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

    /// Resolve the DEM tile + sub-UV a ground layer should drape `drawn_tile`'s
    /// geometry onto. Returns `(source_tile, dem_uv_origin, dem_uv_size)`:
    ///   • `source_tile` is `drawn_tile` itself when resident, else the nearest
    ///     cached ancestor (so deep zooms still drape on a shallow DEM).
    ///   • `dem_uv_origin`/`dem_uv_size` map the tile-local `[0,1]` base position
    ///     into the matching sub-rectangle of that source's *haloed* heightmap
    ///     (the `halo_uv` inset keeps the sample inside the non-halo interior).
    /// Falls back to `(None, [0,0], 1)` — the flat 1×1 placeholder UV — when no
    /// terrain covers the tile yet. This is the ancestor-walk the raster basemap
    /// already uses (`resolve_dem_subuv`), lifted so the vector pipeline drapes
    /// lines/fills at any zoom instead of only when the exact DEM tile is loaded.
    pub(crate) fn resolve_subuv(
        &mut self,
        drawn_tile: TileId,
        halo_uv: f32,
    ) -> (Option<TileId>, [f32; 2], f32) {
        let interior = 1.0 - 2.0 * halo_uv;
        let Some(binding) = self.bind_for(drawn_tile) else {
            return (None, [0.0, 0.0], 1.0);
        };
        let source = binding.source_tile;
        let Some(sub) = drawn_tile.sub_uv_in(source) else {
            return (None, [0.0, 0.0], 1.0);
        };
        let origin = [
            halo_uv + sub.origin.x as f32 * interior,
            halo_uv + sub.origin.y as f32 * interior,
        ];
        (Some(source), origin, sub.size as f32 * interior)
    }

    /// Decoded elevation (metres) at world-space `(x, y)` on the ground
    /// plane, sampled from the deepest DEM tile currently resident that
    /// covers the point (so markers/paths anchor to the finest available
    /// detail). `None` when no covering tile is loaded yet — the caller
    /// then treats it as flat (z=0), same as the 2D map.
    pub(crate) fn elevation_at_world(&self, world: (f64, f64)) -> Option<f32> {
        self.sample_deepest(world).map(|(_, e)| e)
    }

    /// Deepest (finest) resident DEM sample at `world`, with the zoom it came
    /// from. `None` if no covering tile is resident.
    fn sample_deepest(&self, world: (f64, f64)) -> Option<(u8, f32)> {
        self.sample_deepest_capped(world, self.finest_resident_zoom())
    }

    /// Finest DEM zoom currently resident — the ceiling for [`sample_deepest`]'s
    /// top-down scan. Starting at a fixed `z=22` wastes ~10 guaranteed-miss
    /// hashmap probes PER SAMPLE (no DEM tile that fine is ever resident), which
    /// dominated the 256² shadow-heightfield reassembly (~655k probes/rebuild).
    /// Capping at the actual finest zoom is behaviour-identical — the skipped
    /// probes could only have missed — and ~10× cheaper. `0` when empty (the
    /// scan then immediately returns `None`).
    pub(crate) fn finest_resident_zoom(&self) -> u8 {
        self.heights.keys().map(|t| t.z).max().unwrap_or(0)
    }

    /// As [`sample_deepest`] but with an explicit scan ceiling, so a hot loop
    /// (the shadow grid) computes the finest resident zoom ONCE and reuses it
    /// across all 65k samples instead of recomputing — or rescanning from 22 —
    /// per cell.
    fn sample_deepest_capped(&self, world: (f64, f64), max_z: u8) -> Option<(u8, f32)> {
        let (wx, wy) = world;
        if !(0.0..=1.0).contains(&wx) || !(0.0..=1.0).contains(&wy) {
            return None;
        }
        // Deepest zoom first — finest resident detail wins.
        for z in (0u8..=max_z).rev() {
            let n = 1u32 << z as u32;
            let nf = n as f64;
            let tx = (wx * nf).floor().min((n - 1) as f64) as u32;
            let ty = (wy * nf).floor().min((n - 1) as f64) as u32;
            if let Some(ht) = self.heights.get(&TileId::new(z, tx, ty)) {
                let u = (wx * nf - tx as f64) as f32;
                let v = (wy * nf - ty as f64) as f32;
                return Some((z, ht.sample(u, v)));
            }
        }
        None
    }

    /// Bulk elevation sampler for the cast-shadow / AO heightfield: the same
    /// "deepest resident wins" lookup as [`elevation_at_world`], but with the
    /// finest-zoom ceiling resolved once for the whole call rather than per
    /// sample. `f` receives each `(index, elevation)`; missing samples yield
    /// `None`. This is the loop that was burning ~655k miss-probes per rebuild.
    pub(crate) fn sample_grid<F: FnMut(usize, Option<f32>)>(
        &self,
        origin: (f64, f64),
        cell: f64,
        dim: usize,
        f: F,
    ) {
        self.sample_grid_rows(origin, cell, dim, 0, dim, f);
    }

    /// As [`Self::sample_grid`] but only rows `[row0, row1)` — lets the caller
    /// AMORTISE a big field across several frames (sample a chunk of rows per
    /// frame) so no single frame eats the whole 256² walk. `idx` is still the
    /// full-grid `j*dim + i`, so chunks write into one shared buffer.
    pub(crate) fn sample_grid_rows<F: FnMut(usize, Option<f32>)>(
        &self,
        origin: (f64, f64),
        cell: f64,
        dim: usize,
        row0: usize,
        row1: usize,
        mut f: F,
    ) {
        let max_z = self.finest_resident_zoom();
        for j in row0..row1 {
            let ay = origin.1 + j as f64 * cell;
            for i in 0..dim {
                let ax = origin.0 + i as f64 * cell;
                let e = self.sample_deepest_capped((ax, ay), max_z).map(|(_, e)| e);
                f(j * dim + i, e);
            }
        }
    }

    /// Elevation for **marker/overlay projection**, stabilised against DEM tile
    /// churn. Returns the finest resident sample, but remembers it per world
    /// cell and never regresses to a coarser zoom when the fine tile evicts (it
    /// only refines when a finer one arrives) — so anchored markers don't snap
    /// up/down as tiles stream in. Distinct from [`elevation_at_world`], which
    /// the shadow grid uses raw (and would otherwise flood this cache).
    pub(crate) fn elevation_at_world_stable(&self, world: (f64, f64)) -> Option<f32> {
        // ~1e-6 of the world span ≈ a few metres at these latitudes: fine enough
        // to separate distinct markers, coarse enough to reuse across frames.
        let cell = ((world.0 * 1_048_576.0) as i64, (world.1 * 1_048_576.0) as i64);
        let fresh = self.sample_deepest(world);
        let mut sticky = self.sticky_elev.lock().unwrap_or_else(|p| p.into_inner());
        match (fresh, sticky.get(&cell).copied()) {
            // A resident sample at least as fine as what we remember → trust + refine.
            (Some((z, e)), prev) if prev.is_none_or(|(pz, _)| z >= pz) => {
                sticky.insert(cell, (z, e));
                Some(e)
            }
            // Only a coarser sample is resident now (fine tile evicted) → hold the
            // last finer value instead of snapping.
            (_, Some((_, e))) => Some(e),
            // Coarser sample, nothing remembered yet → use it (better than floating).
            (Some((_, e)), None) => Some(e),
            (None, None) => None,
        }
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

/// Pure DEM-bytes → elevation-grid decode (no GPU), extracted from
/// `TerrainCache::decode_height_tile` so its bounds handling is unit-testable
/// against adversarial dimensions. Returns `None` on any input that can't
/// yield a valid interior grid — never panics, never indexes out of bounds.
fn decode_height_grid(
    rgba: &[u8],
    width: u32,
    height: u32,
    halo: u32,
    encoding: DemEncoding,
) -> Option<HeightTile> {
    // Length check in `usize` — `width * height * 4` in u32 overflows for a
    // malformed tile claiming huge dimensions (e.g. 65536² wraps to 0), which
    // would pass a naive u32 guard and then index past the buffer below. usize
    // math on 64-bit can't wrap here.
    let required = (width as usize)
        .checked_mul(height as usize)
        .and_then(|px| px.checked_mul(4))?;
    if width == 0 || height == 0 || rgba.len() < required {
        return None;
    }
    // Interior pixel span (exclusive upper bound).
    let lo = halo;
    let hi_x = width.saturating_sub(halo);
    let hi_y = height.saturating_sub(halo);
    if hi_x <= lo || hi_y <= lo {
        return None;
    }
    let span_x = (hi_x - lo) as f32;
    let span_y = (hi_y - lo) as f32;
    let n = CPU_HEIGHT_DIM;
    let mut grid = vec![0.0f32; n * n];
    for gy in 0..n {
        // Map grid cell centre → interior pixel.
        let py = lo + ((gy as f32 + 0.5) / n as f32 * span_y) as u32;
        let py = py.min(hi_y - 1);
        for gx in 0..n {
            let px = lo + ((gx as f32 + 0.5) / n as f32 * span_x) as u32;
            let px = px.min(hi_x - 1);
            // usize index to match the usize length guard — a u32 `py * width`
            // would overflow for a large (validated) width.
            let i = (py as usize * width as usize + px as usize) * 4;
            // Mapbox Terrain-RGB marks "no data" (sea / out of coverage) as
            // alpha 0; treat as sea level so markers and paths over water sit
            // at 0 m, not at a -10 km cliff.
            let elev = if rgba[i + 3] < 128 {
                0.0
            } else {
                decode_elevation(encoding, rgba[i], rgba[i + 1], rgba[i + 2])
            };
            grid[gy * n + gx] = elev;
        }
    }
    Some(HeightTile { grid })
}

#[cfg(test)]
mod tests {
    use super::{decode_height_grid, DemEncoding, HeightTile, CPU_HEIGHT_DIM};

    #[test]
    fn decode_height_grid_rejects_malformed_dimensions_without_panicking() {
        let enc = DemEncoding::MapboxRgb;
        // Huge dims whose u32 byte-count (w*h*4) overflows to a small number —
        // the bug a naive guard let through. Must be rejected, not indexed.
        assert!(decode_height_grid(&[0u8; 16], 65536, 65536, 0, enc).is_none());
        assert!(decode_height_grid(&[0u8; 16], u32::MAX, u32::MAX, 1, enc).is_none());
        // Buffer shorter than the claimed dimensions.
        assert!(decode_height_grid(&[0u8; 16], 256, 256, 0, enc).is_none());
        // Degenerate dims.
        assert!(decode_height_grid(&[0u8; 16], 0, 10, 0, enc).is_none());
        assert!(decode_height_grid(&[0u8; 16], 10, 0, 0, enc).is_none());
        // Halo larger than the tile → empty interior, rejected.
        assert!(decode_height_grid(&[0u8; 258 * 258 * 4], 8, 8, 64, enc).is_none());
        // A well-formed buffer decodes to a full grid.
        let ok = decode_height_grid(&[10u8; 258 * 258 * 4], 258, 258, 1, enc);
        assert!(ok.is_some());
        assert_eq!(ok.unwrap().grid.len(), CPU_HEIGHT_DIM * CPU_HEIGHT_DIM);
    }


    /// A grid that ramps 0 → (n-1) west-to-east, constant north-south.
    fn ramp_x() -> HeightTile {
        let n = CPU_HEIGHT_DIM;
        let mut grid = vec![0.0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                grid[y * n + x] = x as f32;
            }
        }
        HeightTile { grid }
    }

    #[test]
    fn height_sample_hits_corners_and_interpolates() {
        let ht = ramp_x();
        let n = (CPU_HEIGHT_DIM - 1) as f32;
        // Corners map to the exact grid values.
        assert!((ht.sample(0.0, 0.0) - 0.0).abs() < 1e-4);
        assert!((ht.sample(1.0, 0.0) - n).abs() < 1e-4);
        // Midpoint of the west→east ramp is the mean of the ends.
        assert!((ht.sample(0.5, 0.5) - n * 0.5).abs() < 0.6);
        // Out-of-range UV clamps rather than panicking.
        assert!((ht.sample(-1.0, 2.0) - 0.0).abs() < 1e-4);
        assert!((ht.sample(2.0, -1.0) - n).abs() < 1e-4);
    }
}
