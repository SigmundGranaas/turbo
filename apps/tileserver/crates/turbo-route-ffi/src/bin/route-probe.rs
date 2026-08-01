//! Plan one route and print a digest of it.
//!
//! Exists to answer a question the host test suite structurally cannot:
//! **does the solver produce the same geometry on the ISA and libc the
//! phone actually runs?** `cargo test` runs on x86_64 against glibc. The
//! phone is aarch64 against bionic. Tobler's term is `exp()`, which is
//! not correctly-rounded and is a *different implementation* in each
//! libc — so a route that costs 1 ULP differently can take a different
//! edge out of the priority queue and diverge from there.
//!
//! Build static and run under qemu to compare:
//!
//! ```text
//! cargo run --bin route-probe -- tools/ci-pack
//! RUSTFLAGS="-C target-feature=+crt-static" \
//!   cargo ndk -t arm64-v8a build --release --bin route-probe
//! qemu-aarch64-static target/aarch64-linux-android/release/route-probe tools/ci-pack
//! ```
//!
//! Static linking is deliberate: it pulls bionic's own `libm` into the
//! binary, so the comparison is against the real implementation rather
//! than against whatever the emulator's host happens to provide.

use turbo_route_ffi::{GeoPoint, RouteEngine, RouteOptions};

/// FNV-1a over the raw bits of every coordinate.
///
/// Bits, not rounded decimals: the entire point is to catch a
/// last-place difference, and formatting to 6 places would hide exactly
/// the divergence this is looking for.
fn digest(points: &[GeoPoint]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in points {
        for b in p
            .lon
            .to_bits()
            .to_le_bytes()
            .iter()
            .chain(&p.lat.to_bits().to_le_bytes())
        {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tools/ci-pack".to_string());

    let engine = match RouteEngine::open(dir.clone()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("open {dir}: {e}");
            std::process::exit(2);
        }
    };

    // The same two points the host round-trip test uses, on the
    // Sjunkhatten network the CI pack was cut from.
    let from = GeoPoint {
        lon: 15.04048,
        lat: 67.065016,
    };
    let to = GeoPoint {
        lon: 15.0555,
        lat: 67.0685,
    };

    let route = match engine.plan(vec![from, to], RouteOptions::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("plan: {e}");
            std::process::exit(3);
        }
    };

    println!("arch      {}", std::env::consts::ARCH);
    println!("os        {}", std::env::consts::OS);
    println!("points    {}", route.geometry.len());
    println!("length_m  {:.9}", route.length_m);
    println!("duration  {:.9}", route.duration_s);
    println!("ascent_m  {:.9}", route.ascent_m);
    println!("len_bits  {:016x}", route.length_m.to_bits());
    println!("dur_bits  {:016x}", route.duration_s.to_bits());
    println!("asc_bits  {:016x}", route.ascent_m.to_bits());
    println!("geom_fnv  {:016x}", digest(&route.geometry));
}
