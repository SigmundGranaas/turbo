//! The credibility golden: **real OpenStreetMap data** (central Bergen,
//! OpenMapTiles schema, from OpenFreeMap — ODbL © OpenStreetMap
//! contributors) rendered from the committed PMTiles fixture with a
//! designed basemap style: road casings + class hierarchy, landcover,
//! buildings, dashed paths and admin boundaries, street names along the
//! real street centerlines, ranked place labels.
//!
//! Unlike the synthetic-lattice goldens (which verify schema/data-path
//! correctness), this one pins what the renderer makes of *actual city
//! geometry* — the closest headless proxy for "does it look like a map".
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
use turbomap_scene::SourceDef;
use turbomap_tiles_pmtiles::PMTilesSource;

/// Land base + the committed real-data archive (file backend).
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
fn real_bergen_renders_like_a_basemap() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    // Rendered at 2x device-pixel ratio — what a phone actually shows.
    // Logical viewport 640x440, physical 1280x880.
    let (width, height) = (1280, 880);
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        // Bergen sentrum — Torgallmenningen-ish. Camera zoom is in
        // *logical* px scale: at 2x device-pixel ratio the same framing as
        // a 640x440 z14 view is zoom 15 over 1280x880 physical px. The
        // archive only holds z14, so the source bounds clamp tile requests
        // to z14 — the data zoom — while the display densifies.
        CameraState::new(LatLng::new(60.3920, 5.3242), 15.0),
        MapOptions {
            fade_in_secs: 0.0,
            pixel_ratio: 2.0,
            ..Default::default()
        },
        Box::new(BergenResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(bergen_scene());
    let stats = engine.pump_tiles();
    assert!(
        stats.vector_tiles >= 4,
        "several real tiles should load from the archive, got {stats:?}"
    );
    assert!(engine.unsupported_layers().is_empty());

    let image = render_to_image(&gpu, width, height, |enc, view| engine.render(enc, view));
    engine.after_submit();

    let near =
        |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol);
    let water = image
        .pixels()
        .filter(|p| near(p, [163, 201, 224], 12))
        .count();
    let buildings = image
        .pixels()
        .filter(|p| near(p, [222, 218, 210], 6))
        .count();
    let white_roads = image
        .pixels()
        .filter(|p| near(p, [255, 255, 255], 6))
        .count();
    let ink = image.pixels().filter(|p| near(p, [60, 64, 74], 35)).count();
    eprintln!("bergen: water={water} buildings={buildings} roads={white_roads} ink={ink}");
    assert!(
        water > 12000,
        "Bergen's harbour should be visible, got {water}"
    );
    assert!(
        buildings > 8000,
        "building footprints should render, got {buildings}"
    );
    assert!(
        white_roads > 8000,
        "the street grid should render, got {white_roads}"
    );
    assert!(ink > 200, "labels should render, got {ink}");

    assert_golden(
        "omt-real-bergen",
        &image,
        GoldenConfig {
            max_channel_diff: 6,
            max_outlier_frac: 0.02,
        },
    );
}
