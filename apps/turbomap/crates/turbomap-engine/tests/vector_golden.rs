//! Golden test for the GeoJSON line path: a route drawn over a raster base
//! through the scene-driven engine. Proves `Layer::Line` over a GeoJSON
//! source tessellates and renders, and pins the result against regressions.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{assert_golden, headless, render_to_image, GoldenConfig, TARGET_FORMAT};
use turbomap_scene::{Color, Filter, Layer, Paint, Scene, SourceDef};

fn route_scene() -> Scene {
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
    // A route across the Bergen area.
    scene.sources.insert(
        "route".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[
                [5.10,60.30],[5.22,60.34],[5.32,60.39],[5.40,60.45],[5.55,60.48]
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Line {
        id: "route".to_string(),
        source: "route".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(220, 30, 60)),
        width: Paint::Const(5.0),
    });
    scene
}

/// raster base + route line + measure-point circles — the overlay set the
/// app actually draws, all through the scene path.
fn overlay_scene() -> Scene {
    let mut scene = route_scene();
    scene.sources.insert(
        "points".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"MultiPoint","coordinates":[
                [5.22,60.34],[5.32,60.39],[5.40,60.45]
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Circle {
        id: "measure-points".to_string(),
        source: "points".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(255, 200, 0)),
        radius: Paint::Const(9.0),
    });
    scene
}

#[test]
fn full_overlay_set_renders() {
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

    engine.apply(overlay_scene());
    engine.pump_tiles();
    assert!(
        engine.unsupported_layers().is_empty(),
        "raster + line + circle should all be supported, unsupported = {:?}",
        engine.unsupported_layers()
    );
    // Three measure points → three markers.
    assert_eq!(engine.map().markers().len(), 3, "expected 3 circle markers");

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    assert_golden(
        "overlays-bergen",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}

#[test]
fn geojson_fill_renders_over_raster() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let (width, height) = (512, 384);
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
        "area".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"Polygon","coordinates":[[
                [5.20,60.34],[5.44,60.34],[5.44,60.46],[5.20,60.46],[5.20,60.34]
            ]]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "area".to_string(),
        source: "area".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgba(40, 120, 220, 150)),
        opacity: Paint::Const(1.0),
    });

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

    engine.apply(scene);
    let stats = engine.pump_tiles();
    assert!(stats.vector_tiles > 0, "expected fill tiles, got {stats:?}");
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    assert_golden(
        "fill-bergen",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}

#[test]
fn geojson_line_renders_over_raster() {
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

    engine.apply(route_scene());
    let stats = engine.pump_tiles();
    assert!(
        stats.raster_tiles > 0 && stats.vector_tiles > 0,
        "expected raster + vector tiles to drain, got {stats:?}"
    );
    assert!(
        engine.unsupported_layers().is_empty(),
        "line layer should be supported, unsupported = {:?}",
        engine.unsupported_layers()
    );

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    assert_golden(
        "geojson-line-bergen",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}
