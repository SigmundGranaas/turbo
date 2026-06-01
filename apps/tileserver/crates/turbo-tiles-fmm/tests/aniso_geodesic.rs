//! Regression: the anisotropic geodesic must not balloon into a giant
//! detour when the direct line crosses a modest hill.
//!
//! Motivated by the terrain-corpus eval (force-off-trail mode): real
//! hikes on gentle terrain produced solver routes 2.5–3.4× longer than
//! the straight line, looping the corridor perimeter to avoid crossing
//! a brief ~16° bank — a detour far larger than any cost the bank could
//! justify (a 16° slope is only ~2.7× base pace, over a fraction of the
//! route). That smells like a solver/extraction correctness bug rather
//! than a calibration trade-off. This test reproduces the shape in
//! isolation so we can tell whether the arrival field is wrong or the
//! gradient-descent extraction is.

use turbo_tiles_fmm::{
    bake_aniso_corridor, extract_path_aniso, solve_2d_anisotropic, ArrayElevation, GridShape,
    PathPoint, StopCondition, ToblerAnisotropic,
};

/// Flat plain (z=0) with a single smooth Gaussian hill centred in the
/// grid. Peak height + width chosen so the flanks reach ~15–20° — the
/// gentle-terrain regime where the real failures appeared.
fn hill_dem(n: u32, cell_m: f64, peak_m: f32, sigma_cells: f32) -> ArrayElevation {
    let c = n as f32 / 2.0;
    let mut data = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let dx = i as f32 - c;
            let dy = j as f32 - c;
            let r2 = dx * dx + dy * dy;
            let z = peak_m * (-r2 / (2.0 * sigma_cells * sigma_cells)).exp();
            let _ = cell_m;
            data.push(Some(z));
        }
    }
    ArrayElevation { data, nx: n, ny: n }
}

/// Near-flat plain with deterministic sub-metre "DTM10 noise". A gentle
/// constant tilt (a few degrees) plus ±`noise_m` pseudo-random bumps —
/// mimics the real gentle-terrain regime where the corpus failures
/// appeared. Noise is hash-based so the test is reproducible.
fn noisy_plain_dem(n: u32, tilt_deg: f32, noise_m: f32) -> ArrayElevation {
    let tan = tilt_deg.to_radians().tan();
    let rise_per_cell = tan * 10.0;
    let mut data = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            // Cheap deterministic hash → [-1, 1].
            let h = (i.wrapping_mul(73856093) ^ j.wrapping_mul(19349663)) as f32;
            let frac = (h * 0.000_000_1).sin();
            let z = i as f32 * rise_per_cell + frac * noise_m;
            data.push(Some(z));
        }
    }
    ArrayElevation { data, nx: n, ny: n }
}

fn path_len_m(p: &[PathPoint]) -> f64 {
    p.windows(2)
        .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
        .sum()
}

#[test]
fn geodesic_does_not_balloon_around_a_modest_hill() {
    let n = 81u32;
    let cell_m = 10.0;
    let shape = GridShape::new_2d(n, n, 0.0, 0.0, cell_m);
    // Peak 60 m over sigma=8 cells (80 m): max flank slope ~ atan(60 / ~130) ≈ 25°,
    // with the seed→goal line crossing the gentler shoulders (~15°).
    let elev = hill_dem(n, cell_m, 60.0, 8.0);
    let metric = ToblerAnisotropic {
        elev,
        refuse_above_deg: 45.0,
        base_pace_s_per_m: 0.714,
        off_trail_factor: 1.0,
        gain_factor_k: 0.0,
    };
    let forms = bake_aniso_corridor(shape, &metric);

    // Seed left-centre, goal right-centre: the straight line runs east
    // through the hill's centre column.
    let (si, sj) = (8u32, n / 2);
    let (gi, gj) = (n - 9, n / 2);
    let r = solve_2d_anisotropic(shape, &forms, &[(si, sj, 0.0)], StopCondition::AllAccepted);

    let (sx, sy) = shape.cell_centre(si, sj);
    let (gx, gy) = shape.cell_centre(gi, gj);
    let start = PathPoint { x: sx, y: sy };
    let goal = PathPoint { x: gx, y: gy };
    let path = extract_path_aniso(&shape, &r.arrival, &forms, start, goal, None, None)
        .expect("extraction should converge");

    let straight = ((gx - sx).powi(2) + (gy - sy).powi(2)).sqrt();
    let len = path_len_m(&path);
    let tortuosity = len / straight;

    // The arrival field MUST be monotone non-increasing along the
    // extracted path (gradient descent). If it isn't, the field has
    // spurious local structure (solve bug). If it IS monotone but the
    // path is still huge, the field genuinely rates the detour as
    // cheapest (metric/calibration), and extraction is innocent.
    let mut max_increase = 0.0f32;
    let mut prev = f32::INFINITY;
    for p in &path {
        if let Some((i, j)) = shape.world_to_cell(p.x, p.y) {
            let u = r.arrival.get(i, j, 0);
            if u.is_finite() {
                if u > prev {
                    max_increase = max_increase.max(u - prev);
                }
                prev = u;
            }
        }
    }

    eprintln!(
        "straight={straight:.0}m len={len:.0}m tortuosity={tortuosity:.2} \
         goal_arrival={:.1}s max_u_increase_along_path={max_increase:.3}s",
        r.arrival.get(gi, gj, 0)
    );

    // A hiker detours modestly around a 25° dome; anything past ~1.8×
    // the straight line is the ballooning failure we are hunting.
    assert!(
        tortuosity < 1.8,
        "geodesic ballooned: tortuosity={tortuosity:.2} (len {len:.0}m vs straight {straight:.0}m)"
    );
}

