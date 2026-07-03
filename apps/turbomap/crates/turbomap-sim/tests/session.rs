//! Behavioural session tests — the assertions a user would make watching
//! the map: everything paints, zooming never blanks the screen, the
//! picture settles when motion stops, and frame cost stays bounded.
#![cfg(feature = "gpu-tests")]

use std::time::Duration;

use turbomap_core::MapOptions;
use turbomap_engine::{CameraState, LatLng, MapEngine};
use turbomap_sim::{
    basemap_scene, basemap_scene_3d, fraction_near, session, PerfSummary, Sim,
};

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
        640,
        420,
        Sim::camera_at_major_crossroads(11.0),
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
    // Screen-space assertions compare against the ONSCREEN_* constants — the
    // authored palette after the HDR post pipeline (see their doc comment in
    // `session.rs`). Water has no stable exact on-screen colour (small share,
    // AA-blended lake edges), so it uses a blue-dominance heuristic like
    // landuse's green one: tonemapped water ≈ (171,196,209), blue leads red
    // by ~38 while every other palette entry is near-neutral (Δ ≤ 4).
    let land = fraction_near(img, session::ONSCREEN_LAND_SRGB, 8);
    let water = img
        .pixels()
        .filter(|p| p.0[2] > p.0[0].saturating_add(20) && p.0[2] > 150)
        .count() as f64
        / (img.width() * img.height()) as f64;
    let landuse = img
        .pixels()
        .filter(|p| p.0[1] > p.0[0] && p.0[1] > p.0[2] && p.0[1] > 150)
        .count() as f64
        / (img.width() * img.height()) as f64;
    let road_white = fraction_near(img, session::ONSCREEN_ROAD_INNER_SRGB, 10);
    let road_major = fraction_near(img, session::ONSCREEN_ROAD_MAJOR_SRGB, 14);
    let blank = fraction_near(img, session::ONSCREEN_CLEAR_SRGB, 6);

    assert!(land > 0.25, "land raster should dominate, got {land:.3}");
    assert!(water > 0.005, "lakes should be visible, got {water:.4}");
    assert!(landuse > 0.005, "landuse (parks/woods) should be visible, got {landuse:.4}");
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

#[test]
fn heavy_roaming_under_a_tight_cache_budget_keeps_reloading_tiles() {
    // Regression for the "grey tiles that never reload until you switch the
    // map layer" bug. The GPU texture cache evicts tiles past its budget,
    // but the engine's per-layer `ingested` set used to be insert-only — so
    // an evicted tile stayed marked resident, dropped out of `pending`
    // forever, and never got re-requested. Switching layers rebuilt the
    // Scene (fresh `ingested`) which is why it "fixed itself".
    //
    // Here we pin a tight cache budget, roam across far-apart regions to
    // force eviction of the start area, then return to the exact start. The
    // map MUST fully re-paint — proving evicted-but-still-desired tiles get
    // re-requested. Before the fix this returned a grey, hole-ridden frame.
    let opts = MapOptions {
        fade_in_secs: 0.0,
        // ~48 tiles — comfortably holds one screen, but a fraction of what
        // roaming across six regions loads, so eviction is guaranteed.
        cache_budget_bytes: 16 * 1024 * 1024,
        ..Default::default()
    };
    let Some(mut sim) = sim_or_skip(12.0, opts) else {
        return;
    };
    sim.engine.apply(basemap_scene());

    let start = sim.camera();
    assert!(sim.run_until_stable(200, 0.002).is_some(), "start must settle");
    let start_blank = sim.step().blank_frac;
    assert!(start_blank < 0.05, "start should be covered, blank={start_blank:.3}");

    // Roam far east across several distinct regions, settling at each so the
    // cache fills with new tiles and the cold start-area tiles get evicted.
    for region in 1..=6u32 {
        let mut cam = start;
        cam.center.lng += region as f64 * 0.4;
        cam.center.lat += (region as f64 * 0.13).sin() * 0.2;
        sim.engine.set_camera(cam);
        assert!(
            sim.run_until_stable(200, 0.003).is_some(),
            "region {region} must settle"
        );
    }

    // The test only means something if eviction actually happened.
    let evictions: u64 = sim
        .engine
        .last_frame_metrics()
        .layers
        .iter()
        .map(|l| l.cache.evictions)
        .sum();
    assert!(
        evictions > 0,
        "tight budget + roaming must force cache evictions (got {evictions})"
    );

    // Return to the exact start. The evicted start tiles must be re-fetched
    // and the frame fully re-paints; the bug left permanent grey holes here.
    sim.engine.set_camera(start);
    assert!(
        sim.run_until_stable(300, 0.003).is_some(),
        "return must settle"
    );
    let back_blank = sim.step().blank_frac;
    assert!(
        back_blank < 0.05,
        "returning after eviction must re-load tiles, blank={back_blank:.3} \
         (regression: evicted tiles never re-requested → permanent grey)"
    );
}

