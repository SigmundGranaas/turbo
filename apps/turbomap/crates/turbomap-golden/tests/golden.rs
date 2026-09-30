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

/// Tile uploads through a host's byte budget (turbomap_core::upload), at
/// 1 MiB a frame — Edits' per-view inline budget. It holds the first frame's
/// essential writes (~296 KB of placeholders, measured) and a fraction of a
/// scene's tile data (raster-parchment: ~9.4 MB in 243 mip writes), so the
/// tiles are spread over several frames; the final frame must be exactly the
/// golden — a budget changes when tiles appear, never what they look like.
fn run_budgeted(name: &str, cfg: GoldenConfig) -> Option<turbomap_golden::BudgetedReplay> {
    let gpu = gpu_or_skip(name)?;
    let trace = load_trace(name);
    let r = turbomap_golden::replay_with_budget(&trace, &gpu, 1 << 20, 200);
    eprintln!("[budget] {name}: settled after {} frames", r.frames);
    assert!(
        r.frames > 1,
        "the budget never deferred a tile in '{name}', so this measured nothing"
    );
    assert_golden(name, &r.last, cfg);
    Some(r)
}

#[test]
fn golden_raster_parchment_through_a_small_upload_budget() {
    // A flat colour: a fallback ancestor looks like the final frame, so only
    // convergence is asserted here; the hillshade test below shows deferral.
    run_budgeted(
        "raster-parchment",
        GoldenConfig {
            max_channel_diff: 2,
            max_outlier_frac: 0.001,
        },
    );
}

/// A gradient, where an ancestor and the final tile differ: the first
/// budgeted frame is not the final picture. So a tile whose upload had not
/// been handed over was not drawn — its ancestor was — and it was drawn once
/// it had been.
#[test]
fn golden_hillshade_bergen_through_a_small_upload_budget() {
    let Some(r) = run_budgeted(
        "hillshade-bergen",
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    ) else {
        return;
    };
    assert_ne!(
        r.first.as_raw(),
        r.last.as_raw(),
        "the first budgeted frame already showed the final picture"
    );
}

/// The same scenes into a half-float target (an HDR-capable plane's
/// format): read back linear, encoded to sRGB, and held to the SAME goldens —
/// so the renderer's look does not depend on its target doing the encode.
/// One code step of tolerance more than the sRGB run: the encode happens on
/// the CPU here, in the texture unit there.
#[test]
fn golden_raster_parchment_on_a_half_float_target() {
    let name = "raster-parchment";
    let Some(gpu) = gpu_or_skip(name) else { return };
    let img = turbomap_golden::replay_as(&load_trace(name), &gpu, wgpu::TextureFormat::Rgba16Float);
    assert_golden(
        name,
        &img,
        GoldenConfig {
            max_channel_diff: 3,
            max_outlier_frac: 0.001,
        },
    );
}

#[test]
fn golden_hillshade_bergen_on_a_half_float_target() {
    let name = "hillshade-bergen";
    let Some(gpu) = gpu_or_skip(name) else { return };
    let img = turbomap_golden::replay_as(&load_trace(name), &gpu, wgpu::TextureFormat::Rgba16Float);
    assert_golden(
        name,
        &img,
        GoldenConfig {
            max_channel_diff: 7,
            max_outlier_frac: 0.02,
        },
    );
}
