//! E0c — what does routing the float path through `libm` cost?
//!
//! `exp` is the hottest transcendental in the router (Tobler pace, evaluated
//! per cell and per edge sub-sample). If the portability fix is 2x slower on
//! exp it needs to be a considered trade, not a reflex.

use std::hint::black_box;
use std::time::Instant;

const N: usize = 20_000_000;

fn bench(label: &str, f: impl Fn(f64) -> f64) -> f64 {
    // Warm up.
    let mut acc = 0.0f64;
    for i in 0..100_000 {
        acc += f(black_box(i as f64 * 1e-5));
    }
    black_box(acc);

    let t = Instant::now();
    let mut acc = 0.0f64;
    for i in 0..N {
        acc += f(black_box(-3.5 * (i as f64 * 1e-7 + 0.05)));
    }
    let el = t.elapsed();
    black_box(acc);
    let ns = el.as_secs_f64() * 1e9 / N as f64;
    println!("  {label:<16} {ns:>7.2} ns/call   ({:>6.1} ms for {N} calls)",
             el.as_secs_f64() * 1e3);
    ns
}

fn bench32(label: &str, f: impl Fn(f32) -> f32) -> f64 {
    let mut acc = 0.0f32;
    for i in 0..100_000 {
        acc += f(black_box(i as f32 * 1e-5));
    }
    black_box(acc);

    let t = Instant::now();
    let mut acc = 0.0f32;
    for i in 0..N {
        acc += f(black_box(-3.5 * (i as f32 * 1e-7 + 0.05)));
    }
    let el = t.elapsed();
    black_box(acc);
    let ns = el.as_secs_f64() * 1e9 / N as f64;
    println!("  {label:<16} {ns:>7.2} ns/call   ({:>6.1} ms for {N} calls)",
             el.as_secs_f64() * 1e3);
    ns
}

fn main() {
    println!("arch {}", std::env::consts::ARCH);
    println!("exp f64:");
    let a = bench("std", |x| x.exp());
    let b = bench("libm", libm::exp);
    println!("  -> libm/std = {:.2}x", b / a);

    println!("exp f32:");
    let c = bench32("std", |x| x.exp());
    let d = bench32("libm", libm::expf);
    println!("  -> libm/std = {:.2}x", d / c);

    println!("atan f32:");
    let e = bench32("std", |x| x.atan());
    let f = bench32("libm", libm::atanf);
    println!("  -> libm/std = {:.2}x", f / e);

    // Budget context: a solve touches ~50k cells; the slope-family
    // contributors sample ~5 points per cell edge after memoisation.
    let per_solve_calls = 250_000.0;
    println!();
    println!("At ~{per_solve_calls:.0} exp calls/solve:");
    println!("  std  f64: {:.2} ms", a * per_solve_calls / 1e6);
    println!("  libm f64: {:.2} ms  (delta {:+.2} ms)",
             b * per_solve_calls / 1e6, (b - a) * per_solve_calls / 1e6);
    println!("  against a ~250 ms mean solve, that is {:+.3}%",
             100.0 * (b - a) * per_solve_calls / 1e6 / 250.0);
}
