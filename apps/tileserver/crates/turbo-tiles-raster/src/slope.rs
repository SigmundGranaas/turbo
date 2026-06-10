//! Slope-angle ("bratthet") overlay tiles from our own DEM — the self-hosted
//! replacement for the NVE steepness overlay the app pulls from
//! `gis3.nve.no` today.
//!
//! Per pixel: slope = atan(|∇z|) from central differences over the same
//! haloed elevation grid the hillshade uses, mapped through one continuous
//! gradient — transparent on gentle ground, fading in as yellow around the
//! avalanche-relevant angles and sliding smoothly to red on the steepest
//! faces. No discrete bands: a single calm heat ramp that tints the
//! terrain instead of painting zones over it.
//!
//! The math is pure (`slope_degrees`, `color_for_slope`) so it's unit- and
//! visually-testable on synthetic surfaces without a DEM artifact.

use tiny_skia::Pixmap;
use turbo_tiles_elev::Dem;

use crate::hillshade::sample_grid;
use crate::style::Rgba;

/// The continuous slope gradient: piecewise-linear stops as
/// `(degrees, r, g, b, alpha)`, interpolated smoothly between, clamped at
/// the ends. Transparent until ~25°, yellow established by 30°, red from
/// 45° up. Alpha stays ≤ 50% so hillshade and contours read through.
pub const GRADIENT: &[(f32, u8, u8, u8, u8)] = &[
    (25.0, 0xE2, 0xC4, 0x4A, 0),   // fade starts: yellow, fully transparent
    (30.0, 0xE2, 0xC4, 0x4A, 95),  // yellow established
    (38.0, 0xD8, 0x80, 0x3C, 112), // amber midpoint
    (45.0, 0xC4, 0x40, 0x30, 126), // red, full strength
];

/// Overlay colour for a slope angle: the gradient above, `None` for nodata
/// or fully-transparent ground.
pub fn color_for_slope(slope_deg: f32) -> Option<Rgba> {
    if slope_deg.is_nan() || slope_deg <= GRADIENT[0].0 {
        return None;
    }
    let last = GRADIENT[GRADIENT.len() - 1];
    if slope_deg >= last.0 {
        return Some(Rgba { r: last.1, g: last.2, b: last.3, a: last.4 });
    }
    for w in GRADIENT.windows(2) {
        let (d0, r0, g0, b0, a0) = w[0];
        let (d1, r1, g1, b1, a1) = w[1];
        if slope_deg < d1 {
            let t = (slope_deg - d0) / (d1 - d0);
            let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            return Some(Rgba { r: m(r0, r1), g: m(g0, g1), b: m(b0, b1), a: m(a0, a1) });
        }
    }
    unreachable!("gradient covers the range")
}

/// Per-pixel slope in degrees for the interior `size²` of a `(size+2)²`
/// haloed elevation grid (`f32::NAN` cells → `NAN` slope).
pub fn slope_degrees(grid: &[f32], size: u32, px_size_m: f32) -> Vec<f32> {
    let g = (size + 2) as usize;
    let inv2px = 1.0 / (2.0 * px_size_m.max(1e-3));
    let mut out = vec![f32::NAN; (size * size) as usize];
    for j in 0..size as usize {
        for i in 0..size as usize {
            let (gi, gj) = (i + 1, j + 1);
            let l = grid[gj * g + gi - 1];
            let r = grid[gj * g + gi + 1];
            let u = grid[(gj - 1) * g + gi];
            let d = grid[(gj + 1) * g + gi];
            if l.is_nan() || r.is_nan() || u.is_nan() || d.is_nan() {
                continue;
            }
            let dzdx = (r - l) * inv2px;
            let dzdy = (d - u) * inv2px;
            out[j * size as usize + i] = (dzdx * dzdx + dzdy * dzdy).sqrt().atan().to_degrees();
        }
    }
    out
}

