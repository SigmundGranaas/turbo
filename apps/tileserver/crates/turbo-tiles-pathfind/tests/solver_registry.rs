//! D1 — the solver seam.
//!
//! Before D1, which router ran was an `if` on one preference flag
//! inside `solve_inner`, and both routers were private methods reaching
//! into `Pathfinder`'s fields. Adding a third meant editing the engine;
//! swapping one for an experiment meant editing the engine.
//!
//! These tests pin the two properties that make the seam real rather
//! than decorative: selection is driven by each solver's own `accepts`,
//! and a third-party solver — one this crate has never heard of — can
//! be registered and will actually run.

use std::sync::Arc;

use turbo_tiles_pathfind::{
    FmmGradeLimited, Path, PathfindError, Pathfinder, Prefs, SolveContext, SolveRequest, Solver,
    SolverSet, UnifiedAStar,
};

fn ctx_without_terrain<'a>(
    contributors: &'a [Arc<dyn turbo_tiles_pathfind::CostContributor>],
    cfg: &'a turbo_tiles_pathfind::CostConfig,
) -> SolveContext<'a> {
    SolveContext {
        terrain: None,
        network: None,
        contributors,
        cost_config: cfg,
    }
}

#[test]
fn selection_follows_the_preference_not_a_hardcoded_branch() {
    let cfg = cfg();
    let contributors: Vec<Arc<dyn turbo_tiles_pathfind::CostContributor>> = Vec::new();
    let set = SolverSet::production();

    let p = turbo_tiles_pathfind::Point::new(500_000.0, 7_500_000.0);
    let q = turbo_tiles_pathfind::Point::new(500_500.0, 7_500_500.0);

    // Default prefs → the unified router.
    let prefs = Prefs::default();
    let ctx = ctx_without_terrain(&contributors, &cfg);
    let req = SolveRequest {
        from: p,
        to: q,
        prefs: &prefs,
    };
    assert_eq!(
        set.select(&ctx, &req).map(|s| s.name()),
        Some("unified_astar")
    );

    // `force_off_trail` with no terrain → NOTHING accepts. This is the
    // case the trait's `accepts` exists for: the FMM solver declines up
    // front instead of failing halfway through, and the engine reports
    // "no solver" rather than a straight line across ground nobody
    // measured.
    let prefs = Prefs {
        force_off_trail: true,
        ..Default::default()
    };
    let req = SolveRequest {
        from: p,
        to: q,
        prefs: &prefs,
    };
    assert!(
        set.select(&ctx, &req).is_none(),
        "force_off_trail without terrain must have no acceptor"
    );
}

/// A solver defined entirely outside the crate that owns the trait.
/// If this compiles and gets selected, "swap the algorithm" is a
/// composition decision — which is the whole claim D1 makes.
struct AlwaysStraightLine;

impl Solver for AlwaysStraightLine {
    fn name(&self) -> &'static str {
        "test_straight_line"
    }
    fn accepts(&self, _ctx: &SolveContext<'_>, _req: &SolveRequest<'_>) -> bool {
        true
    }
    fn solve(
        &self,
        _ctx: &SolveContext<'_>,
        req: &SolveRequest<'_>,
    ) -> Result<Path, PathfindError> {
        let dx = req.to.x - req.from.x;
        let dy = req.to.y - req.from.y;
        let len = (dx * dx + dy * dy).sqrt();
        Ok(Path {
            strategy: turbo_tiles_pathfind::PathStrategy::OffTrail,
            geometry: vec![req.from, req.to],
            distances_m: vec![0.0, len],
            length_m: len,
            cost: len,
            on_trail_pct: 0.0,
            fkb_breakdown: Default::default(),
            legs: Vec::new(),
            waypoint_legs: Vec::new(),
            refused_by: Vec::new(),
            debug: None,
            recording: None,
        })
    }
}

#[test]
fn a_third_party_solver_can_be_registered_and_selected() {
    let cfg = cfg();
    let contributors: Vec<Arc<dyn turbo_tiles_pathfind::CostContributor>> = Vec::new();
    let ctx = ctx_without_terrain(&contributors, &cfg);

    // Registration order IS priority order: listing the outsider first
    // makes it win over both production solvers.
    let set = SolverSet::new(vec![
        Arc::new(AlwaysStraightLine),
        Arc::new(FmmGradeLimited),
        Arc::new(UnifiedAStar),
    ]);
    let prefs = Prefs::default();
    let req = SolveRequest {
        from: turbo_tiles_pathfind::Point::new(0.0, 0.0),
        to: turbo_tiles_pathfind::Point::new(300.0, 400.0),
        prefs: &prefs,
    };

    let chosen = set.select(&ctx, &req).expect("a solver must accept");
    assert_eq!(chosen.name(), "test_straight_line");

    let path = chosen.solve(&ctx, &req).expect("the outsider solves");
    assert!((path.length_m - 500.0).abs() < 1e-9, "3-4-5 triangle");
}

/// A heightfield that is a flat plane, in memory.
///
/// Worth noticing that this is possible at all: before the C1 ports the
/// engine held `Arc<Dem>`, so any test needing terrain had to write a
/// zstd-compressed tiled artifact to a temp directory first. Twenty
/// lines of arithmetic now stand in for the whole file format.
struct FlatPlane;

