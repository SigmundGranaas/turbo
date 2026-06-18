//! Precomputed, **tileable** 3D cloud noise — the device-ready path.
//!
//! The analytic 3D Perlin-Worley / Worley the shader can compute per march
//! step is far too expensive for a mobile (or software) GPU: the volumetric
//! march samples density ~150× per pixel, each with dozens of hashes. Nubis
//! solves this by baking the noise into a small 3D texture once and *sampling*
//! it in the shader. This module generates that volume on the CPU.
//!
//! The volume is **periodic** (every octave's lattice/feature grid wraps at a
//! divisor of `n`), so the shader can sample it with `AddressMode::Repeat` at
//! any frequency with no visible seam. Channels:
//!
//! - **R** = Perlin-Worley base (the primary cloud shape).
//! - **G** = high-frequency Worley billow (the detail that erodes edges).
//! - **B** = mid-frequency Worley billow (spare / future use).
//! - **A** = 255.

/// FNV-1a-style integer hash of a wrapped lattice cell → `u32`.
fn hash_cell(x: i32, y: i32, z: i32) -> u32 {
    let mut h = 2_166_136_261u32;
    for v in [x, y, z] {
        h = (h ^ (v as u32)).wrapping_mul(16_777_619);
    }
    // A couple of avalanche steps so neighbouring cells decorrelate.
    h ^= h >> 15;
    h = h.wrapping_mul(2_246_822_519);
    h ^= h >> 13;
    h
}

/// `h` → float in `[0,1)`.
fn rand01(h: u32) -> f32 {
    (h >> 8) as f32 / ((1u32 << 24) as f32)
}

fn wrap(i: i32, period: i32) -> i32 {
    ((i % period) + period) % period
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Periodic value noise at `p` (in unit space) over `freq` cells per axis.
fn vnoise(p: [f32; 3], freq: i32) -> f32 {
    let c = [p[0] * freq as f32, p[1] * freq as f32, p[2] * freq as f32];
    let i = [
        c[0].floor() as i32,
        c[1].floor() as i32,
        c[2].floor() as i32,
    ];
    let f = [c[0] - i[0] as f32, c[1] - i[1] as f32, c[2] - i[2] as f32];
    let u = [fade(f[0]), fade(f[1]), fade(f[2])];
    let corner = |dx: i32, dy: i32, dz: i32| -> f32 {
        let h = hash_cell(
            wrap(i[0] + dx, freq),
            wrap(i[1] + dy, freq),
            wrap(i[2] + dz, freq),
        );
        rand01(h)
    };
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), u[0]);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), u[0]);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), u[0]);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), u[0]);
    lerp(lerp(x00, x10, u[1]), lerp(x01, x11, u[1]), u[2])
}

/// Periodic Worley F1 (nearest jittered feature) over `freq` cells per axis,
/// returned as a normalised distance in roughly `0..1`.
fn worley(p: [f32; 3], freq: i32) -> f32 {
    let c = [p[0] * freq as f32, p[1] * freq as f32, p[2] * freq as f32];
    let i = [
        c[0].floor() as i32,
        c[1].floor() as i32,
        c[2].floor() as i32,
    ];
    let f = [c[0] - i[0] as f32, c[1] - i[1] as f32, c[2] - i[2] as f32];
    let mut min_d = 9.0f32;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let cell = [i[0] + dx, i[1] + dy, i[2] + dz];
                let h = hash_cell(
                    wrap(cell[0], freq),
                    wrap(cell[1], freq),
                    wrap(cell[2], freq),
                );
                // Jittered feature point inside the cell.
                let ox = rand01(h);
                let oy = rand01(h.wrapping_mul(747_796_405).wrapping_add(1));
                let oz = rand01(h.wrapping_mul(2_891_336_453).wrapping_add(1));
                let r = [
                    dx as f32 + ox - f[0],
                    dy as f32 + oy - f[1],
                    dz as f32 + oz - f[2],
                ];
                min_d = min_d.min(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
            }
        }
    }
    min_d.sqrt()
}

/// Fractal sum of value noise over the given (period-dividing) frequencies.
fn vfbm(p: [f32; 3], freqs: &[i32]) -> f32 {
    let mut amp = 0.5;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for &fq in freqs {
        sum += amp * vnoise(p, fq);
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

/// Fractal inverted-Worley billows over the given frequencies → rounded lumps.
fn wfbm_billow(p: [f32; 3], freqs: &[i32]) -> f32 {
    let mut amp = 0.6;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for &fq in freqs {
        sum += amp * (1.0 - worley(p, fq));
        norm += amp;
        amp *= 0.5;
    }
    (sum / norm).clamp(0.0, 1.0)
}

fn remap(x: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    c + (x - a) * (d - c) / (b - a)
}

/// Generate the `n³` RGBA8 tileable noise volume (frequencies are divisors of
/// the common sizes 32/48/64 so the result wraps seamlessly).
pub fn generate(n: u32) -> Vec<u8> {
    let mut out = vec![0u8; (n * n * n * 4) as usize];
    let inv = 1.0 / n as f32;
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let p = [
                    (x as f32 + 0.5) * inv,
                    (y as f32 + 0.5) * inv,
                    (z as f32 + 0.5) * inv,
                ];
                // Perlin-Worley base: perlin fBm remapped up toward billows.
                let perlin = vfbm(p, &[2, 4, 8]);
                let billow_lo = wfbm_billow(p, &[2, 4, 8]);
                let pw = remap(perlin, billow_lo - 1.0, 1.0, 0.0, 1.0).clamp(0.0, 1.0);
                let detail = wfbm_billow(p, &[8, 16]);
                let mid = wfbm_billow(p, &[4, 8]);
                let i = ((z * n * n + y * n + x) * 4) as usize;
                out[i] = (pw * 255.0).round() as u8;
                out[i + 1] = (detail * 255.0).round() as u8;
                out[i + 2] = (mid * 255.0).round() as u8;
                out[i + 3] = 255;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_has_expected_size_and_is_not_flat() {
        let n = 16u32;
        let v = generate(n);
        assert_eq!(v.len(), (n * n * n * 4) as usize);
        // Channels should carry real variation, not a constant.
        let r_min = v.iter().step_by(4).copied().min().unwrap();
        let r_max = v.iter().step_by(4).copied().max().unwrap();
        assert!(r_max - r_min > 40, "R channel too flat: {r_min}..{r_max}");
    }

    #[test]
    fn worley_is_periodic_across_the_wrap() {
        // Sampling just inside 0 and just inside 1 (same lattice phase) should
        // match closely — proving the volume tiles seamlessly.
        let eps = 0.001;
        for fq in [2, 4, 8] {
            let a = worley([eps, 0.3, 0.7], fq);
            let b = worley([1.0 + eps, 0.3, 0.7], fq);
            assert!((a - b).abs() < 1e-4, "freq {fq} not periodic: {a} vs {b}");
        }
    }
}
