//! Full-scene serde round-trips and validation, including schema defaults.

use turbomap_scene::style::Filter;
use turbomap_scene::{Color, Layer, Paint, Scene, SceneError, SourceDef};

fn sample_scene() -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "base".to_string(),
        SourceDef::RasterXyz {
            tiles: vec!["https://example.test/{z}/{x}/{y}.png".to_string()],
            tile_size: 256,
            min_zoom: 0,
            max_zoom: 18,
            attribution: Some("© Test".to_string()),
        },
    );
    scene.sources.insert(
        "route".to_string(),
        SourceDef::GeoJson {
            data: "{\"type\":\"FeatureCollection\",\"features\":[]}".to_string(),
        },
    );
    scene.layers.push(Layer::Raster {
        id: "base-l".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    scene.layers.push(Layer::Line {
        id: "route-l".to_string(),
        source: "route".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(4, 132, 255)),
        width: Paint::Const(5.0),
        dash_array: None,
    });
    scene
}

#[test]
fn scene_roundtrips_through_json() {
    let scene = sample_scene();
    let json = serde_json::to_string_pretty(&scene).unwrap();
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, scene);
}

#[test]
fn schema_defaults_fill_in_omitted_fields() {
    // A terse raster layer: only id + source. opacity defaults to 1.0.
    let json = r#"{
        "sources": { "base": { "type": "raster-xyz", "tiles": ["t/{z}/{x}/{y}.png"] } },
        "layers": [ { "type": "raster", "id": "base-l", "source": "base" } ]
    }"#;
    let scene: Scene = serde_json::from_str(json).unwrap();
    assert_eq!(scene.layers.len(), 1);
    match &scene.layers[0] {
        Layer::Raster { opacity, .. } => assert_eq!(opacity.at(0.0), 1.0),
        other => panic!("unexpected layer {other:?}"),
    }
    // Source tile_size/max_zoom defaulted.
    match scene.sources.get("base").unwrap() {
        SourceDef::RasterXyz {
            tile_size,
            max_zoom,
            ..
        } => {
            assert_eq!(*tile_size, 256);
            assert_eq!(*max_zoom, 22);
        }
        other => panic!("unexpected source {other:?}"),
    }
}

#[test]
fn validate_accepts_a_well_formed_scene() {
    assert!(sample_scene().validate().is_ok());
}

#[test]
fn validate_rejects_duplicate_layer_ids() {
    let mut scene = sample_scene();
    scene.layers.push(Layer::Raster {
        id: "base-l".to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    });
    assert_eq!(
        scene.validate(),
        Err(SceneError::DuplicateLayerId("base-l".to_string()))
    );
}

#[test]
fn validate_rejects_unknown_source() {
    let mut scene = sample_scene();
    scene.layers.push(Layer::Raster {
        id: "ghost".to_string(),
        source: "missing".to_string(),
        opacity: Paint::Const(1.0),
    });
    assert_eq!(
        scene.validate(),
        Err(SceneError::UnknownSource {
            layer: "ghost".to_string(),
            source: "missing".to_string(),
        })
    );
}

// ---- PMTiles source variants (plan slice B5.2, decisions D2/D7) ---------

#[test]
fn pmtiles_sources_roundtrip_with_kebab_case_tags() {
    use turbomap_scene::DemEncoding;
    let mut scene = Scene::new();
    scene.sources.insert(
        "baseline".to_string(),
        SourceDef::PmtilesVector { location: "/data/norway-z10.pmtiles".to_string() },
    );
    scene.sources.insert(
        "sat".to_string(),
        SourceDef::PmtilesRaster { location: "https://cdn.example/planet.pmtiles".to_string() },
    );
    scene.sources.insert(
        "dem".to_string(),
        SourceDef::PmtilesDem {
            location: "/data/dem.pmtiles".to_string(),
            encoding: DemEncoding::MapboxRgb,
            halo: 1,
        },
    );
    let json = serde_json::to_string(&scene).unwrap();
    // Tagged kebab-case like every other source kind — host bindings parse
    // one convention.
    assert!(json.contains("\"type\":\"pmtiles-vector\""), "{json}");
    assert!(json.contains("\"type\":\"pmtiles-raster\""), "{json}");
    assert!(json.contains("\"type\":\"pmtiles-dem\""), "{json}");
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, scene);
}

#[test]
fn pmtiles_dem_halo_defaults_to_zero() {
    let json = r#"{"type":"pmtiles-dem","location":"/d.pmtiles","encoding":"mapbox-rgb"}"#;
    let def: SourceDef = serde_json::from_str(json).unwrap();
    assert!(matches!(def, SourceDef::PmtilesDem { halo: 0, .. }));
}

// ---- provider chains (plan B6.2, decisions D2/D7) ------------------------

