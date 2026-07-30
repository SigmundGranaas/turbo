//! **Every user-visible knob must be provably load-bearing.**
//!
//! `boundary_check.sh` enforces ten invariants and every one of them is
//! structural — who may name whom, which crate may read a file. Not one
//! asks whether anything *does* something. This is the behavioural
//! counterpart, and the gap it fills is not hypothetical: six knobs in
//! `CostConfigPatch` were completely inert, and the whole refactor ran
//! green over them because the corpus gate cannot see this property.
//!
//! # What "inert" means here
//!
//! A knob is inert when setting it to an extreme value produces
//! byte-identical routes. It compiles, it parses, it reaches the API,
//! it renders in the SPA dropdown, and it changes nothing. There is no
//! error and no warning — the only symptom is a user saying "every mode
//! gives me the same route".
//!
//! The cause is structural rather than incidental: contributors are
//! constructed once at boot and copy config scalars into their own
//! fields, while a per-request patch only reaches what the *solvers*
//! read from `SolveContext::cost_config` at solve time. A knob whose
//! only consumer is a contributor field is dead by construction.
//!
//! # Why these fixtures and not the corpus
//!
//! See `tools/preference-scenarios.toml`. The Sjunkhatten corpus
//! measures fidelity to walked trails and cannot measure responsiveness
//! to preference — its routes already sit on the trail, where off-trail
//! cost is irrelevant. Measured on the corpus, the `direct` preset
//! moves route length by **0.07%**; on these fixtures the same class of
//! change moves it by 50–155%.
//!
//! # It was written before the fix, and it failed
//!
//! That was the point. A fitness function written after the fix proves
//! nothing about whether it could have caught the bug. On its first run
//! seven knobs reported INERT; six were real and the `Tuning` split
//! fixed them, and the seventh turned out to be a bad probe — see
//! `every_probe_actually_changes_the_config`.

use std::sync::Arc;

use turbo_tiles_pathfind::{CostConfigPatch, Pathfinder, Point, Prefs};

// ---- fixtures -------------------------------------------------------

struct Scenario {
    name: String,
    from: Point,
    to: Point,
}

fn scenarios() -> Vec<Scenario> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/preference-scenarios.toml");
    let doc: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc["scenario"]
        .as_array()
        .expect("preference-scenarios.toml must have [[scenario]] entries")
        .iter()
        .map(|s| {
            let pt = |k: &str| {
                let a = s[k].as_array().unwrap();
                Point::new(a[0].as_float().unwrap(), a[1].as_float().unwrap())
            };
            Scenario {
                name: s["name"].as_str().unwrap().to_string(),
                from: pt("from"),
                to: pt("to"),
            }
        })
        .collect()
}

fn engine() -> Pathfinder {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/ci-pack")
        .canonicalize()
        .expect("tools/ci-pack is committed");
    let dem = turbo_geodata_artifacts::heightfield(Arc::new(
        turbo_tiles_elev::Dem::open(d.join("norway.dem")).unwrap(),
    ));
    let mask = turbo_tiles_mask::Mask::open(d.join("norway.mask"))
        .ok()
        .map(Arc::new);
    let graph = turbo_tiles_graph::Graph::open(d.join("norway.graph"))
        .ok()
        .map(|mut g| {
            let _ = g.attach_geom(d.join("norway.graph_geom"));
            Arc::new(g)
        });
    Pathfinder::with_defaults(
        Some(dem),
        mask,
        graph,
        turbo_profile_no::cost_config().expect("calibrated config"),
    )
}

