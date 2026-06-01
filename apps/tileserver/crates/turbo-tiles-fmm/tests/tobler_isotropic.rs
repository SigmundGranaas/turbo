//! Phase 2 validation: slope-aware FMM on synthetic terrain.
//!
//! With the isotropic Tobler metric, the wave front should:
//!   (a) propagate fastest on flat ground (~0.714 s/m baseline);
//!   (b) slow down on steep slopes proportionally to Tobler;
//!   (c) detour around refused (cliff) regions;
//!   (d) produce arrival-time isocontours that visibly deform
//!       around terrain features.
//!
//! Phase 2 doesn't yet claim contour-FOLLOWING — that's the
//! anisotropic / state-augmented elastica work in phase 5. What
//! phase 2 must demonstrate is that *magnitude* of slowdown is
//! correct: a cell uphill of the seed has a higher arrival time
//! than a cell the same horizontal distance away on flat ground.

use turbo_tiles_fmm::{
    solve_2d_with_metric, ArrayElevation, GridShape, StopCondition, ToblerIsotropic,
};

fn make_metric(elev: ArrayElevation) -> ToblerIsotropic<ArrayElevation> {
    ToblerIsotropic {
        elev,
        refuse_above_deg: 50.0,
        base_pace_s_per_m: 0.714,
        off_trail_factor: 1.0,
    }
}

#[test]
fn flat_terrain_matches_isotropic_baseline() {
    // 100×100 grid, h=10, flat. Wave from one corner should give
    // arrival ≈ euclidean distance × base pace. Compare against a
    // pure isotropic with the same constant cost.
    let nx = 100u32;
    let ny = 100u32;
    let h = 10.0_f64;
    let shape = GridShape::new_2d(nx, ny, 0.0, 0.0, h);
    let elev = ArrayElevation {
        data: vec![Some(100.0); (nx * ny) as usize],
        nx,
        ny,
    };
    let metric = make_metric(elev);
    let result = solve_2d_with_metric(shape, &metric, &[(0, 0, 0.0)], StopCondition::AllAccepted);

    // Check the far corner (99, 99). Distance ≈ √2 · 99 · h ≈ 1400 m.
    // Base pace 0.714 s/m → expected arrival ≈ 1000 s.
    let u_corner = result.arrival.get(99, 99, 0);
    let expected = 99.0_f32 * h as f32 * 2.0_f32.sqrt() * 0.714;
    let rel_err = (u_corner - expected).abs() / expected;
    // Allow the 4-neighbour Sethian's diagonal overestimate.
    assert!(
        rel_err < 0.2,
        "flat far-corner arrival {} vs expected {}; rel_err {:.3}",
        u_corner,
        expected,
        rel_err
    );
}

#[test]
fn uphill_ramp_slows_wave() {
    // 50×50 grid, h=10. Linear ramp: z(i, j) = i · rise where
    // rise = tan(30°) · h ≈ 5.77 m. The wave from cell (0, 25)
    // propagates in two directions:
    //   - +x: uphill, slow (Tobler pace at 30°)
    //   - -x: downhill, also slow in phase 2 (isotropic — same
    //     magnitude). Equally slowed.
    //
    // Compared to a flat-terrain baseline, both directions are
    // slower. Assert that the wave reaches cell (10, 25) in MORE
    // time than on flat terrain.
    let nx = 50u32;
    let ny = 50u32;
    let h = 10.0_f64;
    let shape = GridShape::new_2d(nx, ny, 0.0, 0.0, h);

    let rise = (30.0_f32.to_radians().tan()) * h as f32;
    let ramp_data: Vec<Option<f32>> = (0..ny)
        .flat_map(|_| (0..nx).map(move |i| Some(i as f32 * rise)))
        .collect();
    let ramp = ArrayElevation {
        data: ramp_data,
        nx,
        ny,
    };
    let metric_ramp = make_metric(ramp);
    let r_ramp = solve_2d_with_metric(
        shape,
        &metric_ramp,
        &[(0, 25, 0.0)],
        StopCondition::AllAccepted,
    );

    let flat_data: Vec<Option<f32>> = vec![Some(100.0); (nx * ny) as usize];
    let flat = ArrayElevation {
        data: flat_data,
        nx,
        ny,
    };
    let metric_flat = make_metric(flat);
    let r_flat = solve_2d_with_metric(
        shape,
        &metric_flat,
        &[(0, 25, 0.0)],
        StopCondition::AllAccepted,
    );

    let u_ramp = r_ramp.arrival.get(10, 25, 0);
    let u_flat = r_flat.arrival.get(10, 25, 0);
    let ratio = u_ramp / u_flat;
    eprintln!(
        "ramp arrival {} vs flat {}; ratio {}",
        u_ramp, u_flat, ratio
    );
    // Tobler at 30° vs flat is roughly 5×. The 4-cell distance is
    // short enough that the dominant effect is the per-cell pace
    // ratio.
    assert!(
        ratio > 3.0,
        "30° ramp should slow wave significantly; got {}×",
        ratio
    );
}

