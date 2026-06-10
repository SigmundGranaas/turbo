//! Slope-angle ("bratthet") overlay tiles from our own DEM — the self-hosted
//! replacement for the NVE steepness overlay the app pulls from
//! `gis3.nve.no` today.
//!
//! Per pixel: slope = atan(|∇z|) from central differences over the same
//! haloed elevation grid the hillshade uses, classified into the avalanche
//! bands ski tourers read (27–30–35–40–45°), rendered as a semi-transparent
//! PNG meant to sit over the basemap. Below 27° is fully transparent, so the
//! overlay only paints where steepness matters.
//!
//! The math is pure (`slope_degrees`, `classify`) so it's unit- and
//! visually-testable on synthetic surfaces without a DEM artifact.

use tiny_skia::Pixmap;
use turbo_tiles_elev::Dem;

use crate::hillshade::sample_grid;
use crate::style::Rgba;

/// Avalanche-angle bands and their overlay colours (straight alpha).
/// Hue semantics follow the NVE/Varsom convention ski tourers already read
/// (yellow → orange → red → purple with rising angle), but the values are
/// **muted and lightness-equalised**, and the alpha is deliberately low
/// (~40–50%) so hillshade and contours stay legible underneath — the
/// overlay should tint the terrain, not paint over it.
pub const BANDS: &[(f32, Rgba)] = &[
    (27.0, Rgba { r: 0xDD, g: 0xC4, b: 0x5E, a: 100 }), // 27–30° straw
    (30.0, Rgba { r: 0xD9, g: 0x9A, b: 0x46, a: 110 }), // 30–35° ochre
    (35.0, Rgba { r: 0xC9, g: 0x60, b: 0x4C, a: 118 }), // 35–40° terracotta
    (40.0, Rgba { r: 0xA8, g: 0x5E, b: 0x96, a: 124 }), // 40–45° plum
    (45.0, Rgba { r: 0x6E, g: 0x4D, b: 0x7E, a: 130 }), // 45°+   muted violet
];

/// Width of the crossfade between adjacent bands, in degrees. Hard band
/// edges between different hues read as psychedelic rings on smooth
/// terrain; a small feather keeps the bands readable while letting the
/// colour glide across the boundary. The fade is centred on each
/// threshold, so the *midpoint* of every boundary still sits exactly at
/// the conventional angle.
const FEATHER_DEG: f32 = 1.5;

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

/// Overlay colour for a slope angle, with a ±`FEATHER_DEG`/2 crossfade
/// around every band threshold (including a fade-in from transparent at
/// 27°). `None` for nodata or well below the first band.
pub fn classify(slope_deg: f32) -> Option<Rgba> {
    if slope_deg.is_nan() {
        return None;
    }
    let half = FEATHER_DEG / 2.0;
    let first = BANDS[0].0;
    if slope_deg < first - half {
        return None;
    }

    // The band whose range contains this angle (by threshold midpoints).
    let idx = BANDS
        .iter()
        .rposition(|(min, _)| slope_deg >= *min)
        // Below the first threshold but inside its fade-in.
        .unwrap_or(0);
    let cur = BANDS[idx].1;

    // Fade-in from transparent across the first threshold.
    if idx == 0 && slope_deg < first + half {
        let t = ((slope_deg - (first - half)) / FEATHER_DEG).clamp(0.0, 1.0);
        return Some(Rgba { a: (cur.a as f32 * t).round() as u8, ..cur });
    }
    // Crossfade with the *previous* band just above its threshold…
    let near_lower = idx > 0 && slope_deg < BANDS[idx].0 + half;
    // …or with the *next* band just below the next threshold.
    let near_upper = idx + 1 < BANDS.len() && slope_deg >= BANDS[idx + 1].0 - half;
    if near_lower {
        let t = ((slope_deg - (BANDS[idx].0 - half)) / FEATHER_DEG).clamp(0.0, 1.0);
        return Some(lerp(BANDS[idx - 1].1, cur, t));
    }
    if near_upper {
        let t = ((slope_deg - (BANDS[idx + 1].0 - half)) / FEATHER_DEG).clamp(0.0, 1.0);
        return Some(lerp(cur, BANDS[idx + 1].1, t));
    }
    Some(cur)
}

fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Rgba { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: m(a.a, b.a) }
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
        if let Some(c) = classify(s) {
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
    fn band_centres_match_the_varsom_convention_hues() {
        assert!(classify(10.0).is_none(), "gentle ground is transparent");
        assert!(classify(26.0).is_none(), "below the fade-in is transparent");
        // Band centres (away from any feather) are the pure band colours.
        assert_eq!(classify(28.5).unwrap(), BANDS[0].1, "27–30 straw");
        assert_eq!(classify(32.5).unwrap(), BANDS[1].1, "30–35 ochre");
        assert_eq!(classify(37.5).unwrap(), BANDS[2].1, "35–40 terracotta");
        assert_eq!(classify(42.5).unwrap(), BANDS[3].1, "40–45 plum");
        assert_eq!(classify(60.0).unwrap(), BANDS[4].1, "45+ violet");
        assert!(classify(f32::NAN).is_none(), "nodata transparent");
    }

    #[test]
    fn band_edges_feather_instead_of_jumping() {
        // The 30° boundary: just below and just above must be close in
        // colour (no hard hue jump), and the exact threshold is the 50/50
        // blend of straw and ochre.
        let below = classify(29.9).unwrap();
        let above = classify(30.1).unwrap();
        assert!(
            (below.r as i16 - above.r as i16).abs() < 12
                && (below.g as i16 - above.g as i16).abs() < 12,
            "boundary must be continuous: {below:?} vs {above:?}"
        );
        let mid = classify(30.0).unwrap();
        let expect_r = (BANDS[0].1.r as f32 + BANDS[1].1.r as f32) / 2.0;
        assert!((mid.r as f32 - expect_r).abs() <= 1.0, "50/50 at the threshold");

        // Fade-in: alpha ramps from 0 below 27° to the full band alpha.
        let lo = classify(26.4).unwrap();
        let hi = classify(27.7).unwrap();
        assert!(lo.a < hi.a && hi.a <= BANDS[0].1.a, "alpha ramps in: {lo:?} {hi:?}");
    }

    #[test]
    fn overlay_alpha_stays_translucent() {
        // The whole point of the rework: the overlay tints, never paints.
        for (deg, c) in BANDS {
            assert!(c.a <= 135, "band at {deg}° too opaque: alpha {}", c.a);
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
