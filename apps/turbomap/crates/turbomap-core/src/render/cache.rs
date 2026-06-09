//! GPU texture cache for decoded raster tiles. Bounded LRU.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use crate::tile::TileId;

pub(crate) struct CacheEntry {
    // `bind_group` keeps the underlying texture+view alive through wgpu's
    // internal Arcs, so we don't need to hold them separately here.
    pub bind_group: wgpu::BindGroup,
    pub bytes: usize,
    pub created_at: Instant,
}

pub(crate) struct TextureCache {
    entries: HashMap<TileId, CacheEntry>,
    lru: VecDeque<TileId>,
    bytes_used: usize,
    budget_bytes: usize,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bind_group_layout: Arc<wgpu::BindGroupLayout>,
    sampler: Arc<wgpu::Sampler>,
    /// Texture format used for every entry. Raster basemaps want sRGB
    /// (so colours decode for display); DEM/Terrain-RGB and anything
    /// where the bytes encode *data*, not colour, MUST use the linear
    /// `Rgba8Unorm` variant — otherwise the GPU applies an sRGB→linear
    /// curve and warps the byte values before the shader sees them.
    format: wgpu::TextureFormat,
    /// Generate a full mip chain on upload? `true` for raster basemaps
    /// (linear minification at zoom-out kills shimmer); `false` for
    /// hillshade DEM — the fragment shader's gradient kernel uses a
    /// fixed 1-texel step against the base level, so dropping LOD
    /// would mismatch the kernel scale.
    gen_mips: bool,
    /// Stat counters. Bumped on every `get()` and `insert()` so
    /// `Map::last_frame_metrics()` can surface cache effectiveness
    /// without instrumenting every call site.
    stat_hits: u64,
    stat_misses: u64,
    stat_inserts: u64,
    stat_evictions: u64,
}

