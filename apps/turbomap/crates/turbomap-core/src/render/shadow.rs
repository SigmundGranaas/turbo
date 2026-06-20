//! Terrain cast shadows via a CPU horizon-march.
//!
//! The Lambertian `N·L` term in `shader.wgsl` is *self*-shading: a slope
//! facing away from the sun darkens. It carries no *occlusion* — a peak does
//! not throw a shadow across the valley behind it, because each terrain draw
//! only binds its own DEM tile (`@group(2)`) and can't see the neighbouring
//! tile that holds the shadow-caster.
//!
//! Cast shadows therefore need a *frame-global* view of the heightfield. We
//! assemble the visible relief into one square grid (sampled from the shared
//! [`TerrainCache`](super::terrain::TerrainCache), which already spans tiles),
//! march each cell toward the sun, and record a per-cell *visibility* in
//! `[0,1]` (1 = lit, 0 = fully shadowed, fractional = penumbra). The renderer
//! uploads this as a single texture the terrain fragment shader samples by
//! world-xy and folds into the *direct* light term — skylight/ambient still
//! reaches shadowed ground, so it darkens rather than going black.
//!
//! Doing the march on the CPU (not a per-fragment shader ray-march) is a
//! deliberate device-safety call: it keeps the mobile GPU off a long,
//! divergent loop that risks the same driver hangs the NaN gate guards
//! against, and it makes the result deterministic and unit-testable without a
//! device. The field is recomputed only when the sun, the covered region, or
//! the resident DEM changes — not every frame — so its cost is decoupled from
//! the frame rate.

use std::sync::Arc;

/// Per-side resolution of the shadow grid. 192² = 36 864 cells. Fine enough
/// that a cell is a few metres at hiking zooms yet cheap to march on the CPU
/// (the march is `O(dim² · MAX_MARCH_CELLS)`, recomputed only on change).
pub(crate) const SHADOW_DIM: usize = 192;

/// How many cells along the sun ray a cell looks for an occluder before giving
/// up and calling itself lit. Bounds the march cost and reflects that a caster
/// far enough away rarely shadows within a single screen at usable sun
/// altitudes. `√2 · dim` would be full-diagonal coverage; this is a deliberate
/// cap well below that.
const MAX_MARCH_CELLS: usize = 128;

/// A computed terrain shadow grid: square, axis-aligned in world-xy, holding a
/// visibility value per cell. The renderer turns `visibility` into a texture
/// and uses `origin`/`world_size` to map a fragment's world-xy into `[0,1]` UV.
#[derive(Debug, Clone)]
pub(crate) struct ShadowField {
    /// Cells per side. `visibility.len() == dim * dim`, row-major (y-major).
    pub(crate) dim: usize,
    /// World-xy of the *centre of cell (0,0)*. With `world_size` this defines
    /// the affine map world-xy → grid coordinates the shader inverts.
    pub(crate) origin: [f32; 2],
    /// World-xy extent the grid spans, edge to edge (cell-centre 0 to
    /// cell-centre `dim-1` is `world_size`). Square.
    pub(crate) world_size: f32,
    /// Per-cell visibility in `[0,1]`: 1 = fully lit, 0 = fully shadowed.
    pub(crate) visibility: Vec<f32>,
}

impl ShadowField {
    /// A field that shadows nothing (every cell lit) — the resident value when
    /// shadows are disabled or the sun is at/below the horizon. Keeps the
    /// renderer's binding non-optional.
    pub(crate) fn fully_lit(dim: usize) -> Self {
        Self {
            dim,
            origin: [0.0, 0.0],
            world_size: 1.0,
            visibility: vec![1.0; dim * dim],
        }
    }

    /// Bilinearly sample visibility at world-xy `(wx, wy)`. Outside the grid is
    /// treated as lit (1.0) — the camera footprint that drove `compute` is the
    /// region we have heights for; beyond it we make no shadow claim.
    ///
    /// The GPU samples the uploaded texture directly; this CPU mirror exists to
    /// verify the field's semantics in tests (the shader's UV map matches it).
    #[cfg(test)]
    pub(crate) fn sample(&self, wx: f32, wy: f32) -> f32 {
        if self.dim == 0 || self.world_size <= 0.0 {
            return 1.0;
        }
        let cell = self.world_size / (self.dim - 1).max(1) as f32;
        let gx = (wx - self.origin[0]) / cell;
        let gy = (wy - self.origin[1]) / cell;
        let max = (self.dim - 1) as f32;
        if gx < 0.0 || gy < 0.0 || gx > max || gy > max {
            return 1.0;
        }
        let x0 = gx.floor() as usize;
        let y0 = gy.floor() as usize;
        let x1 = (x0 + 1).min(self.dim - 1);
        let y1 = (y0 + 1).min(self.dim - 1);
        let tx = gx - x0 as f32;
        let ty = gy - y0 as f32;
        let at = |x: usize, y: usize| self.visibility[y * self.dim + x];
        let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
        let bot = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
        top * (1.0 - ty) + bot * ty
    }
}

