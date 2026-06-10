//! Server-side hillshade for the raster fallback.
//!
//! Samples the DEM over a tile (+1 px halo for edge gradients), computes a
//! Lambertian relief intensity per pixel, and composites it as a luminance
//! multiply onto the already-drawn fills. The MapLibre GL path gets the same
//! effect for free from a native `hillshade` layer over our Terrain-RGB
//! tiles; this brings the `flutter_map`/tiny-skia path to parity.
//!
//! Intensity math is pure (`intensity`) so it can be unit- and
//! visually-tested on synthetic surfaces without a DEM artifact present.

use std::f32::consts::PI;

use tiny_skia::Pixmap;
use turbo_tiles_elev::{wgs84_to_utm33n, Dem, PointXY};

const WORLD_M: f64 = 20_037_508.342_789_244;

/// Light + relief parameters. Defaults match the basemap style's MapLibre
/// hillshade layer (NW sun, low angle, gentle strength).
#[derive(Debug, Clone, Copy)]
pub struct HillshadeParams {
    /// Sun azimuth, compass degrees (0 = N, clockwise). 315 = NW (standard).
    pub sun_azimuth_deg: f32,
    /// Sun altitude above the horizon, degrees.
    pub sun_altitude_deg: f32,
    /// Vertical exaggeration of the terrain normal.
    pub exaggeration: f32,
    /// Composite strength: 0 = no effect, 1 = full ±luminance swing.
    pub strength: f32,
}

impl Default for HillshadeParams {
    fn default() -> Self {
        Self {
            sun_azimuth_deg: 315.0,
            sun_altitude_deg: 45.0,
            exaggeration: 1.4,
            strength: 0.55,
        }
    }
}

/// Neutral intensity (flat ground / nodata) — composites to no change.
const NEUTRAL: f32 = 0.5;

/// Sample the DEM into a `(size+2)²` elevation grid (row-major, 1 px halo on
/// every side), `f32::NAN` for out-of-coverage cells. Returns `None` when the
/// whole tile is outside coverage, so the caller can skip compositing.
pub fn sample_grid(dem: &Dem, env3857: (f64, f64, f64, f64), size: u32) -> Option<Vec<f32>> {
    let (xmin, _ymin, xmax, ymax) = env3857;
    let span = xmax - xmin; // square tile
    let px = span / size as f64;
    let g = (size + 2) as usize;
    let mut grid = vec![f32::NAN; g * g];
    let mut any = false;
    for j in 0..g {
        // pixel-centre 3857 coords, shifted by one px for the halo
        let y = ymax - (j as f64 - 0.5) * px;
        for i in 0..g {
            let x = xmin + (i as f64 - 0.5) * px;
            let (lng, lat) = inverse_mercator(x, y);
            let utm = wgs84_to_utm33n(lng, lat);
            if let Ok(Some(e)) = dem.sample(PointXY { x: utm.x, y: utm.y }) {
                grid[j * g + i] = e;
                any = true;
            }
        }
    }
    any.then_some(grid)
}

/// Per-pixel relief intensity in `[0,1]` (0 = deep shadow, 0.5 = flat, 1 =
/// full sun) for the interior `size²` from a `(size+2)²` elevation grid.
/// `px_size_m` is the ground spacing between samples.
pub fn intensity(grid: &[f32], size: u32, px_size_m: f32, p: &HillshadeParams) -> Vec<f32> {
    let g = (size + 2) as usize;
    // Light direction toward the sun, in (east, south, up).
    let az = p.sun_azimuth_deg.to_radians();
    let alt = p.sun_altitude_deg.to_radians();
    let (sx, sy, sz) = (
        alt.cos() * az.sin(),  // east
        alt.cos() * -az.cos(), // south (north is -y)
        alt.sin(),             // up
    );
    let inv2px = 1.0 / (2.0 * px_size_m.max(1e-3));

    let mut out = vec![NEUTRAL; (size * size) as usize];
    for j in 0..size as usize {
        for i in 0..size as usize {
            let (gi, gj) = (i + 1, j + 1); // interior cell in the haloed grid
            let l = grid[gj * g + gi - 1];
            let r = grid[gj * g + gi + 1];
            let u = grid[(gj - 1) * g + gi];
            let d = grid[(gj + 1) * g + gi];
            if l.is_nan() || r.is_nan() || u.is_nan() || d.is_nan() {
                continue; // leave neutral at coverage edges / nodata
            }
            let dzdx = (r - l) * inv2px;
            let dzdy = (d - u) * inv2px; // +y is south
            // Up-facing surface normal, exaggerated.
            let nx = -dzdx * p.exaggeration;
            let ny = -dzdy * p.exaggeration;
            let nz = 1.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            let dot = (nx * sx + ny * sy + nz * sz) / len;
            // Lambert, remapped so flat ground (dot≈sin(alt)) lands near 0.5.
            let flat = alt.sin();
            let shade = NEUTRAL + 0.5 * (dot - flat) / (1.0 - flat).max(1e-3);
            out[j * size as usize + i] = shade.clamp(0.0, 1.0);
        }
    }
    out
}

