//! Scene-declared route tubes (plan P5.2): a `Layer::Tube` over a GeoJSON
//! LineString installs the overlays subsystem's raised 3D tube, and removing
//! the layer removes the mesh. This is THE route/track path — the imperative
//! `set_route_tube` side-door is gone from every host surface.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{headless, Gpu, TARGET_FORMAT};
use turbomap_scene::{Color, Layer, Scene, SourceDef};

fn scene_with_tube() -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "t_route".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[[5.1,60.3],[5.32,60.39],[5.55,60.48]]}"#
                .to_string(),
        },
    );
    scene.layers.push(Layer::Tube {
        id: "route".to_string(),
        source: "t_route".to_string(),
        color: Color::rgba(143, 76, 56, 255),
        radius_px: 8.0,
    });
    scene
}

fn engine(gpu: &Gpu) -> TurbomapEngine {
    TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (512, 384),
        CameraState::new(LatLng::new(60.39, 5.32), 9.0),
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
        Some(g) => Some(g),
        None => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but no wgpu adapter available");
            }
            eprintln!("SKIP: no wgpu adapter available");
            None
        }
    }
}

/// The overlays subsystem's live tube count, from the inspect surface (S7).
fn route_tubes(e: &TurbomapEngine) -> u64 {
    let inspect: serde_json::Value = serde_json::from_str(&e.inspect_json()).expect("inspect json");
    inspect["overlays"]["state"]["route_tubes"]
        .as_u64()
        .expect("overlays inspect should report route_tubes")
}

#[test]
fn a_scene_declared_tube_installs_and_a_scene_without_it_removes() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut e = engine(&gpu);

    e.apply(scene_with_tube());
    assert!(
        e.unsupported_layers().is_empty(),
        "tube must be renderable: {:?}",
        e.unsupported_layers()
    );
    assert_eq!(route_tubes(&e), 1, "the declared tube should be installed");

    // Removing the layer from the scene removes the mesh — content has one
    // owner, the scene diff.
    e.apply(Scene::new());
    assert_eq!(route_tubes(&e), 0, "an undeclared tube must be removed");
}
