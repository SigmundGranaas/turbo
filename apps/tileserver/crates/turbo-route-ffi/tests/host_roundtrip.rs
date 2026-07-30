//! Drive the FFI surface exactly as a Kotlin or Swift host would.
//!
//! Against the **real committed pack** (`tools/ci-pack`, 4.6 MB), not a
//! fixture — because the thing worth checking is that the whole stack
//! composes on a directory of files someone downloaded, which a
//! synthetic heightfield cannot tell you.
//!
//! Nothing here touches an engine internal. If a test in this file needs
//! `Pathfinder`, `SolveContext`, `CostConfig` or a port type, the façade
//! is leaking and the host would need to know about Rust's layering —
//! which is the entire thing L6 exists to prevent.

use turbo_route_ffi::{GeoPoint, RouteEngine, RouteError, RouteOptions, TravelMode};

/// The committed CI pack, resolved from the crate rather than the CWD so
/// `cargo test` works from anywhere in the workspace.
fn pack_dir() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/ci-pack")
        .canonicalize()
        .expect("tools/ci-pack must exist — it is committed");
    p.to_string_lossy().into_owned()
}

/// Two points inside the CI pack's coverage, on the Sjunkhatten trail
/// network the corpus was sampled from.
const FROM: GeoPoint = GeoPoint {
    lon: 15.04048,
    lat: 67.065016,
};
const TO: GeoPoint = GeoPoint {
    lon: 15.0555,
    lat: 67.0685,
};

#[test]
fn a_host_can_open_a_pack_and_plan_a_route() {
    let engine = RouteEngine::open(pack_dir()).expect("the committed pack must open");

    let cov = engine.coverage();
    assert!(
        cov.min_lon < FROM.lon && FROM.lon < cov.max_lon,
        "the probe point must be inside the reported coverage: {cov:?}"
    );
    assert!(engine.has_coverage(FROM), "terrain data at the start");

    let route = engine
        .plan(vec![FROM, TO], RouteOptions::default())
        .expect("a route between two covered points");

    assert!(route.geometry.len() >= 2, "a route needs a polyline");
    assert!(route.length_m > 0.0, "a route has length");
    assert!(route.duration_s > 0.0, "a route has a duration");

    // Geometry must come back geographic. The engine is planar (C4), so
    // an unprojected polyline would be metres — around 5e5 — and would
    // plot somewhere in the Gulf of Guinea. Cheap to assert, and the
    // failure it catches is silent everywhere else.
    for p in &route.geometry {
        assert!(
            (4.0..32.0).contains(&p.lon) && (57.0..72.0).contains(&p.lat),
            "geometry must be WGS84 degrees, got ({}, {})",
            p.lon,
            p.lat
        );
    }

    // The endpoints must actually be the endpoints, within a snap radius.
    let first = route.geometry.first().unwrap();
    assert!(
        (first.lon - FROM.lon).abs() < 0.01 && (first.lat - FROM.lat).abs() < 0.01,
        "the route must start near the requested start"
    );
}

#[test]
fn surface_breakdown_reaches_the_host_sorted() {
    let engine = RouteEngine::open(pack_dir()).unwrap();
    let route = engine
        .plan(vec![FROM, TO], RouteOptions::default())
        .unwrap();

    let total: f64 = route.surface_breakdown.iter().map(|s| s.length_m).sum();
    assert!(
        (total - route.length_m).abs() < route.length_m * 0.05,
        "the surface breakdown should account for the route: {total} vs {}",
        route.length_m
    );
    // Longest first, so a host can render "mostly trail" without
    // sorting it again.
    for w in route.surface_breakdown.windows(2) {
        assert!(w[0].length_m >= w[1].length_m, "breakdown must be sorted");
    }
}

