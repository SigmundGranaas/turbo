//! A deterministic synthetic world, served as real MVT bytes.
//!
//! The world is defined analytically in normalized world space `[0,1]²`,
//! so every tile at every zoom cuts its geometry from the same global
//! truth — exactly like a real basemap, with none of the IO:
//!
//! - **Roads**: an axis-aligned grid at three hierarchy levels (major /
//!   minor / local spacing). Denser levels appear as you zoom in, so a
//!   zoom journey changes geometry density the way a real city does.
//! - **Water**: square lakes scattered on the major lattice.
//! - **Places**: named points at major intersections (label coverage).
//!
//! Geometry extends into a 64/4096 tile buffer (the MVT convention) so
//! strokes join seamlessly across tile boundaries.

use turbomap_mvt::encode::TileEncoder;
use turbomap_mvt::Value;

const EXTENT: i64 = 4096;
const BUFFER: i64 = 64;

/// Road hierarchy: (kind, spacing as a power of two of world units).
/// Levels continue past street scale so that — like a real basemap —
/// zooming in keeps *adding* geometry instead of running out of data.
const LEVELS: &[(&str, i32)] = &[
    ("major", 9),
    ("minor", 12),
    ("local", 14),
    ("lane", 16),
    ("path", 18),
];

/// Encode the synthetic world's tile at `(z, x, y)` as MVT bytes with
/// layers `water`, `roads`, and `places`.
pub fn world_tile(z: u8, x: u32, y: u32) -> Vec<u8> {
    let span = 1.0 / (1u64 << z) as f64; // tile size in world units
    let wx0 = x as f64 * span;
    let wy0 = y as f64 * span;
    // World → tile-local extent units.
    let lx = |w: f64| -> i64 { ((w - wx0) / span * EXTENT as f64).round() as i64 };
    let ly = |w: f64| -> i64 { ((w - wy0) / span * EXTENT as f64).round() as i64 };
    let clamp = |v: i64| -> i32 { v.clamp(-BUFFER, EXTENT + BUFFER) as i32 };

    // Buffered tile rect in world units.
    let buf_w = span * BUFFER as f64 / EXTENT as f64;
    let (bx0, bx1) = (wx0 - buf_w, wx0 + span + buf_w);
    let (by0, by1) = (wy0 - buf_w, wy0 + span + buf_w);

    // Lakes + places live on the minor lattice (one cell per z12 tile),
    // so any screenful at city zooms contains several of each — a real
    // basemap's density, not a sparse statistical accident.
    let feature_spacing = 2f64.powi(-LEVELS[1].1);

    let mut water = TileEncoder::new().layer("water", EXTENT as u32);
    let lake_half = feature_spacing * 0.30;
    // Lakes fill the midpoints of cells where (i+j) is even.
    let i0 = ((bx0 - lake_half) / feature_spacing).floor() as i64;
    let i1 = ((bx1 + lake_half) / feature_spacing).ceil() as i64;
    let j0 = ((by0 - lake_half) / feature_spacing).floor() as i64;
    let j1 = ((by1 + lake_half) / feature_spacing).ceil() as i64;
    for i in i0..=i1 {
        for j in j0..=j1 {
            if (i + j).rem_euclid(2) != 0 {
                continue;
            }
            let cx = (i as f64 + 0.5) * feature_spacing;
            let cy = (j as f64 + 0.5) * feature_spacing;
            // Skip lakes wholly outside the buffered rect.
            if cx + lake_half < bx0 || cx - lake_half > bx1 || cy + lake_half < by0 || cy - lake_half > by1 {
                continue;
            }
            let ring = [
                (clamp(lx(cx - lake_half)), clamp(ly(cy - lake_half))),
                (clamp(lx(cx + lake_half)), clamp(ly(cy - lake_half))),
                (clamp(lx(cx + lake_half)), clamp(ly(cy + lake_half))),
                (clamp(lx(cx - lake_half)), clamp(ly(cy + lake_half))),
            ];
            // Degenerate after clamping (entirely in buffer corner)? Skip.
            if ring[0].0 != ring[1].0 && ring[1].1 != ring[2].1 {
                water = water.polygon(&ring, &[]);
            }
        }
    }
    let tile = water.finish();

    let mut roads = tile.layer("roads", EXTENT as u32);
    // Levels included at this zoom (≤ ~8 lines/axis), coarsest first —
    // the equivalent of a real basemap's per-zoom data selection.
    let included: Vec<(usize, &str, f64)> = LEVELS
        .iter()
        .enumerate()
        .map(|(idx, &(kind, pow))| (idx, kind, 2f64.powi(-pow)))
        .filter(|&(_, _, s)| s >= span / 8.0)
        .collect();
    for &(level_idx, kind, spacing) in &included {
        // The lattices nest (each level subdivides the coarser one), so a
        // line position owned by a coarser included level is *skipped*
        // here: like real data, a road is one feature with one class, not
        // a stack of duplicates that would paint over each other.
        let owned_by_coarser = |index: i64| -> bool {
            included.iter().take_while(|&&(li, ..)| li < level_idx).any(
                |&(_, _, coarser_spacing)| {
                    let ratio = (coarser_spacing / spacing).round() as i64;
                    ratio > 1 && index.rem_euclid(ratio) == 0
                },
            )
        };
        let props = [("kind", Value::String(kind.to_string()))];
        // Vertical lines crossing the buffered rect.
        let i0 = (bx0 / spacing).ceil() as i64;
        let i1 = (bx1 / spacing).floor() as i64;
        for i in i0..=i1 {
            if owned_by_coarser(i) {
                continue;
            }
            let wx = i as f64 * spacing;
            roads = roads.line(
                &[(clamp(lx(wx)), -(BUFFER as i32)), (clamp(lx(wx)), (EXTENT + BUFFER) as i32)],
                &props,
            );
        }
        // Horizontal lines.
        let j0 = (by0 / spacing).ceil() as i64;
        let j1 = (by1 / spacing).floor() as i64;
        for j in j0..=j1 {
            if owned_by_coarser(j) {
                continue;
            }
            let wy = j as f64 * spacing;
            roads = roads.line(
                &[(-(BUFFER as i32), clamp(ly(wy))), ((EXTENT + BUFFER) as i32, clamp(ly(wy)))],
                &props,
            );
        }
    }
    let tile = roads.finish();

    // Places at the lattice intersections of odd cells (off the lakes).
    let mut places = tile.layer("places", EXTENT as u32);
    let i0 = (wx0 / feature_spacing).ceil() as i64;
    let i1 = ((wx0 + span) / feature_spacing).floor() as i64;
    let j0 = (wy0 / feature_spacing).ceil() as i64;
    let j1 = ((wy0 + span) / feature_spacing).floor() as i64;
    for i in i0..=i1 {
        for j in j0..=j1 {
            if (i + j).rem_euclid(2) != 1 {
                continue;
            }
            let px = lx(i as f64 * feature_spacing);
            let py = ly(j as f64 * feature_spacing);
            if (0..EXTENT).contains(&px) && (0..EXTENT).contains(&py) {
                places = places.point(
                    (px as i32, py as i32),
                    &[("name", Value::String(format!("K{i}-{j}")))],
                );
            }
        }
    }
    places.finish().finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbomap_mvt::decode;

    #[test]
    fn world_tiles_decode_and_are_deterministic() {
        let a = world_tile(12, 2120, 1110);
        let b = world_tile(12, 2120, 1110);
        assert_eq!(a, b, "same tile must encode identically");
        let tile = decode(&a).expect("world tile decodes");
        let names: Vec<&str> = tile.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["water", "roads", "places"]);
        let roads = &tile.layers[1];
        assert!(!roads.features.is_empty(), "roads grid expected at z12");
    }

    #[test]
    fn every_city_zoom_has_road_coverage() {
        // The property that matters on screen: no zoom level in the
        // interactive range hits a gap in the hierarchy — there is always
        // a road level dense enough to draw a visible grid.
        for z in 9..=18u8 {
            let t = decode(&world_tile(z, 1 << (z - 2), 1 << (z - 2))).unwrap();
            let roads = t.layers[1].features.len();
            assert!(roads >= 4, "z{z} should have a visible grid, got {roads} roads");
        }
    }

    #[test]
    fn adjacent_tiles_agree_on_shared_roads() {
        // A vertical road on the boundary between tiles (z, x, y) and
        // (z, x+1, y) must appear in both (in the buffer of one, the body
        // of the other) — that's what makes strokes seamless.
        let z = 12u8;
        let left = decode(&world_tile(z, 2119, 1110)).unwrap();
        let right = decode(&world_tile(z, 2120, 1110)).unwrap();
        assert!(!left.layers[1].features.is_empty());
        assert!(!right.layers[1].features.is_empty());
    }
}
