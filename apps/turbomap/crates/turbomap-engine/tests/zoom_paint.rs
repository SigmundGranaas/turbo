//! Phase 3: zoom-curve colour evaluated on the GPU per frame. A line whose
//! colour is a zoom curve must change colour as the camera zooms — without
//! re-tessellating — because the engine pushes the evaluated colour as a
//! per-layer shader override, not by rebuilding geometry.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use image::RgbaImage;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{headless, render_to_image, Gpu, TARGET_FORMAT};
use turbomap_scene::style::ZoomStop;
use turbomap_scene::{Color, Filter, Layer, Paint, Scene, SourceDef};

const RED: Color = Color {
    r: 220,
    g: 30,
    b: 60,
    a: 255,
};
const BLUE: Color = Color {
    r: 30,
    g: 60,
    b: 220,
    a: 255,
};

fn zoom_color_scene() -> Scene {
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
        "route".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[[5.05,60.30],[5.32,60.39],[5.58,60.48]]}"#
                .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    // Colour ramps red→blue across [9.0, 9.5] — entirely within tile
    // level 9, so changing zoom in that band cannot trigger a re-fetch.
    scene.layers.push(Layer::Line {
        id: "route".to_string(),
        source: "route".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Zoom {
            stops: vec![
                ZoomStop { zoom: 9.0, value: RED },
                ZoomStop { zoom: 9.5, value: BLUE },
            ],
        },
        width: Paint::Const(6.0),
        dash_array: None,
    });
    scene
}

/// Average RGB over the line pixels (those far from the parchment base).
fn avg_line_color(img: &RgbaImage) -> (f64, f64, f64) {
    let bg = [226i32, 218, 198];
    let (mut r, mut g, mut b, mut n) = (0f64, 0f64, 0f64, 0u64);
    for px in img.pixels() {
        let d = (0..3).map(|i| (px.0[i] as i32 - bg[i]).abs()).max().unwrap();
        if d > 60 {
            r += px.0[0] as f64;
            g += px.0[1] as f64;
            b += px.0[2] as f64;
            n += 1;
        }
    }
    assert!(n > 50, "too few line pixels found ({n}) — is the route visible?");
    (r / n as f64, g / n as f64, b / n as f64)
}

fn camera(zoom: f64) -> CameraState {
    CameraState {
        center: LatLng::new(60.39, 5.32),
        zoom,
        pitch_deg: 0.0,
        bearing_deg: 0.0,
    }
}

fn engine(gpu: &Gpu, zoom: f64) -> TurbomapEngine {
    TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (512, 384),
        camera(zoom),
        MapOptions {
            fade_in_secs: 0.0,
            ..Default::default()
        },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine")
}

#[test]
fn zoom_curve_colour_updates_without_retessellation() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let mut e = engine(&gpu, 9.0);
    e.apply(zoom_color_scene());
    let loaded = e.pump_tiles();
    assert!(loaded.vector_tiles > 0, "route should load: {loaded:?}");

    // At zoom 9.0 the curve is fully RED.
    let img = render_to_image(&gpu, 512, 384, |enc, v| e.render(enc, v));
    e.after_submit();
    let (r1, g1, b1) = avg_line_color(&img);
    assert!(
        r1 > b1 + 30.0,
        "at zoom 9.0 the route must be red-dominant, got rgb=({r1:.0},{g1:.0},{b1:.0})"
    );

    // Zoom to 9.49 — same tile level, so NO new geometry. Only the GPU
    // colour override changes (the curve is now ~blue).
    e.set_camera(camera(9.49));
    let after = e.pump_tiles();
    assert_eq!(
        after.vector_tiles, 0,
        "a zoom change within one tile level must not re-tessellate, got {after:?}"
    );

    let img = render_to_image(&gpu, 512, 384, |enc, v| e.render(enc, v));
    e.after_submit();
    let (r2, g2, b2) = avg_line_color(&img);
    assert!(
        b2 > r2 + 30.0,
        "at zoom 9.49 the route must have followed the curve to blue, got rgb=({r2:.0},{g2:.0},{b2:.0})"
    );
    let _ = (g1, g2);
}