/// The distinction that makes an app feel working rather than broken:
/// "download more map" and "move your pin off the lake" are different
/// problems with different fixes, and collapsing them into one "no
/// route" tells the user nothing.
#[test]
fn outside_coverage_is_its_own_error() {
    let engine = RouteEngine::open(pack_dir()).unwrap();
    let far_away = GeoPoint {
        lon: 10.7522,
        lat: 59.9139, // Oslo — real place, nowhere near this pack
    };
    assert!(!engine.has_coverage(far_away));

    // One endpoint 850 km away. The budget must stop this before any
    // solving — see the timing note on `RouteOptions::max_span_km`.
    let t = std::time::Instant::now();
    match engine.plan(vec![far_away, TO], RouteOptions::default()) {
        Err(RouteError::TooLong(_)) => {}
        other => panic!("an 850 km request must be refused by the budget, got {other:?}"),
    }
    assert!(
        t.elapsed().as_secs_f64() < 1.0,
        "the budget must be checked BEFORE solving; took {:.1}s",
        t.elapsed().as_secs_f64()
    );

    // Both endpoints out, and close enough together to pass the budget:
    // now the coverage precheck is what must fire.
    let also_far = GeoPoint {
        lon: 10.80,
        lat: 59.95,
    };
    match engine.plan(vec![far_away, also_far], RouteOptions::default()) {
        Err(RouteError::OutsideCoverage(_)) => {}
        other => panic!("two out-of-coverage endpoints must say so, got {other:?}"),
    }
}

#[test]
fn an_unknown_preset_names_the_valid_ones() {
    let engine = RouteEngine::open(pack_dir()).unwrap();
    let opts = RouteOptions {
        preset: Some("definitely-not-a-preset".into()),
        ..RouteOptions::default()
    };
    match engine.plan(vec![FROM, TO], opts) {
        Err(RouteError::InvalidRequest(msg)) => {
            assert!(
                msg.contains("balanced"),
                "the error must name a valid preset so the host can recover: {msg}"
            );
        }
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[test]
fn a_missing_pack_is_a_pack_error_not_a_panic() {
    match RouteEngine::open("/nonexistent/pack/dir".into()) {
        Err(RouteError::Pack(_)) => {}
        Err(e) => panic!("expected Pack error, got {e}"),
        Ok(_) => panic!("opening a nonexistent directory must fail"),
    }
}

#[test]
fn one_point_is_rejected_before_any_solving() {
    let engine = RouteEngine::open(pack_dir()).unwrap();
    match engine.plan(vec![FROM], RouteOptions::default()) {
        Err(RouteError::InvalidRequest(_)) => {}
        other => panic!("a one-point route is not a route, got {other:?}"),
    }
    match engine.plan(vec![], RouteOptions::default()) {
        Err(RouteError::InvalidRequest(_)) => {}
        other => panic!("an empty route is not a route, got {other:?}"),
    }
}

/// Every travel mode must at least be plumbed through. Ski and bicycle
/// have their own cost tables, and a mode that silently fell back to
/// foot would look like it worked.
#[test]
fn every_travel_mode_is_wired() {
    let engine = RouteEngine::open(pack_dir()).unwrap();
    for mode in [TravelMode::Foot, TravelMode::Bicycle, TravelMode::Ski] {
        let opts = RouteOptions {
            mode,
            ..RouteOptions::default()
        };
        // Bicycle/ski may legitimately find no route on a footpath
        // network — what must not happen is an internal error.
        match engine.plan(vec![FROM, TO], opts) {
            Ok(_) | Err(RouteError::NoRoute) | Err(RouteError::OutsideCoverage(_)) => {}
            Err(e) => panic!("{mode:?} must be wired, got {e}"),
        }
    }
}

/// The engine handle must be shareable across threads, because a host
/// holds one for the app's lifetime and calls it from wherever the UI
/// happens to be. Construction bulk-loads the trail R-trees (E4: 555 ms
/// at national scale), so per-request construction is not an option.
#[test]
fn the_engine_is_shareable_across_threads() {
    let engine = std::sync::Arc::new(RouteEngine::open(pack_dir()).unwrap());
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let e = engine.clone();
            std::thread::spawn(move || e.plan(vec![FROM, TO], RouteOptions::default()).is_ok())
        })
        .collect();
    for h in handles {
        assert!(h.join().unwrap(), "concurrent plans must all succeed");
    }
}