#[test]
fn long_roaming_session_keeps_caches_within_budget() {
    // Soak regression for "crashes after frequent long use" — the OOM class.
    // A long panning session churns the tile caches continuously (every move
    // evicts + loads). If the LRU byte accounting ever drifted (e.g. an evict
    // path that didn't decrement `bytes_used`), the cache would grow without
    // bound across the session until the device ran out of GPU memory. Roam
    // across many far-apart regions and assert EVERY layer cache stays at or
    // under its byte budget the whole time — bounded memory, no leak.
    let budget = 16 * 1024 * 1024usize;
    let opts = MapOptions {
        fade_in_secs: 0.0,
        cache_budget_bytes: budget,
        ..Default::default()
    };
    let Some(mut sim) = sim_or_skip(12.0, opts) else {
        return;
    };
    sim.engine.apply(basemap_scene());

    let start = sim.camera();
    assert!(sim.run_until_stable(200, 0.002).is_some(), "start must settle");

    // A long, churny session: 80 regions on a wandering path, each settled so
    // the cache fully turns over. Far more tiles than the budget can hold.
    let mut peak_bytes = 0usize;
    let mut max_entries = 0usize;
    for region in 1..=80u32 {
        let r = region as f64;
        let mut cam = start;
        cam.center.lng += (r * 0.37).sin() * 1.5;
        cam.center.lat += (r * 0.21).cos() * 0.8;
        sim.engine.set_camera(cam);
        sim.run_until_stable(120, 0.004);
        sim.step();

        // The invariant: no layer cache may exceed its budget at any point.
        for layer in &sim.engine.last_frame_metrics().layers {
            let c = &layer.cache;
            peak_bytes = peak_bytes.max(c.bytes_used);
            max_entries = max_entries.max(c.entries);
            assert!(
                c.bytes_used <= c.budget_bytes,
                "region {region}: layer '{}' cache exceeded budget — \
                 bytes_used={} > budget={} (LRU accounting leak → eventual OOM)",
                layer.id,
                c.bytes_used,
                c.budget_bytes,
            );
        }
    }

    // Sanity: the session actually churned (evictions happened) and the cache
    // did fill (so the budget assertion above was meaningful, not vacuous).
    let evictions: u64 = sim
        .engine
        .last_frame_metrics()
        .layers
        .iter()
        .map(|l| l.cache.evictions)
        .sum();
    assert!(evictions > 0, "long session must churn the cache");
    assert!(peak_bytes > 0, "cache must have filled during the session");
    assert!(
        peak_bytes <= budget,
        "peak cache bytes {peak_bytes} must stay within budget {budget}"
    );
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

/// Device-equivalent gate for the cast-shadow render-thread cost — the bug that
/// FROZE then crashed sun mode on device. Cast shadows run a synchronous CPU
/// horizon-march inside `render()`; the regression was it recomputing on every
/// ANIMATING frame (heightfield lookups × 36k), stalling the render thread →
/// ANR. The fix skips the march while the camera animates. This drives the real
/// 3D path over the synthetic DEM and asserts the invariant headlessly.
///
/// Runs on a software rasteriser, so absolute ms are inflated — the assertion is
/// RELATIVE (shadows-on vs -off over the SAME pan), which is machine-robust.
#[test]
fn terrain_cast_shadows_do_not_stall_the_render_thread_while_panning() {
    let Some(mut sim) = sim_or_skip(14.0, no_fade()) else {
        return;
    };
    sim.engine.apply(basemap_scene_3d());
    sim.engine.pitch_by(45.0); // 3D, but a modest tilt so the footprint-sized
                               // shadow grid overlaps the visible terrain well
    sim.set_sun(90.0, 16.0); // a LOW sun so the synthetic relief self-occludes
    // Settle: load the DEM + raster tiles and let any animation finish.
    for _ in 0..80 {
        let (in_flight, animating) = {
            let s = sim.step();
            (s.in_flight, s.animating)
        };
        if in_flight == 0 && !animating {
            break;
        }
    }

    // Pan (an ease that moves the CENTRE — the exact input that makes the shadow
    // field's cache key change every frame, i.e. what the buggy code recomputed
    // on). Return the worst CPU ms over the frames that were actually animating.
    fn pan_worst_cpu(sim: &mut Sim) -> f64 {
        let cam = sim.camera();
        // Move only the CENTRE (keep pitch/zoom/bearing) — the input that makes
        // the shadow field's cache key change every animating frame.
        let target = CameraState {
            center: LatLng::new(cam.center.lat + 0.03, cam.center.lng + 0.03),
            ..cam
        };
        sim.engine.ease_to(target, Duration::from_millis(700));
        let mut worst = 0.0f64;
        for _ in 0..40 {
            let (animating, cpu) = {
                let s = sim.step();
                (s.animating, s.cpu_ms)
            };
            if animating {
                worst = worst.max(cpu);
            }
        }
        worst
    }

    sim.set_terrain_shadows(0.0);
    let off = pan_worst_cpu(&mut sim);
    sim.set_terrain_shadows(0.85);
    let on = pan_worst_cpu(&mut sim);

    assert!(
        on <= off * 1.8 + 3.0,
        "cast-shadow horizon-march is running every animating frame: \
         shadows-on worst frame {on:.1}ms vs shadows-off {off:.1}ms — \
         this is the render-thread stall that froze sun mode on device"
    );

    // Correctness: at a SETTLED pose, turning shadows on must darken a real
    // patch of on-screen terrain. A whole-frame mean is too coarse (the shadow
    // is a fraction of a sky-heavy 3D frame), so diff PER-PIXEL between the
    // off/on renders at the identical pose and count the darkened pixels —
    // catches a silent no-op (the @group(3) sample never reaching terrain).
    fn settle_frame(sim: &mut Sim, strength: f32) -> image::RgbaImage {
        sim.set_terrain_shadows(strength);
        for _ in 0..40 {
            if !sim.step().animating {
                break;
            }
        }
        // A few settled frames so the (non-animating) recompute + render lands.
        for _ in 0..4 {
            sim.step();
        }
        sim.last.clone().expect("a rendered frame")
    }
    let off_img = settle_frame(&mut sim, 0.0);
    let on_img = settle_frame(&mut sim, 0.85);
    let luma =
        |p: &image::Rgba<u8>| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
    let darkened = off_img
        .pixels()
        .zip(on_img.pixels())
        .filter(|(a, b)| luma(a) - luma(b) > 2.0)
        .count();
    assert!(
        darkened > 200,
        "cast shadows changed only {darkened} px between off/on at a fixed pose — \
         the @group(3) shadow sample isn't reaching the on-screen terrain"
    );
}
