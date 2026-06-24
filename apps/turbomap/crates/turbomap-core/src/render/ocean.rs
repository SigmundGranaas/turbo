//! The **Ocean Field** — the analytic, world-space description of the sea
//! surface, produced as a small set of mip-filtered cascade textures that the
//! water pipeline samples (vertex: horizontal displacement + height to displace
//! the grid; fragment: normal + foam). This is the spine of the realistic-water
//! system; see `docs/architecture/2026-06-aaa-water-architecture.md`.
//!
//! ## Why textures (not a procedural fragment function)
//! Every previous water attempt summed waves *per fragment* in the water shader.
//! That cannot be anti-aliased across map zooms — when the camera pulls back the
//! sub-pixel ripples alias into a tiling grid (the recurring failure). Here the
//! field is rendered into **textures with mip chains**, so distance sampling
//! averages down smoothly (hardware tri-/anisotropic filtering). That is the
//! actual fix for "alive at every zoom, no grid".
//!
//! ## How the field is built
//! A few **cascades** (fixed-size world patches, e.g. 457 m / 97 m / 23 m) each
//! hold a band of the spectrum. Per cascade we pick the most energetic samples
//! of a directional **Phillips spectrum** (Tessendorf) on the patch's wave-number
//! grid `k = 2π/L · n` — so every component is periodic in `L` and the texture
//! tiles seamlessly. Each frame a fullscreen pass evaluates the inverse transform
//! *directly* (sum of the selected cosines, Gerstner horizontal displacement,
//! Jacobian → foam) into the cascade texture; each **mip level is rendered
//! band-limited** to its Nyquist `k_max` (proper pre-filtering, no box-blur).
//!
//! Patch sizes are mutually incommensurate so the summed field has no visible
//! repeat within any on-screen extent.
//!
//! This is the v1 `OceanFieldSource` (render-pass spectral sum): robust, needs no
//! compute/storage-texture features, and runs on WebGL2. A compute FFT is a
//! drop-in optimisation behind the same texture interface (the water shader,
//! which only *samples* these textures, is unchanged).

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use super::HDR_FORMAT;

/// Per-cascade texture resolution (level 0).
pub(crate) const CASCADE_N: u32 = 256;
/// Number of cascades summed.
pub(crate) const CASCADES: usize = 3;
/// Significant spectral components summed per cascade.
const WAVES: usize = 64;
/// Mip levels: `log2(256) + 1`.
const MIPS: u32 = 9;
/// Patch size (metres) per cascade — large swell → fine chop. Mutually
/// incommensurate so the combined field doesn't visibly tile.
pub(crate) const PATCH_M: [f32; CASCADES] = [457.0, 97.0, 23.0];
/// 256-byte stride so consecutive uniform entries are dynamic-offset aligned.
const U_STRIDE: u64 = 256;

const G: f32 = 9.81;

/// One spectral component (storage-buffer element): wave-vector + amplitude +
/// phase. 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Wave {
    k: [f32; 2],
    amp: f32,
    phase: f32,
}

/// Per (cascade, mip) field-gen uniform, padded to [`U_STRIDE`].
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OceanUniform {
    time: f32,
    patch_l: f32,
    /// Nyquist cap for this mip level — skip waves with `|k| > k_max` so each
    /// level is pre-filtered (no aliasing into coarser mips).
    k_max: f32,
    choppiness: f32,
    wave_count: u32,
    _pad: [u32; 3],
    _tail: [f32; 56],
}

impl OceanUniform {
    fn new(time: f32, patch_l: f32, k_max: f32, choppiness: f32, wave_count: u32) -> Self {
        Self {
            time,
            patch_l,
            k_max,
            choppiness,
            wave_count,
            _pad: [0; 3],
            _tail: [0.0; 56],
        }
    }
}

pub(crate) struct OceanField {
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    /// One per cascade: the field texture (mipped) + a sampling view (all mips).
    cascade_tex: Vec<wgpu::Texture>,
    cascade_view: Vec<wgpu::TextureView>,
    /// Per cascade, per mip: a single-level render-target view.
    mip_views: Vec<Vec<wgpu::TextureView>>,
    /// Per (cascade × mip) uniform buffer (dynamic offset).
    uniform_buf: wgpu::Buffer,
    /// Per cascade: the wave storage buffer + its field-gen bind group.
    wave_buf: Vec<wgpu::Buffer>,
    bind_group: Vec<wgpu::BindGroup>,
    /// Linear+mip sampler the water shader uses to read the field.
    sampler: wgpu::Sampler,
    choppiness: f32,
}

