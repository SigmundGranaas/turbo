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
use turbomap_golden::{
    assert_golden, headless, render_to_image, sources::FlatBasemap, GoldenConfig, TARGET_FORMAT,
};
use turbomap_scene::{
    Color, Filter, FilterValue, Layer, MatchCase, Paint, Scene, SourceDef, SymbolPlacement,
};
use turbomap_tiles_pmtiles::PMTilesSource;

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../turbomap-golden/tests/fixtures/bergen-omt.pmtiles")
}

/// Land base + the committed real-data archive (file backend).
struct BergenResolver;

impl SourceResolver for BergenResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => {
                ResolvedSource::Raster(Arc::new(FlatBasemap([242, 240, 235])))
            }
            SourceDef::VectorXyz { .. } => ResolvedSource::Vector(Arc::new(
                PMTilesSource::open(fixture_path()).expect("open bergen fixture"),
            )),
            _ => ResolvedSource::Unsupported,
        }
    }
}

fn s(v: &str) -> FilterValue {
    FilterValue::String(v.to_string())
}

fn class_in(values: &[&str]) -> Filter {
    Filter::In("class".to_string(), values.iter().map(|v| s(v)).collect())
}

/// The designed basemap style, keyed on the OpenMapTiles taxonomy.
fn bergen_scene() -> Scene {
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
        "omt".to_string(),
        SourceDef::VectorXyz {
            tiles: vec!["pmtiles://bergen".to_string()],
            min_zoom: 14,
            max_zoom: 14,
        },
    );

    scene.layers.push(Layer::Raster {
        id: "land".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    // Greens under everything else.
    scene.layers.push(Layer::Fill {
        id: "landcover-green".to_string(),
        source: "omt".to_string(),
        source_layer: Some("landcover".to_string()),
        filter: class_in(&["grass", "meadow", "garden", "park", "recreation_ground"]),
        color: Paint::Const(Color::rgb(205, 227, 197)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "landcover-wood".to_string(),
        source: "omt".to_string(),
        source_layer: Some("landcover".to_string()),
        filter: class_in(&["wood", "forest"]),
        color: Paint::Const(Color::rgb(186, 214, 183)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "park".to_string(),
        source: "omt".to_string(),
        source_layer: Some("park".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(208, 229, 199)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "landuse-residential".to_string(),
        source: "omt".to_string(),
        source_layer: Some("landuse".to_string()),
        filter: class_in(&["residential", "suburbs", "neighbourhood"]),
        color: Paint::Const(Color::rgb(236, 233, 227)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "water".to_string(),
        source: "omt".to_string(),
        source_layer: Some("water".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(163, 201, 224)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Fill {
        id: "buildings".to_string(),
        source: "omt".to_string(),
        source_layer: Some("building".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(222, 218, 210)),
        opacity: Paint::Const(1.0),
    });

    // Streets: grey casing under, class-coloured inner over.
    let street_classes = [
        "motorway", "trunk", "primary", "secondary", "tertiary", "minor", "service",
    ];
    scene.layers.push(Layer::Line {
        id: "road-casing".to_string(),
        source: "omt".to_string(),
        source_layer: Some("transportation".to_string()),
        filter: class_in(&street_classes),
        color: Paint::Const(Color::rgb(196, 192, 186)),
        width: Paint::Match {
            property: "class".to_string(),
            cases: vec![
                MatchCase { value: s("motorway"), result: 11.0f32 },
                MatchCase { value: s("trunk"), result: 10.0f32 },
                MatchCase { value: s("primary"), result: 8.5f32 },
                MatchCase { value: s("secondary"), result: 7.0f32 },
                MatchCase { value: s("tertiary"), result: 6.0f32 },
                MatchCase { value: s("minor"), result: 5.0f32 },
            ],
            default: Box::new(3.4f32),
        },
        dash_array: None,
    });
    scene.layers.push(Layer::Line {
        id: "road-inner".to_string(),
        source: "omt".to_string(),
        source_layer: Some("transportation".to_string()),
        filter: class_in(&street_classes),
        color: Paint::Match {
            property: "class".to_string(),
            cases: vec![
                MatchCase { value: s("motorway"), result: Color::rgb(250, 200, 108) },
                MatchCase { value: s("trunk"), result: Color::rgb(252, 212, 130) },
            ],
            default: Box::new(Color::rgb(255, 255, 255)),
        },
        width: Paint::Match {
            property: "class".to_string(),
            cases: vec![
                MatchCase { value: s("motorway"), result: 8.0f32 },
                MatchCase { value: s("trunk"), result: 7.2f32 },
                MatchCase { value: s("primary"), result: 6.0f32 },
                MatchCase { value: s("secondary"), result: 4.8f32 },
                MatchCase { value: s("tertiary"), result: 4.0f32 },
                MatchCase { value: s("minor"), result: 3.2f32 },
            ],
            default: Box::new(2.0f32),
        },
        dash_array: None,
    });
    // Footpaths/tracks: thin dashed, distinct from streets.
    scene.layers.push(Layer::Line {
        id: "paths".to_string(),
        source: "omt".to_string(),
        source_layer: Some("transportation".to_string()),
        filter: class_in(&["path", "track", "pedestrian"]),
        color: Paint::Const(Color::rgb(172, 166, 158)),
        width: Paint::Const(1.6),
        dash_array: Some(vec![3.0, 2.6]),
    });
    scene.layers.push(Layer::Line {
        id: "rail".to_string(),
        source: "omt".to_string(),
        source_layer: Some("transportation".to_string()),
        filter: class_in(&["rail", "transit"]),
        color: Paint::Const(Color::rgb(200, 199, 204)),
        width: Paint::Const(1.8),
        dash_array: None,
    });
    scene.layers.push(Layer::Line {
        id: "boundary".to_string(),
        source: "omt".to_string(),
        source_layer: Some("boundary".to_string()),
        filter: Filter::In(
            "admin_level".to_string(),
            vec![
                FilterValue::Number(4.0),
                FilterValue::Number(6.0),
                FilterValue::Number(7.0),
                FilterValue::Number(8.0),
            ],
        ),
        color: Paint::Const(Color::rgb(172, 160, 186)),
        width: Paint::Const(1.4),
        dash_array: Some(vec![6.0, 4.5]),
    });

    // Labels: water names, street names along the centerline, places.
    scene.layers.push(Layer::Symbol {
        id: "water-names".to_string(),
        source: "omt".to_string(),
        source_layer: Some("water_name".to_string()),
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(13.0),
        color: Paint::Const(Color::rgb(92, 122, 152)),
        halo_color: Paint::Const(Color::rgb(246, 248, 250)),
        halo_width: Paint::Const(1.2),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
        icon_color: Paint::Const(Color::rgb(70, 78, 92)),
    });
    scene.layers.push(Layer::Symbol {
        id: "street-names".to_string(),
        source: "omt".to_string(),
        source_layer: Some("transportation_name".to_string()),
        filter: Filter::Always,
        text_field: "name".to_string(),
        text_size: Paint::Const(12.0),
        color: Paint::Const(Color::rgb(90, 92, 100)),
        halo_color: Paint::Const(Color::rgb(250, 250, 250)),
        halo_width: Paint::Const(1.4),
        sort_key: None,
        placement: SymbolPlacement::Line,
        icon_image: None,
        icon_size: Paint::Const(24.0),
        icon_color: Paint::Const(Color::rgb(70, 78, 92)),
    });
    scene.layers.push(Layer::Symbol {
        id: "places".to_string(),
        source: "omt".to_string(),
        source_layer: Some("place".to_string()),
        filter: class_in(&["city", "town", "suburb", "village", "neighbourhood", "quarter"]),
        text_field: "name".to_string(),
        text_size: Paint::Match {
            property: "class".to_string(),
            cases: vec![
                MatchCase { value: s("city"), result: 19.0f32 },
                MatchCase { value: s("town"), result: 16.0f32 },
                MatchCase { value: s("suburb"), result: 14.0f32 },
                MatchCase { value: s("village"), result: 13.0f32 },
            ],
            default: Box::new(12.0f32),
        },
        color: Paint::Const(Color::rgb(60, 64, 74)),
        halo_color: Paint::Const(Color::rgb(248, 248, 250)),
        halo_width: Paint::Const(1.6),
        sort_key: None,
        placement: SymbolPlacement::Point,
        icon_image: None,
        icon_size: Paint::Const(24.0),
        icon_color: Paint::Const(Color::rgb(70, 78, 92)),
    });
    scene
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

    let (width, height) = (640, 440);
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        // Bergen sentrum — Torgallmenningen-ish.
        CameraState::new(LatLng::new(60.3920, 5.3242), 14.0),
        MapOptions { fade_in_secs: 0.0, ..Default::default() },
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

    let near = |p: &image::Rgba<u8>, rgb: [u8; 3], tol: u8| {
        (0..3).all(|i| p.0[i].abs_diff(rgb[i]) <= tol)
    };
    let water = image.pixels().filter(|p| near(p, [163, 201, 224], 12)).count();
    let buildings = image.pixels().filter(|p| near(p, [222, 218, 210], 6)).count();
    let white_roads = image.pixels().filter(|p| near(p, [255, 255, 255], 6)).count();
    let ink = image.pixels().filter(|p| near(p, [60, 64, 74], 35)).count();
    eprintln!("bergen: water={water} buildings={buildings} roads={white_roads} ink={ink}");
    assert!(water > 3000, "Bergen's harbour should be visible, got {water}");
    assert!(buildings > 2000, "building footprints should render, got {buildings}");
    assert!(white_roads > 2000, "the street grid should render, got {white_roads}");
    assert!(ink > 50, "labels should render, got {ink}");

    assert_golden(
        "omt-real-bergen",
        &image,
        GoldenConfig { max_channel_diff: 6, max_outlier_frac: 0.02 },
    );
}
