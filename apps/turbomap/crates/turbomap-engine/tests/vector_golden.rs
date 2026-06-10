//! Golden test for the GeoJSON line path: a route drawn over a raster base
//! through the scene-driven engine. Proves `Layer::Line` over a GeoJSON
//! source tessellates and renders, and pins the result against regressions.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{assert_golden, headless, render_to_image, GoldenConfig, TARGET_FORMAT};
use turbomap_scene::{
    Color, Filter, FilterValue, Layer, MatchCase, Paint, Scene, SourceDef, SymbolPlacement,
};

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
fn line_width_is_pixel_constant_across_zoom() {
    // A road must stay the same number of screen pixels wide as the camera
    // zooms — the GPU extrudes from the centerline per frame. Zoom within
    // one tile level (no re-tessellation) and assert the thickness holds.
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let (w, h) = (512, 384);
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
        "line".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[[5.0,60.39],[5.64,60.39]]}"#.to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Line {
        id: "road".to_string(),
        source: "line".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(200, 40, 60)),
        width: Paint::Const(8.0),
    });

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (w, h),
        CameraState::new(LatLng::new(60.39, 5.32), 9.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("engine");

    // Measure the vertical thickness of the horizontal line in the centre
    // column (a run of red pixels).
    let thickness = |img: &image::RgbaImage| -> u32 {
        let cx = w / 2;
        let mut run = 0u32;
        let mut best = 0u32;
        for y in 0..h {
            let p = img.get_pixel(cx, y);
            if p.0[0] > 150 && p.0[1] < 100 && p.0[2] < 110 {
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
        best
    };

    engine.apply(scene);
    engine.pump_tiles();
    let img0 = render_to_image(&gpu, w, h, |e, v| engine.render(e, v));
    engine.after_submit();
    let t0 = thickness(&img0);

    // Zoom in 0.6 — same tile level (floor == 9), so no re-fetch.
    engine.set_camera(CameraState::new(LatLng::new(60.39, 5.32), 9.6));
    let drained = engine.pump_tiles();
    assert_eq!(drained.vector_tiles, 0, "zoom within a level must not re-tessellate");
    let img1 = render_to_image(&gpu, w, h, |e, v| engine.render(e, v));
    engine.after_submit();
    let t1 = thickness(&img1);

    assert!(t0 >= 6, "line should be ~8px thick, got {t0}");
    assert!(
        t0.abs_diff(t1) <= 2,
        "width must stay pixel-constant across zoom: {t0}px at z9.0 vs {t1}px at z9.6"
    );
}

#[test]
fn label_importance_ranking_wins_collisions() {
    // Two labels at the SAME spot (guaranteed collision). The high-rank
    // CAPITAL (dark) must win over the low-rank HAMLET (red), regardless
    // of placement order — so the frame shows dark ink and NO red.
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
        "places".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"name":"CAPITAL","rank":100},"geometry":{"type":"Point","coordinates":[5.32,60.39]}},
                {"type":"Feature","properties":{"name":"HAMLET","rank":1},"geometry":{"type":"Point","coordinates":[5.32,60.39]}}
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Symbol {
        id: "labels".to_string(),
        source: "places".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(22.0),
        // Per-feature colour so we can tell the winner from the loser.
        color: Paint::Match {
            property: "name".to_string(),
            cases: vec![MatchCase {
                value: FilterValue::String("HAMLET".into()),
                result: Color::rgb(230, 30, 30),
            }],
            default: Box::new(Color::rgb(20, 20, 30)),
        },
        halo_color: Paint::Const(Color::rgba(0, 0, 0, 0)),
        halo_width: Paint::Const(0.0),
        sort_key: Some("rank".to_string()),
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
    });

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), 9.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(scene);
    engine.pump_tiles();
    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    let dark = image
        .pixels()
        .filter(|p| (0..3).all(|i| p.0[i].abs_diff([20, 20, 30][i]) <= 45))
        .count();
    let red = image
        .pixels()
        .filter(|p| p.0[0] > 170 && p.0[1] < 90 && p.0[2] < 90)
        .count();
    assert!(dark > 60, "the high-rank CAPITAL label should render, dark px = {dark}");
    assert_eq!(red, 0, "the low-rank HAMLET label must be suppressed, red px = {red}");

    assert_golden(
        "label-importance",
        &image,
        GoldenConfig { max_channel_diff: 6, max_outlier_frac: 0.02 },
    );
}