impl TextureCache {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        bind_group_layout: Arc<wgpu::BindGroupLayout>,
        sampler: Arc<wgpu::Sampler>,
        budget_bytes: usize,
        format: wgpu::TextureFormat,
        gen_mips: bool,
    ) -> Self {
        Self {
            entries: HashMap::new(),
            lru: VecDeque::new(),
            bytes_used: 0,
            budget_bytes,
            device,
            queue,
            bind_group_layout,
            sampler,
            format,
            gen_mips,
            stat_hits: 0,
            stat_misses: 0,
            stat_inserts: 0,
            stat_evictions: 0,
        }
    }

    pub(crate) fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.entries.len(),
            bytes_used: self.bytes_used,
            budget_bytes: self.budget_bytes,
            hits: self.stat_hits,
            misses: self.stat_misses,
            inserts: self.stat_inserts,
            evictions: self.stat_evictions,
        }
    }

    /// Seconds since `id` was inserted, or `None` if not cached. Read-only —
    /// does *not* bump the LRU.
    pub(crate) fn age_secs(&self, id: TileId) -> Option<f32> {
        self.entries
            .get(&id)
            .map(|e| Instant::now().duration_since(e.created_at).as_secs_f32())
    }

    pub(crate) fn get(&mut self, id: TileId) -> Option<&CacheEntry> {
        if self.entries.contains_key(&id) {
            self.touch(id);
            self.stat_hits += 1;
            self.entries.get(&id)
        } else {
            self.stat_misses += 1;
            None
        }
    }

    /// Read-only lookup — does *not* bump the LRU or the hit/miss
    /// counters. Used at draw time inside a render pass, where every
    /// referenced tile was already touched by the prepare phase.
    pub(crate) fn peek(&self, id: TileId) -> Option<&CacheEntry> {
        self.entries.get(&id)
    }

    /// Walk up the pyramid looking for the nearest ancestor in the cache.
    pub(crate) fn nearest_ancestor(&mut self, id: TileId) -> Option<TileId> {
        for k in 1..=id.z {
            let ancestor = id.ancestor(k)?;
            if self.entries.contains_key(&ancestor) {
                self.touch(ancestor);
                return Some(ancestor);
            }
        }
        None
    }

    /// Cached tiles that lie *inside* `region` (i.e. its descendants at
    /// deeper zoom). Used as a backdrop when the requested tile and its
    /// ancestors are both absent — typically right after a zoom-out.
    ///
    /// The returned order is FULLY deterministic — `(z, x, y)` — even
    /// though the underlying `entries` is a `HashMap` with
    /// randomised iteration. A previous `sort_by_key(|t| t.z)` was
    /// only stable BETWEEN z-levels; tiles at the same z were still
    /// emitted in HashMap order, which produced visibly different
    /// frame-to-frame draw orderings of overlapping descendants and
    /// the user reported the map "fighting about who is on top". A
    /// full `(z, x, y)` sort eliminates that source of frame-to-
    /// frame non-determinism.
    pub(crate) fn covered_descendants(&self, region: TileId, max_levels_deep: u8) -> Vec<TileId> {
        let mut out: Vec<TileId> = self
            .entries
            .keys()
            .copied()
            .filter(|t| {
                if t.z <= region.z || t.z - region.z > max_levels_deep {
                    return false;
                }
                t.ancestor(t.z - region.z) == Some(region)
            })
            .collect();
        out.sort_by(|a, b| (a.z, a.x, a.y).cmp(&(b.z, b.x, b.y)));
        out
    }

    pub(crate) fn insert(&mut self, id: TileId, rgba: &[u8], width: u32, height: u32) {
        if self.entries.contains_key(&id) {
            self.touch(id);
            return;
        }
        let chain = if self.gen_mips {
            build_mip_chain(rgba, width, height, self.format)
        } else {
            vec![rgba.to_vec()]
        };
        let mip_count = chain.len() as u32;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("turbomap-tile-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut bytes = 0usize;
        for (level, data) in chain.iter().enumerate() {
            let lw = (width >> level).max(1);
            let lh = (height >> level).max(1);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * lw),
                    rows_per_image: Some(lh),
                },
                wgpu::Extent3d {
                    width: lw,
                    height: lh,
                    depth_or_array_layers: 1,
                },
            );
            bytes += data.len();
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-tile-bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        // texture + view are kept alive via the bind group's internal Arcs.
        let _ = texture;
        let _ = view;
        self.entries.insert(
            id,
            CacheEntry {
                bind_group,
                bytes,
                created_at: Instant::now(),
            },
        );
        self.lru.push_back(id);
        self.bytes_used += bytes;
        self.stat_inserts += 1;
        self.evict_to_budget();
    }

    fn touch(&mut self, id: TileId) {
        if let Some(pos) = self.lru.iter().position(|&t| t == id) {
            self.lru.remove(pos);
            self.lru.push_back(id);
        }
    }

    fn evict_to_budget(&mut self) {
        while self.bytes_used > self.budget_bytes && self.lru.len() > 1 {
            let Some(victim) = self.lru.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.bytes_used = self.bytes_used.saturating_sub(entry.bytes);
                self.stat_evictions += 1;
            }
        }
    }
}

/// Snapshot of cache state surfaced via `Map::last_frame_metrics`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub entries: usize,
    pub bytes_used: usize,
    pub budget_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
}

