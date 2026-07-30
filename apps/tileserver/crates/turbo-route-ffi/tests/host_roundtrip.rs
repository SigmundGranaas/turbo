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

// ---- the options a host actually sends ------------------------------

/// **Round-trip must produce a loop, not a one-way route.**
///
/// The Android app sends `roundTrip` on every request
/// (`RouteViewModel.roundTrip`). The engine has supported it since
/// `Prefs::round_trip`, and the HTTP API exposes it — but this façade did
/// not, so an on-device round-trip request would have answered with a
/// one-way line. That is the failure mode worth a test: not an error a
/// host can see, but a plausible wrong answer it cannot.
#[test]
fn round_trip_closes_the_loop() {
    let engine = RouteEngine::open(pack_dir()).unwrap();

    let one_way = engine
        .plan(vec![FROM, TO], RouteOptions::default())
        .expect("the one-way route must solve");
    let loop_route = engine
        .plan(
            vec![FROM, TO],
            RouteOptions {
                round_trip: true,
                ..RouteOptions::default()
            },
        )
        .expect("the round trip must solve");

    let ends_where_it_started = |r: &turbo_route_ffi::Route| {
        let (a, b) = (r.geometry.first().unwrap(), r.geometry.last().unwrap());
        // ~1e-4 deg is ~10 m at this latitude — the tolerance of a route
        // that returns to its origin rather than one that happens to end
        // nearby.
        (a.lon - b.lon).abs() < 1e-4 && (a.lat - b.lat).abs() < 1e-4
    };

    assert!(
        !ends_where_it_started(&one_way),
        "the one-way control must NOT close — if it does, this fixture \
         cannot tell a loop from a line and proves nothing"
    );
    assert!(
        ends_where_it_started(&loop_route),
        "a round trip must return to its origin; it ended at {:?} having \
         started at {:?}. If this fails, `round_trip` is not reaching \
         `Prefs` and the app's loop button silently produces one-way routes.",
        loop_route.geometry.last().unwrap(),
        loop_route.geometry.first().unwrap()
    );
    assert!(
        loop_route.length_m > one_way.length_m,
        "a loop back to the start must be longer than the one-way leg \
         ({:.0} m vs {:.0} m)",
        loop_route.length_m,
        one_way.length_m
    );
}

/// **`avoid` must move the route.**
///
/// Judged on geometry rather than on the option being accepted: an
/// option that parses and changes nothing is the exact defect
/// `knob_liveness.rs` was built to catch on the cost knobs, and there is
/// no reason the façade's options deserve a weaker standard.
#[test]
fn avoiding_the_route_moves_it() {
    let engine = RouteEngine::open(pack_dir()).unwrap();

    let base = engine
        .plan(vec![FROM, TO], RouteOptions::default())
        .expect("the baseline route must solve");

    // Avoid the baseline itself — the strongest available probe, and the
    // same trick `round_trip` uses internally.
    let avoided = engine
        .plan(
            vec![FROM, TO],
            RouteOptions {
                avoid: vec![base.geometry.clone()],
                avoid_radius_m: Some(50.0),
                ..RouteOptions::default()
            },
        )
        .expect("avoiding the baseline must still find some route");

    let same = base.geometry.len() == avoided.geometry.len()
        && base
            .geometry
            .iter()
            .zip(&avoided.geometry)
            .all(|(a, b)| (a.lon - b.lon).abs() < 1e-9 && (a.lat - b.lat).abs() < 1e-9);
    assert!(
        !same,
        "asking the router to avoid its own route returned that exact \
         route ({} points, {:.0} m). `avoid` is not reaching `Prefs`.",
        base.geometry.len(),
        base.length_m
    );
}

