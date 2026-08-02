//! Can a pack built by `turbo-pack-build` actually be routed on?
//!
//! Everything upstream of this checks that the artifacts are
//! well-formed — the right bytes, the right positions, the right
//! counts. None of it establishes the thing that matters: that the
//! engine opens the pack and returns a route over it. A pack can be
//! byte-perfect and still be a graph of 4 000 disconnected fragments.
//!
//! This goes through `RouteEngine::open` — the same entry point the
//! Android app calls across the FFI — so what passes here is what the
//! phone would get.
//!
//! ```text
//! TURBO_PACK_DIR=/path/to/pack \
//!   cargo test -p turbo-route-ffi --test pack_routes -- --nocapture
//! ```

use turbo_route_ffi::{GeoPoint, RouteEngine, RouteOptions};

#[test]
fn a_built_pack_opens_and_routes() {
    let Ok(dir) = std::env::var("TURBO_PACK_DIR") else {
        eprintln!("skipped: set TURBO_PACK_DIR");
        return;
    };
    let engine = RouteEngine::open(dir.clone()).expect("open the pack");
    let cov = engine.coverage();
    eprintln!(
        "coverage {:.4},{:.4} - {:.4},{:.4}",
        cov.min_lon, cov.min_lat, cov.max_lon, cov.max_lat
    );

    // Two points inside the coverage, a little in from the edge so the
    // halo is not what is being tested.
    let inset_lon = (cov.max_lon - cov.min_lon) * 0.25;
    let inset_lat = (cov.max_lat - cov.min_lat) * 0.25;
    let a = GeoPoint {
        lon: cov.min_lon + inset_lon,
        lat: cov.min_lat + inset_lat,
    };
    let b = GeoPoint {
        lon: cov.max_lon - inset_lon,
        lat: cov.max_lat - inset_lat,
    };
    assert!(engine.has_coverage(a), "point a is not covered");
    assert!(engine.has_coverage(b), "point b is not covered");

    let route = engine
        .plan(vec![a, b], RouteOptions::default())
        .expect("plan a route across the pack");

    eprintln!(
        "route: {} points, {:.0} m, {:.0} m ascent",
        route.geometry.len(),
        route.length_m,
        route.ascent_m
    );

    assert!(
        route.geometry.len() >= 2,
        "a route needs at least two points"
    );
    assert!(route.length_m > 0.0, "zero-length route");

    // Straight-line distance is the floor. A route shorter than it means
    // the geometry is not in the frame it claims.
    let straight = haversine(a, b);
    assert!(
        route.length_m >= straight * 0.95,
        "route {:.0} m is shorter than the {straight:.0} m straight line",
        route.length_m
    );
    // And an upper bound, or "route" could mean a wander through every
    // trail in the region.
    assert!(
        route.length_m < straight * 5.0,
        "route {:.0} m is more than 5x the {straight:.0} m straight line",
        route.length_m
    );

    // Every vertex must be inside the pack. A route that leaves the
    // coverage is one built on edges whose terrain the pack does not
    // have — the failure clipping the N50 roads to the region prevents.
    for (i, p) in route.geometry.iter().enumerate() {
        assert!(
            p.lon >= cov.min_lon - 0.02
                && p.lon <= cov.max_lon + 0.02
                && p.lat >= cov.min_lat - 0.02
                && p.lat <= cov.max_lat + 0.02,
            "route point {i} at ({:.4}, {:.4}) is outside the pack",
            p.lon,
            p.lat
        );
    }
}

fn haversine(a: GeoPoint, b: GeoPoint) -> f64 {
    let r = 6_371_000.0_f64;
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let dp = (b.lat - a.lat).to_radians();
    let dl = (b.lon - a.lon).to_radians();
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}
