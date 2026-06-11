//! Inertial fling wiring: a release velocity must glide the camera and then
//! settle, driven by `tick_now`. The decay *physics* is unit-tested in
//! `turbomap-core::camera`; this proves the engine→Map plumbing.
//!
//! GPU-gated (engine construction needs an adapter; a software one suffices).
#![cfg(feature = "gpu-tests")]

mod common;

use std::time::Duration;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{headless, TARGET_FORMAT};

#[test]
fn fling_glides_the_camera_then_settles() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (1024, 768),
        CameraState::new(LatLng::new(0.0, 0.0), 4.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    let start_lng = engine.camera().center.lng;

    // Release a rightward flick. The map glides; `tick_now` reports it's live.
    engine.fling((1500.0, 0.0));
    assert!(engine.tick_now(), "fling is animating right after release");

    std::thread::sleep(Duration::from_millis(80));
    engine.tick_now();
    let mid_lng = engine.camera().center.lng;
    assert!(mid_lng != start_lng, "the fling moved the camera");

    // Drive to completion — it must stop on its own (decelerating glide).
    let mut ticks = 0;
    while engine.tick_now() && ticks < 400 {
        std::thread::sleep(Duration::from_millis(8));
        ticks += 1;
    }
    assert!(ticks < 400, "fling settled within a bounded time");
    assert!(!engine.tick_now(), "a settled fling is no longer animating");

    // Once settled the camera holds still — momentum doesn't keep drifting.
    let settled = engine.camera().center.lng;
    std::thread::sleep(Duration::from_millis(20));
    engine.tick_now();
    assert!(
        (engine.camera().center.lng - settled).abs() < 1e-9,
        "settled camera stays put"
    );

    // A fresh pan cancels any momentum (sanity: starting a fling then setting
    // the camera clears it).
    engine.fling((2000.0, 0.0));
    engine.set_camera(CameraState::new(LatLng::new(0.0, 0.0), 4.0));
    assert!(!engine.tick_now(), "set_camera cancels the fling");
}
