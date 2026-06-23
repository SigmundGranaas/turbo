//! Regression: the Android scene sends `"source-layer"` (kebab-case), exactly as
//! TurbomapScene.build emits it. It MUST deserialize into Layer::Fill.source_layer.
use turbomap_scene::Layer;

#[test]
fn fill_source_layer_kebab_deserializes() {
    let json = r#"{"type":"fill","id":"water","source":"v_water","source-layer":"water","color":{"const":{"r":40,"g":90,"b":150,"a":255}}}"#;
    let layer: Layer = serde_json::from_str(json).expect("parse fill");
    match layer {
        Layer::Fill { source_layer, .. } => {
            assert_eq!(
                source_layer.as_deref(),
                Some("water"),
                "source-layer (kebab) must populate source_layer — got {source_layer:?}"
            );
        }
        other => panic!("expected Fill, got {other:?}"),
    }
}