/// Render one slope-overlay tile as PNG bytes. `None` when the whole tile is
/// outside DEM coverage (caller serves a transparent/empty tile).
pub fn render_slope_tile(
    dem: &Dem,
    env3857: (f64, f64, f64, f64),
    size: u32,
) -> Option<Result<Vec<u8>, String>> {
    let grid = sample_grid(dem, env3857, size)?;
    let px_m = ((env3857.2 - env3857.0) / size as f64) as f32;
    let slopes = slope_degrees(&grid, size, px_m);

    let mut pm = match Pixmap::new(size, size) {
        Some(p) => p,
        None => return Some(Err("pixmap alloc".into())),
    };
    let px = pm.pixels_mut();
    for (i, &s) in slopes.iter().enumerate() {
        if let Some(c) = color_for_slope(s) {
            // Premultiply straight alpha for tiny-skia's pixel format.
            let mul = |v: u8| ((v as u16 * c.a as u16) / 255) as u8;
            if let Some(p) =
                tiny_skia::PremultipliedColorU8::from_rgba(mul(c.r), mul(c.g), mul(c.b), c.a)
            {
                px[i] = p;
            }
        }
    }
    Some(pm.encode_png().map_err(|e| e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane_grid(size: u32, dz_per_px: f32) -> Vec<f32> {
        let g = (size + 2) as usize;
        let mut v = vec![0.0f32; g * g];
        for j in 0..g {
            for i in 0..g {
                v[j * g + i] = i as f32 * dz_per_px;
            }
        }
        v
    }

    #[test]
    fn flat_is_zero_and_45_degree_plane_measures_45() {
        let flat = plane_grid(8, 0.0);
        assert!(slope_degrees(&flat, 8, 10.0).iter().all(|&s| s.abs() < 1e-4));

        // Rise 10 m per 10 m pixel → 45°.
        let p45 = plane_grid(8, 10.0);
        let s = slope_degrees(&p45, 8, 10.0);
        assert!((s[4 * 8 + 4] - 45.0).abs() < 0.01, "got {}", s[4 * 8 + 4]);
    }

    #[test]
    fn gradient_is_transparent_then_yellow_then_red() {
        assert!(color_for_slope(10.0).is_none(), "gentle ground transparent");
        assert!(color_for_slope(25.0).is_none(), "ramp start is still transparent");
        assert!(color_for_slope(f32::NAN).is_none(), "nodata transparent");

        let yellow = color_for_slope(30.0).unwrap();
        assert!(yellow.r > 0xD0 && yellow.g > 0xB0, "30° is yellow: {yellow:?}");

        let red = color_for_slope(50.0).unwrap();
        assert!(red.r > 0xB0 && red.g < 0x60, "steep is red: {red:?}");
        assert_eq!(
            color_for_slope(45.0).unwrap(),
            color_for_slope(80.0).unwrap(),
            "clamps at the last stop"
        );
    }

    #[test]
    fn gradient_is_continuous_and_monotonic() {
        // No jumps anywhere: walk the ramp in 0.1° steps and bound the
        // per-step colour delta. Alpha must never decrease (steeper is
        // never *less* visible) and green must never increase (the hue
        // only moves yellow → red).
        let mut prev = color_for_slope(25.05).unwrap();
        let mut deg = 25.1;
        while deg < 55.0 {
            let c = color_for_slope(deg).unwrap();
            assert!(
                (c.r as i16 - prev.r as i16).abs() <= 2
                    && (c.g as i16 - prev.g as i16).abs() <= 2
                    && (c.a as i16 - prev.a as i16).abs() <= 2,
                "jump at {deg}°: {prev:?} → {c:?}"
            );
            assert!(c.a >= prev.a, "alpha dipped at {deg}°");
            assert!(c.g <= prev.g, "hue reversed at {deg}°");
            prev = c;
            deg += 0.1;
        }
    }

    #[test]
    fn overlay_alpha_stays_translucent() {
        // The overlay tints, never paints: hard cap on every stop.
        for (deg, _, _, _, a) in GRADIENT {
            assert!(*a <= 130, "stop at {deg}° too opaque: alpha {a}");
        }
    }

    #[test]
    fn nodata_neighbours_stay_transparent() {
        let g = 4 + 2;
        let mut grid = plane_grid(4, 10.0);
        grid[1 * g] = f32::NAN; // left neighbour of pixel (0,0)
        let s = slope_degrees(&grid, 4, 10.0);
        assert!(s[0].is_nan());
        assert!(!s[5].is_nan(), "interior pixels unaffected");
    }
}
