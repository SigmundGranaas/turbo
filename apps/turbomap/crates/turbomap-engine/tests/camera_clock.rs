//! Camera animations start on the clock the map is TICKED with, not the wall
//! clock. An embedder drives `tick` with a clock of its own (a compositor's
//! frame time, a headless runner's virtual frames); here it runs an hour
//! ahead of the wall. An ease started on the wall would be an hour finished
//! at its first tick, and a zoom fling would land in one step.
//!
//! GPU-gated (engine construction needs an adapter; a software one suffices).
#![cfg(feature = "gpu-tests")]

mod common;

use std::time::{Duration, Instant};

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{headless, TARGET_FORMAT};

fn engine() -> Option<TurbomapEngine> {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return None;
    };
    Some(
        TurbomapEngine::new(
            gpu.device.clone(),
            turbomap_core::upload::UploadQueue::new(gpu.queue.get_timestamp_period()),
            TARGET_FORMAT,
            (1024, 768),
            CameraState::new(LatLng::new(0.0, 0.0), 4.0),
            MapOptions { fade_in_secs: 0.0, ..Default::default() },
            Box::new(SyntheticResolver),
            std::sync::Arc::new(turbomap_core::work::ThreadPool::new("turbomap-decode", std::num::NonZeroUsize::new(2).unwrap())),
        )
        .expect("construct TurbomapEngine"),
    )
}

#[test]
fn an_ease_runs_on_the_clock_the_map_is_ticked_with() {
    let Some(mut engine) = engine() else { return };
    let clock = Instant::now() + Duration::from_secs(3600);
    engine.map_mut().tick(clock);
    engine.ease_to(CameraState::new(LatLng::new(0.0, 10.0), 4.0), Duration::from_secs(1));
    assert!(engine.map_mut().tick(clock + Duration::from_millis(500)), "the ease ended at its midpoint");
    let lng = engine.camera().center.lng;
    assert!(lng > 0.5 && lng < 9.5, "at its midpoint the ease is at lng {lng}, not between its ends");
    assert!(!engine.map_mut().tick(clock + Duration::from_millis(1100)), "the ease outlived its duration");
    assert!((engine.camera().center.lng - 10.0).abs() < 1e-9);
}

#[test]
fn a_zoom_fling_runs_on_the_clock_the_map_is_ticked_with() {
    let Some(mut engine) = engine() else { return };
    let clock = Instant::now() + Duration::from_secs(3600);
    engine.map_mut().tick(clock);
    engine.zoom_fling(2.0, (512.0, 384.0));
    assert!(engine.map_mut().tick(clock + Duration::from_millis(16)), "the zoom fling ended at its first step");
    let zoom = engine.camera().zoom;
    // 2 levels/s × 0.25 s decay: half a level in all; one 16 ms step is a sliver.
    assert!(zoom > 4.0 && zoom < 4.1, "after one step the zoom is {zoom}");
}
