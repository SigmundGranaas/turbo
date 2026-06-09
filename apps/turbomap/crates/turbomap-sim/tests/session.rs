//! Behavioural session tests — the assertions a user would make watching
//! the map: everything paints, zooming never blanks the screen, the
//! picture settles when motion stops, and frame cost stays bounded.
#![cfg(feature = "gpu-tests")]

use std::time::Duration;

use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, MapEngine};
use turbomap_sim::{basemap_scene, fraction_near, session, PerfSummary, Sim};

const W: u32 = 480;
const H: u32 = 320;

fn sim_or_skip(zoom: f64, options: MapOptions) -> Option<Sim> {
    match Sim::new(W, H, Sim::start_camera(zoom), options) {
        Some(sim) => Some(sim),
        None => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but no wgpu adapter available");
            }
            eprintln!("SKIP: no wgpu adapter available");
            None
        }
    }
}

fn no_fade() -> MapOptions {
    MapOptions {
        fade_in_secs: 0.0,
        ..Default::default()
    }
}

#[test]
fn cold_load_paints_every_subsystem() {
    // Centred on a major crossroads, so the amber arterial grid is in
    // view alongside minor roads, lakes, and labels — every styling path
    // on one screen.
    let Some(mut sim) = (match Sim::new(
        W,
        H,
        Sim::camera_at_major_crossroads(12.0),
        no_fade(),
    ) {
        Some(sim) => Some(sim),
        None => {
            if std::env::var("REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("REQUIRE_GPU=1 but no wgpu adapter available");
            }
            eprintln!("SKIP: no wgpu adapter available");
            None
        }
    }) else {
        return;
    };
    sim.engine.apply(basemap_scene());
    let settled = sim.run_until_stable(120, 0.001);
    assert!(settled.is_some(), "cold load must settle, stats: {:?}", sim.stats.last());

    let img = sim.last.as_ref().expect("a rendered frame");
    let land = fraction_near(img, session::LAND_SRGB, 8);
    let water = fraction_near(img, session::WATER_SRGB, 10);
    let road_white = fraction_near(img, session::ROAD_INNER_SRGB, 10);
    let road_major = fraction_near(img, session::ROAD_MAJOR_SRGB, 14);
    let blank = fraction_near(img, session::CLEAR_SRGB, 6);

    assert!(land > 0.30, "land raster should dominate, got {land:.3}");
    assert!(water > 0.005, "lakes should be visible, got {water:.4}");
    assert!(road_white > 0.005, "minor roads should be visible, got {road_white:.4}");
    assert!(road_major > 0.0005, "major (amber) roads should be visible, got {road_major:.5}");
    assert!(blank < 0.01, "nothing should remain unloaded, got {blank:.4}");
    // Labels: dark ink that is neither casing nor land. Count loosely.
    let ink = fraction_near(img, session::LABEL_SRGB, 25);
    assert!(ink > 0.0002, "place labels should be visible, got {ink:.5}");
}

#[test]
fn zoom_journey_never_shows_a_blank_map() {
    // Realistic settings: tile fades ON, 3 frames of network latency.
    let Some(mut sim) = sim_or_skip(11.0, MapOptions::default()) else {
        return;
    };
    sim.engine.apply(basemap_scene());
    assert!(
        sim.run_until_stable(200, 0.002).is_some(),
        "initial load must settle"
    );

    sim.latency_frames = 3;
    let from = sim.camera();
    sim.engine.ease_to(
        CameraState {
            zoom: 13.0,
            ..from
        },
        Duration::from_millis(700),
    );

    let mut worst_blank: f64 = 0.0;
    let mut delivered = 0u64;
    for _ in 0..600 {
        let s = sim.step();
        worst_blank = worst_blank.max(s.blank_frac);
        delivered += s.delivered as u64;
        if !s.animating && s.in_flight == 0 {
            break;
        }
    }
    assert!(
        delivered > 0,
        "crossing two tile-zoom levels must fetch new tiles"
    );
    // The user-facing promise: ancestor fallback + fades keep the map
    // covered even though every tile arrives 3 frames late mid-zoom.
    assert!(
        worst_blank < 0.35,
        "map must never blank out during a zoom, worst was {worst_blank:.3}"
    );
    assert!((sim.camera().zoom - 13.0).abs() < 1e-9, "animation must land");

    // And it must settle afterwards.
    assert!(
        sim.run_until_stable(300, 0.002).is_some(),
        "post-zoom frames must converge, last: {:?}",
        sim.stats.last()
    );
}

#[test]
fn pan_session_stays_covered_and_settles_without_flicker() {
    let Some(mut sim) = sim_or_skip(12.0, MapOptions::default()) else {
        return;
    };
    sim.engine.apply(basemap_scene());
    assert!(sim.run_until_stable(200, 0.002).is_some());

    // Drag east for 40 frames (a steady one-finger pan), tiles 2 frames late.
    sim.latency_frames = 2;
    let start = sim.camera();
    for i in 1..=40u32 {
        let mut cam = start;
        cam.center.lng += i as f64 * 0.0025;
        sim.engine.set_camera(cam);
        let s = sim.step();
        assert!(
            s.blank_frac < 0.35,
            "panning must keep the map covered, frame {i} blank={:.3}",
            s.blank_frac
        );
    }
    assert!(
        (sim.camera().lng_of() - start.center.lng).abs() > 0.05,
        "camera should have moved"
    );

    // Stop: converge, then hold perfectly still — zero flicker.
    assert!(
        sim.run_until_stable(300, 0.002).is_some(),
        "post-pan frames must converge"
    );
    for _ in 0..5 {
        let s = sim.step();
        assert!(
            s.diff_frac <= 0.002,
            "steady state must not flicker, diff={:.4}",
            s.diff_frac
        );
    }
}

/// CameraState helper: lng without reaching through fields in asserts.
trait LngOf {
    fn lng_of(&self) -> f64;
}
impl LngOf for CameraState {
    fn lng_of(&self) -> f64 {
        self.center.lng
    }
}

#[test]
fn frame_cost_stays_within_budget() {
    // Relative regression caps for the software rasteriser. Generous on
    // purpose: they catch "suddenly 3× slower", not absolute mobile perf.
    let Some(mut sim) = sim_or_skip(11.0, MapOptions::default()) else {
        return;
    };
    sim.engine.apply(basemap_scene());
    sim.run_until_stable(200, 0.002);
    sim.stats.clear();

    // Steady state, then a zoom under latency — the expensive regime.
    for _ in 0..30 {
        sim.step();
    }
    sim.latency_frames = 3;
    sim.engine.ease_to(
        CameraState {
            zoom: 13.0,
            ..sim.camera()
        },
        Duration::from_millis(700),
    );
    for _ in 0..300 {
        let s = sim.step();
        if !s.animating && s.in_flight == 0 {
            break;
        }
    }

    let summary = PerfSummary::from_stats(&sim.stats);
    eprintln!("perf: {summary:?}");
    // Persist for inspection/CI artifact.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/sim-reports");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("frame-budget.json"),
        serde_json::to_string_pretty(&summary).expect("serialise summary"),
    );

    assert!(summary.frames > 30, "session too short to judge");
    assert!(
        summary.cpu_ms_p95 < 200.0,
        "p95 frame CPU cost regressed: {summary:?}"
    );
    assert!(
        summary.worst_blank_frac < 0.35,
        "loading quality regressed: {summary:?}"
    );
}