/// Geometry digest for one scenario, or `None` if it does not solve.
/// Rounded to a millimetre so the comparison is about routing decisions
/// rather than float noise.
fn digest(pf: &Pathfinder, s: &Scenario, ctx: &Ctx, patch: CostConfigPatch) -> Option<u64> {
    let path = pf.solve(s.from, s.to, ctx.prefs(patch)).ok()?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in &path.geometry {
        for b in ((p.x * 1e3) as i64)
            .to_le_bytes()
            .iter()
            .chain(((p.y * 1e3) as i64).to_le_bytes().iter())
        {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    Some(h)
}

/// Which `fkb_type`s the fixture pack actually contains.
fn pack_fkb_types() -> std::collections::BTreeSet<u8> {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/ci-pack")
        .canonicalize()
        .unwrap();
    let g = turbo_tiles_graph::Graph::open(d.join("norway.graph")).unwrap();
    (0..g.stats().meta.edge_count)
        .filter_map(|e| g.edge(e).map(|r| r.fkb_type))
        .collect()
}

/// The request context a knob needs before it can possibly bite.
///
/// Without this the test conflates two very different findings — "this
/// knob is broken" and "these fixtures never exercise it". A
/// per-profile knob probed only as Foot, or a grade-limited knob probed
/// only on the unified lane, would report INERT while being perfectly
/// wired. A fitness function that cries wolf gets switched off, so each
/// probe declares the conditions under which it is entitled to bite and
/// is judged only there.
#[derive(Clone, Default)]
struct Ctx {
    profile: Option<turbo_tiles_graph::Profile>,
    /// The grade-limited solver is opt-in and lives on the FMM
    /// off-trail lane; its knobs cannot bite on the unified lane.
    off_trail_lane: bool,
    /// Applied to BOTH the baseline and the probe, so the comparison
    /// isolates the knob under test rather than the conditions it needs
    /// to be reachable. `grade_limited_turn_penalty_s` is the case that
    /// forces this: a turn penalty only shapes a route once the grade
    /// constraint binds hard enough to make the solver switchback, so
    /// its base carries a tight `max_grade_deg`. Without that the probe
    /// measures nothing and reports a wiring bug that is not there.
    base: CostConfigPatch,
    /// An `fkb_type` the pack must contain for this knob to be
    /// judgeable at all. A surface-pace knob for a surface no edge in
    /// the fixture has is *unexercised*, not dead — reporting it as
    /// dead would be the same false alarm this test exists to avoid.
    needs_fkb: Option<u8>,
}

impl Ctx {
    fn prefs(&self, patch: CostConfigPatch) -> Prefs {
        let mut p = Prefs::default();
        if let Some(pr) = self.profile {
            p.profile = pr;
        }
        p.force_off_trail = self.off_trail_lane;
        p.cost_config_override = Some(patch.over(&self.base));
        p
    }
}

/// Every knob, with a value far enough from the calibrated default that
/// a wired knob cannot fail to move *something*, plus the context it
/// needs.
///
/// Extremes on purpose: this asks "is it connected at all", not "is it
/// well tuned". A knob that needs a subtle value to show an effect is
/// indistinguishable here from one that is wired — and that is the
/// right call, because a knob too weak to matter at an extreme is not
/// a knob a user can feel either.
fn probes() -> Vec<(&'static str, CostConfigPatch, Ctx)> {
    use turbo_tiles_graph::Profile;
    macro_rules! p {
        ($field:ident = $v:expr) => {
            p!($field = $v, Ctx::default())
        };
        ($field:ident = $v:expr, $ctx:expr) => {
            (
                stringify!($field),
                CostConfigPatch {
                    $field: Some($v),
                    ..Default::default()
                },
                $ctx,
            )
        };
    }
    let prof = |p: Profile| Ctx {
        profile: Some(p),
        ..Ctx::default()
    };
    // Grade-limited lives on the FMM off-trail lane, and only bites when
    // the constraint binds — so the shared base switches it on with a
    // tight grade, and each probe varies one part of that.
    let gl = |base: CostConfigPatch| Ctx {
        off_trail_lane: true,
        base,
        ..Ctx::default()
    };
    let gl_on = CostConfigPatch {
        grade_limited_enabled: Some(true),
        ..Default::default()
    };
    let gl_tight = CostConfigPatch {
        grade_limited_enabled: Some(true),
        grade_limited_max_grade_deg: Some(8.0),
        ..Default::default()
    };
    // For `enabled` itself the base carries nothing and the probe turns
    // the solver OFF, because the calibrated config already has it ON.
    // Probing `= true` against a boot value of `true` is a patch that
    // patches nothing: identical routes, reported as a dead knob. See
    // `every_probe_actually_changes_the_config`, which now makes that
    // mistake impossible to repeat silently.
    let gl_grade_only = CostConfigPatch::default();
    fn water_ctx() -> Ctx {
        Ctx {
            off_trail_lane: true,
            ..Ctx::default()
        }
    }
    let ski_surface = Ctx {
        profile: Some(Profile::Ski),
        needs_fkb: Some(3),
        ..Ctx::default()
    };

    vec![
        p!(off_trail_base_foot = 9.0),
        p!(off_trail_base_bicycle = 9.0, prof(Profile::Bicycle)),
        p!(off_trail_base_ski = 9.0, prof(Profile::Ski)),
        p!(trail_proximity_influence_radius_m = 500.0),
        p!(trail_proximity_bonus_at_zero = 0.001),
        p!(slope_graph_quadratic_scale_deg = 2.0),
        p!(slope_graph_refuse_above_deg = 5.0),
        p!(total_gain_amplifier = 12.0),
        p!(surface_pace_sti = 9.0),
        p!(surface_pace_vei = 9.0),
        p!(surface_pace_skiloype = 9.0, ski_surface),
        // fkb_type 0 is the "unknown" bucket.
        p!(
            surface_pace_unknown = 9.0,
            Ctx {
                needs_fkb: Some(0),
                ..Ctx::default()
            }
        ),
        // Water prices MESH cells. On the unified lane a route that
        // crosses a lake usually does so on a trail — graph edges are
        // priced from baked metadata at synthetic coordinates, on
        // purpose, so a bridge is not water-vetoed. Judge these on the
        // off-trail lane, where every metre is mesh and a crossing is
        // actually charged.
        p!(water_cost_s_per_m = 99.0, water_ctx()),
        p!(water_shore_band_m = 400.0, water_ctx()),
        p!(grade_limited_enabled = false, gl(gl_grade_only)),
        p!(grade_limited_max_grade_deg = 8.0, gl(gl_on)),
        p!(grade_limited_turn_penalty_s = 600.0, gl(gl_tight)),
    ]
}

/// The probe list is hand-written, so it can rot. This makes it
/// impossible to add a knob without also proving it does something:
/// serialise a fully-populated patch and require every field to have a
/// probe. Without this the test degrades into "the knobs I remembered
/// still work", which is the failure mode it exists to prevent.
#[test]
fn every_knob_has_a_probe() {
    let all = CostConfigPatch {
        off_trail_base_foot: Some(1.0),
        off_trail_base_bicycle: Some(1.0),
        off_trail_base_ski: Some(1.0),
        trail_proximity_influence_radius_m: Some(1.0),
        trail_proximity_bonus_at_zero: Some(1.0),
        slope_graph_quadratic_scale_deg: Some(1.0),
        slope_graph_refuse_above_deg: Some(1.0),
        total_gain_amplifier: Some(1.0),
        surface_pace_sti: Some(1.0),
        surface_pace_vei: Some(1.0),
        surface_pace_skiloype: Some(1.0),
        surface_pace_unknown: Some(1.0),
        water_cost_s_per_m: Some(1.0),
        water_shore_band_m: Some(1.0),
        grade_limited_enabled: Some(true),
        grade_limited_max_grade_deg: Some(1.0),
        grade_limited_turn_penalty_s: Some(1.0),
    };
    let json = serde_json::to_value(&all).unwrap();
    let fields: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
    let probed: Vec<&str> = probes().iter().map(|(n, _, _)| *n).collect();

    let missing: Vec<&String> = fields
        .iter()
        .filter(|f| !probed.contains(&f.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "CostConfigPatch has knobs with no liveness probe: {missing:?}.\n\
         A knob without a probe is a knob nobody has checked does anything — \
         add it to `probes()` in this file.\n\
         (If this fails after adding a field to the `all` literal above, that \
         literal is the completeness oracle and must list every field.)"
    );
}

/// A probe must actually change the resolved config.
///
/// This is the instrument checking its own instrument. `grade_limited_
/// enabled` was probed with `Some(true)` while the calibrated config
/// already sets `enabled = true` — so baseline and probe resolved to the
/// same `CostConfig`, produced the same routes, and the liveness table
/// reported a **dead knob**. It was a dead *probe*. That misdiagnosis
/// cost more than the real bugs did, because it pointed at the engine.
///
/// The check is deliberately generic: resolve both configs and compare
/// them whole, rather than mapping each patch field to its config path.
/// A field-name mapping would need updating with every new knob and
/// would silently pass the ones it forgot — the exact rot this file
/// exists to prevent.
#[test]
fn every_probe_actually_changes_the_config() {
    let boot = turbo_profile_no::cost_config().expect("calibrated config");
    let mut vacuous = Vec::new();
    for (name, patch, ctx) in probes() {
        let baseline = boot.with_patch(&CostConfigPatch::default().over(&ctx.base));
        let probed = boot.with_patch(&patch.over(&ctx.base));
        if serde_json::to_value(&baseline).unwrap() == serde_json::to_value(&probed).unwrap() {
            vacuous.push(name);
        }
    }
    assert!(
        vacuous.is_empty(),
        "probe(s) that resolve to the SAME config as their baseline: {vacuous:?}.\n\
         These test nothing and will report their knob as INERT no matter how \
         well it is wired.\n\
         Usually the probe restates the value the calibrated config already \
         carries — pick a value that differs from `tools/cost-config.toml`, not \
         just from `Default::default()`."
    );
}

// ---- the actual invariant -------------------------------------------

#[test]
fn every_knob_moves_at_least_one_route() {
    let pf = engine();
    let scen = scenarios();
    assert!(!scen.is_empty(), "no scenarios loaded");
    let have = pack_fkb_types();

    let solved = scen
        .iter()
        .filter(|s| digest(&pf, s, &Ctx::default(), CostConfigPatch::default()).is_some())
        .count();
    assert!(
        solved >= 3,
        "only {solved}/{} scenarios solve at all — the fixtures are broken, \
         not the knobs",
        scen.len()
    );

    let (mut dead, mut unexercised) = (Vec::new(), Vec::new());
    println!("\n{:<38} {:>12}  scenarios moved", "knob", "verdict");
    for (name, patch, ctx) in probes() {
        if let Some(fkb) = ctx.needs_fkb {
            if !have.contains(&fkb) {
                unexercised.push((name, fkb));
                println!(
                    "{name:<38} {:>12}  (pack has no fkb_type {fkb})",
                    "unexercised"
                );
                continue;
            }
        }
        let base: Vec<Option<u64>> = scen
            .iter()
            .map(|s| digest(&pf, s, &ctx, CostConfigPatch::default()))
            .collect();
        let moved: Vec<&str> = scen
            .iter()
            .zip(&base)
            .filter(|(s, b)| digest(&pf, s, &ctx, patch.clone()) != **b)
            .map(|(s, _)| s.name.as_str())
            .collect();
        if moved.is_empty() {
            dead.push(name);
            println!("{name:<38} {:>12}", "INERT");
        } else {
            println!("{name:<38} {:>12}  {}", "live", moved.join(", "));
        }
    }

    // Coverage gaps are reported, never silently tolerated — but they
    // are not failures here. A knob nothing in the fixture can exercise
    // is an argument for a better fixture, not evidence of a bug.
    //
    // For these two the better fixture cannot be a bigger slice: the
    // WHOLE Sjunkhatten source pack contains 9,642 edges and not one of
    // them is `fkb_type` 0 or 3, so no widening of the CI pack can reach
    // them. `surface_pace_knobs_bite_on_a_synthetic_network` judges them
    // on a purpose-built network instead.
    if !unexercised.is_empty() {
        println!(
            "\nNOTE: {} knob(s) unexercised by this pack: {:?}. No slice of \
             Sjunkhatten contains these surfaces — see \
             `surface_pace_knobs_bite_on_a_synthetic_network`.",
            unexercised.len(),
            unexercised
        );
    }

    assert!(
        dead.is_empty(),
        "\n{} knob(s) are INERT — an extreme value, in the context where the \
         knob is supposed to bite, changes no route at all:\n  {}\n\n\
         These are wired end to end at the API and the SPA and do nothing.\n\n\
         Three causes have produced this symptom here; check them in this \
         order, cheapest first.\n\n\
         1. A DEAD PROBE. The probe value equals what the calibrated config \
         already carries, so nothing is patched. \
         `every_probe_actually_changes_the_config` catches this now — if it is \
         green, rule this out and move on.\n\n\
         2. A MISSING PRECONDITION. The knob is fine but this context cannot \
         reach it — the wrong profile, the wrong lane, a base that does not \
         make the constraint bind. Declare it on the probe's `Ctx`; that is \
         what `Ctx` is for.\n\n\
         3. THE REAL BUG, and the one this file was built for: a contributor \
         copies the config scalar into its own field at construction, while a \
         per-request patch only reaches what is read from the request. Such a \
         knob cannot be reached per request at all. The fix is the one the \
         `Tuning` split made: read the scalar from `EdgeContext::tuning` at \
         evaluation time instead of owning a copy of it.",
        dead.len(),
        dead.join("\n  ")
    );
}

/// A preset that does not differ from `balanced` is a shipped product
/// bug: a dropdown entry the user picks and nothing happens.
///
/// Distinctness is checked at the *route* level, not by comparing
/// patches — two different patches that both fail to reach the cost
/// model are equally useless, and comparing the patches would call that
/// a pass.
#[test]
fn every_preset_routes_differently_from_balanced() {
    let pf = engine();
    let scen = scenarios();
    let presets = turbo_profile_no::presets();

    let balanced = presets.get("balanced").expect("`balanced` must exist");
    let base: Vec<Option<u64>> = scen
        .iter()
        .map(|s| digest(&pf, s, &Ctx::default(), balanced.patch.clone()))
        .collect();

    let mut inert = Vec::new();
    println!("\n{:<16} scenarios differing from balanced", "preset");
    for p in presets.presets.iter().filter(|p| p.name != "balanced") {
        let moved: Vec<&str> = scen
            .iter()
            .zip(&base)
            .filter(|(s, b)| digest(&pf, s, &Ctx::default(), p.patch.clone()) != **b)
            .map(|(s, _)| s.name.as_str())
            .collect();
        println!(
            "{:<16} {}",
            p.name,
            if moved.is_empty() {
                "NONE".into()
            } else {
                moved.join(", ")
            }
        );
        if moved.is_empty() {
            inert.push(p.name.clone());
        }
    }

    assert!(
        inert.is_empty(),
        "preset(s) that route identically to `balanced`: {inert:?}. \
         A trip style the user can select which changes nothing is a product \
         bug, whatever the patch says."
    );
}

/// The water knobs, checked at the contributor rather than end to end.
///
/// The liveness table judges them too, now that `shoreline_detour` is in
/// the fixture set — but only two scenarios move, and both are the
/// off-trail lane. This pins the mechanism directly: the charge scales
/// *linearly* with the tuned cost, which is the specific thing a baked
/// field cannot do. If a future change makes the water knobs go quiet
/// end to end, the pair of results tells you immediately whether the
/// wiring broke or the fixtures stopped touching water.
#[test]
fn water_knobs_reach_the_cost_model() {
    use turbo_tiles_pathfind::{CostContributor, EdgeContext, EdgeKind, MaskRefusalContributor};

    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/ci-pack")
        .canonicalize()
        .unwrap();
    let mask = Arc::new(turbo_tiles_mask::Mask::open(d.join("norway.mask")).unwrap());
    let contributor = MaskRefusalContributor::new(mask.clone());
    let base = turbo_profile_no::cost_config().unwrap();

    // Any water cell in the pack will do — the contributor prices by
    // classification, not by location.
    let cov = mask.coverage();
    let res = cov.meta.resolution_m as f64;
    let mut water = None;
    'outer: for j in 0..cov.meta.cells_y {
        for i in 0..cov.meta.cells_x {
            let x = cov.meta.min_x + i as f64 * res + res * 0.5;
            let y = cov.meta.max_y - j as f64 * res - res * 0.5;
            if matches!(mask.refused(x, y), Ok(turbo_tiles_mask::RefusalKind::Water)) {
                water = Some((x, y));
                break 'outer;
            }
        }
    }
    let (x, y) = water.expect("the CI pack contains water");

    let charge = |cost_s_per_m: f64| -> f64 {
        let mut cfg = base.clone();
        cfg.water.cost_s_per_m = cost_s_per_m;
        let ctx = EdgeContext {
            fx: x - 5.0,
            fy: y,
            tx: x + 5.0,
            ty: y,
            length_m: 10.0,
            profile: turbo_tiles_graph::Profile::Foot,
            kind: EdgeKind::Mesh,
            elev_probe: None,
            tuning: &cfg,
        };
        contributor.contribute(&ctx)
    };

    let (a, b) = (charge(1.0), charge(50.0));
    assert!(
        a > 0.0 && (b - a * 50.0).abs() < 1e-6,
        "the water charge must scale with the tuned cost — got {a} at 1.0 and \
         {b} at 50.0. If these are equal, the contributor is reading a baked \
         field again instead of `EdgeContext::tuning`."
    );
}

// ---- the surfaces Sjunkhatten does not have --------------------------

/// `surface_pace_skiloype` and `surface_pace_unknown`, judged end to end
/// on a network built to contain them.
///
/// The liveness table above reports both **unexercised**, and no bigger
/// slice can fix that: the full Sjunkhatten pack is 9,642 edges of
/// `fkb_type` 1 (sti) and 2 (vei), with zero of type 3 (skiløype) or 0
/// (the builder's "unclassified" fallback). It is a national park —
/// there are no prepared ski tracks in the FKB data, and the ingest
/// classifies everything it finds. So the knobs are not weakly covered;
/// they are uncoverable there, and the honest options are a network that
/// has these surfaces or no claim at all.
///
/// The network is the smallest thing that can express a *preference*: two
/// corridors between the same endpoints, so the router has a decision to
/// make rather than a single path to follow.
///
/// ```text
///        N (500, 150)          north corridor: the surface under test
///       /              \       length 1044 m
///   A (0,0) ---------- B (1000, 0)
///       \              /       south corridor: sti (the control)
///        S (500, -500)         length 1414 m
/// ```
///
/// At the calibrated pace the north corridor wins on length even after
/// its surface multiplier (1044 × 1.2 = 1253 < 1414). Slow that surface
/// down and the south corridor wins. The route must therefore change —
/// and it changes to another *trail*, not to a straight off-trail line,
/// since cutting across costs 1000 × 2.3 = 2300.
///
/// A flat DEM on purpose: with slope and gain identically zero, the two
/// corridors differ in exactly two things, length and surface, so a
/// route that moves can only have moved because of the surface.
#[test]
fn surface_pace_knobs_bite_on_a_synthetic_network() {
    let dir = tempfile::tempdir().unwrap();
    let dem_path = dir.path().join("flat.dem");
    write_flat_dem(&dem_path, -2000.0, 2000.0, 600, 100.0);
    let dem = turbo_geodata_artifacts::heightfield(Arc::new(
        turbo_tiles_elev::Dem::open(&dem_path).unwrap(),
    ));

    // (knob name, the fkb_type it prices, its calibrated foot value)
    let cases: [(&str, u8, f64); 2] = [
        ("surface_pace_skiloype", 3, 1.2),
        ("surface_pace_unknown", 0, 1.0),
    ];

    for (knob, fkb, calibrated) in cases {
        let g_path = dir.path().join(format!("two_corridor_{fkb}.graph"));
        write_two_corridor_graph(&g_path, fkb);
        let graph = Arc::new(turbo_tiles_graph::Graph::open(&g_path).unwrap());
        let pf = Pathfinder::with_defaults(
            Some(dem.clone()),
            None,
            Some(graph),
            turbo_profile_no::cost_config().expect("calibrated config"),
        );

        let route = |patch: CostConfigPatch| -> Vec<(i64, i64)> {
            let prefs = Prefs {
                cost_config_override: Some(patch.over(&neutral_proximity())),
                ..Default::default()
            };
            pf.solve(Point::new(0.0, 0.0), Point::new(1000.0, 0.0), prefs)
                .unwrap_or_else(|e| panic!("{knob}: fixture must route: {e:?}"))
                .geometry
                .iter()
                .map(|p| ((p.x * 1e3) as i64, (p.y * 1e3) as i64))
                .collect()
        };
        // Which side of the A–B axis the route runs along, by signed area.
        let side = |g: &[(i64, i64)]| -> f64 { g.iter().map(|p| p.1 as f64).sum::<f64>() };

        let base = route(CostConfigPatch::default());
        let mut slow = CostConfigPatch::default();
        match fkb {
            3 => slow.surface_pace_skiloype = Some(9.0),
            0 => slow.surface_pace_unknown = Some(9.0),
            _ => unreachable!(),
        }
        let slowed = route(slow);

        assert!(
            side(&base) > 0.0,
            "{knob}: at the calibrated pace ({calibrated}) the north corridor is \
             the cheaper line (1044 m x {calibrated} = {:.0} < 1414 m), so the \
             baseline route should run north of the A-B axis. It did not, so \
             this fixture is no longer testing what it claims — check the \
             geometry before reading anything into the result below.",
            1044.0 * calibrated
        );
        assert!(
            side(&slowed) < 0.0,
            "{knob} is INERT: making `fkb_type {fkb}` nine times slower left \
             the route on the north corridor, which is built entirely from \
             that surface. The south corridor is sti and 370 m longer, so at \
             this pace it is far cheaper — the router would have to be \
             ignoring the knob to stay north.\n\
             This is the one class of knob the CI pack cannot judge (no \
             Sjunkhatten edge has fkb_type 0 or 3), so this fixture is the \
             only thing standing between it and shipping dead."
        );
        println!("{knob:<38} live  (north -> south on the synthetic network)");
    }
}

/// Trail proximity, switched off — applied to BOTH sides of the
/// comparison above.
///
/// It has to go, and finding out why is the more interesting half of
/// this fixture. `TrailProximityContributor::rtree_for` indexes ONE
/// surface per profile: sti for Foot, vei for Bicycle, skiløype for Ski.
/// So on the Foot profile the south (sti) corridor radiates a 0.15×
/// pace bonus over the mesh beside it and the north corridor radiates
/// nothing — and a mesh line hugging the sti is then cheaper than
/// walking on ANY trail. The first version of this fixture measured
/// exactly that and read it as the surface knob failing.
///
/// That is a real property of the cost model, not a bug: for a walker,
/// nearness to a marked path is genuinely what makes open ground easy.
/// It is simply a much larger effect than the one under test here, so
/// it is pinned on both sides, which is what `Ctx::base` exists to do.
fn neutral_proximity() -> CostConfigPatch {
    CostConfigPatch {
        trail_proximity_bonus_at_zero: Some(1.0),
        ..Default::default()
    }
}

/// Two corridors between the same endpoints — north built from
/// `fkb_type = north_fkb`, south from sti (1). See the test above for
/// the geometry and why it is shaped this way.
fn write_two_corridor_graph(path: &std::path::Path, north_fkb: u8) {
    use byteorder::{LittleEndian, WriteBytesExt};
    use std::io::Write;
    use turbo_tiles_artifacts::{write_header, ArtifactKind, Header};
    use turbo_tiles_graph::{
        write_meta as write_graph_meta, EdgeRecord, GraphMeta, NodePos, GRAPH_FORMAT_VERSION,
    };

    // A, B, N (north waypoint), S (south waypoint).
    let nodes = vec![
        NodePos { x: 0.0, y: 0.0 },
        NodePos { x: 1000.0, y: 0.0 },
        NodePos { x: 500.0, y: 150.0 },
        NodePos {
            x: 500.0,
            y: -500.0,
        },
    ];
    let leg = |a: u32, b: u32, fkb: u8| {
        let (dx, dy) = (
            (nodes[b as usize].x - nodes[a as usize].x) as f64,
            (nodes[b as usize].y - nodes[a as usize].y) as f64,
        );
        let len = (dx * dx + dy * dy).sqrt() as f32;
        // Both directions: the router walks a directed CSR.
        [(a, b), (b, a)].map(|(from_id, to_id)| EdgeRecord {
            from_id,
            to_id,
            length_m: len,
            gain_m: 0.0,
            loss_m: 0.0,
            slope_max_deg: 0.0,
            fkb_type: fkb,
            marking: 0,
            surface: 0,
            source: 1,
            attr_flags: 0,
        })
    };
    let edges: Vec<EdgeRecord> = [
        leg(0, 2, north_fkb),
        leg(2, 1, north_fkb),
        leg(0, 3, 1),
        leg(3, 1, 1),
    ]
    .concat();

    let (nc, ec, pc) = (nodes.len() as u32, edges.len() as u32, 3u32);
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    write_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Graph,
            format_version: GRAPH_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .unwrap();
    write_graph_meta(
        &mut f,
        &GraphMeta {
            node_count: nc,
            edge_count: ec,
            profile_count: pc,
            srid: 25833,
        },
    )
    .unwrap();
    f.write_all(bytemuck::cast_slice(&nodes)).unwrap();
    f.write_all(bytemuck::cast_slice(&edges)).unwrap();

    // CSR: offsets by from_id, then the edge-index table in that order.
    let mut offsets = vec![0u32; nc as usize + 1];
    for e in &edges {
        offsets[e.from_id as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }
    for o in &offsets {
        f.write_u32::<LittleEndian>(*o).unwrap();
    }
    let mut cursor = offsets.clone();
    let mut table = vec![0u32; ec as usize];
    for (ei, e) in edges.iter().enumerate() {
        let slot = &mut cursor[e.from_id as usize];
        table[*slot as usize] = ei as u32;
        *slot += 1;
    }
    for t in &table {
        f.write_u32::<LittleEndian>(*t).unwrap();
    }
    // Baked per-profile cost: length, so the profile channel is neutral
    // and every cost difference comes from the contributor stack.
    for e in &edges {
        for _ in 0..pc {
            f.write_f32::<LittleEndian>(e.length_m).unwrap();
        }
    }
    f.flush().unwrap();
}

/// Single-tile flat DEM at 10 m resolution, upper-left `(ulx, uly)`.
/// Flat because these fixtures are about surface, not terrain: slope and
/// gain are identically zero, so they cannot confound the comparison.
fn write_flat_dem(path: &std::path::Path, ulx: f64, uly: f64, cells: u32, elev: f32) {
    use std::io::Write;
    use turbo_tiles_artifacts::{write_header, ArtifactKind, Header, HEADER_BYTES};
    use turbo_tiles_elev::{
        write_meta as write_dem_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD,
        DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
    };
    let mut f = std::fs::File::create(path).unwrap();
    write_header(
        &mut f,
        &Header {
            kind: ArtifactKind::Dem,
            format_version: DEM_FORMAT_VERSION,
            build_timestamp_unix_sec: 0,
        },
    )
    .unwrap();
    write_dem_meta(
        &mut f,
        &DemMeta {
            tile_count: 1,
            tile_cells: cells,
            pixel_size_m: 10.0,
            nodata: NODATA_SENTINEL,
            compression: COMPRESSION_ZSTD,
        },
    )
    .unwrap();
    let data = vec![elev; (cells * cells) as usize];
    let compressed = zstd::encode_all(bytemuck::cast_slice::<f32, u8>(&data), 1).unwrap();
    write_tile_entry(
        &mut f,
        &TileEntry {
            ulx,
            uly,
            offset: (HEADER_BYTES + DEM_META_BYTES + TILE_ENTRY_BYTES) as u64,
            compressed_size: compressed.len() as u32,
        },
    )
    .unwrap();
    f.write_all(&compressed).unwrap();
    f.sync_all().unwrap();
}
