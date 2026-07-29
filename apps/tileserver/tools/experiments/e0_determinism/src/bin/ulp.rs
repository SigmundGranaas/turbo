//! E0b — size the f32::atan divergence.
//!
//! `libm` is bit-identical on both arches (proven by E0), so it serves as the
//! reference. Counting std-vs-libm mismatches per arch bounds how often the
//! platform implementations can disagree with each other.
//!
//! Also reports the routing consequence: unified.rs:451 does
//! `grad.atan().to_degrees()` and compares against CLIFF_DEG = 60.0, so a
//! divergence near that threshold can flip a hard refusal.

const N: u32 = 4_000_000;
const CLIFF_DEG: f32 = 60.0;

fn ulp_diff(a: f32, b: f32) -> i64 {
    (a.to_bits() as i64 - b.to_bits() as i64).abs()
}

fn main() {
    let mut mismatches = 0u32;
    let mut max_ulp = 0i64;
    let mut worst_x = 0.0f32;

    // Count how many samples land within one ULP of the cliff threshold in
    // degrees -- i.e. where a 1-ULP atan difference could flip the refusal.
    let mut near_cliff = 0u32;
    let mut cliff_flips = 0u32;

    for i in 0..N {
        let x = -4.0 + 8.0 * (i as f32) / (N as f32);
        let s = x.atan();
        let l = libm::atanf(x);
        if s.to_bits() != l.to_bits() {
            mismatches += 1;
            let u = ulp_diff(s, l);
            if u > max_ulp {
                max_ulp = u;
                worst_x = x;
            }
        }
        let ds = s.to_degrees();
        let dl = l.to_degrees();
        if (ds - CLIFF_DEG).abs() < 1e-4 {
            near_cliff += 1;
        }
        // Would the two implementations disagree about refusing this edge?
        if (ds > CLIFF_DEG) != (dl > CLIFF_DEG) {
            cliff_flips += 1;
        }
    }

    println!("arch                 {}", std::env::consts::ARCH);
    println!("samples              {N}");
    println!("std vs libm atanf");
    println!("  mismatching        {mismatches}  ({:.4}%)",
             100.0 * mismatches as f64 / N as f64);
    println!("  max ULP            {max_ulp}");
    println!("  worst at x         {worst_x}");
    println!("cliff threshold ({CLIFF_DEG} deg)");
    println!("  samples within 1e-4 deg  {near_cliff}");
    println!("  refusal decision flips   {cliff_flips}");
}