#[test]
fn data_driven_match_width_builds_road_hierarchy() {
    // Three parallel lines tagged major/minor/local. A single layer with
    // Match colour AND Match width on `kind` must render them in three
    // colours AND three widths — the road hierarchy.
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
        "roads".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"kind":"major"},"geometry":{"type":"LineString","coordinates":[[5.10,60.45],[5.55,60.45]]}},
                {"type":"Feature","properties":{"kind":"minor"},"geometry":{"type":"LineString","coordinates":[[5.10,60.39],[5.55,60.39]]}},
                {"type":"Feature","properties":{"kind":"local"},"geometry":{"type":"LineString","coordinates":[[5.10,60.33],[5.55,60.33]]}}
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    let by_kind_color = Paint::Match {
        property: "kind".to_string(),
        cases: vec![
            MatchCase { value: FilterValue::String("major".into()), result: Color::rgb(230, 150, 30) },
            MatchCase { value: FilterValue::String("minor".into()), result: Color::rgb(90, 90, 110) },
        ],
        default: Box::new(Color::rgb(150, 150, 160)),
    };
    let by_kind_width = Paint::Match {
        property: "kind".to_string(),
        cases: vec![
            MatchCase { value: FilterValue::String("major".into()), result: 11.0f32 },
            MatchCase { value: FilterValue::String("minor".into()), result: 6.0f32 },
        ],
        default: Box::new(3.0f32),
    };
    scene.layers.push(Layer::Line {
        id: "roads".to_string(),
        source: "roads".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: by_kind_color,
        width: by_kind_width,
    });

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), 9.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(scene);
    engine.pump_tiles();
    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // Scan the centre column top-to-bottom; the three horizontal lines are
    // three runs of road pixels. Their vertical thickness is the rendered
    // width, so top→bottom (major/minor/local) must be strictly decreasing.
    let cx = width / 2;
    let mut runs: Vec<u32> = Vec::new();
    let mut run = 0u32;
    for y in 0..height {
        let p = image.get_pixel(cx, y);
        let is_bg = (0..3).all(|i| p.0[i].abs_diff([226, 218, 198][i]) <= 30);
        if is_bg {
            if run > 0 {
                runs.push(run);
                run = 0;
            }
        } else {
            run += 1;
        }
    }
    if run > 0 {
        runs.push(run);
    }
    assert_eq!(runs.len(), 3, "expected 3 road lines in the centre column, got {runs:?}");
    assert!(
        runs[0] > runs[1] && runs[1] > runs[2],
        "width hierarchy must decrease major>minor>local top-to-bottom, got {runs:?}"
    );

    assert_golden(
        "datadriven-width-hierarchy",
        &image,
        GoldenConfig { max_channel_diff: 6, max_outlier_frac: 0.02 },
    );
}

