//! E0 — Is the routing float path bit-reproducible across architectures?
//!
//! Hashes the transcendental functions the router actually calls, in the
//! precisions it actually uses. Run on x86_64 and aarch64; diff the hashes.
//!
//! Routing-path transcendentals (from the tree at 36d2038):
//!   exp   — Tobler pace, 10 sites, f32 and f64
//!   tan   — slope_deg -> gradient (native_contributors.rs:186, :826)
//!   atan  — gradient -> grade_deg (unified.rs:451)
//!   powf  — projection meridional radius (pathfinder.rs:1955)
//!   sqrt  — everywhere (IEEE-exact, included as a control)
//!
//! Each block is hashed with FNV-1a over the raw IEEE bits, so a single
//! ULP anywhere changes the digest.

const N: u32 = 1_000_000;

#[inline]
fn fnv1a(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
}
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Sweep x over [lo, hi] in N steps, apply f, hash the result bits.
fn hash_f64(lo: f64, hi: f64, f: impl Fn(f64) -> f64) -> u64 {
    let mut h = FNV_OFFSET;
    for i in 0..N {
        let x = lo + (hi - lo) * (i as f64) / (N as f64);
        fnv1a(&mut h, &f(x).to_bits().to_le_bytes());
    }
    h
}

fn hash_f32(lo: f32, hi: f32, f: impl Fn(f32) -> f32) -> u64 {
    let mut h = FNV_OFFSET;
    for i in 0..N {
        let x = lo + (hi - lo) * (i as f32) / (N as f32);
        fnv1a(&mut h, &f(x).to_bits().to_le_bytes());
    }
    h
}

/// The symmetric Tobler kernel as it appears in the four f32 mesh copies.
#[inline]
fn tobler_f32_std(g: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (g.abs() + 0.05)).exp();
    if v < 1e-4 { 1.0e6 } else { 1.0 / v }
}

#[inline]
fn tobler_f32_libm(g: f32) -> f32 {
    let v = 1.6667 * libm::expf(-3.5 * (g.abs() + 0.05));
    if v < 1e-4 { 1.0e6 } else { 1.0 / v }
}

/// The asymmetric f64 contributor kernel.
#[inline]
fn tobler_f64_std(s: f64) -> f64 {
    let v = 1.6667 * (-3.5 * (s + 0.05).abs()).exp();
    if v <= 1e-6 { 100.0 } else { 1.0 / v }
}

#[inline]
fn tobler_f64_libm(s: f64) -> f64 {
    let v = 1.6667 * libm::exp(-3.5 * (s + 0.05).abs());
    if v <= 1e-6 { 100.0 } else { 1.0 / v }
}

/// FMA-contraction probe. On aarch64 the compiler may contract `a*b + c`
/// into a single fused instruction (one rounding); on x86_64 without
/// explicit FMA codegen it is two roundings. Values chosen so the
/// difference is representable.
#[inline]
fn fma_probe(x: f64) -> f64 {
    let a = x * 1.000_000_000_000_000_2;
    let b = 0.999_999_999_999_999_9;
    let c = -x;
    // Deliberately the shape the slope/dot-product code uses.
    a * b + c
}

/// The dot-product + normalise shape from DirectionalSlopeContributor.
#[inline]
fn dot_norm(t: f64) -> f64 {
    let (ex, ey) = (t.cos(), t.sin());
    let (dx, dy) = ((-t).sin(), (-t).cos());
    let len = (ex * ex + ey * ey).sqrt();
    (ex / len) * dx + (ey / len) * dy
}

fn main() {
    let arch = std::env::consts::ARCH;
    println!("# E0 determinism digest");
    println!("arch          {arch}");
    println!("pointer_width {}", usize::BITS);
    println!("samples       {N}");
    println!();

    // Slope range: +-2.0 gradient covers 0-63 degrees, past the cliff cutoff.
    println!("tobler_f32_std   {:016x}", hash_f32(-2.0, 2.0, tobler_f32_std));
    println!("tobler_f32_libm  {:016x}", hash_f32(-2.0, 2.0, tobler_f32_libm));
    println!("tobler_f64_std   {:016x}", hash_f64(-2.0, 2.0, tobler_f64_std));
    println!("tobler_f64_libm  {:016x}", hash_f64(-2.0, 2.0, tobler_f64_libm));
    println!();
    println!("exp_f32_std      {:016x}", hash_f32(-8.0, 2.0, |x| x.exp()));
    println!("exp_f32_libm     {:016x}", hash_f32(-8.0, 2.0, libm::expf));
    println!("exp_f64_std      {:016x}", hash_f64(-8.0, 2.0, |x| x.exp()));
    println!("exp_f64_libm     {:016x}", hash_f64(-8.0, 2.0, libm::exp));
    println!();
    println!("tan_f64_std      {:016x}", hash_f64(-1.5, 1.5, |x| x.tan()));
    println!("tan_f64_libm     {:016x}", hash_f64(-1.5, 1.5, libm::tan));
    println!("atan_f32_std     {:016x}", hash_f32(-4.0, 4.0, |x| x.atan()));
    println!("atan_f32_libm    {:016x}", hash_f32(-4.0, 4.0, libm::atanf));
    println!("powf_f64_std     {:016x}", hash_f64(0.5, 1.0, |x| x.powf(1.5)));
    println!("powf_f64_libm    {:016x}", hash_f64(0.5, 1.0, |x| libm::pow(x, 1.5)));
    println!();
    println!("sqrt_f64_ctrl    {:016x}", hash_f64(0.0, 1e6, |x| x.sqrt()));
    println!("fma_probe        {:016x}", hash_f64(1.0, 2.0, fma_probe));
    println!("dot_norm         {:016x}", hash_f64(0.0, 6.283, dot_norm));

    // A couple of raw values for eyeballing which way any drift goes.
    println!();
    println!("# spot values (bits)");
    for g in [0.05f64, 0.3, 1.0] {
        println!("  tobler_f64_std({g})  = {:.17e}  {:016x}",
                 tobler_f64_std(g), tobler_f64_std(g).to_bits());
        println!("  tobler_f64_libm({g}) = {:.17e}  {:016x}",
                 tobler_f64_libm(g), tobler_f64_libm(g).to_bits());
    }
}