/// Build a complete 2×2 box-filter mip chain. For sRGB formats the
/// averaging is performed in linear light (decode → average → re-encode)
/// so a chequerboard of pure black and pure white correctly minifies
/// to sRGB mid-grey ~ 188, not 128. For non-sRGB (Rgba8Unorm) the
/// bytes are averaged directly — the format implies the consumer is
/// already operating in linear/data space.
///
/// Stops at min(w, h) == 1.
fn build_mip_chain(rgba: &[u8], w: u32, h: u32, format: wgpu::TextureFormat) -> Vec<Vec<u8>> {
    let srgb = matches!(format, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut levels: Vec<Vec<u8>> = Vec::new();
    levels.push(rgba.to_vec());
    let mut prev_w = w;
    let mut prev_h = h;
    while prev_w > 1 && prev_h > 1 {
        let nw = (prev_w / 2).max(1);
        let nh = (prev_h / 2).max(1);
        let prev = levels.last().expect("at least base level");
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let mut sum = [0.0f32; 4];
                for dy in 0..2 {
                    for dx in 0..2 {
                        let px = (x * 2 + dx).min(prev_w - 1);
                        let py = (y * 2 + dy).min(prev_h - 1);
                        let i = ((py * prev_w + px) * 4) as usize;
                        for c in 0..4 {
                            let b = prev[i + c];
                            let v = if srgb && c < 3 {
                                srgb_to_linear(b)
                            } else {
                                b as f32 / 255.0
                            };
                            sum[c] += v;
                        }
                    }
                }
                let out_i = ((y * nw + x) * 4) as usize;
                for c in 0..4 {
                    let avg = sum[c] / 4.0;
                    let byte = if srgb && c < 3 {
                        linear_to_srgb(avg)
                    } else {
                        (avg * 255.0).round().clamp(0.0, 255.0) as u8
                    };
                    next[out_i + c] = byte;
                }
            }
        }
        levels.push(next);
        prev_w = nw;
        prev_h = nh;
    }
    levels
}

fn srgb_to_linear(b: u8) -> f32 {
    let s = b as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let s = if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_chain_for_256_has_nine_levels() {
        // 256 → 128 → 64 → 32 → 16 → 8 → 4 → 2 → 1 = 9 levels.
        let rgba = vec![128u8; 256 * 256 * 4];
        let chain = build_mip_chain(&rgba, 256, 256, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(chain.len(), 9);
        assert_eq!(chain[0].len(), 256 * 256 * 4);
        assert_eq!(chain[1].len(), 128 * 128 * 4);
        assert_eq!(chain.last().unwrap().len(), 4);
    }

    #[test]
    fn srgb_mip_of_black_and_white_is_perceptual_grey() {
        // 2×2 chequerboard of pure black / pure white in sRGB. Naïve
        // byte averaging would yield 128; gamma-correct averaging
        // yields linear 0.5 → sRGB byte ~188. Verifies the chain
        // builder isn't silently darkening mipmapped basemaps.
        let mut rgba = vec![0u8; 4 * 4];
        rgba[..4].copy_from_slice(&[255, 255, 255, 255]); // top-left white
        rgba[4..8].copy_from_slice(&[0, 0, 0, 255]); // top-right black
        rgba[8..12].copy_from_slice(&[0, 0, 0, 255]); // bottom-left black
        rgba[12..16].copy_from_slice(&[255, 255, 255, 255]); // bottom-right white
        let chain = build_mip_chain(&rgba, 2, 2, wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(chain.len(), 2);
        let mip1 = &chain[1];
        assert_eq!(mip1.len(), 4);
        for (c, &v) in mip1.iter().take(3).enumerate() {
            assert!(
                (180..=195).contains(&v),
                "channel {c}: got {v}, expected ~188 (sRGB(linear 0.5))"
            );
        }
        assert_eq!(mip1[3], 255, "alpha should remain fully opaque");
    }

    #[test]
    fn linear_mip_of_black_and_white_is_byte_average() {
        // Same chequerboard but in linear Rgba8Unorm. Byte average
        // is the correct answer here (no gamma curve).
        let mut rgba = vec![0u8; 4 * 4];
        rgba[..4].copy_from_slice(&[255, 255, 255, 255]);
        rgba[4..8].copy_from_slice(&[0, 0, 0, 255]);
        rgba[8..12].copy_from_slice(&[0, 0, 0, 255]);
        rgba[12..16].copy_from_slice(&[255, 255, 255, 255]);
        let chain = build_mip_chain(&rgba, 2, 2, wgpu::TextureFormat::Rgba8Unorm);
        let mip1 = &chain[1];
        for (c, &v) in mip1.iter().take(3).enumerate() {
            assert!(
                (125..=130).contains(&v),
                "channel {c}: got {v}, expected ~128 (raw average)"
            );
        }
    }
}
