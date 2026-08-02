//! E1 — is the WHOLE solver bit-reproducible across architectures?
//!
//! E0 answered this per-function: of every transcendental on the routing
//! path, only `f32::atan` differed between x86_64-glibc and aarch64-glibc.
//! That is a necessary condition, not a sufficient one — a single flipped
//! comparison anywhere in the search can still fork a route.
//!
//! This drives the real `Pathfinder` over the real Sjunkhatten artifacts and
//! hashes the resulting geometry, on both architectures.
//!
//! Deliberately standalone rather than cross-compiling `tileserver`: the
//! server binary pulls sqlx, rustls and axum, whose C dependencies make an
//! aarch64 build a yak-shave unrelated to the question. The solver crates
//! are pure Rust apart from zstd, which cross-compiles with the gcc
//! aarch64 toolchain.
//!
//! NOTE ON SCOPE: this is glibc-vs-glibc under QEMU user-mode emulation.
//! Android links **bionic**, a different libm. This bounds the ISA question,
//! not the platform one. See the results doc.
//!
//! Usage: e1 <artifacts-dir>

use std::sync::Arc;

use turbo_tiles_elev::Dem;
use turbo_tiles_graph::Graph;
use turbo_tiles_mask::Mask;
use turbo_tiles_pathfind::{Pathfinder, Prefs};

/// Ground-truth endpoints inside the Sjunkhatten DEM cell (lon, lat).
/// Drawn from the `nordland` corpus hikes that fall inside coverage.
const ROUTES: &[([f64; 2], [f64; 2], &str)] = &[
    ([15.961509, 66.996683], [15.981, 67.004], "n-3496285"),
    ([15.001894, 66.866072], [15.019, 66.874], "n-3894666"),
    ([15.495658, 66.921925], [15.513, 66.930], "n-1821249"),
    ([15.287597, 66.741357], [15.340, 66.770], "n-1884462"),
    ([15.000533, 67.238367], [15.018, 67.246], "n-4220456"),
    ([16.097354, 67.053586], [16.140, 67.075], "n-1895277"),
];

fn fnv1a(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: e1 <artifacts-dir>");
        std::process::exit(2);
    });
    let p = std::path::Path::new(&dir);

    let dem = Arc::new(Dem::open(p.join("norway.dem")).expect("dem"));
    let mask = Arc::new(Mask::open(p.join("norway.mask")).expect("mask"));
    let graph = Arc::new(Graph::open(p.join("norway.graph")).expect("graph"));

    let pf = Pathfinder::with_defaults(Some(dem), Some(mask), Some(graph));

    println!("# E1 cross-ISA solver determinism");
    println!("arch    {}", std::env::consts::ARCH);
    println!("layers  {:?}", pf.layer_names());
    println!();

    for mode in ["off-trail", "unified"] {
        let mut corpus = 0xcbf2_9ce4_8422_2325u64;
        let mut solved = 0;
        println!("## lane: {mode}");
        for (from, to, id) in ROUTES {
            let mut prefs = Prefs {
                max_off_trail_km: 20.0,
                ..Default::default()
            };
            if mode == "off-trail" {
                prefs.force_off_trail = true;
                prefs.snap_radius_m = 0.0;
                prefs.bridge_radius_m = 0.0;
            }
            match pf.solve(*from, *to, prefs) {
                Ok(path) => {
                    let mut h = 0xcbf2_9ce4_8422_2325u64;
                    for c in &path.geometry {
                        fnv1a(&mut h, &c[0].to_bits().to_le_bytes());
                        fnv1a(&mut h, &c[1].to_bits().to_le_bytes());
                    }
                    fnv1a(&mut corpus, &h.to_le_bytes());
                    solved += 1;
                    println!(
                        "  {id:<12} pts={:<5} len_m={:<10.3} hash={h:016x}",
                        path.geometry.len(),
                        path.length_m
                    );
                }
                Err(e) => {
                    // Failures must also be identical across arches.
                    let s = format!("{e:?}");
                    fnv1a(&mut corpus, s.as_bytes());
                    println!("  {id:<12} ERR {s}");
                }
            }
        }
        println!("  {mode} corpus_hash = {corpus:016x}  ({solved}/{} solved)", ROUTES.len());
        println!();
    }
}
