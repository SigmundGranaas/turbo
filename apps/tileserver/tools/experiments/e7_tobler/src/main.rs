//! E7 — Do the six Tobler copies agree?
//!
//! Each function below is transcribed VERBATIM from its site in the tree at
//! commit 36d2038. Nothing is normalised; the point is to find out whether
//! they are the same function.
//!
//!   A fmm/tobler.rs:99            f32  pace_from_grad
//!   B fmm/tobler_aniso.rs:169     f32  tobler_pace
//!   C fmm/elastica.rs:145         f32  tobler_pace
//!   D pathfind/unified.rs:96      f32  tobler_pace
//!   E pathfind/native_contributors.rs:187  f64  delta_seconds_per_metre
//!   F pathfind/native_contributors.rs:198  f64  pace_at_signed_slope
//!   G pathfind/native_contributors.rs:829  f64  inline (directional slope)

const BASE_PACE_S_PER_M: f64 = 1.0 / 1.4;

// ---- A: fmm/tobler.rs:99 -------------------------------------------------
fn a_tobler_rs(grad: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (grad.abs() + 0.05)).exp();
    if v < 1e-4 {
        return 1.0e6;
    }
    1.0 / v
}

// ---- B: fmm/tobler_aniso.rs:169 ------------------------------------------
fn b_tobler_aniso(grad_mag: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (grad_mag.abs() + 0.05)).exp();
    if v < 1e-4 { 1.0e6 } else { 1.0 / v }
}

// ---- C: fmm/elastica.rs:145 ----------------------------------------------
fn c_elastica(grad_mag: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (grad_mag.abs() + 0.05)).exp();
    if v < 1e-4 { 1.0e6 } else { 1.0 / v }
}

// ---- D: pathfind/unified.rs:96 -------------------------------------------
fn d_unified(grad_mag: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (grad_mag.abs() + 0.05)).exp();
    if v < 1e-4 { 1.0e6 } else { 1.0 / v }
}

// ---- E: native_contributors.rs:187 (returns a DELTA, not a pace) ---------
fn e_delta_seconds_per_metre(slope_deg: f32) -> f64 {
    let s_rad = (slope_deg as f64).to_radians();
    let grad = s_rad.tan().abs();
    let v = 1.6667 * (-3.5 * (grad + 0.05).abs()).exp();
    if v <= 1e-6 {
        return 100.0;
    }
    let pace = 1.0 / v;
    pace - BASE_PACE_S_PER_M
}

// ---- F: native_contributors.rs:198 ---------------------------------------
fn f_pace_at_signed_slope(slope: f64) -> f64 {
    let v = 1.6667 * (-3.5 * (slope + 0.05).abs()).exp();
    if v <= 1e-6 { 100.0 } else { 1.0 / v }
}

// ---- G: native_contributors.rs:829 (inline; s_offset already abs'd) ------
fn g_directional(signed_slope: f64) -> f64 {
    let s_offset = (signed_slope + 0.05).abs();
    let v = 1.6667 * (-3.5 * s_offset).exp();
    if v <= 1e-6 { 100.0 } else { 1.0 / v }
}

