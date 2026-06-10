//! The shared OpenMapTiles **Bergen basemap fixture** — one canonical
//! `Scene` + fixture path used by both the `omt-real-bergen` golden and
//! the `visual_lab` example, so the credibility golden and the iteration
//! tool can never drift apart. Real OSM data (OpenFreeMap, ODbL ©
//! OpenStreetMap contributors) lives in `tests/fixtures/bergen-omt.pmtiles`.

use turbomap_scene::{
    Color, Filter, FilterValue, Layer, MatchCase, Paint, Scene, SourceDef, SymbolPlacement,
};

/// The flat "land" ground colour the raster base paints under the vector
/// layers.
pub const LAND: [u8; 3] = [242, 240, 235];

/// Path to the committed real-data PMTiles archive.
pub fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bergen-omt.pmtiles")
}

fn s(v: &str) -> FilterValue {
    FilterValue::String(v.to_string())
}

fn class_in(values: &[&str]) -> Filter {
    Filter::In("class".to_string(), values.iter().map(|v| s(v)).collect())
}

/// The designed basemap style, keyed on the OpenMapTiles taxonomy: road
/// casings + class hierarchy, landcover/park greens, buildings, dashed
/// admin boundaries, water, street names along real centerlines, ranked
/// place labels.
pub fn bergen_scene() -> Scene {
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
    // Buildings: a warm fill with a slightly darker outline so adjacent
    // footprints read as separate shapes instead of one grey blob.
    scene.layers.push(Layer::Fill {
        id: "buildings".to_string(),
        source: "omt".to_string(),
        source_layer: Some("building".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(224, 217, 206)),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Line {
        id: "building-outline".to_string(),
        source: "omt".to_string(),
        source_layer: Some("building".to_string()),
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(198, 189, 174)),
        width: Paint::Const(0.8),
        dash_array: None,
    });

    // Streets: grey casing under, class-coloured inner over. `service`
    // (driveways, parking aisles) is deliberately excluded — at city zoom
    // it's a web of clutter, especially over the hillsides, and a basemap
    // reads cleaner without it (the same call Google/MapLibre make here).
    let street_classes = [
        "motorway", "trunk", "primary", "secondary", "tertiary", "minor",
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
    // Footpaths: pedestrian streets only, thin and very light so the
    // network recedes. `path`/`track` (the hillside trail web) is dropped
    // at this zoom — it was the dominant source of speckle noise.
    scene.layers.push(Layer::Line {
        id: "paths".to_string(),
        source: "omt".to_string(),
        source_layer: Some("transportation".to_string()),
        filter: class_in(&["pedestrian"]),
        color: Paint::Const(Color::rgb(208, 202, 194)),
        width: Paint::Const(1.0),
        dash_array: None,
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

    // Labels: places, then water names, then street names — prepare order
    // is placement priority for the frame-wide collision set.
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
        // Footpath and service-road names are clutter at city zoom.
        filter: class_in(&["motorway", "trunk", "primary", "secondary", "tertiary", "minor"]),
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
    scene
}
