//! The `MapEngine` conformance suite.
//!
//! One behavioral spec that *every* engine implementation must satisfy —
//! the wgpu `turbomap` engine and each legacy adapter alike. Run it
//! against an engine factory; each check is a named `pub fn` so a failure
//! points at the exact contract clause that broke.
//!
//! These checks are renderer-agnostic and GPU-free: they pin down scene
//! diffing, camera, and flat projection. Pixel-level behavior is covered
//! by the golden suite (`turbomap-golden`); cross-engine pixel parity by
//! the future shadow/differential harness.

use crate::diff::{LayerChange, SourceChange};
use crate::engine::{CameraState, MapEngine};
use crate::geo::LatLng;
use crate::scene::{Layer, Scene, SourceDef};
use crate::style::{Color, Filter, Paint};

// ---- scene builders -------------------------------------------------------

fn raster_scene(layer_ids: &[&str]) -> Scene {
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
    for id in layer_ids {
        scene.layers.push(Layer::Raster {
            id: (*id).to_string(),
            source: "base".to_string(),
            opacity: Paint::Const(1.0),
        });
    }
    scene
}

fn line_scene(color: Color) -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "route".to_string(),
        SourceDef::GeoJson {
            data: "{\"type\":\"FeatureCollection\",\"features\":[]}".to_string(),
        },
    );
    scene.layers.push(Layer::Line {
        id: "route-l".to_string(),
        source: "route".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(color),
        width: Paint::Const(2.0),
        dash_array: None,
    });
    scene
}

fn geojson_scene(data: &str) -> Scene {
    let mut scene = Scene::new();
    scene.sources.insert(
        "route".to_string(),
        SourceDef::GeoJson {
            data: data.to_string(),
        },
    );
    scene.layers.push(Layer::Line {
        id: "route-l".to_string(),
        source: "route".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(0, 0, 0)),
        width: Paint::Const(2.0),
        dash_array: None,
    });
    scene
}

// ---- checks ---------------------------------------------------------------

/// A backend must report at least a usable texture size.
pub fn check_capabilities(engine: &mut dyn MapEngine) {
    let caps = engine.capabilities();
    assert!(
        caps.max_texture_size > 0,
        "capabilities.max_texture_size must be positive"
    );
}

/// `camera()` round-trips `set_camera()`.
pub fn check_camera_roundtrips(engine: &mut dyn MapEngine) {
    let cam = CameraState {
        center: LatLng::new(60.39, 5.32),
        zoom: 11.0,
        pitch_deg: 0.0,
        bearing_deg: 0.0,
    };
    engine.set_camera(cam);
    assert_eq!(
        engine.camera(),
        cam,
        "camera() must return what set_camera() stored"
    );
}

/// Applying a scene records it; re-applying the identical scene is a no-op.
pub fn check_reapply_is_noop(engine: &mut dyn MapEngine) {
    let scene = raster_scene(&["base-l"]);
    engine.apply(scene.clone());
    assert_eq!(
        engine.scene(),
        &scene,
        "scene() must reflect the applied scene"
    );
    let delta = engine.apply(scene);
    assert!(
        delta.is_empty(),
        "re-applying the same scene must produce an empty delta, got {delta:?}"
    );
}

/// Applying the empty scene removes every layer.
pub fn check_empty_scene_removes_all(engine: &mut dyn MapEngine) {
    engine.apply(raster_scene(&["a", "b"]));
    let delta = engine.apply(Scene::new());
    assert!(
        engine.scene().layers.is_empty(),
        "scene must be empty after applying an empty scene"
    );
    let removed = delta
        .layers
        .iter()
        .filter(|c| matches!(c, LayerChange::Removed { .. }))
        .count();
    assert_eq!(
        removed, 2,
        "emptying a 2-layer scene must remove exactly 2 layers, got {delta:?}"
    );
}

/// Adding then removing a layer is reported as Added then Removed.
pub fn check_add_then_remove(engine: &mut dyn MapEngine) {
    let added = engine.apply(raster_scene(&["a"]));
    assert!(
        added
            .layers
            .iter()
            .any(|c| matches!(c, LayerChange::Added { id, .. } if id == "a")),
        "adding a layer must report Added, got {added:?}"
    );
    let removed = engine.apply(Scene::new());
    assert!(
        removed
            .layers
            .iter()
            .any(|c| matches!(c, LayerChange::Removed { id } if id == "a")),
        "removing a layer must report Removed, got {removed:?}"
    );
}

/// Reordering layers is the minimal set of moves and nothing else.
pub fn check_reorder_is_minimal(engine: &mut dyn MapEngine) {
    engine.apply(raster_scene(&["a", "b", "c"]));
    let delta = engine.apply(raster_scene(&["b", "a", "c"]));
    assert!(
        !delta.layers.iter().any(|c| matches!(
            c,
            LayerChange::Added { .. } | LayerChange::Removed { .. } | LayerChange::Updated { .. }
        )),
        "a pure reorder must not add/remove/update, got {delta:?}"
    );
    let moves = delta
        .layers
        .iter()
        .filter(|c| matches!(c, LayerChange::Moved { .. }))
        .count();
    assert_eq!(
        moves, 1,
        "swapping two adjacent layers must be exactly one Moved, got {delta:?}"
    );
}

