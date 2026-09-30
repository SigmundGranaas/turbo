//! GPU texture cache for decoded raster tiles. Bounded LRU.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use web_time::Instant;

use crate::tile::TileId;

pub(crate) struct CacheEntry {
    // `bind_group` keeps the underlying texture+view alive through wgpu's
    // internal Arcs, so we don't need to hold them separately here.
    pub bind_group: wgpu::BindGroup,
    pub bytes: usize,
    pub created_at: Instant,
    /// The tile's texture upload (every mip level; tickets are handed over
    /// in order, so the last level's covers them all). Until it is ready the
    /// entry is held but not drawable — see [`TextureCache::peek`].
    pub uploaded: crate::upload::UploadTicket,
}

pub(crate) struct TextureCache {
    entries: HashMap<TileId, CacheEntry>,
    lru: VecDeque<TileId>,
    bytes_used: usize,
    budget_bytes: usize,
    device: Arc<wgpu::Device>,
    queue: crate::upload::UploadQueue,
    bind_group_layout: Arc<wgpu::BindGroupLayout>,
    sampler: Arc<wgpu::Sampler>,
    /// Texture format used for every entry. Raster basemaps want sRGB
    /// (so colours decode for display); overlays that carry *data*, not
    /// colour, use a linear format — `Rgba8Unorm` for image data,
    /// `Rg16Float` for decoded DEM heights (metres + coverage).
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
        queue: crate::upload::UploadQueue,
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
    ///
    /// `None` too for a tile whose upload has not been handed to the host:
    /// callers pick drawable tiles by this (hillshade's `prepare` does), and
    /// it must agree with [`Self::peek`], which the draw then relies on.
    pub(crate) fn age_secs(&self, id: TileId) -> Option<f32> {
        self.peek(id)
            .map(|e| Instant::now().duration_since(e.created_at).as_secs_f32())
    }

    /// Whether `id`'s texture has been handed to the host (see
    /// `crate::upload`): an entry is only drawable once it has.
    fn drawable(&self, id: TileId) -> bool {
        self.entries
            .get(&id)
            .is_some_and(|e| self.queue.is_ready(e.uploaded))
    }

    pub(crate) fn get(&mut self, id: TileId) -> Option<&CacheEntry> {
        if self.drawable(id) {
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
    ///
    /// A tile whose upload has not been handed to the host yet is absent
    /// here, so the draw falls back to its ancestor exactly as for a tile
    /// still in flight — never an uninitialised texture.
    pub(crate) fn peek(&self, id: TileId) -> Option<&CacheEntry> {
        self.entries
            .get(&id)
            .filter(|e| self.queue.is_ready(e.uploaded))
    }

    /// Walk up the pyramid looking for the nearest ancestor in the cache.
    pub(crate) fn nearest_ancestor(&mut self, id: TileId) -> Option<TileId> {
        for k in 1..=id.z {
            let ancestor = id.ancestor(k)?;
            if self.drawable(ancestor) {
                self.touch(ancestor);
                return Some(ancestor);
            }
        }
        None
    }

    /// Insert a decoded tile, evicting LRU tiles if the budget is
    /// exceeded. Returns the ids that were evicted so the caller can
    /// drop them from its "ingested" bookkeeping — otherwise the scene
    /// would believe an evicted tile is still resident and never
    /// re-request it (the "grey tile that won't reload" bug).
    ///
    /// `texels` is raw texel data in the cache's format — RGBA bytes for
    /// the 8-bit colour formats, packed f16 `(height, coverage)` pairs for
    /// `Rg16Float` DEM tiles (the mip-chain builder is RGBA-only, so
    /// `gen_mips` requires an RGBA format).
    pub(crate) fn insert(
        &mut self,
        id: TileId,
        texels: &[u8],
        width: u32,
        height: u32,
    ) -> Vec<TileId> {
        if self.entries.contains_key(&id) {
            self.touch(id);
            return Vec::new();
        }
        let texel_bytes = bytes_per_texel(self.format);
        let chain = if self.gen_mips {
            debug_assert!(
                matches!(
                    self.format,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb
                ),
                "mip chain builder is RGBA-only"
            );
            build_mip_chain(texels, width, height, self.format)
        } else {
            vec![texels.to_vec()]
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
        let mut uploaded = None;
        for (level, data) in chain.iter().enumerate() {
            let lw = (width >> level).max(1);
            let lh = (height >> level).max(1);
            uploaded = Some(self.queue.write_texture_deferrable(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(texel_bytes * lw),
                    rows_per_image: Some(lh),
                },
                wgpu::Extent3d {
                    width: lw,
                    height: lh,
                    depth_or_array_layers: 1,
                },
            ));
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
                uploaded: uploaded.expect("a mip chain has at least its base level"),
            },
        );
        self.lru.push_back(id);
        self.bytes_used += bytes;
        self.stat_inserts += 1;
        self.evict_to_budget()
    }

    fn touch(&mut self, id: TileId) {
        if let Some(pos) = self.lru.iter().position(|&t| t == id) {
            self.lru.remove(pos);
            self.lru.push_back(id);
        }
    }

    fn evict_to_budget(&mut self) -> Vec<TileId> {
        let mut evicted = Vec::new();
        while self.bytes_used > self.budget_bytes && self.lru.len() > 1 {
            let Some(victim) = self.lru.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&victim) {
                self.bytes_used = self.bytes_used.saturating_sub(entry.bytes);
                self.stat_evictions += 1;
                evicted.push(victim);
            }
        }
        evicted
    }
}