impl OceanField {
    pub(crate) fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("turbomap-ocean-field-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ocean_field.wgsl").into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("turbomap-ocean-bgl"),
            entries: &[
                // OceanUniform (dynamic offset per cascade×mip).
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: std::num::NonZeroU64::new(U_STRIDE),
                    },
                    count: None,
                },
                // Wave components (read-only storage).
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("turbomap-ocean-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("turbomap-ocean-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Field textures: one per cascade, mip-chained, RGBA16F =
        // (disp.x, disp.y, height, foam).
        let mut cascade_tex = Vec::with_capacity(CASCADES);
        let mut cascade_view = Vec::with_capacity(CASCADES);
        let mut mip_views = Vec::with_capacity(CASCADES);
        for c in 0..CASCADES {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("turbomap-ocean-cascade"),
                size: wgpu::Extent3d {
                    width: CASCADE_N,
                    height: CASCADE_N,
                    depth_or_array_layers: 1,
                },
                mip_level_count: MIPS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            cascade_view.push(tex.create_view(&wgpu::TextureViewDescriptor::default()));
            let mut levels = Vec::with_capacity(MIPS as usize);
            for level in 0..MIPS {
                levels.push(tex.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("turbomap-ocean-mip"),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                }));
            }
            mip_views.push(levels);
            cascade_tex.push(tex);
            let _ = c;
        }

        // Dynamic uniform buffer: CASCADES × MIPS entries.
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("turbomap-ocean-uniform"),
            size: U_STRIDE * (CASCADES as u64) * (MIPS as u64),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Wave buffers + bind groups (one per cascade). Filled by set_sea_state.
        let mut wave_buf = Vec::with_capacity(CASCADES);
        let mut bind_group = Vec::with_capacity(CASCADES);
        for _ in 0..CASCADES {
            let wb = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("turbomap-ocean-waves"),
                size: (std::mem::size_of::<Wave>() * WAVES) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("turbomap-ocean-bg"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniform_buf,
                            offset: 0,
                            size: std::num::NonZeroU64::new(U_STRIDE),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wb.as_entire_binding(),
                    },
                ],
            });
            wave_buf.push(wb);
            bind_group.push(bg);
        }

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-ocean-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let mut field = Self {
            queue,
            pipeline,
            cascade_tex,
            cascade_view,
            mip_views,
            uniform_buf,
            wave_buf,
            bind_group,
            sampler,
            choppiness: 1.0,
        };
        // Default sea state: a moderate wind from the west.
        field.set_sea_state(8.0, 270.0, 1.5);
        field
    }

    /// Sampling views (all-mips) for the water pipeline's bind group — one per
    /// cascade, in scale order (large → small patch).
    pub(crate) fn cascade_views(&self) -> &[wgpu::TextureView] {
        &self.cascade_view
    }

    pub(crate) fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Recompute the spectral components from a sea state. `wind_speed_ms` sets
    /// the energy + peak wavelength, `wind_from_deg` the dominant direction,
    /// `amplitude_scale` the overall steepness (from forecast wave height).
    /// Cheap; call only when the forecast changes.
    pub(crate) fn set_sea_state(&mut self, wind_speed_ms: f32, wind_from_deg: f32, amplitude_scale: f32) {
        // Moderate choppiness: enough to pinch the steepest crests (→ occasional
        // whitecap foam via the Jacobian) without folding the whole surface into
        // froth. Scales gently with sea state.
        self.choppiness = (0.5 + amplitude_scale * 0.4).clamp(0.4, 1.8);
        let wind = wind_speed_ms.max(1.0);
        // Direction the wind blows TOWARD (compass `from` + 180°), world x=E y=S.
        let to = (wind_from_deg + 180.0).to_radians();
        let wdir = [-to.sin(), to.cos()];
        // Per-cascade RMS target (metres). Weighted toward the big-swell + medium
        // cascades so the sea reads as distinct rolling WAVES (visible crests/
        // troughs) rather than uniform fine grain; the finest cascade only adds a
        // little surface tooth on top.
        const CASCADE_GAIN: [f32; CASCADES] = [2.2, 1.7, 0.3];
        for c in 0..CASCADES {
            let target = (0.5 * amplitude_scale * CASCADE_GAIN[c]).clamp(0.05, 6.0);
            let waves = phillips_waves(wind, wdir, PATCH_M[c], target, c as u32);
            self.queue
                .write_buffer(&self.wave_buf[c], 0, bytemuck::cast_slice(&waves));
        }
    }

    /// Render the field for `time` (seconds): every cascade, every mip level,
    /// band-limited. Run once per frame before the water pass. All the uniform
    /// variants are written up-front (a single `write_buffer` per entry would
    /// race within one encoder), then bound by dynamic offset per pass.
    pub(crate) fn generate(&self, encoder: &mut wgpu::CommandEncoder, time: f32) {
        // Write every (cascade, mip) uniform once.
        for c in 0..CASCADES {
            let patch_l = PATCH_M[c];
            for level in 0..MIPS {
                let res = (CASCADE_N >> level).max(1) as f32;
                // Nyquist: a level of `res` texels over `patch_l` resolves
                // wavelengths ≥ 2·patch_l/res, i.e. |k| ≤ π·res/patch_l.
                let k_max = std::f32::consts::PI * res / patch_l;
                let u = OceanUniform::new(time, patch_l, k_max, self.choppiness, WAVES as u32);
                let idx = (c as u64) * (MIPS as u64) + (level as u64);
                self.queue
                    .write_buffer(&self.uniform_buf, idx * U_STRIDE, bytemuck::bytes_of(&u));
            }
        }
        for c in 0..CASCADES {
            for level in 0..MIPS as usize {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("turbomap-ocean-gen"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.mip_views[c][level],
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.pipeline);
                let offset = ((c as u64) * (MIPS as u64) + level as u64) * U_STRIDE;
                pass.set_bind_group(0, &self.bind_group[c], &[offset as u32]);
                pass.draw(0..3, 0..1);
            }
        }
    }
}