fn main() {
    println!("E7 — Tobler implementation divergence\n");

    // ---------------------------------------------------------------------
    // 1. Are the four f32 copies bit-identical?
    // ---------------------------------------------------------------------
    let mut f32_mismatch = 0u32;
    let mut n = 0u32;
    let mut g = -2.0f32;
    while g <= 2.0 {
        let (a, b, c, d) = (a_tobler_rs(g), b_tobler_aniso(g), c_elastica(g), d_unified(g));
        if a.to_bits() != b.to_bits() || a.to_bits() != c.to_bits() || a.to_bits() != d.to_bits() {
            f32_mismatch += 1;
        }
        n += 1;
        g += 0.0001;
    }
    println!("1. The four f32 copies (A,B,C,D) over grad in [-2,2], {n} samples:");
    println!("   bit-level mismatches: {f32_mismatch}  -> {}",
             if f32_mismatch == 0 { "IDENTICAL" } else { "DIVERGENT" });

    // ---------------------------------------------------------------------
    // 2. Symmetric (f32 mesh) vs asymmetric (f64 contributor) as PACE.
    //    This is the real question: are they the same physical model?
    // ---------------------------------------------------------------------
    println!("\n2. Symmetric f32 mesh pace (D) vs asymmetric f64 contributor pace (F):");
    println!("   {:>9} {:>9} {:>12} {:>12} {:>10} {:>9}",
             "slope", "deg", "D_sym s/m", "F_asym s/m", "abs diff", "rel %");
    let mut worst_rel = 0.0f64;
    let mut worst_at = 0.0f64;
    for &s in &[-0.60, -0.40, -0.30, -0.20, -0.15, -0.10, -0.05, 0.0,
                0.05, 0.10, 0.20, 0.30, 0.40, 0.60] {
        let d = d_unified(s as f32) as f64;
        let f = f_pace_at_signed_slope(s);
        let diff = (d - f).abs();
        let rel = 100.0 * diff / f;
        if rel > worst_rel { worst_rel = rel; worst_at = s; }
        println!("   {:>9.2} {:>9.1} {:>12.4} {:>12.4} {:>10.4} {:>8.1}%",
                 s, s.atan().to_degrees(), d, f, diff, rel);
    }
    println!("   worst relative divergence: {:.1}% at slope {:.2} ({:.1}deg)",
             worst_rel, worst_at, worst_at.atan().to_degrees());

    // ---------------------------------------------------------------------
    // 3. F vs G — same structure, should agree.
    // ---------------------------------------------------------------------
    let mut fg_mismatch = 0u32;
    let mut s = -2.0f64;
    while s <= 2.0 {
        if f_pace_at_signed_slope(s).to_bits() != g_directional(s).to_bits() {
            fg_mismatch += 1;
        }
        s += 0.0001;
    }
    println!("\n3. F vs G (both f64, signed): mismatches = {fg_mismatch} -> {}",
             if fg_mismatch == 0 { "IDENTICAL" } else { "DIVERGENT" });

    // ---------------------------------------------------------------------
    // 4. E is derived from grad.abs() then |grad+0.05| — so E is SYMMETRIC
    //    in slope but uses the f64 guard. Compare E's implied pace to D.
    // ---------------------------------------------------------------------
    println!("\n4. E (f64, symmetric, returns delta) vs D (f32, symmetric) as pace:");
    let mut worst_e = 0.0f64;
    for deg in [0.0f32, 5.0, 10.0, 20.0, 30.0, 45.0, 60.0] {
        let e_pace = e_delta_seconds_per_metre(deg) + BASE_PACE_S_PER_M;
        let grad = (deg as f64).to_radians().tan();
        let d_pace = d_unified(grad as f32) as f64;
        let rel = 100.0 * (e_pace - d_pace).abs() / d_pace;
        if rel > worst_e { worst_e = rel; }
        println!("   {:>5.1}deg  E={:>10.4}  D={:>10.4}  rel={:>7.3}%", deg, e_pace, d_pace, rel);
    }
    println!("   worst E-vs-D relative divergence: {:.3}%", worst_e);

    // ---------------------------------------------------------------------
    // 5. Guard thresholds and sentinels.
    // ---------------------------------------------------------------------
    println!("\n5. Refusal guards:");
    println!("   f32 copies (A-D): v < 1e-4  -> pace 1.0e6");
    println!("   f64 copies (E-G): v <= 1e-6 -> pace 100.0 (E) / 100.0 (F,G)");
    // Where does each guard trip, in slope terms?
    let trip_f32 = (-(1e-4f64 / 1.6667).ln() / 3.5) - 0.05;
    let trip_f64 = (-(1e-6f64 / 1.6667).ln() / 3.5) - 0.05;
    println!("   f32 guard trips at |grad| = {:.3} ({:.1}deg)",
             trip_f32, trip_f32.atan().to_degrees());
    println!("   f64 guard trips at |grad| = {:.3} ({:.1}deg)",
             trip_f64, trip_f64.atan().to_degrees());
    println!("   sentinel ratio: 1.0e6 / 100.0 = 10000x");
}