/// **The cross-country budget must refuse, and only that lane.**
///
/// `max_off_trail_km` was inert engine-wide until it was enforced in
/// `FmmGradeLimited::solve`: declared, defaulted, hashed into the leg
/// fingerprint, echoed by the debug endpoint, read by nothing. This pins
/// both halves of the fix — that the budget bites on the expensive lane,
/// and that it does not touch the cheap one.
#[test]
fn the_cross_country_budget_is_enforced_per_lane() {
    let engine = RouteEngine::open(pack_dir()).unwrap();

    // A tiny budget the FROM–TO pair (~1 km) certainly exceeds.
    let tight = RouteOptions {
        force_off_trail: true,
        max_off_trail_km: 0.1,
        ..RouteOptions::default()
    };
    match engine.plan(vec![FROM, TO], tight) {
        Err(RouteError::TooLong(msg)) => {
            assert!(
                msg.contains("off-trail"),
                "the message should say which budget refused: {msg}"
            );
        }
        other => panic!(
            "a 1 km cross-country request under a 100 m budget must be \
             refused as TooLong, got {other:?}"
        ),
    }

    // The SAME tiny budget on the unified lane must change nothing: that
    // lane is flat in distance and is bounded by `max_span_km` instead.
    // Without this half, "enforced" could mean "refuses everything".
    let unified_under_the_same_budget = RouteOptions {
        force_off_trail: false,
        max_off_trail_km: 0.1,
        ..RouteOptions::default()
    };
    assert!(
        engine
            .plan(vec![FROM, TO], unified_under_the_same_budget)
            .is_ok(),
        "the cross-country budget must not bound the unified lane"
    );
}

// ---- the pack manifest ----------------------------------------------

/// A pack from a NEWER build must be refused, not misread.
///
/// The failure this prevents is specific to shipping on a phone. A
/// server reading a pack it half-understands is a bad deploy, noticed in
/// minutes and rolled back. A phone has the pack on the user's disk
/// until they delete it, and the symptom — routes that are subtly wrong
/// rather than absent — is one nobody reports as a bug.
#[test]
fn a_pack_from_the_future_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    // Copy the committed pack, then plant a manifest from a later build.
    for f in std::fs::read_dir(pack_dir()).unwrap() {
        let f = f.unwrap();
        std::fs::copy(f.path(), tmp.path().join(f.file_name())).unwrap();
    }
    std::fs::write(
        tmp.path().join("pack.toml"),
        "[pack]\nformat_version = 999\nframe = \"utm33n\"\n\
         extent = [15.0, 67.0, 15.1, 67.1]\n",
    )
    .unwrap();

    match RouteEngine::open(tmp.path().to_string_lossy().into_owned()) {
        Err(RouteError::Pack(msg)) => assert!(
            msg.contains("999") && msg.contains("update"),
            "the error must say the pack is too new AND what to do: {msg}"
        ),
        Err(e) => panic!("a v999 pack must be refused as a Pack error, got {e}"),
        Ok(_) => panic!("a v999 pack must be refused at open, but it opened"),
    }
}

/// A manifest's extent is what `coverage()` reports — and it is the
/// requested region, not the wider ground the pack happens to contain.
#[test]
fn the_manifest_extent_is_the_coverage_a_host_sees() {
    let tmp = tempfile::tempdir().unwrap();
    for f in std::fs::read_dir(pack_dir()).unwrap() {
        let f = f.unwrap();
        std::fs::copy(f.path(), tmp.path().join(f.file_name())).unwrap();
    }
    // Deliberately narrower than the artifacts cover.
    std::fs::write(
        tmp.path().join("pack.toml"),
        "[pack]\nformat_version = 1\nframe = \"utm33n\"\n\
         extent = [15.00, 67.05, 15.06, 67.07]\nhalo_m = 1000.0\n",
    )
    .unwrap();

    let engine = RouteEngine::open(tmp.path().to_string_lossy().into_owned()).unwrap();
    let cov = engine.coverage();
    assert!(
        (cov.min_lon - 15.00).abs() < 1e-9 && (cov.max_lat - 67.07).abs() < 1e-9,
        "coverage must come from the manifest, got {cov:?}"
    );

    // The manifest narrows the ADVERTISED region; it does not shrink the
    // data. A point inside it still routes — which is the whole reason
    // `coverage()` is documented as a UI gate rather than a promise.
    assert!(engine.plan(vec![FROM, TO], RouteOptions::default()).is_ok());
}

/// The committed pack has no manifest, and must still open.
///
/// Packs predate the manifest, and the artifacts alone have always been
/// enough to route. A version marker that made existing packs unopenable
/// would be a migration, not a safety net.
#[test]
fn a_pack_without_a_manifest_still_opens() {
    let engine = RouteEngine::open(pack_dir()).expect("the committed pack has no pack.toml");
    let cov = engine.coverage();
    assert!(cov.max_lat > cov.min_lat, "fell back to the DEM's bounds");
}