#[test]
fn refused_cliff_creates_island() {
    // 60×60 grid, h=5. Wave from (0, 30). A 5-cell vertical strip
    // of cliffs (>50° slope) at i=30 blocks direct propagation. The
    // wave must wrap around the top and bottom edges of the strip.
    //
    // Assert:
    //   - the cliff cells themselves have +∞ arrival
    //   - cells past the cliff (i = 35, j = 30) ARE reachable
    //     (i.e. finite) but with a detour cost > straight-line
    let nx = 60u32;
    let ny = 60u32;
    let h = 5.0_f64;
    let shape = GridShape::new_2d(nx, ny, 0.0, 0.0, h);

    // Steep cliff: dz/dx = tan(60°) for i ∈ [29, 31] (3 cells thick
    // so the central-difference catches it even on cell 30).
    let mut data = Vec::with_capacity((nx * ny) as usize);
    let cliff_rise = (60.0_f32.to_radians().tan()) * h as f32;
    for j in 0..ny {
        for i in 0..nx {
            let _ = j;
            // Outside the cliff zone, elevation = 100 m.
            // Inside the cliff zone (29 ≤ i ≤ 31), elevation jumps.
            if (29..=31).contains(&i) {
                data.push(Some(100.0 + (i as i32 - 29) as f32 * cliff_rise));
            } else if i > 31 {
                data.push(Some(100.0 + 2.0 * cliff_rise));
            } else {
                data.push(Some(100.0));
            }
        }
    }
    let metric = make_metric(ArrayElevation { data, nx, ny });
    let result = solve_2d_with_metric(shape, &metric, &[(0, 30, 0.0)], StopCondition::AllAccepted);

    // The cliff's CENTRE cell sees the maximum slope (central
    // difference over both elevation jumps) and is refused.
    // Neighbouring cells (i=29, i=31) see a milder slope through
    // the central-difference smoothing and stay walkable — that's
    // a known artefact of finite-difference slope sampling at a
    // discontinuity, not a bug in the refusal logic.
    let u_centre = result.arrival.get(30, 30, 0);
    assert!(
        u_centre.is_infinite(),
        "cliff centre should be refused; got {}",
        u_centre
    );

    // Past-cliff cell on the same row. Even with the centre cell
    // refused, the wave can squeeze through the adjacent slow cells
    // — they're walkable, just expensive (high pace from the steep
    // slope). Assert the arrival is finite AND substantially higher
    // than a flat-terrain crossing of the same distance.
    let u_past = result.arrival.get(40, 30, 0);
    assert!(
        u_past.is_finite(),
        "post-cliff cell should be reachable, got {}",
        u_past
    );
    let flat_baseline = 40.0_f32 * h as f32 * 0.714; // ≈ 142.8 s
    assert!(
        u_past > 1.5 * flat_baseline,
        "post-cliff arrival should be at least 1.5× flat baseline ({}); got {}",
        flat_baseline,
        u_past
    );
    eprintln!(
        "cliff test: post-cliff arrival {:.0} s vs flat baseline {:.0} s ({:.2}× slowdown)",
        u_past,
        flat_baseline,
        u_past / flat_baseline
    );
}

#[test]
fn slope_field_visible_in_arrival_isocontour() {
    // Conical-mountain synthetic: cell elevation = max(0, R - dist)
    // where dist is distance from grid centre. Seed at one foot of
    // the cone; the arrival-time field should be visibly compressed
    // on the steeper side. In phase 2 (isotropic) the path doesn't
    // wrap the contour, but the time-to-reach a cell on the OTHER
    // side of the peak is much higher than the same horizontal
    // distance going around — assert that asymmetry.
    let n = 80u32;
    let h = 10.0_f64;
    let shape = GridShape::new_2d(n, n, 0.0, 0.0, h);
    let ci = n as f32 / 2.0;
    let cj = n as f32 / 2.0;
    let r_cone = 30.0_f32;
    let rise_per_cell = 4.0_f32; // 40 m/cell rise toward summit
    let mut data = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let d = ((i as f32 - ci).powi(2) + (j as f32 - cj).powi(2)).sqrt();
            if d < r_cone {
                data.push(Some(100.0 + (r_cone - d) * rise_per_cell));
            } else {
                data.push(Some(100.0));
            }
        }
    }
    let metric = make_metric(ArrayElevation { data, nx: n, ny: n });

    // Seed at the foot of the cone, west side.
    let seed_i = (ci - r_cone) as u32 - 5;
    let seed_j = cj as u32;
    let result = solve_2d_with_metric(
        shape,
        &metric,
        &[(seed_i, seed_j, 0.0)],
        StopCondition::AllAccepted,
    );

    // Goal A: directly across the cone (east side, same row).
    let goal_a = (((ci + r_cone) as u32) + 5, cj as u32);
    // Goal B: same horizontal distance but going around the cone
    // (south of the peak, then east).
    let goal_b = (((ci + r_cone) as u32) + 5, (cj - r_cone * 1.5) as u32);

    let ua = result.arrival.get(goal_a.0, goal_a.1, 0);
    let ub = result.arrival.get(goal_b.0, goal_b.1, 0);
    eprintln!("through-peak arrival: {}; around-cone arrival: {}", ua, ub);

    // Without anisotropy (phase 2), the through-peak path slows
    // down (steep terrain), and the around-cone path is longer.
    // BOTH should be finite; the around path should be CHEAPER
    // when the cone is steep enough that the slowdown over the
    // peak exceeds the extra horizontal distance.
    assert!(ua.is_finite() && ub.is_finite());
    // The through-peak crossing IS slower than the around path —
    // proving the slope cost field deforms the wave away from
    // steep terrain even in the isotropic phase.
    assert!(
        ub < ua,
        "around-cone path should be cheaper than through-peak; got around={} through={}",
        ub,
        ua
    );
}
