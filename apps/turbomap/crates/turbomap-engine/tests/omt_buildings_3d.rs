//! 3D building extrusion under a tilted camera. The flat Bergen scene plus a
//! `FillExtrusion` building layer, viewed at pitch — proves the extrusion
//! tessellation (roof + walls), the per-vertex height (z), and the
//! depth-sorted vector pipeline render real footprints as 3D prisms.
#![cfg(feature = "gpu-tests")]

use std::sync::Arc;

use turbomap_core::MapOptions;
use turbomap_engine::{
    CameraState, LatLng, MapEngine, ResolvedSource, SourceResolver, TurbomapEngine,
};
use turbomap_golden::omt::{bergen_scene, fixture_path, LAND};
use turbomap_golden::{
    assert_golden, headless, render_to_image, sources::FlatBasemap, GoldenConfig, TARGET_FORMAT,
};
use turbomap_scene::{Color, Filter, Layer, Paint, SourceDef};
use turbomap_tiles_pmtiles::PMTilesSource;

struct BergenResolver;
impl SourceResolver for BergenResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(FlatBasemap(LAND))),
            SourceDef::VectorXyz { .. } => ResolvedSource::Vector(Arc::new(
                PMTilesSource::open(fixture_path()).expect("open bergen fixture"),
            )),
            _ => ResolvedSource::Unsupported,
        }
    }
}

#[test]
fn buildings_extrude_under_a_tilted_camera() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    // The flat city style, plus a 3D building layer. Each footprint extrudes
    // to its own OMT `render_height`, with a low fallback for any missing it.
    let mut scene = bergen_scene();
    scene.layers.push(Layer::FillExtrusion {
        id: "buildings-3d".to_string(),
        source: "omt".to_string(),
        source_layer: Some("building".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(223, 214, 200)),
        height_m: Paint::Const(6.0),
        height_property: Some("render_height".to_string()),
        min_height_property: Some("render_min_height".to_string()),
    });

    let (width, height) = (1280, 880);
    let mut camera = CameraState::new(LatLng::new(60.3952, 5.3242), 16.0);
    camera.pitch_deg = 55.0; // tilt so the extrusions stand up

    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        camera,
        MapOptions {
            fade_in_secs: 0.0,
            pixel_ratio: 2.0,
            ..Default::default()
        },
        Box::new(BergenResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(scene);
    let stats = engine.pump_tiles();
    assert!(stats.vector_tiles >= 4, "tiles should load, got {stats:?}");
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    // Roofs are the full colour; walls are directionally shaded, so both a
    // dark (shadowed, ~0.52) and a light (sunlit, ~0.82) band must appear —
    // that's the proof the shading is *directional*, not a flat darken.
    let near =
        |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol);
    let band = |f: f32| [(223.0 * f) as u8, (214.0 * f) as u8, (200.0 * f) as u8];
    let roof = image
        .pixels()
        .filter(|p| near(p, [223, 214, 200], 8))
        .count();
    let shadow_wall = image.pixels().filter(|p| near(p, band(0.52), 12)).count();
    let lit_wall = image.pixels().filter(|p| near(p, band(0.82), 12)).count();
    eprintln!("3d buildings: roof={roof} shadow_wall={shadow_wall} lit_wall={lit_wall}");
    assert!(roof > 3000, "building roofs should render, got {roof}");
    assert!(
        shadow_wall > 400,
        "shadowed wall faces should render, got {shadow_wall}"
    );
    assert!(
        lit_wall > 400,
        "sunlit wall faces should render, got {lit_wall}"
    );

    assert_golden(
        "omt-bergen-3d",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}