/// Constant-slope ramp: z grows linearly with the x index, so the
/// gradient is uniform (+x) and the contour direction is +y. Anisotropy
/// is uniform and strong everywhere.
fn ramp_dem(n: u32, slope_deg: f32) -> ArrayElevation {
    let rise = slope_deg.to_radians().tan() * 10.0;
    let mut data = Vec::with_capacity((n * n) as usize);
    for _j in 0..n {
        for i in 0..n {
            data.push(Some(i as f32 * rise));
        }
    }
    ArrayElevation { data, nx: n, ny: n }
}

#[test]
fn geodesic_does_not_drift_on_long_anisotropic_traverse() {
    // Large corridor + long path on a uniform 20° ramp. The true
    // anisotropic geodesic between two points on a constant ramp is the
    // straight diagonal (tortuosity 1.0) — total climb is path-independent,
    // so any detour is pure added cost. This caught two real bugs: (1) the
    // grid-edge metric collapsing to isotropic base pace turned the corridor
    // border into a cheap racetrack the wave flooded along (fixed: clamped
    // one-sided gradients at edges), and (2) -∇u extraction drifting off the
    // anisotropic characteristic (fixed: extract_path_aniso steps -G*∇u).
    // Before the fixes this ballooned to tortuosity ~1.96 with the arrival
    // field at ~0.59× the true optimum; now ~1.01 at ratio ~1.00.
    let n = 201u32;
    let cell_m = 10.0;
    let shape = GridShape::new_2d(n, n, 0.0, 0.0, cell_m);
    let elev = ramp_dem(n, 20.0);
    let metric = ToblerAnisotropic {
        elev,
        refuse_above_deg: 45.0,
        base_pace_s_per_m: 0.714,
        off_trail_factor: 1.0,
        gain_factor_k: 0.0,
    };
    let forms = bake_aniso_corridor(shape, &metric);
    // Seed bottom-left-ish, goal top-right-ish: the path must both climb
    // (+x) and traverse (+y), so the optimal route is a smooth diagonal
    // curve biased toward the contour.
    let (si, sj) = (20u32, 20);
    let (gi, gj) = (n - 21, n - 21);
    let r = solve_2d_anisotropic(shape, &forms, &[(si, sj, 0.0)], StopCondition::AllAccepted);
    let (sx, sy) = shape.cell_centre(si, sj);
    let (gx, gy) = shape.cell_centre(gi, gj);
    let path = extract_path_aniso(
        &shape,
        &r.arrival,
        &forms,
        PathPoint { x: sx, y: sy },
        PathPoint { x: gx, y: gy },
        None,
        None,
    )
    .expect("extraction should converge");
    let straight = ((gx - sx).powi(2) + (gy - sy).powi(2)).sqrt();
    let len = path_len_m(&path);
    let tortuosity = len / straight;
    // Analytic optimum on a constant-coefficient metric is the straight
    // diagonal: time = sqrt(τ_along² + τ_perp²)/√2 · straight_len.
    let tan = 20.0_f64.to_radians().tan();
    let v = 1.6667 * (-3.5 * (tan + 0.05)).exp();
    let tau_along = (1.0 / v).max(0.714);
    let tau_perp = 0.714_f64;
    let analytic =
        ((tau_along * tau_along + tau_perp * tau_perp).sqrt() / 2.0_f64.sqrt()) * straight;
    let goal_u = r.arrival.get(gi, gj, 0) as f64;
    // Monotonicity of arrival along the extracted path.
    let mut max_inc = 0.0f32;
    let mut prev = f32::INFINITY;
    for pp in &path {
        if let Some((i, j)) = shape.world_to_cell(pp.x, pp.y) {
            let u = r.arrival.get(i, j, 0);
            if u.is_finite() {
                if u > prev {
                    max_inc = max_inc.max(u - prev);
                }
                prev = u;
            }
        }
    }
    let npts = path.len();
    let ratio = goal_u / analytic;
    eprintln!(
        "RAMP-LONG: straight={straight:.0}m len={len:.0}m tortuosity={tortuosity:.2} npts={npts} \
         goal_arrival={goal_u:.0}s analytic_optimum={analytic:.0}s ratio={ratio:.2} max_u_increase={max_inc:.2}s"
    );
    assert!(
        tortuosity < 1.8,
        "long anisotropic traverse ballooned: tortuosity={tortuosity:.2} \
         (len {len:.0}m vs straight {straight:.0}m)"
    );
}

