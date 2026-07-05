//! Exhaustive `diff` matrix — the minimal change set for every kind of edit.

use turbomap_scene::diff::{LayerChange, SourceChange};
use turbomap_scene::style::Filter;
use turbomap_scene::{diff, Color, Layer, Paint, Scene, SourceDef};

fn base_source() -> SourceDef {
    SourceDef::RasterXyz {
        tiles: vec!["https://example.test/{z}/{x}/{y}.png".to_string()],
        tile_size: 256,
        min_zoom: 0,
        max_zoom: 22,
        attribution: None,
    }
}

fn raster(id: &str) -> Layer {
    Layer::Raster {
        id: id.to_string(),
        source: "base".to_string(),
        opacity: Paint::Const(1.0),
    }
}

fn line(id: &str, color: Color) -> Layer {
    Layer::Line {
        id: id.to_string(),
        source: "base".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(color),
        width: Paint::Const(2.0),
        dash_array: None,
    }
}

fn scene(layers: Vec<Layer>) -> Scene {
    let mut s = Scene::new();
    s.sources.insert("base".to_string(), base_source());
    s.layers = layers;
    s
}

#[test]
fn identical_scenes_produce_empty_delta() {
    let s = scene(vec![raster("a"), raster("b")]);
    assert!(diff(&s, &s).is_empty());
}

#[test]
fn append_layer_is_added_at_end() {
    let old = scene(vec![raster("a")]);
    let new = scene(vec![raster("a"), raster("b")]);
    let d = diff(&old, &new);
    assert_eq!(
        d.layers,
        vec![LayerChange::Added {
            id: "b".into(),
            index: 1
        }]
    );
}

#[test]
fn insert_layer_in_middle_reports_correct_index() {
    let old = scene(vec![raster("a"), raster("c")]);
    let new = scene(vec![raster("a"), raster("b"), raster("c")]);
    let d = diff(&old, &new);
    assert_eq!(
        d.layers,
        vec![LayerChange::Added {
            id: "b".into(),
            index: 1
        }]
    );
}

#[test]
fn removed_layer_is_reported() {
    let old = scene(vec![raster("a"), raster("b")]);
    let new = scene(vec![raster("a")]);
    let d = diff(&old, &new);
    assert_eq!(d.layers, vec![LayerChange::Removed { id: "b".into() }]);
}

#[test]
fn repaint_is_update_only() {
    let old = scene(vec![line("route", Color::rgb(255, 0, 0))]);
    let new = scene(vec![line("route", Color::rgb(0, 0, 255))]);
    let d = diff(&old, &new);
    assert_eq!(d.layers, vec![LayerChange::Updated { id: "route".into() }]);
    assert!(d.sources.is_empty());
}

#[test]
fn swapping_two_layers_is_one_minimal_move() {
    let old = scene(vec![raster("a"), raster("b"), raster("c")]);
    let new = scene(vec![raster("b"), raster("a"), raster("c")]);
    let d = diff(&old, &new);
    let moves: Vec<_> = d
        .layers
        .iter()
        .filter(|c| matches!(c, LayerChange::Moved { .. }))
        .collect();
    assert_eq!(moves.len(), 1, "expected exactly one move, got {d:?}");
    // Nothing else churns.
    assert_eq!(d.layers.len(), 1, "{d:?}");
    if let LayerChange::Moved { from, to, .. } = moves[0] {
        assert_ne!(from, to);
    }
}

#[test]
fn moved_and_repainted_layer_reports_both() {
    let old = scene(vec![line("x", Color::rgb(1, 1, 1)), raster("y")]);
    let new = scene(vec![raster("y"), line("x", Color::rgb(2, 2, 2))]);
    let d = diff(&old, &new);
    assert!(d
        .layers
        .iter()
        .any(|c| matches!(c, LayerChange::Updated { id } if id == "x")));
    assert!(d
        .layers
        .iter()
        .any(|c| matches!(c, LayerChange::Moved { id, .. } if id == "x" || id == "y")));
}

#[test]
fn source_added_updated_removed() {
    let mut old = scene(vec![raster("a")]);
    let mut new = old.clone();
    new.sources.insert(
        "overlay".to_string(),
        SourceDef::GeoJson {
            data: "{}".to_string(),
        },
    );
    assert_eq!(
        diff(&old, &new).sources,
        vec![SourceChange::Added("overlay".into())]
    );

    // Update the existing geojson data.
    old = new.clone();
    new.sources.insert(
        "overlay".to_string(),
        SourceDef::GeoJson {
            data: "{\"x\":1}".to_string(),
        },
    );
    assert_eq!(
        diff(&old, &new).sources,
        vec![SourceChange::Updated("overlay".into())]
    );

    // Remove it.
    old = new.clone();
    new.sources.remove("overlay");
    assert_eq!(
        diff(&old, &new).sources,
        vec![SourceChange::Removed("overlay".into())]
    );
}

#[test]
fn add_and_remove_in_one_diff() {
    let old = scene(vec![raster("a"), raster("b")]);
    let new = scene(vec![raster("a"), raster("c")]);
    let d = diff(&old, &new);
    assert!(d.layers.contains(&LayerChange::Removed { id: "b".into() }));
    assert!(d.layers.contains(&LayerChange::Added {
        id: "c".into(),
        index: 1
    }));
}

#[test]
fn an_environment_edit_is_an_environment_only_delta() {
    use turbomap_scene::{diff::diff, LightingDef, Scene};
    let a = Scene::new();
    let mut b = Scene::new();
    b.environment.lighting = LightingDef::TimeTracked { unix_seconds: 1.0 };
    let d = diff(&a, &b);
    assert_eq!(d.environment.as_ref(), Some(&b.environment));
    assert!(d.sources.is_empty() && d.layers.is_empty());
    assert!(
        diff(&b, &b).is_empty(),
        "identical environments must not dirty the delta"
    );
}
