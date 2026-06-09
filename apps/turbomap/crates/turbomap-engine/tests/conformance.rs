//! The wgpu `TurbomapEngine` must satisfy the *same* `MapEngine`
//! conformance suite as the reference `ModelEngine`. That is the whole
//! point of the contract: a host can hold either behind it.
//!
//! GPU-gated (engine construction needs an adapter); a software adapter
//! is sufficient.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{headless, Gpu, TARGET_FORMAT};
use turbomap_scene::conformance;

fn engine(gpu: &Gpu) -> Box<dyn MapEngine> {
    Box::new(
        TurbomapEngine::new(
            gpu.device.clone(),
            gpu.queue.clone(),
            TARGET_FORMAT,
            (1024, 768),
            CameraState::new(LatLng::new(0.0, 0.0), 0.0),
            MapOptions {
                fade_in_secs: 0.0,
                ..Default::default()
            },
            Box::new(SyntheticResolver),
        )
        .expect("construct TurbomapEngine"),
    )
}

#[test]
fn turbomap_engine_satisfies_contract() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };
    eprintln!("conformance on adapter: {}", gpu.adapter_name);
    // One device, a fresh engine (fresh scene state) per check.
    conformance::run_all(&|| engine(&gpu));
}
