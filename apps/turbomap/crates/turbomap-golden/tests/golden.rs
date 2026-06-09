//! GPU-backed golden render tests. Behind the `gpu-tests` feature so the
//! default workspace test lane (no GPU) skips them at compile time.
#![cfg(feature = "gpu-tests")]

use std::path::PathBuf;

use turbomap_golden::{assert_golden, headless, replay, GoldenConfig, Gpu, Trace};

fn load_trace(name: &str) -> Trace {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("traces")
        .join(format!("{name}.json"));
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read trace {}: {e}", path.display()));
    Trace::from_json(&json).unwrap_or_else(|e| panic!("parse trace {name}: {e}"))
}

/// Acquire a headless context, or skip — unless `REQUIRE_GPU=1` (set in
/// the CI golden lane), where a missing adapter is a hard failure so a
/// broken Lavapipe install can't silently pass the suite.
fn gpu_or_skip(name: &str) -> Option<Gpu> {
    match headless() {
        Some(gpu) => {
            eprintln!("golden '{name}' on adapter: {}", gpu.adapter_name);
            Some(gpu)
        }
        None => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but no wgpu adapter available for golden '{name}'");
            }
            eprintln!("SKIP golden '{name}': no wgpu adapter available");
            None
        }
    }
}

fn run(name: &str, cfg: GoldenConfig) {
    let Some(gpu) = gpu_or_skip(name) else {
        return;
    };
    let trace = load_trace(name);
    let img = replay(&trace, &gpu);
    assert_golden(name, &img, cfg);
}

#[test]
fn golden_raster_parchment() {
    // Flat colour — essentially driver-independent, so hold it tight.
    run(
        "raster-parchment",
        GoldenConfig {
            max_channel_diff: 2,
            max_outlier_frac: 0.001,
        },
    );
}

#[test]
fn golden_hillshade_bergen() {
    // Gradient shading drifts a little across llvmpipe versions; allow a
    // small outlier budget so dev-box vs CI driver skew doesn't flap.
    run(
        "hillshade-bergen",
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}
