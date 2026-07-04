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