/// Compute a [`ShadowField`] over a square world region.
///
/// - `origin` / `world_size`: the world-xy square to cover (cell (0,0) centre
///   at `origin`, cell `(dim-1, dim-1)` centre at `origin + world_size`).
/// - `sun_dir`: unit vector *towards* the sun in the engine world frame
///   (x=E, y=S, z=up) — the same vector the shader shades with. Its `z` is
///   `sin(altitude)`; the horizontal part points toward the sun on the ground.
/// - `softness`: world-z height (same units as the heightfield) over which an
///   occluder fades a cell from lit to shadowed, giving a soft penumbra. 0 =
///   hard edge.
/// - `height_at`: world-z (displaced, i.e. `elev · meters_to_world ·
///   exaggeration`) of the ground at a world-xy. Called once per grid cell;
///   the march then samples the assembled grid, so a slow per-call lookup
///   (tile cache walk) is paid `dim²` times, not `dim² · MAX_MARCH_CELLS`.
///
/// World-z and world-xy share units (normalised Mercator, metres folded
/// through `meters_to_world`), so the ray-height test `h(Q) > h(P) + s·tanα`
/// is dimensionally consistent with `s` measured in the same world-xy units.
pub(crate) fn compute(
    dim: usize,
    origin: [f32; 2],
    world_size: f32,
    sun_dir: [f32; 3],
    softness: f32,
    height_at: impl Fn(f32, f32) -> f32,
) -> ShadowField {
    if dim < 2 || world_size <= 0.0 {
        return ShadowField::fully_lit(dim.max(1));
    }

    // Horizontal sun direction + the rise-per-horizontal-distance (tan of the
    // solar altitude). With the sun at or below the horizon, or within a hair
    // of the zenith, there are no meaningful cast shadows — everything lit.
    let len_xy = (sun_dir[0] * sun_dir[0] + sun_dir[1] * sun_dir[1]).sqrt();
    if sun_dir[2] <= 1.0e-3 || len_xy <= 1.0e-4 {
        let mut f = ShadowField::fully_lit(dim);
        f.origin = origin;
        f.world_size = world_size;
        return f;
    }
    let ux = sun_dir[0] / len_xy; // toward-sun unit, world-xy
    let uy = sun_dir[1] / len_xy;
    let tan_alt = sun_dir[2] / len_xy; // world-z rise per unit world-xy toward sun

    let cell = world_size / (dim - 1) as f32;
    // World-z gained per one-cell step toward the sun along the grazing ray.
    let rise_per_cell = tan_alt * cell;

    // Sample the heightfield once per cell.
    let mut heights = vec![0.0f32; dim * dim];
    for j in 0..dim {
        let wy = origin[1] + j as f32 * cell;
        for i in 0..dim {
            let wx = origin[0] + i as f32 * cell;
            heights[j * dim + i] = height_at(wx, wy);
        }
    }

    let max = (dim - 1) as f32;
    let bilinear = |fx: f32, fy: f32| -> f32 {
        let x0 = fx.floor().clamp(0.0, max) as usize;
        let y0 = fy.floor().clamp(0.0, max) as usize;
        let x1 = (x0 + 1).min(dim - 1);
        let y1 = (y0 + 1).min(dim - 1);
        let tx = (fx - x0 as f32).clamp(0.0, 1.0);
        let ty = (fy - y0 as f32).clamp(0.0, 1.0);
        let at = |x: usize, y: usize| heights[y * dim + x];
        let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
        let bot = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
        top * (1.0 - ty) + bot * ty
    };

    let mut visibility = vec![1.0f32; dim * dim];
    for j in 0..dim {
        for i in 0..dim {
            let h0 = heights[j * dim + i];
            // The grazing ray rises as it heads toward the sun; an upstream
            // cell shadows us if it pokes above that ray. Track the largest
            // amount any occluder exceeds the ray by, for a soft edge.
            let mut over = 0.0f32;
            let mut fx = i as f32;
            let mut fy = j as f32;
            for s in 1..=MAX_MARCH_CELLS {
                fx += ux;
                fy += uy;
                if fx < 0.0 || fy < 0.0 || fx > max || fy > max {
                    break; // marched off the covered region
                }
                let ray_z = h0 + s as f32 * rise_per_cell;
                let hz = bilinear(fx, fy);
                let excess = hz - ray_z;
                if excess > over {
                    over = excess;
                }
            }
            // over <= 0  → lit. over >= softness → fully shadowed.
            let vis = if over <= 0.0 {
                1.0
            } else if softness <= 0.0 {
                0.0
            } else {
                (1.0 - over / softness).clamp(0.0, 1.0)
            };
            visibility[j * dim + i] = vis;
        }
    }

    ShadowField {
        dim,
        origin,
        world_size,
        visibility,
    }
}

