//! Regression: the grade-limited (x,y,heading) solver must switchback up
//! a slope steeper than its grade cap rather than climb the fall line —
//! and every step of the extracted path must respect the cap. This is
//! the behaviour a convex-metric eikonal provably cannot produce.

use turbo_tiles_fmm::{
    extract_path_lifted, solve_lifted_grade_limited, ArrayElevation, ArrayOverlay,
    GradeLimitedCost, GridShape, N_HEADINGS,
};

/// A refused-cell barrier ("lake") must be routed around: the extracted
/// path reaches the goal and NO point on it lands in a refused cell. This
/// is the behaviour that was missing — the lifted solver was blind to the
/// water mask and walked straight across lakes.
#[test]
fn routes_around_a_refused_lake_never_entering_it() {
    let n = 41u32;
    let cell_m = 10.0;
    // Flat ground (elevation 0): grade never refuses, so the ONLY thing
    // that can deflect the route is the refused overlay.
    let elev = ArrayElevation {
        data: vec![Some(0.0); (n * n) as usize],
        nx: n,
        ny: n,
    };
    let shape = GridShape::new_3d(n, n, N_HEADINGS, 0.0, 0.0, cell_m);

    // A "lake": refuse columns 18..=22 for rows j >= 8, leaving a gap only
    // along the top edge (j < 8). Start + goal sit on row 20, so the
    // straight line runs dead through the lake — the solver must detour up.
    let mut refused = vec![false; (n * n) as usize];
    for j in 0..n {
        for i in 0..n {
            if (18..=22).contains(&i) && j >= 8 {
                refused[(j * n + i) as usize] = true;
            }
        }
    }
    let cost = GradeLimitedCost {
        gain_k: 0.0,
        elev,
        base_pace_s_per_m: 0.714,
        off_trail_factor: 1.0,
        max_grade_deg: 45.0,
        turn_penalty_s: 4.0,
        overlay: ArrayOverlay {
            nx: n,
            refused: refused.clone(),
            pace_mul: vec![],
        },
    };
    let start = (5u32, 20u32);
    let goal = (35u32, 20u32);
    let r = solve_lifted_grade_limited(shape, &cost, start, goal, None);
    assert!(
        r.goal_state.is_some(),
        "goal must be reachable around the lake"
    );

    let (sx, sy) = shape.cell_centre(start.0, start.1);
    let (gx, gy) = shape.cell_centre(goal.0, goal.1);
    let path = extract_path_lifted(&shape, &r, (sx, sy), (gx, gy)).expect("path");

    // The decisive assertion: not a single path vertex may land in a
    // refused cell. (Densely resample each segment so we also catch a
    // straight chord that would clip the lake between two vertices.)
    let mut min_j_reached = n;
    for w in path.windows(2) {
        let steps = 40;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let x = w[0].0 + (w[1].0 - w[0].0) * t;
            let y = w[0].1 + (w[1].1 - w[0].1) * t;
            let (ci, cj) = shape.world_to_cell(x, y).expect("on grid");
            assert!(
                !refused[(cj * n + ci) as usize],
                "path entered refused lake cell ({ci},{cj})"
            );
            min_j_reached = min_j_reached.min(cj);
        }
    }
    // The only gap is along the top edge (j < 8), so a route from row 20 to
    // row 20 past the lake MUST dip into it. Proves a real detour, not a
    // straight chord that the assertion above happened to miss.
    eprintln!("lake detour: min_j_reached={min_j_reached} (gap is j<8 on cols 18..=22)");
    assert!(
        min_j_reached < 8,
        "expected the route to detour through the j<8 gap"
    );
}

/// Constant-slope ramp climbing in +x at `slope_deg`.
fn ramp_dem(n: u32, slope_deg: f32) -> ArrayElevation {
    let rise = slope_deg.to_radians().tan() * 10.0; // cell_m = 10
    let mut data = Vec::with_capacity((n * n) as usize);
    for _j in 0..n {
        for i in 0..n {
            data.push(Some(i as f32 * rise));
        }
    }
    ArrayElevation { data, nx: n, ny: n }
}