#[test]
fn symbol_halo_keeps_labels_readable_over_busy_lines() {
    // The readability test: dark labels with a white halo, sitting on top
    // of thick dark-blue lines. Without the halo the ink would blend into
    // the lines where they cross; the halo must keep the glyphs legible.
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
    // Thick dark lines crossing where the labels sit.
    scene.sources.insert(
        "lines".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","geometry":{"type":"LineString","coordinates":[[5.10,60.45],[5.55,60.45]]}},
                {"type":"Feature","geometry":{"type":"LineString","coordinates":[[5.10,60.39],[5.55,60.39]]}},
                {"type":"Feature","geometry":{"type":"LineString","coordinates":[[5.10,60.33],[5.55,60.33]]}}
            ]}"#
            .to_string(),
        },
    );
    scene.sources.insert(
        "places".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"name":"NORTH"},"geometry":{"type":"Point","coordinates":[5.32,60.45]}},
                {"type":"Feature","properties":{"name":"BERGEN"},"geometry":{"type":"Point","coordinates":[5.32,60.39]}},
                {"type":"Feature","properties":{"name":"SOUTH"},"geometry":{"type":"Point","coordinates":[5.32,60.33]}}
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
        id: "lines".to_string(),
        source: "lines".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(30, 40, 90)),
        width: Paint::Const(10.0),
    });
    scene.layers.push(Layer::Symbol {
        id: "labels".to_string(),
        source: "places".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(20.0),
        color: Paint::Const(Color::rgb(20, 20, 30)),
        halo_color: Paint::Const(Color::rgb(250, 250, 252)),
        halo_width: Paint::Const(2.0),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
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
    engine.pump_tiles();

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // The halo paints white pixels that are NOT the line colour and NOT the
    // dark ink — proof the outline is actually there around the glyphs.
    let near = |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| {
        (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol)
    };
    let halo_px = image
        .pixels()
        .filter(|p| near(p, [250, 250, 252], 12))
        .count();
    assert!(
        halo_px > 80,
        "halo outline should be visible around glyphs, got {halo_px} white px"
    );

    assert_golden(
        "symbol-halo-busy",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}

#[test]
fn cjk_labels_render_via_fallback_font() {
    // A non-Latin label only renders if the host registers a covering font
    // and the glyph-id atlas falls back to it. Register a system CJK face,
    // label two places in Japanese, and prove dark glyph ink appears.
    // Skips cleanly where there's no GPU or no system CJK font.
    const CJK_FONT: &str = "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf";
    let Ok(font_bytes) = std::fs::read(CJK_FONT) else {
        eprintln!("SKIP: no system CJK font at {CJK_FONT}");
        return;
    };
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
        "places".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"name":"東京"},"geometry":{"type":"Point","coordinates":[5.32,60.42]}},
                {"type":"Feature","properties":{"name":"大阪"},"geometry":{"type":"Point","coordinates":[5.32,60.36]}}
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Symbol {
        id: "labels".to_string(),
        source: "places".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(28.0),
        color: Paint::Const(Color::rgb(25, 25, 35)),
        halo_color: Paint::Const(Color::rgb(250, 250, 252)),
        halo_width: Paint::Const(1.6),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
    });

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), 11.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    // The host supplies the CJK face; without it these labels would be
    // empty (.notdef) boxes.
    assert!(engine.add_fallback_font(font_bytes), "CJK font must parse");

    engine.apply(scene);
    engine.pump_tiles();
    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    let dark = image
        .pixels()
        .filter(|p| (0..3).all(|i| p.0[i].abs_diff([25, 25, 35][i]) <= 50))
        .count();
    assert!(dark > 120, "CJK label ink should render, dark px = {dark}");

    assert_golden(
        "cjk-labels-fallback",
        &image,
        GoldenConfig { max_channel_diff: 6, max_outlier_frac: 0.02 },
    );
}

#[test]
fn complex_scripts_render_with_shaping_and_bidi() {
    // Arabic (RTL + joining), Devanagari (reordering), and a mixed
    // Latin+Arabic label, all shaped by HarfBuzz and bidi-ordered, drawn
    // through the full engine via a registered FreeSerif face (covers both
    // scripts). Skips where there's no GPU or no FreeSerif.
    const FREESERIF: &str = "/usr/share/fonts/truetype/freefont/FreeSerif.ttf";
    let Ok(font_bytes) = std::fs::read(FREESERIF) else {
        eprintln!("SKIP: no FreeSerif at {FREESERIF}");
        return;
    };
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
        "places".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"name":"مرحبا"},"geometry":{"type":"Point","coordinates":[5.32,60.44]}},
                {"type":"Feature","properties":{"name":"नमस्ते"},"geometry":{"type":"Point","coordinates":[5.32,60.39]}},
                {"type":"Feature","properties":{"name":"Bergen مرحبا"},"geometry":{"type":"Point","coordinates":[5.32,60.34]}}
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Symbol {
        id: "labels".to_string(),
        source: "places".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(30.0),
        color: Paint::Const(Color::rgb(25, 25, 35)),
        halo_color: Paint::Const(Color::rgb(250, 250, 252)),
        halo_width: Paint::Const(1.6),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
    });

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), 11.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    assert!(engine.add_fallback_font(font_bytes), "FreeSerif must parse");

    engine.apply(scene);
    engine.pump_tiles();
    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    let dark = image
        .pixels()
        .filter(|p| (0..3).all(|i| p.0[i].abs_diff([25, 25, 35][i]) <= 50))
        .count();
    assert!(dark > 150, "shaped multi-script ink should render, dark px = {dark}");

    assert_golden(
        "complex-scripts-shaping",
        &image,
        GoldenConfig { max_channel_diff: 6, max_outlier_frac: 0.02 },
    );
}