/// GPU side of the cast-shadow feature: a single `SHADOW_DIM²` `R8Unorm`
/// texture (sun visibility per cell) plus its bind group, bound at `@group(3)`
/// of the raster pipeline. Map-level (one per renderer), uploaded from a
/// [`ShadowField`] whenever the sun / covered region / resident DEM changes.
///
/// `R8Unorm` is 1 byte/cell → 36 KiB for the whole grid; trivial next to the
/// raster tile budget, and a linear filter gives free penumbra softening
/// between cells.
pub(crate) struct ShadowMap {
    pub(crate) layout: Arc<wgpu::BindGroupLayout>,
    texture: wgpu::Texture,
    pub(crate) bind_group: wgpu::BindGroup,
    queue: Arc<wgpu::Queue>,
}

impl ShadowMap {
    pub(crate) fn new(device: &wgpu::Device, queue: &Arc<wgpu::Queue>) -> Self {
        let layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("turbomap-shadow-bgl"),
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
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("turbomap-shadow-tex"),
            size: wgpu::Extent3d {
                width: SHADOW_DIM as u32,
                height: SHADOW_DIM as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Initialise fully lit so a frame rendered before the first upload
        // (or with shadows off) shows no spurious darkening.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &vec![255u8; SHADOW_DIM * SHADOW_DIM],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SHADOW_DIM as u32),
                rows_per_image: Some(SHADOW_DIM as u32),
            },
            wgpu::Extent3d {
                width: SHADOW_DIM as u32,
                height: SHADOW_DIM as u32,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("turbomap-shadow-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("turbomap-shadow-bg"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            layout,
            texture,
            bind_group,
            queue: queue.clone(),
        }
    }

    /// Upload a computed field's visibility into the texture. The field must be
    /// `SHADOW_DIM²` (the renderer always computes at that resolution); a
    /// mismatch is ignored rather than panicking, leaving the prior contents.
    pub(crate) fn upload(&self, field: &ShadowField) {
        if field.dim != SHADOW_DIM || field.visibility.len() != SHADOW_DIM * SHADOW_DIM {
            log::warn!(
                "turbomap: shadow field dim {} != texture dim {}, skipping upload",
                field.dim,
                SHADOW_DIM
            );
            return;
        }
        let bytes: Vec<u8> = field
            .visibility
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
            .collect();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SHADOW_DIM as u32),
                rows_per_image: Some(SHADOW_DIM as u32),
            },
            wgpu::Extent3d {
                width: SHADOW_DIM as u32,
                height: SHADOW_DIM as u32,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sun in the east (toward +x) at 45° altitude: dir ≈ (cos45, 0, sin45).
    fn sun_east_45() -> [f32; 3] {
        let c = std::f32::consts::FRAC_1_SQRT_2;
        [c, 0.0, c]
    }

    fn at(f: &ShadowField, i: usize, j: usize) -> f32 {
        f.visibility[j * f.dim + i]
    }

    #[test]
    fn flat_terrain_is_fully_lit() {
        let f = compute(32, [0.0, 0.0], 1.0, sun_east_45(), 0.01, |_, _| 0.0);
        assert!(f.visibility.iter().all(|&v| v >= 0.999), "flat ground casts no shadow");
    }

    #[test]
    fn sun_at_zenith_casts_nothing() {
        // Straight up: no horizontal component → no cast shadow even over a spike.
        let f = compute(32, [0.0, 0.0], 1.0, [0.0, 0.0, 1.0], 0.01, |wx, _| {
            if wx > 0.45 && wx < 0.55 { 0.2 } else { 0.0 }
        });
        assert!(f.visibility.iter().all(|&v| v >= 0.999));
    }

    #[test]
    fn sun_below_horizon_casts_nothing() {
        let f = compute(32, [0.0, 0.0], 1.0, [0.7, 0.0, -0.1], 0.01, |_, _| 0.0);
        assert!(f.visibility.iter().all(|&v| v >= 0.999));
    }

    #[test]
    fn spike_shadows_the_anti_sun_side_not_the_sun_side() {
        // A tall, thin wall at x≈0.5 spanning all y. Sun is toward +x (east),
        // so light travels toward -x: the shadow falls on the WEST (-x, lower
        // i) side of the wall. Cells just EAST of the wall (toward the sun)
        // stay lit.
        let dim = 64;
        let wall_x = 0.5f32;
        let half = 0.02f32;
        // Tall enough that at 45° its shadow reaches well west of it.
        let wall_h = 0.25f32;
        let f = compute(dim, [0.0, 0.0], 1.0, sun_east_45(), 0.001, |wx, _| {
            if (wx - wall_x).abs() < half {
                wall_h
            } else {
                0.0
            }
        });

        let cell = 1.0 / (dim - 1) as f32;
        let wall_i = (wall_x / cell).round() as usize;
        // A cell a little WEST of the wall (down-sun) must be shadowed.
        let west = wall_i.saturating_sub(6);
        // A cell a little EAST of the wall (up-sun, toward the light) is lit.
        let east = (wall_i + 6).min(dim - 1);

        let j = dim / 2;
        assert!(
            at(&f, west, j) < 0.5,
            "west of the wall should be in shadow, got {}",
            at(&f, west, j)
        );
        assert!(
            at(&f, east, j) > 0.9,
            "east of the wall (toward the sun) should be lit, got {}",
            at(&f, east, j)
        );
    }

    #[test]
    fn lower_sun_casts_a_longer_shadow() {
        // Same wall, two sun altitudes. The lower sun's shadow must reach
        // farther west (more shadowed cells on the anti-sun side).
        let dim = 96;
        let wall_x = 0.6f32;
        let half = 0.015f32;
        let wall_h = 0.15f32;
        let field = |sun: [f32; 3]| {
            compute(dim, [0.0, 0.0], 1.0, sun, 0.001, |wx, _| {
                if (wx - wall_x).abs() < half {
                    wall_h
                } else {
                    0.0
                }
            })
        };
        let count_shadow = |f: &ShadowField| {
            let j = dim / 2;
            (0..dim).filter(|&i| at(f, i, j) < 0.5).count()
        };
        // 50° vs 20° altitude, both toward +x.
        let hi = (50f32).to_radians();
        let lo = (20f32).to_radians();
        let f_hi = field([hi.cos(), 0.0, hi.sin()]);
        let f_lo = field([lo.cos(), 0.0, lo.sin()]);
        assert!(
            count_shadow(&f_lo) > count_shadow(&f_hi),
            "lower sun must cast a longer shadow: lo={} hi={}",
            count_shadow(&f_lo),
            count_shadow(&f_hi)
        );
    }

    #[test]
    fn sample_maps_world_xy_and_clamps_outside() {
        let mut f = ShadowField::fully_lit(4);
        f.origin = [0.0, 0.0];
        f.world_size = 1.0;
        // Force a known checkerboard-ish value to test interpolation direction.
        f.visibility = vec![
            0.0, 0.0, 1.0, 1.0, //
            0.0, 0.0, 1.0, 1.0, //
            0.0, 0.0, 1.0, 1.0, //
            0.0, 0.0, 1.0, 1.0, //
        ];
        // Far west → shadowed end.
        assert!(f.sample(0.0, 0.5) < 0.1);
        // Far east → lit end.
        assert!(f.sample(1.0, 0.5) > 0.9);
        // Outside the grid → lit (no claim).
        assert!((f.sample(-5.0, 0.5) - 1.0).abs() < 1e-6);
        assert!((f.sample(0.5, 9.0) - 1.0).abs() < 1e-6);
    }
}