/// Build the `WAVES` most energetic directional-Phillips components for a cascade
/// of patch `l`, on its wave-number grid `k = 2π/l · n` (so each is periodic in
/// `l` → seamless tiling). Band-limited to this cascade's wavelength window so
/// cascades don't double-count mid frequencies. Deterministic phases (stable
/// across frames + runs).
fn phillips_waves(wind: f32, wdir: [f32; 2], l: f32, target_rms: f32, cascade: u32) -> Vec<Wave> {
    let two_pi_l = std::f32::consts::TAU / l;
    let big_l = wind * wind / G; // largest wind-driven wavelength scale
    // Suppress waves much smaller than this (Tessendorf's small-wave cutoff).
    let small = l * 0.5e-2;
    // This cascade carries wavelengths in (l/64, l]; the next finer cascade
    // picks up below l/64. Keeps the bands roughly disjoint.
    let wl_hi = l;
    let wl_lo = l / 64.0;
    let half = (CASCADE_N / 2) as i32;

    let mut cand: Vec<(f32, Wave)> = Vec::new();
    for ny in -half..half {
        for nx in -half..half {
            if nx == 0 && ny == 0 {
                continue;
            }
            let k = [nx as f32 * two_pi_l, ny as f32 * two_pi_l];
            let kmag = (k[0] * k[0] + k[1] * k[1]).sqrt();
            let wl = std::f32::consts::TAU / kmag;
            if wl > wl_hi || wl < wl_lo {
                continue;
            }
            let kh = [k[0] / kmag, k[1] / kmag];
            let kdotw = kh[0] * wdir[0] + kh[1] * wdir[1];
            // Directional Phillips: energy ∝ exp(-1/(kL)²)/k⁴ · (k̂·ŵ)².
            let mut ph = (-1.0 / (kmag * big_l).powi(2)).exp() / kmag.powi(4) * kdotw * kdotw;
            ph *= (-(kmag * small).powi(2)).exp();
            if kdotw < 0.0 {
                ph *= 0.18; // damp waves travelling against the wind
            }
            if !ph.is_finite() || ph <= 0.0 {
                continue;
            }
            // Amplitude of this component ≈ sqrt(spectrum · dk).
            let amp = (ph * two_pi_l * two_pi_l).sqrt();
            let phase = hash_phase(nx, ny, cascade);
            cand.push((amp, Wave { k, amp, phase }));
        }
    }
    // Keep the WAVES most energetic.
    cand.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    cand.truncate(WAVES);
    let mut waves: Vec<Wave> = cand.into_iter().map(|(_, w)| w).collect();
    while waves.len() < WAVES {
        waves.push(Wave { k: [0.0, 0.0], amp: 0.0, phase: 0.0 }); // pad (amp 0 = no-op)
    }
    // Normalise overall height to a sane metric scale (tuned), scaled by the
    // forecast amplitude. RMS of the sum ≈ sqrt(Σ amp²/2).
    let rms = (waves.iter().map(|w| w.amp * w.amp).sum::<f32>() * 0.5).sqrt().max(1e-6);
    let gain = target_rms / rms;
    for w in &mut waves {
        w.amp *= gain;
    }
    waves
}

/// Deterministic phase in [0, 2π) for a grid cell — a cheap integer hash so the
/// sea is identical across frames and runs (no per-frame randomness).
fn hash_phase(nx: i32, ny: i32, cascade: u32) -> f32 {
    let mut h = (nx as u32).wrapping_mul(0x9E3779B1);
    h ^= (ny as u32).wrapping_mul(0x85EBCA77);
    h ^= cascade.wrapping_mul(0xC2B2AE3D);
    h = h.wrapping_mul(0x27D4EB2F);
    h ^= h >> 15;
    (h as f32 / u32::MAX as f32) * std::f32::consts::TAU
}