/// Bytes per texel for the formats this cache stores. New formats must be
/// added here explicitly — a silent wrong stride would corrupt every upload.
fn bytes_per_texel(format: wgpu::TextureFormat) -> u32 {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4,
        // DEM heights: (elevation_m, coverage) as half floats.
        wgpu::TextureFormat::Rg16Float => 4,
        other => unreachable!("TextureCache: unsupported format {other:?}"),
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

    /// The draw-side half of the upload contract (`crate::upload`): a tile
    /// whose texture upload has not been handed to the host is held but not
    /// drawable — `peek`, `get`, `age_secs` and `nearest_ancestor` all miss —
    /// until a flush with room hands it over. Over budget, it stays unready.
    #[test]
    fn a_tile_is_drawable_only_once_its_upload_has_been_handed_over() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                force_fallback_adapter: true,
                ..Default::default()
            }))
        else {
            eprintln!("no software adapter: covered by the budgeted goldens instead");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        let uploads = crate::upload::UploadQueue::new(1.0);
        let mut cache = TextureCache::new(
            Arc::new(device),
            uploads.clone(),
            Arc::new(layout),
            Arc::new(sampler),
            64 << 20,
            wgpu::TextureFormat::Rgba8Unorm,
            false,
        );
        let parent = TileId::new(3, 1, 1);
        let child = TileId::new(4, 2, 2);
        cache.insert(parent, &vec![200u8; 16 * 16 * 4], 16, 16);
        cache.insert(child, &vec![100u8; 16 * 16 * 4], 16, 16);
        assert!(
            cache.peek(child).is_none() && cache.get(child).is_none(),
            "drawable before its upload was handed over"
        );
        assert!(cache.age_secs(child).is_none());
        assert_eq!(
            cache.nearest_ancestor(child),
            None,
            "the parent is not drawable either yet"
        );

        // A budget with room for only the parent (1 KiB each).
        struct Tight<'a>(&'a wgpu::Queue, u64);
        impl crate::upload::Uploader for Tight<'_> {
            fn remaining(&self) -> Option<u64> {
                Some(self.1)
            }
            fn write_buffer(
                &mut self,
                _: &wgpu::Buffer,
                _: u64,
                _: &[u8],
            ) -> Result<(), crate::upload::Refused> {
                Err(crate::upload::Refused)
            }
            fn write_texture(
                &mut self,
                dst: wgpu::TexelCopyTextureInfo<'_>,
                data: &[u8],
                layout: wgpu::TexelCopyBufferLayout,
                size: wgpu::Extent3d,
            ) -> Result<(), crate::upload::Refused> {
                self.1 = self
                    .1
                    .checked_sub(data.len() as u64)
                    .ok_or(crate::upload::Refused)?;
                self.0.write_texture(dst, data, layout, size);
                Ok(())
            }
        }
        uploads.flush_deferrable(&mut Tight(&queue, 1024));
        assert!(cache.peek(parent).is_some(), "the parent's upload fitted");
        assert!(
            cache.peek(child).is_none(),
            "the child's did not: still not drawable"
        );
        assert_eq!(
            cache.nearest_ancestor(child),
            Some(parent),
            "so the draw falls back to the parent"
        );

        uploads.flush_deferrable(&mut Tight(&queue, 1024));
        assert!(
            cache.peek(child).is_some(),
            "handed over on the next frame, and drawable now"
        );
    }

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
