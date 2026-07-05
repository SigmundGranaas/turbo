//! Regression: a `vector-xyz` source with a water `fill` layer — exactly what
//! the Android app emits for realistic water (`TurbomapScene.build`) — must
//! enumerate **host-fetchable Vector pending tiles**. The existing roundtrip
//! test only uses a GeoJSON vector source (drains in-process), so the
//! host-driven vector-tile path was never exercised. If `pending_tiles` never
//! lists the water source, the host never fetches it, nothing ingests, and the
//! water surface stays empty — "topo tiles, no water".
#![cfg(feature = "gpu-tests")]

use turbomap_ffi::{Camera, TileKind, TurboMap};

fn camera() -> Camera {
    Camera {
        lat: 67.28, // Bodø coast — sea + land
        lng: 14.40,
        zoom: 11.0,
        pitch_deg: 0.0,
        bearing_deg: 0.0,
    }
}

/// Mirrors `TurbomapScene.build`: a raster basemap under a vector water fill.
fn scene_with_vector_water() -> String {
    r##"{
        "sources": {
            "r_norgeskart": { "type": "raster-xyz",
                "tiles": ["https://example.test/r/{z}/{x}/{y}.png"], "min_zoom": 0, "max_zoom": 18 },
            "v_water": { "type": "vector-xyz",
                "tiles": ["https://example.test/v/{z}/{x}/{y}.mvt"], "min_zoom": 4, "max_zoom": 15 }
        },
        "layers": [
            { "type": "raster", "id": "norgeskart", "source": "r_norgeskart" },
            { "type": "fill", "id": "water", "source": "v_water", "source-layer": "water",
              "color": { "const": { "r": 40, "g": 90, "b": 150, "a": 255 } } }
        ]
    }"##
    .to_string()
}

fn new_map() -> Option<TurboMap> {
    match TurboMap::headless(512, 384, camera()) {
        Ok(map) => Some(map),
        Err(e) => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but headless map failed: {e}");
            }
            eprintln!("SKIP: {e}");
            None
        }
    }
}

#[test]
fn vector_water_source_enumerates_host_pending_tiles() {
    let Some(map) = new_map() else { return };

    map.apply_scene(scene_with_vector_water())
        .expect("apply scene");

    let plan: serde_json::Value =
        serde_json::from_str(&map.streaming_plan_json(u32::MAX)).expect("plan json");
    let start = plan["start"].as_array().expect("start array").clone();
    let vector: Vec<_> = start.iter().filter(|t| t["kind"] == "vector").collect();

    assert!(
        !vector.is_empty(),
        "the vector water source must produce host-fetchable vector plan starts, \
         else the host never fetches water; planned kinds were: {:?}",
        start
            .iter()
            .map(|t| (t["kind"].clone(), t["layer"].clone()))
            .collect::<Vec<_>>()
    );
    assert!(
        vector
            .iter()
            .all(|t| t.layer_id.as_deref() == Some("water")),
        "vector pending tiles must be keyed by the layer id the host resolves URLs with"
    );
}