#[test]
fn chain_roundtrips_and_validates() {
    let mut scene = Scene::new();
    scene.sources.insert(
        "basemap".to_string(),
        SourceDef::Chain {
            providers: vec![
                SourceDef::PmtilesVector { location: "/data/norway-z8.pmtiles".to_string() },
                SourceDef::VectorXyz {
                    tiles: vec!["https://tiles.example/{z}/{x}/{y}.pbf".to_string()],
                    min_zoom: 0,
                    max_zoom: 15,
                },
            ],
        },
    );
    let json = serde_json::to_string(&scene).unwrap();
    assert!(json.contains("\"type\":\"chain\""), "{json}");
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, scene);
    assert_eq!(scene.validate(), Ok(()));
}

#[test]
fn chain_rejects_empty_nested_and_mixed_kinds() {
    let case = |def: SourceDef| {
        let mut scene = Scene::new();
        scene.sources.insert("bad".to_string(), def);
        scene.validate()
    };
    assert!(matches!(
        case(SourceDef::Chain { providers: vec![] }),
        Err(SceneError::InvalidChain { .. })
    ));
    assert!(matches!(
        case(SourceDef::Chain {
            providers: vec![SourceDef::Chain { providers: vec![] }],
        }),
        Err(SceneError::InvalidChain { .. })
    ));
    assert!(matches!(
        case(SourceDef::Chain {
            providers: vec![
                SourceDef::PmtilesVector { location: "/a.pmtiles".to_string() },
                SourceDef::PmtilesRaster { location: "/b.pmtiles".to_string() },
            ],
        }),
        Err(SceneError::InvalidChain { .. })
    ));
    // GeoJSON needs no fallback chain — rejected for kind, not silently allowed.
    assert!(matches!(
        case(SourceDef::Chain {
            providers: vec![SourceDef::GeoJson { data: "{}".to_string() }],
        }),
        Err(SceneError::InvalidChain { .. })
    ));
}

// ---- environment block + field sources (plan C1, architecture S4) --------

#[test]
fn environment_roundtrips_and_pre_c1_documents_stay_valid() {
    use turbomap_scene::{EnvironmentDef, LightingDef};
    let mut scene = sample_scene();
    scene.environment = EnvironmentDef {
        lighting: LightingDef::Fixed { azimuth_deg: 135.0, altitude_deg: 30.0 },
        terrain_shadows: 0.85,
        terrain_lit: true,
        aerial_haze: false,
        basemap_gain: 0.9,
        clouds: None,
    };
    let json = serde_json::to_string(&scene).unwrap();
    assert!(json.contains("\"mode\":\"fixed\""), "{json}");
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, scene);

    // A pre-C1 document (no environment key) parses to the neutral default.
    let old: Scene = serde_json::from_str(r#"{"sources":{},"layers":[]}"#).unwrap();
    assert_eq!(old.environment, EnvironmentDef::default());
}

#[test]
fn field2d_roundtrips_and_cannot_chain() {
    let mut scene = Scene::new();
    scene.sources.insert(
        "radar".to_string(),
        SourceDef::Field2D { bounds: [4.0, 57.0, 31.0, 71.0] },
    );
    let json = serde_json::to_string(&scene).unwrap();
    assert!(json.contains("\"type\":\"field2d\"") || json.contains("\"type\":\"field-2d\""), "{json}");
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, scene);

    let mut bad = Scene::new();
    bad.sources.insert(
        "c".to_string(),
        SourceDef::Chain {
            providers: vec![SourceDef::Field2D { bounds: [0.0, 0.0, 1.0, 1.0] }],
        },
    );
    assert!(matches!(bad.validate(), Err(SceneError::InvalidChain { .. })));
}

#[test]
fn clouds_declaration_roundtrips_and_requires_a_field_source() {
    use turbomap_scene::CloudsDef;
    let mut scene = Scene::new();
    scene.sources.insert(
        "radar".to_string(),
        SourceDef::Field2D { bounds: [4.0, 57.0, 31.0, 71.0] },
    );
    scene.environment.clouds = Some(CloudsDef {
        source: "radar".to_string(),
        grid: [128, 128],
        visible: true,
    });
    assert_eq!(scene.validate(), Ok(()));
    let json = serde_json::to_string(&scene).unwrap();
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, scene);

    // A clouds block pointing at a missing or non-field source is invalid.
    scene.sources.remove("radar");
    assert!(matches!(scene.validate(), Err(SceneError::UnknownSource { .. })));
    scene.sources.insert(
        "radar".to_string(),
        SourceDef::GeoJson { data: "{}".to_string() },
    );
    assert!(matches!(scene.validate(), Err(SceneError::UnknownSource { .. })));
}
