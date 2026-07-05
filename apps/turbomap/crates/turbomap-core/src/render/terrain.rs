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
//! The cache stores decoded ELEVATIONS as `Rg16Float` textures — `.r` is
//! metres, `.g` the source's coverage mask (1 = data, 0 = "no data"; the
//! hillshade overlay keys transparency-over-water off it, exactly as it
//! keyed off the alpha channel before). Ingest hands us real heights (the
//! RGB→metres decode is the codec's job, [`crate::dem::decode_dem_rgba`],
//! run off the render thread), and the shaders sample `.r` directly (plan
//! slice D3 — no per-vertex decode, no encoding uniform). Tiles are
//! typically 258×258: halo is required, since without it vertex
//! displacement at adjacent tile edges samples different DEM cells and the
//! mesh cracks.

use std::collections::HashMap;
use std::sync::Arc;

use crate::dem::DecodedDem;
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
}

// `Terrain`'s ground queries live behind the `Surface` trait — the
// `HeightfieldSurface` impl in `crate::surface` (plan slice D3).

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
        let layout = Arc::new(
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            }),
        );
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
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // 0 m elevation, full coverage — heights are stored decoded, so the
        // flat placeholder is literally (0 m, covered).
        let zero_covered: [half::f16; 2] = [half::f16::ZERO, half::f16::ONE];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &placeholder_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&zero_covered),
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
    ) -> Self {
        let cache = TextureCache::new(
            device,
            queue,
            shared.bind_group_layout.clone(),
            shared.sampler.clone(),
            budget_bytes,
            // Decoded (metres, coverage) as filterable half floats — the
            // shaders sample `.r` directly. (f16 is exact to 1 m up to
            // 2048 m and 2 m up to 4096 m: within the DEM sources' own
            // error, and float filtering of real heights is at least as
            // correct as the old filtered-then-decoded Terrain-RGB bytes.)
            wgpu::TextureFormat::Rg16Float,
            // No mipmaps. Vertex displacement needs the base-level
            // height; mipping would smooth peaks at distance.
            false,
        );
        Self {
            cache,
            bind_group_layout: shared.bind_group_layout.clone(),
            sampler: shared.sampler.clone(),
            halo_px,
            heights: HashMap::new(),
            sticky_elev: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn halo_px(&self) -> u32 {
        self.halo_px
    }

    /// Ingest a DECODED DEM tile (the codec already ran — see
    /// [`crate::dem::decode_dem_rgba`]). Uploads `(metres, coverage)` as an
    /// `Rg16Float` texture and keeps the CPU grid in lock-step. Returns the
    /// ids evicted to stay within budget, so the caller drops them from the
    /// terrain scene's "ingested" set and they get re-requested when next
    /// desired.
    pub(crate) fn ingest(&mut self, tile: TileId, dem: &DecodedDem) -> Vec<TileId> {
        let (width, height) = (dem.width, dem.height);
        let px = (width as usize).saturating_mul(height as usize);
        if width == 0 || height == 0 || dem.heights_m.len() < px || dem.coverage.len() < px {
            return Vec::new();
        }
        let mut texels: Vec<half::f16> = Vec::with_capacity(px * 2);
        for i in 0..px {
            texels.push(half::f16::from_f32(dem.heights_m[i]));
            texels.push(half::f16::from_f32(dem.coverage[i] as f32));
        }
        let evicted = self
            .cache
            .insert(tile, bytemuck::cast_slice(&texels), width, height);
        // Keep a CPU-side elevation grid in lock-step with the GPU cache.
        if let Some(ht) = height_grid_from_heights(&dem.heights_m, width, height, self.halo_px) {
            self.heights.insert(tile, ht);
        }
        for e in &evicted {
            self.heights.remove(e);
        }
        evicted
    }

    /// Age of the cached tile in seconds, or `None` if not present.
    /// Mirrors `TextureCache::age_secs` so the hillshade pipeline can
    /// compute per-tile fade-in directly off the shared cache.
    pub(crate) fn age_secs(&self, id: TileId) -> Option<f32> {
        self.cache.age_secs(id)
    }

    /// Touch + return the cache entry for `id`. Bumps the LRU.
    pub(crate) fn get_entry(&mut self, id: TileId) -> Option<&crate::render::cache::CacheEntry> {
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
    /// then treats it as flat (z=0), same as the 2D map. The raw
    /// (non-sticky) point query — `Surface::normal_at` differentiates it.
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

    /// Bulk elevation sampler for the cast-shadow / AO heightfield
    /// (`Surface::sample_height_rows` answers from this): the same "deepest
    /// resident wins" lookup as [`elevation_at_world`], but with the
    /// finest-zoom ceiling resolved once for the whole call rather than per
    /// sample (the naive loop burnt ~655k miss-probes per rebuild). Rows
    /// `[row0, row1)` only, so the caller can AMORTISE a big field across
    /// several frames; `idx` is still the full-grid `j*dim + i`, so chunks
    /// write into one shared buffer.
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
        let cell = (
            (world.0 * 1_048_576.0) as i64,
            (world.1 * 1_048_576.0) as i64,
        );
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

/// Downsample a full-resolution decoded height grid (metres, halo included)
/// into the halo-trimmed CPU `HeightTile`. `width`/`height` include the halo
/// ring (e.g. 258 for a 256-tile with 1 px halo); we sample the geographic
/// interior only, so adjacent tiles' grids line up at their shared edge.
/// Returns `None` on any input that can't yield a valid interior grid —
/// never panics, never indexes out of bounds.
fn height_grid_from_heights(
    heights_m: &[f32],
    width: u32,
    height: u32,
    halo: u32,
) -> Option<HeightTile> {
    // Length check in `usize` — `width * height` in u32 overflows for a
    // malformed tile claiming huge dimensions (e.g. 65536² wraps to 0), which
    // would pass a naive u32 guard and then index past the buffer below. usize
    // math on 64-bit can't wrap here.
    let required = (width as usize).checked_mul(height as usize)?;
    if width == 0 || height == 0 || heights_m.len() < required {
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
            grid[gy * n + gx] = heights_m[py as usize * width as usize + px as usize];
        }
    }
    Some(HeightTile { grid })
}

#[cfg(test)]
mod tests {
    use super::{height_grid_from_heights, HeightTile, CPU_HEIGHT_DIM};

    #[test]
    fn height_grid_rejects_malformed_dimensions_without_panicking() {
        // Huge dims whose u32 count (w*h) overflows to a small number —
        // the bug a naive guard let through. Must be rejected, not indexed.
        assert!(height_grid_from_heights(&[0.0; 16], 65536, 65536, 0).is_none());
        assert!(height_grid_from_heights(&[0.0; 16], u32::MAX, u32::MAX, 1).is_none());
        // Buffer shorter than the claimed dimensions.
        assert!(height_grid_from_heights(&[0.0; 16], 256, 256, 0).is_none());
        // Degenerate dims.
        assert!(height_grid_from_heights(&[0.0; 16], 0, 10, 0).is_none());
        assert!(height_grid_from_heights(&[0.0; 16], 10, 0, 0).is_none());
        // Halo larger than the tile → empty interior, rejected.
        assert!(height_grid_from_heights(&[0.0; 258 * 258], 8, 8, 64).is_none());
        // A well-formed buffer downsamples to a full grid.
        let ok = height_grid_from_heights(&[123.0; 258 * 258], 258, 258, 1);
        assert!(ok.is_some());
        let grid = ok.unwrap().grid;
        assert_eq!(grid.len(), CPU_HEIGHT_DIM * CPU_HEIGHT_DIM);
        assert!(grid.iter().all(|&h| h == 123.0));
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
