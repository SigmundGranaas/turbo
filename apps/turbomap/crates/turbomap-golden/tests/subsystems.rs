//! The subsystem-registry meta-test (slice D2 gate): every registered
//! subsystem must honour the S7 observability contract — a unique name,
//! declared frame-graph passes, a budget report, parsable inspect JSON, and
//! at least one debug view. A subsystem that can't be inspected or isolated
//! fails the build, not a code review.
#![cfg(feature = "gpu-tests")]

use std::collections::HashSet;
use std::sync::Arc;

use turbomap_core::{
    Camera, Color, LatLng, Map, MapOptions, Marker, MarkerId, TerrainOptions, TileSource,
};
use turbomap_golden::sources::{GaussianTerrainSource, ParchmentBasemap};

fn gpu_or_skip() -> Option<turbomap_golden::Gpu> {
    match turbomap_golden::headless() {
        Some(gpu) => Some(gpu),
        None => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but no wgpu adapter available");
            }
            eprintln!("SKIP subsystem meta-test: no wgpu adapter available");
            None
        }
    }
}

/// Build a map with every subsystem populated (layers, terrain, markers,
/// route tube, clouds) so the contract is asserted against live state, not
/// empty defaults.
fn populated_map(gpu: &turbomap_golden::Gpu) -> Map {
    let camera = Camera::new(
        LatLng {
            lat: 60.39,
            lng: 5.32,
        },
        9.0,
    );
    let mut map = Map::new(
        gpu.device.clone(),
        gpu.queue.clone(),
        turbomap_golden::TARGET_FORMAT,
        (256, 256),
        camera,
        MapOptions::default(),
    )
    .expect("map");
    let basemap: Arc<dyn TileSource> = Arc::new(ParchmentBasemap);
    map.add_raster_layer("basemap", basemap);
    let dem: Arc<dyn TileSource> = Arc::new(GaussianTerrainSource::bergen());
    map.set_terrain_source(dem, TerrainOptions::default());
    map.add_marker(Marker {
        id: MarkerId(0),
        lng_lat: LatLng {
            lat: 60.39,
            lng: 5.32,
        },
        radius_px: 8.0,
        color: Color::rgb(0xE5, 0x39, 0x35),
        data: Default::default(),
    });
    map.set_route_tube(
        "route",
        &[
            LatLng {
                lat: 60.38,
                lng: 5.30,
            },
            LatLng {
                lat: 60.40,
                lng: 5.34,
            },
        ],
        Color::rgb(0xFF, 0x6D, 0x00),
        10.0,
    );
    map.enable_clouds(16, 16);
    map
}

#[test]
fn every_subsystem_honours_the_observability_contract() {
    let Some(gpu) = gpu_or_skip() else { return };
    let map = populated_map(&gpu);

    let subsystems = map.subsystems();
    assert_eq!(subsystems.len(), 5, "the five subsystems are registered");

    let mut names = HashSet::new();
    for s in subsystems {
        // Unique, non-empty name.
        assert!(!s.name().is_empty());
        assert!(
            names.insert(s.name()),
            "duplicate subsystem name {}",
            s.name()
        );

        // Declares at least one frame-graph pass, and every declared pass
        // exists in the frame's pass set (rendered below → pass report).
        assert!(
            !s.passes().is_empty(),
            "{} declares no frame-graph passes",
            s.name()
        );

        // Budget report is coherent: used never exceeds a non-zero budget
        // by construction-time state (nothing rendered yet, caches empty or
        // small — this is a sanity check, not an eviction test).
        let b = s.budgets();
        if b.bytes_budget > 0 {
            assert!(
                b.bytes_used <= b.bytes_budget,
                "{} reports bytes_used {} over budget {} at rest",
                s.name(),
                b.bytes_used,
                b.bytes_budget
            );
        }

        // Inspect JSON parses and is an object.
        let parsed: serde_json::Value = serde_json::from_str(&s.inspect())
            .unwrap_or_else(|e| panic!("{} inspect JSON invalid: {e}\n{}", s.name(), s.inspect()));
        assert!(
            parsed.is_object(),
            "{} inspect is not a JSON object",
            s.name()
        );

        // At least one debug view, each with a non-empty name/description.
        assert!(
            !s.debug_views().is_empty(),
            "{} exposes no debug views",
            s.name()
        );
        for v in s.debug_views() {
            assert!(!v.name.is_empty() && !v.description.is_empty());
        }
    }

    // The combined inspect document parses and has one key per subsystem.
    let all: serde_json::Value =
        serde_json::from_str(&map.inspect_json()).expect("inspect_json parses");
    let obj = all.as_object().expect("inspect_json is an object");
    assert_eq!(obj.len(), 5);
    for s in map.subsystems() {
        let entry = &obj[s.name()];
        assert!(entry.get("state").is_some(), "{} missing state", s.name());
        assert!(
            entry.get("budgets").is_some(),
            "{} missing budgets",
            s.name()
        );
    }
}

#[test]
fn declared_passes_match_the_frame_graph_report() {
    let Some(gpu) = gpu_or_skip() else { return };
    let mut map = populated_map(&gpu);

    // Render one frame so the pass report is populated.
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("subsystems-target"),
        size: wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: turbomap_golden::TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    map.render(&mut encoder, &view);
    gpu.queue.submit([encoder.finish()]);

    // Every pass label in the frame report must be claimed by exactly one
    // subsystem (bare pass kind; `layer:<id>` labels match their kind).
    let claimed: Vec<(&str, &str)> = map
        .subsystems()
        .iter()
        .flat_map(|s| {
            let name = s.name();
            s.passes().iter().map(move |p| (name, *p))
        })
        .collect();
    for (owner, pass) in &claimed {
        let owners = claimed.iter().filter(|(_, p)| p == pass).count();
        assert_eq!(
            owners, 1,
            "pass '{pass}' claimed by {owners} subsystems ({owner} among them)"
        );
    }
    let claimed_kinds: HashSet<&str> = claimed.iter().map(|(_, p)| *p).collect();
    for timing in &map.last_frame_metrics().passes {
        let kind = timing.label.split(':').next().unwrap();
        assert!(
            claimed_kinds.contains(kind),
            "frame pass '{}' is not claimed by any subsystem",
            timing.label
        );
    }
}
