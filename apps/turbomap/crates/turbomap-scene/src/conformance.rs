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
use crate::geo::{LatLng, ScreenPoint};
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

/// `Layer::Custom` is an ordinary stack layer at the IR level (plan D4):
/// applying a scene with one reports `Added`, removing it reports
/// `Removed`, and a backend claiming `capabilities().custom_layers` must
/// accept the apply without panicking — binding-or-degrading is the
/// engine's business, but the CONTRACT semantics are backend-independent.
pub fn check_custom_layer_roundtrip(engine: &mut dyn MapEngine) {
    assert!(
        engine.capabilities().custom_layers,
        "custom layers are contract-real since plan D4 — a backend that \
         cannot bind them must degrade per layer, not deny the capability"
    );
    let mut scene = raster_scene(&["base"]);
    scene.layers.push(Layer::Custom {
        id: "fx".to_string(),
        kind: "flow-field".to_string(),
    });
    let added = engine.apply(scene);
    assert!(
        added
            .layers
            .iter()
            .any(|c| matches!(c, LayerChange::Added { id, .. } if id == "fx")),
        "adding a custom layer must report Added, got {added:?}"
    );
    let removed = engine.apply(raster_scene(&["base"]));
    assert!(
        removed
            .layers
            .iter()
            .any(|c| matches!(c, LayerChange::Removed { id } if id == "fx")),
        "removing a custom layer must report Removed, got {removed:?}"
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
    lit.environment.lighting = LightingDef::TimeTracked {
        unix_seconds: 1_750_000_000.0,
    };
    lit.environment.terrain_shadows = 0.85;
    let delta = engine.apply(lit.clone());
    assert_eq!(
        delta.environment.as_ref(),
        Some(&lit.environment),
        "an environment edit must surface as delta.environment"
    );
    assert!(
        delta.sources.is_empty(),
        "environment edit must not churn sources"
    );
    assert!(
        delta.layers.is_empty(),
        "environment edit must not churn layers"
    );

    let again = engine.apply(lit);
    assert!(
        again.is_empty(),
        "reapplying the same environment must be a no-op"
    );
}

/// Updating a `Field2D` source's definition is a source `Updated` with no
/// layer churn — field data rides the same declarative rails as tiles.
pub fn check_field_source_update(engine: &mut dyn MapEngine) {
    let mut scene = raster_scene(&["base-l"]);
    scene.sources.insert(
        "radar".to_string(),
        SourceDef::Field2D {
            bounds: [4.0, 57.0, 31.0, 71.0],
        },
    );
    engine.apply(scene.clone());

    scene.sources.insert(
        "radar".to_string(),
        SourceDef::Field2D {
            bounds: [3.0, 56.0, 32.0, 72.0],
        },
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

/// Plan P6.4: scene-declared point content answers hit tests with its
/// geo-json feature properties. The pinned contract: a tap on a `circle`
/// layer's point (top-down camera) yields a hit whose `layer_id` is the
/// circle layer and whose `properties` carry the feature's attributes; a tap
/// far from any content yields none. `feature_id` is engine-internal and
/// deliberately NOT pinned.
pub fn check_hit_test_semantics(engine: &mut dyn MapEngine) {
    engine.resize(512, 512);
    engine.set_camera(CameraState {
        center: LatLng::new(60.39, 5.32),
        zoom: 12.0,
        pitch_deg: 0.0,
        bearing_deg: 0.0,
    });
    let mut scene = Scene::new();
    scene.sources.insert(
        "pins-src".into(),
        SourceDef::GeoJson {
            data: r#"{"type":"FeatureCollection","features":[
                {"type":"Feature","properties":{"id":"mk-1","name":"Bergen"},
                 "geometry":{"type":"Point","coordinates":[5.32,60.39]}}]}"#
                .into(),
        },
    );
    scene.layers.push(Layer::Circle {
        id: "pins".into(),
        source: "pins-src".into(),
        source_layer: None,
        filter: Filter::default(),
        color: Paint::constant(Color {
            r: 229,
            g: 57,
            b: 53,
            a: 255,
        }),
        radius: Paint::constant(8.0),
    });
    engine.apply(scene);

    let centre = engine
        .project(LatLng::new(60.39, 5.32))
        .expect("pin projects on-screen top-down");
    let hits = engine.hit_test(centre, 12.0);
    let hit = hits
        .iter()
        .find(|h| h.layer_id == "pins")
        .unwrap_or_else(|| panic!("a tap on the pin must hit the circle layer; got {hits:?}"));
    assert_eq!(
        hit.properties.get("id").map(String::as_str),
        Some("mk-1"),
        "geo-json feature properties must ride into the hit: {hit:?}"
    );
    assert_eq!(
        hit.properties.get("name").map(String::as_str),
        Some("Bergen"),
        "geo-json feature properties must ride into the hit: {hit:?}"
    );

    let far = ScreenPoint::new(centre.x + 200.0, centre.y + 200.0);
    let miss = engine.hit_test(far, 12.0);
    assert!(
        miss.iter().all(|h| h.layer_id != "pins"),
        "a tap 200px away must not hit the pin: {miss:?}"
    );
}

/// Plan P6.5 — compositing honesty. The scene's layer order is ONE ordered
/// stack across kinds: a `circle` interleaves with `fill`/`line`/`tube` at
/// its declared index (C3's "overlay track" exception is retired), so a
/// cross-kind reorder must be an ordinary minimal `Moved` — never a
/// remove/re-add of "track" content — and the applied scene must hold the
/// interleaved order verbatim (index = draw order; pixel truth is pinned by
/// the `interleave-content` golden).
///
/// The one documented residual: `symbol` label/icon CONTENT renders in the
/// screen-space symbol track above the stack (see `Layer::Symbol` docs);
/// its slot still diffs/orders like every layer, which this check includes.
pub fn check_cross_track_ordering(engine: &mut dyn MapEngine) {
    fn layer_ids(scene: &Scene) -> Vec<&str> {
        scene.layers.iter().map(|l| l.id()).collect()
    }
    // One geo-json source feeds line + circle + tube; fill gets a polygon.
    let mut scene = raster_scene(&["base"]);
    scene.sources.insert(
        "content".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"LineString","coordinates":[[5.1,60.3],[5.32,60.39],[5.55,60.48]]}"#
                .to_string(),
        },
    );
    scene.sources.insert(
        "area".to_string(),
        SourceDef::GeoJson {
            data: r#"{"type":"Polygon","coordinates":[[[5.2,60.34],[5.45,60.34],[5.45,60.45],[5.2,60.45],[5.2,60.34]]]}"#
                .to_string(),
        },
    );
    let line = Layer::Line {
        id: "trail".to_string(),
        source: "content".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(20, 90, 200)),
        width: Paint::Const(3.0),
        dash_array: None,
    };
    let circle = Layer::Circle {
        id: "pins".to_string(),
        source: "content".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(255, 200, 0)),
        radius: Paint::Const(8.0),
    };
    let tube = Layer::Tube {
        id: "route-3d".to_string(),
        source: "content".to_string(),
        color: Color::rgba(143, 76, 56, 255),
        radius_px: 6.0,
    };
    let fill = Layer::Fill {
        id: "zone".to_string(),
        source: "area".to_string(),
        source_layer: None,
        filter: Filter::Always,
        color: Paint::Const(Color::rgb(40, 140, 90)),
        opacity: Paint::Const(1.0),
    };

    // An interleaved cross-kind stack must be held verbatim: tube below
    // circle below fill ("circle below a fill" is the exact ordering C3
    // called inexpressible).
    let mut a = scene.clone();
    a.layers
        .extend([line.clone(), tube.clone(), circle.clone(), fill.clone()]);
    engine.apply(a.clone());
    assert_eq!(
        layer_ids(engine.scene()),
        vec!["base", "trail", "route-3d", "pins", "zone"],
        "the applied scene must hold the interleaved cross-kind order"
    );

    // Moving the circle ABOVE the fill is a pure reorder — one Moved, no
    // add/remove/update churn. Cross-kind order is ordinary stack order.
    let mut b = scene;
    b.layers.extend([line, tube, fill, circle]);
    let delta = engine.apply(b);
    assert!(
        !delta.layers.iter().any(|c| matches!(
            c,
            LayerChange::Added { .. } | LayerChange::Removed { .. } | LayerChange::Updated { .. }
        )),
        "a cross-kind reorder must not add/remove/update, got {delta:?}"
    );
    let moves = delta
        .layers
        .iter()
        .filter(|c| matches!(c, LayerChange::Moved { .. }))
        .count();
    assert_eq!(
        moves, 1,
        "swapping a circle over a fill must be exactly one Moved, got {delta:?}"
    );
    assert_eq!(
        layer_ids(engine.scene()),
        vec!["base", "trail", "route-3d", "zone", "pins"],
        "the reordered cross-kind stack must be held verbatim"
    );
}

/// Run the entire suite against engines from `factory`. A fresh engine is
/// built per check so mutations don't leak between cases.
pub fn run_all(factory: &dyn Fn() -> Box<dyn MapEngine>) {
    check_capabilities(&mut *factory());
    check_camera_roundtrips(&mut *factory());
    check_reapply_is_noop(&mut *factory());
    check_empty_scene_removes_all(&mut *factory());
    check_add_then_remove(&mut *factory());
    check_custom_layer_roundtrip(&mut *factory());
    check_reorder_is_minimal(&mut *factory());
    check_cross_track_ordering(&mut *factory());
    check_repaint_is_update_only(&mut *factory());
    check_source_update_detected(&mut *factory());
    check_environment_diffing(&mut *factory());
    check_field_source_update(&mut *factory());
    check_projection_roundtrips(&mut *factory());
    check_hit_test_semantics(&mut *factory());
}
