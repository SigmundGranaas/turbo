//! The E2 gates: the cloud overlay is a SIMULATION — it ticks from the
//! frame's Environment (clock, sun, wind) instead of waiting for a host to
//! scrub its clock.
//!
//! - **Deterministic replay:** same `(fields, time, seed)` ⇒ identical
//!   frame. The clock is pinned via `set_time_override`, radar frames are
//!   ingested identically, and two renders must be byte-identical (plus a
//!   golden pinning the look).
//! - **The sim is alive:** advancing the pinned clock moves the weather
//!   (drift + radar advection) with zero host nudges, and an active sim
//!   reports as animation so render-on-demand hosts keep pumping frames.
//! - **Coherent lighting:** the clouds shade under the ONE Environment
//!   sun — isolate the cloud contribution (pass masking) at two sun
//!   positions and it must differ.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use image::RgbaImage;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{assert_golden, headless, render_to_image, GoldenConfig, Gpu, TARGET_FORMAT};
use turbomap_scene::{CloudsDef, Layer, LightingDef, Paint, Scene, SourceDef};

const GRID: u32 = 96;

/// A drifting frontal band + a broad mass, like the blocky raster MET
/// serves; `phase` slides the front so two frames genuinely differ.
/// Returns `(precip, coverage)` grids for `ingest_field`.
fn synthetic_radar(phase: f32) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (GRID, GRID);
    let mut precip = vec![0u8; (w * h) as usize];
    let mut coverage = vec![0u8; (w * h) as usize];
    let front = -0.2 + phase * 1.3;
    for y in 0..h {
        for x in 0..w {
            let nx = (x as f32 + 0.5) / w as f32;
            let ny = (y as f32 + 0.5) / h as f32;
            let band = (-((nx + (ny - 0.5) * 0.3 - front).powi(2)) / (2.0 * 0.10 * 0.10)).exp();
            let mass = (-((nx - 0.7).powi(2) + (ny - 0.35).powi(2)) / (2.0 * 0.16 * 0.16)).exp();
            let cov = (band * 0.85 + mass * 0.8).clamp(0.0, 1.0);
            let pr = (band * band * 0.7).clamp(0.0, 1.0);
            let i = (y * w + x) as usize;
            coverage[i] = (cov * 255.0) as u8;
            precip[i] = (pr * 255.0) as u8;
        }
    }
    (precip, coverage)
}

fn storm_scene(sun_altitude_deg: f32) -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "base".to_string(),
        SourceDef::RasterXyz {
            tiles: vec!["https://example.test/{z}/{x}/{y}.png".to_string()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 22,
            attribution: None,
        },
    );
    scene.sources.insert(
        "radar".to_string(),
        SourceDef::Field2D {
            bounds: [4.0, 59.0, 7.0, 61.5],
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.environment.clouds = Some(CloudsDef {
        source: "radar".to_string(),
        grid: [GRID, GRID],
        visible: true,
        animate: true,
    });
    scene.environment.lighting = LightingDef::Fixed {
        azimuth_deg: 225.0,
        altitude_deg: sun_altitude_deg,
    };
    scene
}

fn engine(gpu: &Gpu, size: (u32, u32)) -> TurbomapEngine {
    TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        size,
        CameraState::new(LatLng::new(60.39, 5.32), 8.0),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine")
}

fn gpu_or_skip() -> Option<Gpu> {
    match headless() {
        Some(gpu) => Some(gpu),
        None => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but no wgpu adapter available");
            }
            eprintln!("SKIP: no wgpu adapter available");
            None
        }
    }
}

fn ingest_storm(engine: &mut TurbomapEngine) {
    let (pa, ca) = synthetic_radar(0.15);
    let (pb, cb) = synthetic_radar(0.55);
    assert!(engine.ingest_field("radar", 0, GRID, GRID, &pa, &ca));
    assert!(engine.ingest_field("radar", 1, GRID, GRID, &pb, &cb));
}

fn diff_fraction(a: &RgbaImage, b: &RgbaImage, tol: u8) -> f64 {
    let n = (a.width() * a.height()) as f64;
    let mut d = 0u64;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        if (0..3).any(|c| pa.0[c].abs_diff(pb.0[c]) > tol) {
            d += 1;
        }
    }
    d as f64 / n
}

#[test]
fn storm_sim_replays_deterministically_and_advances() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (width, height) = (512, 384);
    let mut engine = engine(&gpu, (width, height));

    // Pin the clock BEFORE ingest: the radar advection stamp reads the same
    // frame clock, so replay is a pure function of (fields, time).
    engine.set_time_override(Some(100.0));
    engine.apply(storm_scene(35.0));
    engine.pump_tiles();
    ingest_storm(&mut engine);
    assert!(
        engine.is_animating(),
        "an active cloud sim must report as animation (the E2 redraw contract)"
    );

    engine.set_time_override(Some(112.0));
    let img_a = render_to_image(&gpu, width, height, |e, v| engine.render(e, v));
    engine.after_submit();
    let img_a2 = render_to_image(&gpu, width, height, |e, v| engine.render(e, v));
    engine.after_submit();
    assert_eq!(
        img_a.as_raw(),
        img_a2.as_raw(),
        "same (fields, time) must replay the exact frame"
    );

    // Advance the pinned clock: drift + radar advection move the weather
    // with no host scrubbing.
    engine.set_time_override(Some(124.0));
    let img_b = render_to_image(&gpu, width, height, |e, v| engine.render(e, v));
    engine.after_submit();
    let moved = diff_fraction(&img_a, &img_b, 4);
    assert!(
        moved > 0.02,
        "advancing the clock must move the weather (diff {moved:.4})"
    );

    assert_golden(
        "clouds-sim-storm",
        &img_a,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}

#[test]
fn cloud_contribution_follows_the_environment_sun() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (width, height) = (384, 288);

    // The cloud CONTRIBUTION (frame minus frame-without-clouds, via pass
    // masking) at a high vs a grazing sun must differ — the overlay shades
    // under the one Environment sun, not a private constant.
    let contribution = |altitude_deg: f32| -> (RgbaImage, RgbaImage) {
        let mut engine = engine(&gpu, (width, height));
        engine.set_time_override(Some(100.0));
        engine.apply(storm_scene(altitude_deg));
        engine.pump_tiles();
        ingest_storm(&mut engine);
        engine.set_time_override(Some(110.0));
        let on = render_to_image(&gpu, width, height, |e, v| engine.render(e, v));
        engine.after_submit();
        engine.map_mut().set_pass_enabled("clouds", false);
        let off = render_to_image(&gpu, width, height, |e, v| engine.render(e, v));
        engine.after_submit();
        (on, off)
    };

    let (on_noon, off_noon) = contribution(80.0);
    let (on_low, off_low) = contribution(8.0);
    // Same scene sans clouds — the base frames are flat 2D, so the sun
    // change alone must not move them (guards the isolation).
    assert!(
        diff_fraction(&off_noon, &off_low, 4) < 0.001,
        "the flat base frame must not respond to the sun"
    );
    // But the cloud contribution must.
    let cloud_response = diff_fraction(&on_noon, &on_low, 4);
    assert!(
        cloud_response > 0.02,
        "cloud shading must respond to the Environment sun (diff {cloud_response:.4})"
    );
}
