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
        turbomap_core::upload::UploadQueue::new(gpu.queue.get_timestamp_period()),
        TARGET_FORMAT,
        (1024, 768),
        CameraState::new(LatLng::new(0.0, 0.0), 4.0),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(SyntheticResolver),
        std::sync::Arc::new(turbomap_core::work::ThreadPool::new(
            "turbomap-decode",
            std::num::NonZeroUsize::new(2).unwrap(),
        )),
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

/// `fit_bounds` jumps and `fly_to_bounds` eases to the same pose: the one
/// `Camera::fitted_to` gives for the engine's CURRENT size (after a
/// resize), whose screen-space guarantees are unit-tested in core.
#[test]
fn fit_and_fly_to_bounds_land_on_the_fit_for_the_current_viewport() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };
    let make = || {
        TurbomapEngine::new(
            gpu.device.clone(),
            turbomap_core::upload::UploadQueue::new(gpu.queue.get_timestamp_period()),
            TARGET_FORMAT,
            (1024, 768),
            CameraState::new(LatLng::new(0.0, 0.0), 2.0),
            MapOptions {
                fade_in_secs: 0.0,
                ..Default::default()
            },
            Box::new(SyntheticResolver),
            std::sync::Arc::new(turbomap_core::work::ThreadPool::new(
                "turbomap-decode",
                std::num::NonZeroUsize::new(2).unwrap(),
            )),
        )
        .expect("construct TurbomapEngine")
    };
    let ll = turbomap_core::LatLng::new;
    let bergen = turbomap_core::LatLngBounds::new(ll(60.30, 5.20), ll(60.45, 5.45)).unwrap();
    let expected = turbomap_core::Camera::new(ll(0.0, 0.0), 2.0)
        .fitted_to(bergen, (600.0, 400.0), 24.0)
        .unwrap();
    let close = |c: CameraState| {
        (c.center.lat - expected.center.lat).abs() < 1e-7
            && (c.center.lng - expected.center.lng).abs() < 1e-7
            && (c.zoom - expected.zoom).abs() < 1e-7
            && c.pitch_deg == 0.0
    };

    let mut jumped = make();
    jumped.resize(600, 400);
    jumped.fit_bounds(bergen, 24.0).unwrap();
    assert!(
        close(jumped.camera()),
        "jump: {:?} vs {expected:?}",
        jumped.camera()
    );

    let mut flown = make();
    flown.resize(600, 400);
    flown
        .fly_to_bounds(bergen, 24.0, Duration::from_millis(120))
        .unwrap();
    assert!(flown.tick_now(), "the flight animates");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while flown.tick_now() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(8));
    }
    assert!(
        close(flown.camera()),
        "flight end: {:?} vs {expected:?}",
        flown.camera()
    );
}