impl turbo_tiles_pathfind::Heightfield for FlatPlane {
    fn height_at(&self, p: turbo_tiles_pathfind::Point) -> Option<f32> {
        self.covers(p).then_some(100.0)
    }
    fn covers(&self, p: turbo_tiles_pathfind::Point) -> bool {
        (-10_000.0..10_000.0).contains(&p.x) && (-10_000.0..10_000.0).contains(&p.y)
    }
    fn slope_aspect_at(
        &self,
        p: turbo_tiles_pathfind::Point,
    ) -> Option<turbo_tiles_pathfind::SlopeAspect> {
        self.covers(p)
            .then_some(turbo_tiles_pathfind::SlopeAspect::default())
    }
}

/// The engine actually dispatches through the set — not through a
/// leftover branch that happens to agree with it.
#[test]
fn the_engine_dispatches_through_the_registered_set() {
    let field: Arc<dyn turbo_tiles_pathfind::Heightfield> = Arc::new(FlatPlane);
    let mut pf = Pathfinder::with_defaults(Some(field), None, None, cfg());
    pf.solvers = SolverSet::new(vec![Arc::new(AlwaysStraightLine)]);

    let from = turbo_tiles_pathfind::Point::new(0.0, 0.0);
    let to = turbo_tiles_pathfind::Point::new(300.0, 400.0);
    let path = pf
        .solve(from, to, Prefs::default())
        .expect("the registered solver must be the one that runs");

    assert!(
        (path.length_m - 500.0).abs() < 1e-9,
        "got {} m — the engine did not route through the registered set",
        path.length_m
    );
}

/// The calibrated Norwegian config. Tests are a composition root, so
/// **The off-trail budget must actually refuse.**
///
/// `Prefs::max_off_trail_km` was declared, defaulted to 10, hashed into
/// the leg fingerprint, echoed by `/v1/debug/prefs` — and read by
/// nothing. `PathfindError::BboxTooLarge` was declared and mapped to a
/// 400 by the API without ever being constructed. Two halves of a guard
/// that did not exist, and a doc comment on the FFI façade asserting it
/// did.
///
/// The budget is checked before the corridor is sized, so this test
/// needs no terrain: a solver that refuses only after doing the work is
/// not a budget, and running it without a DEM proves it refused first.
#[test]
fn the_off_trail_budget_refuses_before_it_solves() {
    let cfg = cfg();
    let contributors: Vec<Arc<dyn turbo_tiles_pathfind::CostContributor>> = Vec::new();
    let ctx = SolveContext {
        // `FmmGradeLimited::accepts` requires terrain, so `solve` is
        // called directly — this asserts the guard, not the selection.
        terrain: None,
        network: None,
        contributors: &contributors,
        cost_config: &cfg,
    };
    let from = turbo_tiles_pathfind::Point::new(500_000.0, 7_500_000.0);
    let far = turbo_tiles_pathfind::Point::new(530_000.0, 7_500_000.0); // 30 km

    let prefs = Prefs {
        force_off_trail: true,
        ..Default::default()
    };
    assert_eq!(prefs.max_off_trail_km, 10.0, "the default budget moved");
    let req = SolveRequest {
        from,
        to: far,
        prefs: &prefs,
    };
    match FmmGradeLimited.solve(&ctx, &req) {
        Err(PathfindError::BboxTooLarge { extent_km }) => {
            assert!((extent_km - 30.0).abs() < 0.1, "reported {extent_km} km");
        }
        other => panic!(
            "a 30 km off-trail request must be refused by the 10 km budget, got {other:?}.\n\
             If this returns a route, `max_off_trail_km` is inert again and the phone \
             has no bound on the one lane whose cost grows with span."
        ),
    }

    // Raising the budget past the span lets it through to the real
    // solver, which then fails for its own reason (no terrain). The
    // point is that it got that far: the budget gates, it does not veto
    // everything.
    let generous = Prefs {
        force_off_trail: true,
        max_off_trail_km: 50.0,
        ..Default::default()
    };
    let req = SolveRequest {
        from,
        to: far,
        prefs: &generous,
    };
    assert!(
        !matches!(
            FmmGradeLimited.solve(&ctx, &req),
            Err(PathfindError::BboxTooLarge { .. })
        ),
        "a 30 km request under a 50 km budget must not be refused by the budget"
    );

    // 0 = no budget, matching how the rest of the engine reads a zero
    // limit. Without this a host that means "unlimited" gets "refuse
    // everything", which is the opposite.
    let unlimited = Prefs {
        force_off_trail: true,
        max_off_trail_km: 0.0,
        ..Default::default()
    };
    let req = SolveRequest {
        from,
        to: far,
        prefs: &unlimited,
    };
    assert!(
        !matches!(
            FmmGradeLimited.solve(&ctx, &req),
            Err(PathfindError::BboxTooLarge { .. })
        ),
        "`max_off_trail_km = 0` must mean unlimited, not refuse-everything"
    );
}

/// they name the profile explicitly — the engine no longer supplies one
/// (D3).
fn cfg() -> turbo_tiles_pathfind::CostConfig {
    turbo_profile_no::cost_config().expect("the calibrated config must parse")
}