/// Changing only paint is a single Updated, touching no sources.
pub fn check_repaint_is_update_only(engine: &mut dyn MapEngine) {
    engine.apply(line_scene(Color::rgb(255, 0, 0)));
    let delta = engine.apply(line_scene(Color::rgb(0, 0, 255)));
    assert_eq!(
        delta.layers.len(),
        1,
        "a repaint should be a single layer change, got {delta:?}"
    );
    assert!(
        matches!(delta.layers[0], LayerChange::Updated { .. }),
        "a repaint must be an Updated, got {delta:?}"
    );
    assert!(
        delta.sources.is_empty(),
        "a repaint must not touch sources, got {delta:?}"
    );
}

/// Changing source data is a source Updated with no layer churn.
pub fn check_source_update_detected(engine: &mut dyn MapEngine) {
    engine.apply(geojson_scene("{\"v\":1}"));
    let delta = engine.apply(geojson_scene("{\"v\":2}"));
    assert!(
        delta
            .sources
            .iter()
            .any(|c| matches!(c, SourceChange::Updated(_))),
        "changing source data must report a source Updated, got {delta:?}"
    );
    assert!(
        delta.layers.is_empty(),
        "a source-only change must not churn layers, got {delta:?}"
    );
}

/// `project` then `unproject` returns the original coordinate (pitch 0).
/// Changing only the scene-declared environment yields an
/// environment-only delta — no source or layer churn — and reapplying the
/// same environment is a no-op (plan C1: the side-doors are absorbed, so
/// environment edits must be as observable and minimal as any other).
pub fn check_environment_diffing(engine: &mut dyn MapEngine) {
    use crate::scene::LightingDef;
    let base = raster_scene(&["base-l"]);
    engine.apply(base.clone());

    let mut lit = base.clone();
    lit.environment.lighting = LightingDef::TimeTracked { unix_seconds: 1_750_000_000.0 };
    lit.environment.terrain_shadows = 0.85;
    let delta = engine.apply(lit.clone());
    assert_eq!(
        delta.environment.as_ref(),
        Some(&lit.environment),
        "an environment edit must surface as delta.environment"
    );
    assert!(delta.sources.is_empty(), "environment edit must not churn sources");
    assert!(delta.layers.is_empty(), "environment edit must not churn layers");

    let again = engine.apply(lit);
    assert!(again.is_empty(), "reapplying the same environment must be a no-op");
}

/// Updating a `Field2D` source's definition is a source `Updated` with no
/// layer churn — field data rides the same declarative rails as tiles.
pub fn check_field_source_update(engine: &mut dyn MapEngine) {
    let mut scene = raster_scene(&["base-l"]);
    scene.sources.insert(
        "radar".to_string(),
        SourceDef::Field2D { bounds: [4.0, 57.0, 31.0, 71.0] },
    );
    engine.apply(scene.clone());

    scene.sources.insert(
        "radar".to_string(),
        SourceDef::Field2D { bounds: [3.0, 56.0, 32.0, 72.0] },
    );
    let delta = engine.apply(scene);
    assert_eq!(
        delta.sources,
        vec![SourceChange::Updated("radar".to_string())],
        "a field-source edit must be a source Updated"
    );
    assert!(delta.layers.is_empty(), "field edit must not churn layers");
}

pub fn check_projection_roundtrips(engine: &mut dyn MapEngine) {
    engine.resize(1024, 768);
    engine.set_camera(CameraState {
        center: LatLng::new(60.39, 5.32),
        zoom: 11.0,
        pitch_deg: 0.0,
        bearing_deg: 0.0,
    });
    for &(lat, lng) in &[(60.39, 5.32), (60.40, 5.33), (60.38, 5.30)] {
        let original = LatLng::new(lat, lng);
        if let Some(screen) = engine.project(original) {
            let back = engine
                .unproject(screen)
                .expect("unproject of a projected point");
            assert!(
                (back.lat - lat).abs() < 1e-6 && (back.lng - lng).abs() < 1e-6,
                "project∘unproject drifted: {original:?} -> {screen:?} -> {back:?}"
            );
        }
    }
}

/// Run the entire suite against engines from `factory`. A fresh engine is
/// built per check so mutations don't leak between cases.
pub fn run_all(factory: &dyn Fn() -> Box<dyn MapEngine>) {
    check_capabilities(&mut *factory());
    check_camera_roundtrips(&mut *factory());
    check_reapply_is_noop(&mut *factory());
    check_empty_scene_removes_all(&mut *factory());
    check_add_then_remove(&mut *factory());
    check_reorder_is_minimal(&mut *factory());
    check_repaint_is_update_only(&mut *factory());
    check_source_update_detected(&mut *factory());
    check_environment_diffing(&mut *factory());
    check_field_source_update(&mut *factory());
    check_projection_roundtrips(&mut *factory());
}
