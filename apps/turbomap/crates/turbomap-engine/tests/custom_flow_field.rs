//! The D4 gate: `Layer::Custom` is REAL. A scene declaring the built-in
//! `flow-field` kind renders through the frame graph as a phase-bound node
//! (`custom:flow`), deterministically under a pinned clock (the same one
//! Rust+WGSL impl compiles into the wasm web host — see the wasm32 build
//! lane); an unknown kind degrades to the unsupported report instead of
//! guessing.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{assert_golden, headless, render_to_image, GoldenConfig, Gpu, TARGET_FORMAT};
use turbomap_scene::{Layer, Paint, Scene, SourceDef};

fn flow_scene(kind: &str) -> Scene {
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
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Custom {
        id: "flow".to_string(),
        kind: kind.to_string(),
    });
    scene
}

fn engine(gpu: &Gpu, size: (u32, u32)) -> TurbomapEngine {
    TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        size,
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

#[test]
fn flow_field_custom_layer_renders_deterministically() {
    let Some(gpu) = gpu_or_skip() else { return };
    let (width, height) = (512, 384);
    let mut engine = engine(&gpu, (width, height));

    assert!(
        engine.capabilities().custom_layers,
        "the engine binds custom layers since D4"
    );
    engine.apply(flow_scene("flow-field"));
    engine.pump_tiles();
    assert!(
        engine.unsupported_layers().is_empty(),
        "flow-field is a registered kind, unsupported = {:?}",
        engine.unsupported_layers()
    );

    // Pin the animation clock: the golden must not depend on wall time.
    engine.set_time_override(Some(120.0));
    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // The custom layer is a named frame-graph node in the pass report.
    let labels: Vec<String> = engine
        .map()
        .last_frame_metrics()
        .passes
        .iter()
        .map(|p| p.label.clone())
        .collect();
    assert!(
        labels.iter().any(|l| l == "custom:flow"),
        "custom layer must appear as its own graph node, passes = {labels:?}"
    );

    // Determinism under a pinned clock: an identical re-render is
    // byte-identical (the streak field is a pure function of world + time).
    let again = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();
    assert_eq!(
        image.as_raw(),
        again.as_raw(),
        "pinned-clock renders must be byte-identical"
    );

    assert_golden(
        "custom-flow-field",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}

#[test]
fn unknown_custom_kind_degrades_to_unsupported() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut engine = engine(&gpu, (256, 256));
    engine.apply(flow_scene("no-such-kind"));
    assert_eq!(
        engine.unsupported_layers(),
        &["flow".to_string()],
        "an unregistered kind must be reported, not guessed at"
    );
    // The frame still renders (the base layer alone) without panicking.
    engine.pump_tiles();
    let _ = render_to_image(&gpu, 256, 256, |enc, view| engine.render(enc, view));
    engine.after_submit();
}
