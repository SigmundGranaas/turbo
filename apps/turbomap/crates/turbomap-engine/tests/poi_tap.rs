//! Tap → feature info: tapping a POI marker in the real-data Bergen scene
//! must return that place's name and class. Closes the loop on the POI work
//! — the markers are now interactive.
//!
//! GPU-gated (engine construction needs an adapter).
#![cfg(feature = "gpu-tests")]

use std::sync::Arc;

use turbomap_core::{
    tile_local_to_world, Geometry, MapOptions, TileId, VectorTileSource, VectorValue, WorldPoint,
};
use turbomap_engine::{
    CameraState, LatLng, MapEngine, ResolvedSource, ScreenPoint, SourceResolver, TurbomapEngine,
};
use turbomap_golden::omt::{bergen_scene, fixture_path, LAND};
use turbomap_golden::{headless, sources::FlatBasemap, TARGET_FORMAT};
use turbomap_scene::SourceDef;
use turbomap_tiles_pmtiles::PMTilesSource;

struct BergenResolver;
impl SourceResolver for BergenResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(FlatBasemap(LAND))),
            SourceDef::VectorXyz { .. } => ResolvedSource::Vector(Arc::new(
                PMTilesSource::open(fixture_path()).expect("open bergen fixture"),
            )),
            _ => ResolvedSource::Unsupported,
        }
    }
}

/// Find a named food POI in the fixture and return `(name, lat, lng)`.
fn pick_food_poi() -> (String, f64, f64) {
    let src = PMTilesSource::open(fixture_path()).expect("fixture");
    for x in 8432..=8436u32 {
        for y in 4721..=4726u32 {
            let Ok(tile) =
                <PMTilesSource as VectorTileSource>::request(&src, TileId::new(14, x, y))
            else {
                continue;
            };
            for layer in &tile.layers {
                if layer.name != "poi" {
                    continue;
                }
                for f in &layer.features {
                    let class = match f.properties.get("class") {
                        Some(VectorValue::String(c)) => c.as_str(),
                        _ => continue,
                    };
                    if !matches!(class, "restaurant" | "cafe" | "bar" | "fast_food" | "pub") {
                        continue;
                    }
                    let Some(VectorValue::String(name)) = f.properties.get("name") else {
                        continue;
                    };
                    if let Geometry::Point(points) = &f.geometry {
                        if let Some(&p) = points.first() {
                            let (wx, wy) =
                                tile_local_to_world(TileId::new(14, x, y), layer.extent, p);
                            let ll = WorldPoint::new(wx, wy).to_lat_lng();
                            return (name.clone(), ll.lat, ll.lng);
                        }
                    }
                }
            }
        }
    }
    panic!("no named food POI in the fixture");
}

#[test]
fn tapping_a_poi_returns_its_name_and_class() {
    let Some(gpu) = headless() else {
        if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
            panic!("REQUIRE_GPU=1 but no wgpu adapter available");
        }
        eprintln!("SKIP: no wgpu adapter available");
        return;
    };

    let (name, lat, lng) = pick_food_poi();

    let (width, height) = (1280, 880);
    let mut engine = TurbomapEngine::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        TARGET_FORMAT,
        (width, height),
        // Centre the camera on the POI so it lands at screen centre.
        CameraState::new(LatLng::new(lat, lng), 15.0),
        MapOptions {
            fade_in_secs: 0.0,
            pixel_ratio: 2.0,
            ..Default::default()
        },
        Box::new(BergenResolver),
    )
    .expect("construct TurbomapEngine");

    engine.apply(bergen_scene());
    engine.pump_tiles();

    // Tap the centre — where the chosen POI sits — and find it by name.
    let centre = ScreenPoint::new(width as f64 / 2.0, height as f64 / 2.0);
    let hits = engine.hit_test(centre, 28.0);
    let hit = hits
        .iter()
        .find(|h| h.properties.get("name") == Some(&name))
        .unwrap_or_else(|| panic!("tap should report POI '{name}'; got {hits:?}"));

    assert!(
        hit.layer_id.starts_with("poi-"),
        "attributed to a POI layer"
    );
    assert!(hit.properties.contains_key("class"), "carries the class");
    assert_eq!(hit.properties.get("name"), Some(&name));

    // A tap on empty water far from any POI returns no feature.
    let empty = engine.hit_test(ScreenPoint::new(8.0, height as f64 - 8.0), 4.0);
    assert!(
        empty
            .iter()
            .all(|h| h.properties.get("name") != Some(&name)),
        "the POI isn't reported from across the map"
    );
}