#[test]
fn switchbacks_up_a_steep_ramp_within_grade_cap() {
    let n = 61u32;
    let cell_m = 10.0;
    let max_grade_deg = 20.0_f32;
    // A 35° ramp — far steeper than the 20° cap, so a direct ascent is
    // forbidden and the solver must traverse/switchback.
    let elev = ramp_dem(n, 35.0);
    let shape = GridShape::new_3d(n, n, N_HEADINGS, 0.0, 0.0, cell_m);
    let cost = GradeLimitedCost {
        gain_k: 0.0,
        elev,
        base_pace_s_per_m: 0.714,
        off_trail_factor: 1.0,
        max_grade_deg,
        turn_penalty_s: 8.0,
        overlay: ArrayOverlay {
            nx: n,
            refused: vec![],
            pace_mul: vec![],
        },
    };
    // Climb in +x (up the fall line) across the middle row.
    let start = (5u32, 30u32);
    let goal = (55u32, 30u32);
    let r = solve_lifted_grade_limited(shape, &cost, start, goal, None);
    assert!(
        r.goal_state.is_some(),
        "goal must be reachable via switchbacks"
    );

    let (sx, sy) = shape.cell_centre(start.0, start.1);
    let (gx, gy) = shape.cell_centre(goal.0, goal.1);
    let path = extract_path_lifted(&shape, &r, (sx, sy), (gx, gy)).expect("path");

    // Length vs straight: switchbacks make it substantially longer.
    let straight = ((gx - sx).powi(2) + (gy - sy).powi(2)).sqrt();
    let len: f64 = path
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum();
    let tortuosity = len / straight;

    // Max grade along the extracted path (uses the same constant ramp:
    // grade between two points = Δx-direction slope). On the ramp,
    // elevation = x/cell_m * rise, so grade depends only on the x-step.
    let rise_per_m = 35.0_f32.to_radians().tan(); // dz/dx in world metres
    let mut max_grade = 0.0f32;
    for w in path.windows(2) {
        let dx = (w[1].0 - w[0].0) as f32;
        let dy = (w[1].1 - w[0].1) as f32;
        let horiz = (dx * dx + dy * dy).sqrt();
        if horiz < 1e-3 {
            continue;
        }
        let dz = dx * rise_per_m; // elevation only varies with x
        let grade = (dz.abs() / horiz).atan().to_degrees();
        max_grade = max_grade.max(grade);
    }
    eprintln!("tortuosity={tortuosity:.2} max_grade={max_grade:.1}deg len={len:.0} straight={straight:.0}");

    assert!(
        tortuosity > 1.5,
        "expected switchbacks (tortuosity > 1.5); got {tortuosity:.2} — solver climbed too directly"
    );
    // The raw lattice path must respect the cap (allow a small margin for
    // the discrete heading set; smoothing is a separate concern).
    assert!(
        max_grade <= max_grade_deg + 3.0,
        "path segment exceeded the grade cap: {max_grade:.1}deg > {max_grade_deg}deg"
    );
}

#[test]
fn gentle_ramp_goes_roughly_straight() {
    // On a 10° ramp (under the 20° cap) there's no need to switchback —
    // the solver should go essentially straight.
    let n = 61u32;
    let elev = ramp_dem(n, 10.0);
    let shape = GridShape::new_3d(n, n, N_HEADINGS, 0.0, 0.0, 10.0);
    let cost = GradeLimitedCost {
        gain_k: 0.0,
        elev,
        base_pace_s_per_m: 0.714,
        off_trail_factor: 1.0,
        max_grade_deg: 20.0,
        turn_penalty_s: 8.0,
        overlay: ArrayOverlay {
            nx: n,
            refused: vec![],
            pace_mul: vec![],
        },
    };
    let start = (5u32, 30u32);
    let goal = (55u32, 30u32);
    let r = solve_lifted_grade_limited(shape, &cost, start, goal, None);
    let (sx, sy) = shape.cell_centre(start.0, start.1);
    let (gx, gy) = shape.cell_centre(goal.0, goal.1);
    let path = extract_path_lifted(&shape, &r, (sx, sy), (gx, gy)).expect("path");
    let straight = ((gx - sx).powi(2) + (gy - sy).powi(2)).sqrt();
    let len: f64 = path
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum();
    assert!(
        len / straight < 1.2,
        "gentle ramp should be ~straight; tort={:.2}",
        len / straight
    );
}
