//! The reference `ModelEngine` must satisfy the full `MapEngine`
//! conformance suite. Every real engine (the wgpu turbomap engine, the
//! MapLibre/MapKit/flutter_map adapters) runs this same suite.

use turbomap_scene::{conformance, MapEngine, ModelEngine};

#[test]
fn model_engine_satisfies_contract() {
    conformance::run_all(&|| Box::new(ModelEngine::new(1024, 768)) as Box<dyn MapEngine>);
}

#[test]
fn individual_checks_are_addressable() {
    // The checks are public so a failing engine can be debugged one
    // clause at a time, not just via the aggregate run_all.
    let mut engine = ModelEngine::new(800, 600);
    conformance::check_camera_roundtrips(&mut engine);
    conformance::check_reapply_is_noop(&mut engine);
    conformance::check_projection_roundtrips(&mut engine);
}
