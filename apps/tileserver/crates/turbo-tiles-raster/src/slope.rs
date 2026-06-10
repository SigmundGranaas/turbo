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
/// Palette follows the NVE/Varsom convention so users keep their mental
/// model: yellow → orange → red → purple → near-black with rising angle.
pub const BANDS: &[(f32, Rgba)] = &[
    (27.0, Rgba { r: 0xF5, g: 0xE0, b: 0x39, a: 110 }), // 27–30°
    (30.0, Rgba { r: 0xF0, g: 0xA0, b: 0x30, a: 120 }), // 30–35°
    (35.0, Rgba { r: 0xE3, g: 0x1A, b: 0x1C, a: 130 }), // 35–40°
    (40.0, Rgba { r: 0x9E, g: 0x1F, b: 0x63, a: 140 }), // 40–45°
    (45.0, Rgba { r: 0x3F, g: 0x1F, b: 0x4E, a: 150 }), // 45°+
];

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

/// Overlay colour for a slope angle; `None` (transparent) below the first
/// band or for nodata.
pub fn classify(slope_deg: f32) -> Option<Rgba> {
    if slope_deg.is_nan() {
        return None;
    }
    let mut hit = None;
    for (min_deg, color) in BANDS {
        if slope_deg >= *min_deg {
            hit = Some(*color);
        }
    }
    hit
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
    fn classification_bands_match_the_varsom_convention() {
        assert!(classify(10.0).is_none(), "gentle ground is transparent");
        assert!(classify(26.9).is_none());
        assert_eq!(classify(28.0).unwrap().r, 0xF5, "27–30 yellow");
        assert_eq!(classify(33.0).unwrap().r, 0xF0, "30–35 orange");
        assert_eq!(classify(37.0).unwrap().r, 0xE3, "35–40 red");
        assert_eq!(classify(42.0).unwrap().r, 0x9E, "40–45 purple");
        assert_eq!(classify(60.0).unwrap().r, 0x3F, "45+ dark");
        assert!(classify(f32::NAN).is_none(), "nodata transparent");
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
