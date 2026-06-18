//! Phase 2 keystone: the *scene-driven* render path must produce the
//! same pixels as the *imperative* path. We render a raster+hillshade
//! Scene through the engine and compare against the committed
//! `hillshade-bergen` reference — the exact image the imperative golden
//! trace produces. Equality proves `apply(Scene)` faithfully drives the
//! existing pipelines.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{assert_golden, headless, render_to_image, GoldenConfig, TARGET_FORMAT};
use turbomap_scene::{DemEncoding, Layer, Paint, Scene, SourceDef};

fn hillshade_scene() -> Scene {
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
        "dem".to_string(),
        SourceDef::DemXyz {
            tiles: vec!["https://example.test/dem/{z}/{x}/{y}.png".to_string()],
            encoding: DemEncoding::MapboxRgb,
            min_zoom: 0,
            max_zoom: 22,
            halo: 0,
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Hillshade {
        id: "hillshade".to_string(),
        source: "dem".to_string(),
        exaggeration: 1.5,
    });
    scene
}

#[test]
fn scene_path_matches_imperative_hillshade_reference() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let (width, height) = (512, 384);
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), 9.0),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(hillshade_scene());
    let stats = engine.pump_tiles();
    assert!(
        stats.raster_tiles > 0 && stats.terrain_tiles > 0,
        "expected raster + terrain tiles to drain, got {stats:?}"
    );

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // Same tolerance as the imperative hillshade golden.
    assert_golden(
        "hillshade-bergen",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}