#[test]
fn geodesic_does_not_balloon_on_noisy_gentle_terrain() {
    // The real failures were on gentle terrain (mean slope ~3°, max ~16°).
    // Hypothesis: sub-metre DTM10 noise creates random local gradients
    // that the anisotropic metric amplifies into conflicting cheap-contour
    // directions, so the geodesic chases phantom slopes and balloons.
    let n = 81u32;
    let cell_m = 10.0;
    let shape = GridShape::new_2d(n, n, 0.0, 0.0, cell_m);
    // 3° base tilt + ±0.5 m noise (representative DTM10 vertical noise).
    let elev = noisy_plain_dem(n, 3.0, 0.5);
    let metric = ToblerAnisotropic {
        elev,
        refuse_above_deg: 45.0,
        base_pace_s_per_m: 0.714,
        off_trail_factor: 1.0,
        gain_factor_k: 0.0,
    };
    let forms = bake_aniso_corridor(shape, &metric);
    let (si, sj) = (8u32, n / 2);
    let (gi, gj) = (n - 9, n / 2);
    let r = solve_2d_anisotropic(shape, &forms, &[(si, sj, 0.0)], StopCondition::AllAccepted);
    let (sx, sy) = shape.cell_centre(si, sj);
    let (gx, gy) = shape.cell_centre(gi, gj);
    let path = extract_path_aniso(
        &shape,
        &r.arrival,
        &forms,
        PathPoint { x: sx, y: sy },
        PathPoint { x: gx, y: gy },
        None,
        None,
    )
    .expect("extraction should converge");
    let straight = ((gx - sx).powi(2) + (gy - sy).powi(2)).sqrt();
    let len = path_len_m(&path);
    let tortuosity = len / straight;
    eprintln!(
        "NOISY: straight={straight:.0}m len={len:.0}m tortuosity={tortuosity:.2} \
         goal_arrival={:.1}s",
        r.arrival.get(gi, gj, 0)
    );
    assert!(
        tortuosity < 1.8,
        "noisy gentle terrain ballooned: tortuosity={tortuosity:.2} \
         (len {len:.0}m vs straight {straight:.0}m)"
    );
}

#[test]
fn gain_factor_raises_uphill_arrival_monotonically() {
    // The directional Naismith gain term (folded into τ_along) must make
    // climbing strictly more expensive as `gain_factor_k` rises, without
    // touching the flat/contour baseline. Guards the opt-in gain knob
    // (pathfinder wires k = profile_k·(total_gain.amplifier − 1)).
    let n = 61u32;
    let cell_m = 10.0;
    let shape = GridShape::new_2d(n, n, 0.0, 0.0, cell_m);
    let mut last_climb = 0.0f32;
    for (idx, k) in [0.0f32, 8.0, 32.0].into_iter().enumerate() {
        let elev = ramp_dem(n, 20.0);
        let metric = ToblerAnisotropic {
            elev,
            refuse_above_deg: 45.0,
            base_pace_s_per_m: 0.714,
            off_trail_factor: 2.3,
            gain_factor_k: k,
        };
        let forms = bake_aniso_corridor(shape, &metric);
        let r = solve_2d_anisotropic(
            shape,
            &forms,
            &[(30u32, 30u32, 0.0)],
            StopCondition::AllAccepted,
        );
        let climb = r.arrival.get(50, 30, 0); // +20 cells uphill (+x)
        eprintln!("gain_factor_k={k}: uphill arrival={climb:.1}");
        assert!(climb.is_finite(), "uphill cell must be reachable");
        if idx > 0 {
            assert!(
                climb > last_climb + 1.0,
                "higher gain_factor_k must raise uphill cost: k={k} climb={climb} prev={last_climb}"
            );
        }
        last_climb = climb;
    }
}
