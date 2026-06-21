//! Diagnostic framebuffer probes for the terrain-LOD / horizon work.
//!
//! These turn a rendered frame (RGBA8, row-major, top-left origin) into single
//! numbers that a test can assert — the "real testing" instruments for the
//! scenario harness (real Bodø DEM → readback → probe) and any headless
//! gpu-test. Pure + dependency-free so they're unit-testable on synthetic
//! images (which is how the probes themselves are validated below).
//!
//! Each probe takes an `is_background` predicate (sky / empty-tile clear) so the
//! caller defines what "not ground" means for the scene under test.

/// Rec. 601 luma of an RGBA pixel, 0..=255.
#[inline]
pub fn luma(px: [u8; 4]) -> f32 {
    0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32
}

#[inline]
fn at(rgba: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
    let i = (y * w + x) * 4;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

/// Fraction (0..=1) of the **lower `band` rows** of the frame that are ground
/// (i.e. NOT `is_background`). The lower band is where a tilted view's near
/// terrain must be — a sky-sliver bug ("pan-down clips everything") drives this
/// toward 0. `band` is a fraction of frame height (e.g. 0.5 = bottom half).
pub fn ground_coverage(
    rgba: &[u8],
    w: usize,
    h: usize,
    band: f64,
    is_background: impl Fn([u8; 4]) -> bool,
) -> f64 {
    if w == 0 || h == 0 {
        return 0.0;
    }
    let y0 = ((1.0 - band.clamp(0.0, 1.0)) * h as f64) as usize;
    let mut ground = 0usize;
    let mut total = 0usize;
    for y in y0..h {
        for x in 0..w {
            total += 1;
            if !is_background(at(rgba, w, x, y)) {
                ground += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        ground as f64 / total as f64
    }
}

/// Largest abs luma jump between two vertically-adjacent rows, averaged across
/// columns — measured over the GROUND band only (rows scanned bottom→up until
/// background dominates). A smooth terrain→haze→sky transition keeps this small;
/// a hard cutaway line (terrain abruptly meeting sky) spikes it. Returns the max
/// per-row mean delta (0..=255).
pub fn max_row_luma_step(
    rgba: &[u8],
    w: usize,
    h: usize,
    is_background: impl Fn([u8; 4]) -> bool,
) -> f32 {
    if w == 0 || h < 2 {
        return 0.0;
    }
    let mut worst = 0.0f32;
    // Bottom-up; stop once a row is mostly background (we're into open sky).
    for y in (1..h).rev() {
        let mut sum_delta = 0.0f32;
        let mut bg = 0usize;
        for x in 0..w {
            let a = at(rgba, w, x, y);
            let b = at(rgba, w, x, y - 1);
            if is_background(a) {
                bg += 1;
            }
            sum_delta += (luma(a) - luma(b)).abs();
        }
        if bg > w * 3 / 4 {
            break;
        }
        worst = worst.max(sum_delta / w as f32);
    }
    worst
}

/// Count of background ("sky") pixels that are enclosed by ground on all four
/// sides within `reach` pixels — i.e. holes in the terrain, which is exactly
/// what a mixed-LOD T-junction crack looks like. Open sky above the horizon is
/// NOT counted (it isn't bounded by ground above). `> 0` means cracks.
pub fn sky_holes(
    rgba: &[u8],
    w: usize,
    h: usize,
    reach: usize,
    is_background: impl Fn([u8; 4]) -> bool,
) -> usize {
    if w == 0 || h == 0 || reach == 0 {
        return 0;
    }
    let ground_within = |x: usize, y: usize, dx: isize, dy: isize| -> bool {
        for step in 1..=reach as isize {
            let nx = x as isize + dx * step;
            let ny = y as isize + dy * step;
            if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                return false;
            }
            if !is_background(at(rgba, w, nx as usize, ny as usize)) {
                return true;
            }
        }
        false
    };
    let mut holes = 0usize;
    for y in 0..h {
        for x in 0..w {
            if !is_background(at(rgba, w, x, y)) {
                continue;
            }
            if ground_within(x, y, -1, 0)
                && ground_within(x, y, 1, 0)
                && ground_within(x, y, 0, -1)
                && ground_within(x, y, 0, 1)
            {
                holes += 1;
            }
        }
    }
    holes
}

#[cfg(test)]
mod tests {
    //! TDD the instruments: each probe is checked against a hand-built image
    //! with a known property, so the harness can trust the numbers it reports.

    use super::*;

    const W: usize = 16;
    const H: usize = 16;
    const SKY: [u8; 4] = [120, 160, 220, 255];
    const GROUND: [u8; 4] = [80, 120, 60, 255];

    fn is_sky(px: [u8; 4]) -> bool {
        // Bluish + brighter than ground.
        px[2] > px[1] && px[2] > 150
    }

    fn filled(top_sky_rows: usize) -> Vec<u8> {
        let mut img = vec![0u8; W * H * 4];
        for y in 0..H {
            for x in 0..W {
                let px = if y < top_sky_rows { SKY } else { GROUND };
                let i = (y * W + x) * 4;
                img[i..i + 4].copy_from_slice(&px);
            }
        }
        img
    }

    #[test]
    fn coverage_is_full_for_ground_and_zero_for_sky() {
        let all_ground = filled(0);
        assert!(ground_coverage(&all_ground, W, H, 0.5, is_sky) > 0.99);
        let all_sky = filled(H);
        assert!(ground_coverage(&all_sky, W, H, 0.5, is_sky) < 0.01);
    }

    #[test]
    fn coverage_catches_a_sky_sliver_lower_band() {
        // Sky covers all but the bottom 2 rows → lower-half coverage is low.
        let sliver = filled(H - 2);
        let cov = ground_coverage(&sliver, W, H, 0.5, is_sky);
        assert!(cov < 0.3, "sky-sliver must read low coverage (got {cov})");
    }

    #[test]
    fn row_step_spikes_on_a_hard_horizon_line_and_is_low_on_a_gradient() {
        // Hard line: ground then sky in one row jump → big step.
        let hard = filled(H / 2);
        let hard_step = max_row_luma_step(&hard, W, H, is_sky);
        // Smooth vertical gradient over the ground band → small per-row step.
        let mut grad = vec![0u8; W * H * 4];
        for y in 0..H {
            let v = (y * 255 / H) as u8;
            for x in 0..W {
                let i = (y * W + x) * 4;
                grad[i..i + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let grad_step = max_row_luma_step(&grad, W, H, |_| false);
        assert!(hard_step > grad_step * 3.0, "hard line ({hard_step}) ≫ gradient ({grad_step})");
    }

    #[test]
    fn sky_holes_detects_an_enclosed_gap_and_ignores_open_sky() {
        // All ground with a single enclosed sky pixel → exactly one hole.
        let mut img = filled(0);
        let (hx, hy) = (8, 8);
        let i = (hy * W + hx) * 4;
        img[i..i + 4].copy_from_slice(&SKY);
        assert_eq!(sky_holes(&img, W, H, 3, is_sky), 1, "enclosed gap = crack");

        // Open sky on top (not bounded above) → no holes.
        let open = filled(H / 2);
        assert_eq!(sky_holes(&open, W, H, 3, is_sky), 0, "open sky is not a hole");
    }
}