#[test]
fn road_name_follows_the_centerline() {
    // A curved road with a `name`, labelled with `placement: line`. The
    // glyphs must run *along* the curve (rotated to the tangent), not sit
    // in a horizontal block at a point. We prove placement two ways: the
    // ink spans a wide band of the route, and the run is rotated (its
    // bounding box is taller than a single horizontal line of text).
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
    // A road that climbs and bends across the view.
    scene.sources.insert(
        "roads".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"Feature","properties":{"name":"RINGVEGEN"},
                "geometry":{"type":"LineString","coordinates":[
                    [5.12,60.33],[5.22,60.36],[5.32,60.40],[5.42,60.43],[5.52,60.45]
                ]}}"#
                .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    // Draw the road itself so the label has a casing to sit on.
    scene.layers.push(Layer::Line {
        id: "road".to_string(),
        source: "roads".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(250, 220, 150)),
        width: Paint::Const(11.0),
    });
    // The road name, placed along the centerline with a readable halo.
    scene.layers.push(Layer::Symbol {
        id: "road-labels".to_string(),
        source: "roads".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(18.0),
        color: Paint::Const(Color::rgb(40, 40, 50)),
        halo_color: Paint::Const(Color::rgb(250, 248, 244)),
        halo_width: Paint::Const(1.6),
        sort_key: None,
        placement: SymbolPlacement::Line,
        icon_image: None,
        icon_size: Paint::Const(24.0),
    });

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.39, 5.32), 10.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(scene);
    engine.pump_tiles();
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // Dark label ink — collect its bounding box.
    let mut min_x = width;
    let mut max_x = 0u32;
    let mut min_y = height;
    let mut max_y = 0u32;
    let mut ink = 0u32;
    for y in 0..height {
        for x in 0..width {
            let p = image.get_pixel(x, y);
            if (0..3).all(|i| p.0[i].abs_diff([40, 40, 50][i]) <= 50) {
                ink += 1;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
    }
    let span_x = max_x.saturating_sub(min_x);
    let span_y = max_y.saturating_sub(min_y);
    eprintln!("road-name: ink={ink} span_x={span_x} span_y={span_y}");
    assert!(ink > 60, "road name ink should be visible, got {ink}");
    // Cross-tile dedup (LINE_LABEL_REPEAT_PX) collapses the road — clipped
    // across several tiles — to a single along-line label, instead of the
    // doubled name it used to draw. One "RINGVEGEN" at 18px is a compact
    // run, not a route-spanning band.
    assert!(
        (40..150).contains(&span_x),
        "expected one deduplicated label (compact x-span), got {span_x}"
    );
    // It still follows the climbing centerline: rotated glyphs span more
    // vertically than a flat 18px line (~20px tall).
    assert!(
        span_y > 24,
        "along-line glyphs should climb with the route, y-span {span_y}"
    );

    assert_golden(
        "road-name-along-line",
        &image,
        GoldenConfig { max_channel_diff: 6, max_outlier_frac: 0.02 },
    );
}

#[test]
fn icons_and_route_shields_render() {
    // POIs and route shields from the sprite atlas: a "dot" sprite at a POI
    // (icon, no text) and a "shield" sprite under a centred road ref (icon
    // + text composing a shield). We prove each sprite is on screen by its
    // signature colour, and that the shield's number sits on top of it.
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
    // A bare POI (icon only, no label).
    scene.sources.insert(
        "pois".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[5.300,60.392]}}"#
                .to_string(),
        },
    );
    // A route shield: a point carrying the road ref.
    scene.sources.insert(
        "refs".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"Feature","properties":{"ref":"E16"},"geometry":{"type":"Point","coordinates":[5.345,60.392]}}"#
                .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    // POI dot — icon, empty text field ⇒ no label.
    scene.layers.push(Layer::Symbol {
        id: "pois".to_string(),
        source: "pois".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: String::new(),
        text_size: Paint::Const(12.0),
        color: Paint::Const(Color::rgb(20, 20, 30)),
        halo_color: Paint::Const(Color::rgba(0, 0, 0, 0)),
        halo_width: Paint::Const(0.0),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: Some("dot".to_string()),
        icon_size: Paint::Const(26.0),
    });
    // Route shield — sprite background with the ref centred on top.
    scene.layers.push(Layer::Symbol {
        id: "shields".to_string(),
        source: "refs".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "ref".to_string(),
        text_size: Paint::Const(16.0),
        color: Paint::Const(Color::rgb(20, 24, 40)),
        halo_color: Paint::Const(Color::rgba(0, 0, 0, 0)),
        halo_width: Paint::Const(0.0),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: Some("shield".to_string()),
        icon_size: Paint::Const(34.0),
    });

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        CameraState::new(LatLng::new(60.392, 5.3225), 12.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
        Box::new(SyntheticResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(scene);
    engine.pump_tiles();
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    let near = |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| {
        (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol)
    };
    // The POI dot's red ring.
    let red_ring = image.pixels().filter(|p| near(p, [232, 64, 60], 40)).count();
    // The shield's dark-blue border.
    let shield_border = image.pixels().filter(|p| near(p, [40, 54, 110], 40)).count();
    // The ref text drawn on top of the shield (dark ink).
    let ink = image.pixels().filter(|p| near(p, [20, 24, 40], 40)).count();
    assert!(red_ring > 30, "POI dot ring should render, red px = {red_ring}");
    assert!(shield_border > 30, "shield border should render, blue px = {shield_border}");
    assert!(ink > 10, "shield ref text should render on top, ink px = {ink}");

    assert_golden(
        "icons-and-shields",
        &image,
        GoldenConfig { max_channel_diff: 6, max_outlier_frac: 0.02 },
    );
}

#[test]
fn symbol_labels_render_over_raster() {
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
        "places".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"name":"NORTH"},"geometry":{"type":"Point","coordinates":[5.32,60.45]}},
                {"type":"Feature","properties":{"name":"BERGEN"},"geometry":{"type":"Point","coordinates":[5.32,60.39]}},
                {"type":"Feature","properties":{"name":"SOUTH"},"geometry":{"type":"Point","coordinates":[5.32,60.33]}}
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Symbol {
        id: "labels".to_string(),
        source: "places".to_string(),
        source_layer: None,
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(18.0),
        color: Paint::Const(Color::rgb(20, 20, 30)),
        halo_color: Paint::Const(Color::rgba(0, 0, 0, 0)),
        halo_width: Paint::Const(0.0),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
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
    assert!(stats.vector_tiles > 0, "expected label tiles, got {stats:?}");
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    assert_golden(
        "symbol-labels-bergen",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}

#[test]
fn data_driven_match_colour_styles_lines_by_property() {
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
    // Three lines at different latitudes, tagged path / water / road.
    scene.sources.insert(
        "ways".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"kind":"path"},"geometry":{"type":"LineString","coordinates":[[5.10,60.45],[5.55,60.46]]}},
                {"type":"Feature","properties":{"kind":"water"},"geometry":{"type":"LineString","coordinates":[[5.10,60.39],[5.55,60.40]]}},
                {"type":"Feature","properties":{"kind":"road"},"geometry":{"type":"LineString","coordinates":[[5.10,60.33],[5.55,60.34]]}}
            ]}"#
            .to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "basemap".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    // Colour by the `kind` property: path→green, water→blue, else grey.
    scene.layers.push(Layer::Line {
        id: "ways".to_string(),
        source: "ways".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Match {
            property: "kind".to_string(),
            cases: vec![
                MatchCase {
                    value: FilterValue::String("path".to_string()),
                    result: Color::rgb(20, 170, 60),
                },
                MatchCase {
                    value: FilterValue::String("water".to_string()),
                    result: Color::rgb(30, 90, 220),
                },
            ],
            default: Box::new(Color::rgb(110, 110, 110)),
        },
        width: Paint::Const(5.0),
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
    assert!(stats.vector_tiles > 0, "expected way tiles, got {stats:?}");
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    assert_golden(
        "datadriven-lines-bergen",
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
