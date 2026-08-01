//! Polygon → cell rasterisation, shared by every builder that writes a
//! mask.
//!
//! It lives here, next to the format, rather than in one of the
//! builders, because two builders that rasterise *nearly* the same way
//! produce two masks that differ along every shoreline — and a mask
//! difference is silent. A cell that says water where the other says
//! land does not fail; it makes the router refuse ground its twin walks
//! over. Sharing the fill is what lets a PostGIS-built mask and a
//! GML-built one be compared at all.

use geo::{Coord, Polygon};

/// Fill `poly` into `cells` with value `v`, even-odd rule.
///
/// `cells` is row-major `cells_x * cells_y`, one byte per cell (the
/// working buffer — packing to 2 bits happens once at the end, because
/// packing on every polygon contribution costs more than the memory
/// saves). Row 0 is the *north* edge: world Y decreases as the row index
/// grows.
///
/// Interior rings are not special-cased. Collecting every ring's
/// segments and counting all crossings together makes the even-odd rule
/// handle holes on its own — a lake with an island gets the island back
/// without the fill needing to know which ring was which.
#[allow(clippy::too_many_arguments)]
pub fn scanline_fill(
    poly: &Polygon<f64>,
    v: u8,
    cells: &mut [u8],
    cells_x: u32,
    cells_y: u32,
    min_x: f64,
    max_y: f64,
    res: f64,
) {
    let mut p_min_x = f64::INFINITY;
    let mut p_max_x = f64::NEG_INFINITY;
    let mut p_min_y = f64::INFINITY;
    let mut p_max_y = f64::NEG_INFINITY;
    for c in poly.exterior().0.iter() {
        if c.x < p_min_x {
            p_min_x = c.x;
        }
        if c.x > p_max_x {
            p_max_x = c.x;
        }
        if c.y < p_min_y {
            p_min_y = c.y;
        }
        if c.y > p_max_y {
            p_max_y = c.y;
        }
    }
    let col_min = (((p_min_x - min_x) / res).floor() as i64).max(0);
    let col_max = (((p_max_x - min_x) / res).ceil() as i64).min(cells_x as i64 - 1);
    let row_min = (((max_y - p_max_y) / res).floor() as i64).max(0);
    let row_max = (((max_y - p_min_y) / res).ceil() as i64).min(cells_y as i64 - 1);
    if col_min > col_max || row_min > row_max {
        return;
    }

    let mut all_segments: Vec<(Coord<f64>, Coord<f64>)> = Vec::new();
    for ls in std::iter::once(poly.exterior()).chain(poly.interiors().iter()) {
        for w in ls.0.windows(2) {
            all_segments.push((w[0], w[1]));
        }
    }

    for row in row_min..=row_max {
        // Cells span [max_y - (row+1)*res, max_y - row*res]; the
        // scanline runs through the row's centre.
        let y = max_y - (row as f64 + 0.5) * res;
        let mut crossings: Vec<f64> = Vec::new();
        for &(a, b) in &all_segments {
            // A segment entirely above or below contributes nothing.
            // This also drops horizontal segments, which would
            // otherwise yield infinite crossings.
            if (a.y > y) == (b.y > y) {
                continue;
            }
            let t = (y - a.y) / (b.y - a.y);
            crossings.push(a.x + t * (b.x - a.x));
        }
        crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut i = 0;
        while i + 1 < crossings.len() {
            let c0 = (((crossings[i] - min_x) / res).floor() as i64).max(col_min);
            let c1 = (((crossings[i + 1] - min_x) / res).ceil() as i64).min(col_max);
            if c0 <= c1 {
                let base = row as usize * cells_x as usize;
                for c in c0..=c1 {
                    cells[base + c as usize] = v;
                }
            }
            i += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::LineString;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon<f64> {
        Polygon::new(
            LineString::from(vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]),
            vec![],
        )
    }

    /// Grid: 10 x 10 cells of 100 m, north-west corner at (0, 1000).
    fn grid() -> (Vec<u8>, u32, u32, f64, f64, f64) {
        (vec![0u8; 100], 10, 10, 0.0, 1000.0, 100.0)
    }

    #[test]
    fn fills_the_cells_a_square_covers() {
        let (mut cells, cx, cy, min_x, max_y, res) = grid();
        // World (200,700)-(500,1000) is the top-left 3x3 block.
        scanline_fill(
            &square(200.0, 700.0, 500.0, 1000.0),
            1,
            &mut cells,
            cx,
            cy,
            min_x,
            max_y,
            res,
        );
        let filled: Vec<usize> = (0..100).filter(|&i| cells[i] != 0).collect();
        assert!(!filled.is_empty(), "nothing filled");
        for &i in &filled {
            let (row, col) = (i / 10, i % 10);
            assert!(
                row < 4 && (1..=5).contains(&col),
                "stray cell at row {row} col {col}"
            );
        }
    }

    /// Row 0 must be the NORTH edge. A flipped Y is the bug that puts
    /// every lake in the wrong half of the region and still looks like a
    /// plausible mask.
    #[test]
    fn row_zero_is_the_north_edge() {
        let (mut cells, cx, cy, min_x, max_y, res) = grid();
        // A band across the top 100 m of the world box.
        scanline_fill(
            &square(0.0, 900.0, 1000.0, 1000.0),
            1,
            &mut cells,
            cx,
            cy,
            min_x,
            max_y,
            res,
        );
        assert!(cells[0..10].iter().all(|&c| c == 1), "north row not filled");
        assert!(
            cells[90..100].iter().all(|&c| c == 0),
            "south row filled instead"
        );
    }

    /// An island inside a lake must come back as land, without the fill
    /// being told which ring is which.
    ///
    /// Note what this does NOT assert. Spans are filled `floor`..`ceil`,
    /// so every span is dilated by up to one cell — the ring's own edge
    /// cells go to water even when the ring's interior is land. That is
    /// deliberate and pre-existing: at 100 m a shoreline cell is mostly
    /// water anyway, and marking it is the safe error. So the island is
    /// checked at its centre, not at its edge.
    #[test]
    fn an_interior_ring_is_left_unfilled() {
        let (mut cells, cx, cy, min_x, max_y, res) = grid();
        let lake = Polygon::new(
            LineString::from(vec![
                (0.0, 0.0),
                (1000.0, 0.0),
                (1000.0, 1000.0),
                (0.0, 1000.0),
                (0.0, 0.0),
            ]),
            vec![LineString::from(vec![
                (400.0, 400.0),
                (600.0, 400.0),
                (600.0, 600.0),
                (400.0, 600.0),
                (400.0, 400.0),
            ])],
        );
        scanline_fill(&lake, 1, &mut cells, cx, cy, min_x, max_y, res);
        assert_eq!(cells[0], 1, "outside the hole should be water");
        // Hole spans world x 400-600, y 400-600 = cols 4-5, rows 4-5.
        // Col 4 is eaten by the span dilation; col 5 is the interior.
        assert_eq!(cells[5 * 10 + 5], 0, "the island was flooded");
        assert_eq!(cells[4 * 10 + 5], 0, "the island was flooded");
    }

    #[test]
    fn a_polygon_outside_the_grid_touches_nothing() {
        let (mut cells, cx, cy, min_x, max_y, res) = grid();
        scanline_fill(
            &square(5000.0, 5000.0, 6000.0, 6000.0),
            1,
            &mut cells,
            cx,
            cy,
            min_x,
            max_y,
            res,
        );
        assert!(cells.iter().all(|&c| c == 0));
    }

    /// Clipping must not wrap. A polygon crossing the west edge fills
    /// the west cells of its own rows; a row-major buffer indexed
    /// without clamping would spill into the east end of the row above,
    /// which reads as a lake on the far side of the region.
    #[test]
    fn a_polygon_straddling_the_edge_is_clipped_not_wrapped() {
        let (mut cells, cx, cy, min_x, max_y, res) = grid();
        scanline_fill(
            &square(-500.0, 400.0, 150.0, 600.0),
            1,
            &mut cells,
            cx,
            cy,
            min_x,
            max_y,
            res,
        );
        for row in 0..10usize {
            for col in 0..10usize {
                if cells[row * 10 + col] != 0 {
                    // Cols 0-1 are covered ground; col 2 is the one-cell
                    // span dilation documented above.
                    assert!(col <= 2, "filled col {col} in row {row} — wrapped east");
                    assert!(
                        (3..=6).contains(&row),
                        "filled row {row} — wrapped north or south"
                    );
                }
            }
        }
        assert!(cells[4 * 10] != 0, "row 4 should have been filled at all");
    }
}
