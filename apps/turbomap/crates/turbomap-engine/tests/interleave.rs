//! Plan P6.5 — compositing honesty, pixel truth. The Scene IR's layer order
//! is the composited order for EVERY layer kind: this scene declares a fill
//! ABOVE a circle layer ABOVE a tube ABOVE a line, and the golden pins that
//! the fill occludes the circles/tube/line under it while the circle outside
//! the fill stays visible — "circle below a fill", the exact ordering C3's
//! retired exception called inexpressible.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{assert_golden, headless, render_to_image, GoldenConfig, TARGET_FORMAT};
use turbomap_scene::{Color, Filter, Layer, Paint, Scene, SourceDef};

/// Raster base, then content interleaved across kinds:
/// line → tube → circle → fill (fill on top occludes all three where they
/// overlap; the westernmost circle sits outside the fill and stays visible).
fn interleaved_scene() -> Scene {
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
    // A west→east track through the Bergen frame; the tube and the dots ride it.
    scene.sources.insert(
        "track".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[
                [5.06,60.32],[5.20,60.36],[5.32,60.39],[5.44,60.43],[5.58,60.46]
            ]}"#
            .to_string(),
        },
    );
    scene.sources.insert(
        "stops".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"MultiPoint","coordinates":[
                [5.10,60.33],[5.32,60.39],[5.47,60.44]
            ]}"#
            .to_string(),
        },
    );
    // An opaque zone covering the middle of the frame — everything declared
    // BELOW it (line, tube, the two eastern dots) must vanish under it.
    scene.sources.insert(
        "zone".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"Polygon","coordinates":[[
                [5.24,60.33],[5.56,60.33],[5.56,60.47],[5.24,60.47],[5.24,60.33]
            ]]}"#
                .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Line {
        id: "trail".to_string(),
        source: "track".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(20, 90, 200)),
        width: Paint::Const(4.0),
        dash_array: None,
    });
    scene.layers.push(Layer::Tube {
        id: "route-3d".to_string(),
        source: "track".to_string(),
        color: Color::rgba(143, 76, 56, 255),
        radius_px: 7.0,
    });
    scene.layers.push(Layer::Circle {
        id: "stops".to_string(),
        source: "stops".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(255, 200, 0)),
        radius: Paint::Const(10.0),
    });
    scene.layers.push(Layer::Fill {
        id: "zone".to_string(),
        source: "zone".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgba(40, 140, 90, 255)),
        opacity: Paint::Const(1.0),
    });
    scene
}

#[test]
fn ir_order_is_the_composited_order_across_kinds() {
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

    engine.apply(interleaved_scene());
    let stats = engine.pump_tiles();
    assert!(stats.vector_tiles > 0, "content tiles expected: {stats:?}");
    assert!(
        engine.unsupported_layers().is_empty(),
        "raster + line + tube + circle + fill must all be supported: {:?}",
        engine.unsupported_layers()
    );
    // The circle layer's markers are in the one store (hit-test source)…
    assert_eq!(engine.map().markers().len(), 3, "expected 3 stop markers");
    // …and a tap on the occluded middle stop still answers (P6.4 semantics
    // are draw-order independent).
    let centre = engine
        .project(turbomap_scene::LatLng::new(60.39, 5.32))
        .expect("stop projects");
    assert!(
        engine
            .hit_test(centre, 8.0)
            .iter()
            .any(|h| h.layer_id == "stops"),
        "the marker store answers hits regardless of compositing order"
    );

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // Pixel assertions BEFORE the golden so a compositing regression names
    // the violated ordering directly. Project the geo anchors instead of
    // hard-coding pixels.
    let px = |lat: f64, lng: f64| {
        let p = engine
            .project(turbomap_scene::LatLng::new(lat, lng))
            .expect("anchor projects");
        (p.x.round() as u32, p.y.round() as u32)
    };
    let rgba = |x: u32, y: u32| image.get_pixel(x, y).0;
    // The western stop lies outside the fill: the circle (declared above
    // line+tube) must be visible — yellow dominant.
    let (wx, wy) = px(60.33, 5.10);
    let w = rgba(wx, wy);
    assert!(
        w[0] > 180 && w[1] > 140 && w[2] < 120,
        "western stop should show the yellow circle, got {w:?}"
    );
    // The middle stop lies inside the fill declared ABOVE the circle layer:
    // the fill must occlude it — green dominant, no yellow.
    let (mx, my) = px(60.39, 5.32);
    let m = rgba(mx, my);
    assert!(
        m[1] > m[0] && m[0] < 120,
        "middle stop must be occluded by the fill above it, got {m:?}"
    );

    assert_golden(
        "interleave-content",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}
