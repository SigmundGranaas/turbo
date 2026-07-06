//! Incremental reconcile: applying a changed scene must do the *minimal*
//! GPU work. We assert behaviourally — by watching tile fetches through
//! `pump_tiles`: an unchanged layer in the prefix must not re-fetch (its
//! cache is preserved), while a newly added / changed layer does.
#![cfg(feature = "gpu-tests")]

mod common;

use common::SyntheticResolver;
use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine, TurbomapEngine};
use turbomap_golden::{headless, Gpu, TARGET_FORMAT};
use turbomap_scene::{Color, Filter, Layer, Paint, Scene, SourceDef};

fn base_scene() -> Scene {
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
    scene
}

fn line_layer(color: Color) -> Layer {
    Layer::Line {
        id: "route".to_string(),
        source: "route".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(color),
        width: Paint::Const(5.0),
        dash_array: None,
    }
}

fn with_route(color: Color) -> Scene {
    let mut scene = base_scene();
    scene.sources.insert(
        "route".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[[5.1,60.3],[5.32,60.39],[5.55,60.48]]}"#
                .to_string(),
        },
    );
    scene.layers.push(line_layer(color));
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

#[test]
fn appending_a_layer_preserves_existing_caches() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut e = engine(&gpu);

    // First load of the raster base.
    e.apply(base_scene());
    let first = e.pump_tiles();
    assert!(first.raster_tiles > 0, "base should load: {first:?}");

    // Append a route line. The raster layer is unchanged and in the
    // prefix, so it must NOT re-fetch; only the new line layer does.
    e.apply(with_route(Color::rgb(220, 30, 60)));
    let second = e.pump_tiles();
    assert_eq!(
        second.raster_tiles, 0,
        "appending must preserve the raster cache (incremental reconcile), got {second:?}"
    );
    assert!(
        second.vector_tiles > 0,
        "the appended line layer must fetch its own tiles, got {second:?}"
    );
}

#[test]
fn repaint_preserves_the_prefix_but_rebuilds_the_changed_layer() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut e = engine(&gpu);

    e.apply(with_route(Color::rgb(220, 30, 60)));
    let first = e.pump_tiles();
    assert!(
        first.raster_tiles > 0 && first.vector_tiles > 0,
        "{first:?}"
    );

    // Recolour only the line. The raster prefix is preserved; the line
    // layer is rebuilt and re-fetches.
    e.apply(with_route(Color::rgb(30, 60, 220)));
    let second = e.pump_tiles();
    assert_eq!(
        second.raster_tiles, 0,
        "raster prefix must be preserved on a repaint, got {second:?}"
    );
    assert!(
        second.vector_tiles > 0,
        "the recoloured line must rebuild + re-fetch, got {second:?}"
    );
}

#[test]
fn source_data_update_rebuilds_only_the_dependent_layer() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut e = engine(&gpu);

    e.apply(with_route(Color::rgb(220, 30, 60)));
    e.pump_tiles();

    // Same layers, but the route's GeoJSON data changes (a live trace).
    let mut updated = base_scene();
    updated.sources.insert(
        "route".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[[5.0,60.2],[5.3,60.4],[5.6,60.5]]}"#
                .to_string(),
        },
    );
    updated.layers.push(line_layer(Color::rgb(220, 30, 60)));
    e.apply(updated);
    let second = e.pump_tiles();

    assert_eq!(
        second.raster_tiles, 0,
        "a route data update must not touch the raster, got {second:?}"
    );
    assert!(
        second.vector_tiles > 0,
        "the route layer must re-tessellate after its data changed, got {second:?}"
    );
}

#[test]
fn reapplying_the_same_scene_fetches_nothing() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut e = engine(&gpu);

    e.apply(with_route(Color::rgb(220, 30, 60)));
    e.pump_tiles();

    // Identical scene → empty delta → no reconcile, nothing to fetch.
    e.apply(with_route(Color::rgb(220, 30, 60)));
    let second = e.pump_tiles();
    assert_eq!(
        (second.raster_tiles, second.vector_tiles),
        (0, 0),
        "re-applying an identical scene must fetch nothing, got {second:?}"
    );
}
