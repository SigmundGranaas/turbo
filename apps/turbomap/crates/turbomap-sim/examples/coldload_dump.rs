//! Inspection tool: run the cold-load session the `cold_load_paints_every_
//! subsystem` gate runs, then SHOW what actually rendered — save the settled
//! frame as a PNG and print a colour census against the gate's expected
//! constants. When the gate fails on a new adapter/driver, this answers
//! "colour-shifted or not drawn at all?" in one run instead of a debugging
//! session. (Execution rule: every gate must be runnable AND inspectable.)
//!
//! Usage: cargo run -p turbomap-sim --example coldload_dump --release
//! Writes target/sim-reports/coldload.png and prints the census to stderr.

use std::collections::HashMap;

use turbomap_engine::MapEngine;
use turbomap_sim::{basemap_scene, session, Sim};

fn main() {
    // First: what does an UNLOADED map look like on screen? (The blank-map
    // gates compare against this; the authored clear colour passes through
    // the post pipeline like everything else.) Separate Sim so the huge
    // latency can't pollute the cold-load run below.
    {
        let Some(mut blank_sim) = Sim::new(
            640,
            420,
            Sim::camera_at_major_crossroads(11.0),
            Default::default(),
        ) else {
            eprintln!("no wgpu adapter available — nothing to inspect");
            std::process::exit(2);
        };
        blank_sim.engine.apply(basemap_scene());
        blank_sim.latency_frames = 1_000_000;
        blank_sim.step();
        let img = blank_sim.last.as_ref().expect("a rendered frame");
        eprintln!("UNLOADED frame census (the on-screen 'blank' colour):");
        census(img, 3);
    }

    let Some(mut sim) = Sim::new(
        640,
        420,
        Sim::camera_at_major_crossroads(11.0),
        Default::default(),
    ) else {
        eprintln!("no wgpu adapter available — nothing to inspect");
        std::process::exit(2);
    };
    sim.engine.apply(basemap_scene());
    let settled = sim.run_until_stable(120, 0.001);
    eprintln!("settled: {settled:?}, stats: {:?}", sim.stats.last());

    let img = sim.last.as_ref().expect("a rendered frame");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sim-reports");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("coldload.png");
    img.save(&path).expect("write png");
    eprintln!("frame → {}", path.display());

    eprintln!("settled cold-load frame census:");
    census(img, 12);

    // The gate's expectations, measured the way the gate measures them
    // (ONSCREEN_* = authored palette after the post pipeline; re-baseline
    // these from the census above when the post pipeline changes).
    for (name, rgb, tol) in [
        ("LAND", session::ONSCREEN_LAND_SRGB, 8u8),
        ("ROAD_INNER", session::ONSCREEN_ROAD_INNER_SRGB, 10),
        ("ROAD_MAJOR", session::ONSCREEN_ROAD_MAJOR_SRGB, 14),
        ("CLEAR(blank)", session::ONSCREEN_CLEAR_SRGB, 6),
    ] {
        let frac = session::fraction_near(img, rgb, tol);
        eprintln!("gate {name:>12} {rgb:?} ±{tol}: {:.4}", frac);
    }
}

/// Print the `n` most frequent exact colours in the frame.
fn census(img: &image::RgbaImage, n: usize) {
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for p in img.pixels() {
        *counts.entry([p.0[0], p.0[1], p.0[2]]).or_default() += 1;
    }
    let total = (img.width() * img.height()) as f64;
    let mut top: Vec<_> = counts.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    for (rgb, count) in top.iter().take(n) {
        eprintln!(
            "  {:>3},{:>3},{:>3}  {:>6.2}%",
            rgb[0],
            rgb[1],
            rgb[2],
            100.0 * *count as f64 / total
        );
    }
}