/// Multiply the hillshade luminance onto every pixel of `pixmap` (assumed
/// `size²`). Flat ground (intensity 0.5) is unchanged; sun brightens, shadow
/// darkens, scaled by `strength`.
pub fn composite(pixmap: &mut Pixmap, shade: &[f32], strength: f32) {
    let pixels = pixmap.pixels_mut();
    for (px, &s) in pixels.iter_mut().zip(shade.iter()) {
        // mul: 0.5→1.0, 0→(1-strength), 1→(1+strength)
        let mul = 1.0 + strength * (2.0 * s - 1.0);
        // Pixels are premultiplied; scaling RGB by the same factor as A would
        // cancel, so scale the (un-premultiplied-equivalent) colour by mul and
        // keep alpha. For opaque basemap fills alpha=255, so this is a plain
        // RGB multiply.
        let a = px.alpha();
        let scale = |c: u8| ((c as f32 * mul).round().clamp(0.0, a as f32)) as u8;
        *px = tiny_skia::PremultipliedColorU8::from_rgba(
            scale(px.red()),
            scale(px.green()),
            scale(px.blue()),
            a,
        )
        .unwrap_or(*px);
    }
}

fn inverse_mercator(x: f64, y: f64) -> (f64, f64) {
    let lng = x / WORLD_M * 180.0;
    let lat = (2.0 * (y / WORLD_M * std::f64::consts::PI).exp().atan()
        - std::f64::consts::PI / 2.0)
        .to_degrees();
    (lng, lat)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_from<F: Fn(usize, usize) -> f32>(size: u32, f: F) -> Vec<f32> {
        let g = (size + 2) as usize;
        let mut v = vec![0.0f32; g * g];
        for j in 0..g {
            for i in 0..g {
                v[j * g + i] = f(i, j);
            }
        }
        v
    }

    #[test]
    fn flat_ground_is_neutral() {
        let grid = grid_from(8, |_, _| 100.0);
        let it = intensity(&grid, 8, 10.0, &HillshadeParams::default());
        for v in it {
            assert!((v - 0.5).abs() < 1e-4, "flat must be ~0.5, got {v}");
        }
    }

    #[test]
    fn nw_light_brightens_nw_facing_slope() {
        // A plane rising toward the SE (z grows with +x east and +y south).
        // Its surface faces NW, so a NW (315°) sun should light it: >0.5.
        let grid = grid_from(8, |i, j| (i as f32 + j as f32) * 5.0);
        let it = intensity(&grid, 8, 10.0, &HillshadeParams::default());
        let mid = it[4 * 8 + 4];
        assert!(mid > 0.55, "NW-facing slope under NW sun should be lit: {mid}");

        // The opposite plane (rising NW, facing SE) should be in shadow.
        let grid2 = grid_from(8, |i, j| ((14 - i) as f32 + (14 - j) as f32) * 5.0);
        let it2 = intensity(&grid2, 8, 10.0, &HillshadeParams::default());
        assert!(it2[4 * 8 + 4] < 0.45, "SE-facing slope should be shadowed");
    }

    #[test]
    fn nodata_neighbours_stay_neutral() {
        let g = 4 + 2; // haloed width
        let mut grid = grid_from(4, |i, j| (i + j) as f32);
        // Left neighbour of interior pixel (0,0) is haloed cell (0,1).
        grid[1 * g] = f32::NAN;
        let it = intensity(&grid, 4, 10.0, &HillshadeParams::default());
        assert!((it[0] - 0.5).abs() < 1e-6, "nodata neighbour → neutral");
    }

    #[test]
    fn composite_multiplies_flat_to_no_change_and_shadow_darkens() {
        let mut pm = Pixmap::new(2, 1).unwrap();
        for px in pm.pixels_mut() {
            *px = tiny_skia::PremultipliedColorU8::from_rgba(200, 100, 50, 255).unwrap();
        }
        composite(&mut pm, &[0.5, 0.0], 0.5);
        let p0 = pm.pixels()[0];
        assert_eq!((p0.red(), p0.green(), p0.blue()), (200, 100, 50), "flat unchanged");
        let p1 = pm.pixels()[1];
        assert!(p1.red() < 200 && p1.green() < 100, "shadow darkens");
    }
}
