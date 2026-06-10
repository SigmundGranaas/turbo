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

/// Universal overlay transparency: every visible pixel is exactly 50%,
/// so the steepness reads purely as hue and the basemap shows through
/// uniformly.
pub const ALPHA: u8 = 128;

/// Where the overlay becomes visible (pure yellow)…
const START_DEG: f32 = 25.0;
/// …and where the curve reaches full red (clamped beyond).
const RED_DEG: f32 = 45.0;

const YELLOW: (u8, u8, u8) = (0xE2, 0xC4, 0x4A);
const RED: (u8, u8, u8) = (0xC4, 0x40, 0x30);

/// Overlay colour for a slope angle: transparent through `START_DEG`, then
/// one smooth (smoothstep-eased) curve from yellow to red at `RED_DEG`,
/// holding red beyond. Constant [`ALPHA`]. `None` for nodata/gentle ground.
pub fn color_for_slope(slope_deg: f32) -> Option<Rgba> {
    if slope_deg.is_nan() || slope_deg <= START_DEG {
        return None;
    }
    let t = ((slope_deg - START_DEG) / (RED_DEG - START_DEG)).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t); // smoothstep: eases in and out
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Some(Rgba {
        r: m(YELLOW.0, RED.0),
        g: m(YELLOW.1, RED.1),
        b: m(YELLOW.2, RED.2),
        a: ALPHA,
    })
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
    fn curve_is_transparent_then_yellow_then_red() {
        assert!(color_for_slope(10.0).is_none(), "gentle ground transparent");
        assert!(color_for_slope(25.0).is_none(), "onset angle still transparent");
        assert!(color_for_slope(f32::NAN).is_none(), "nodata transparent");

        let yellow = color_for_slope(25.5).unwrap();
        assert!(yellow.r > 0xD8 && yellow.g > 0xB8, "onset is yellow: {yellow:?}");

        let red = color_for_slope(45.0).unwrap();
        assert!(red.r > 0xB0 && red.g < 0x60, "45° is red: {red:?}");
        assert_eq!(red, color_for_slope(80.0).unwrap(), "holds red beyond 45°");
    }

    #[test]
    fn curve_is_smooth_and_monotonic_with_constant_alpha() {
        // Walk the ramp in 0.1° steps: no colour jumps, hue only ever moves
        // yellow → red (green strictly non-increasing), and EVERY visible
        // pixel carries exactly the universal 50% alpha.
        let mut prev = color_for_slope(25.05).unwrap();
        let mut deg = 25.1;
        while deg < 55.0 {
            let c = color_for_slope(deg).unwrap();
            assert!(
                (c.r as i16 - prev.r as i16).abs() <= 2
                    && (c.g as i16 - prev.g as i16).abs() <= 2,
                "jump at {deg}°: {prev:?} → {c:?}"
            );
            assert!(c.g <= prev.g, "hue reversed at {deg}°");
            assert_eq!(c.a, ALPHA, "alpha must be universal at {deg}°");
            prev = c;
            deg += 0.1;
        }
    }

    #[test]
    fn universal_alpha_is_fifty_percent() {
        assert_eq!(ALPHA, 128, "the overlay tints at exactly 50%");
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
