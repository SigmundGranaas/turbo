//! Regular grid container used by the FMM solver.
//!
//! Stored row-major in a flat `Vec<T>` with shape `(nx, ny, nz)`.
//! The third dimension is 1 for plain 2D Finsler solves (phases 1–4)
//! and `n_theta` for the state-augmented (x, y, θ) elastica solver
//! that lands in phase 5. Allocating in the 3D shape from day one
//! lets the heap + stencil interfaces stay the same across phases.
//!
//! Origin (`origin_x`, `origin_y`) and `cell_m` place the grid in
//! world coordinates (EPSG:25833 metres). Cell `(i, j, k)` covers
//! the bbox `[origin_x + i*cell_m, origin_x + (i+1)*cell_m] ×
//! [origin_y + j*cell_m, origin_y + (j+1)*cell_m]`, sampled at the
//! cell *centre* `(origin_x + (i+0.5)*cell_m, …)`.

use std::ops::{Index, IndexMut};

/// Shape of an FMM grid. Independent of the value type so the same
/// shape can be reused for `FmmGrid<f32>` (arrival times) and
/// `FmmGrid<u32>` (came-from packed parent index).
#[derive(Debug, Clone, Copy)]
pub struct GridShape {
    pub nx: u32,
    pub ny: u32,
    pub nz: u32,
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell_m: f64,
}

impl GridShape {
    pub fn new_2d(nx: u32, ny: u32, origin_x: f64, origin_y: f64, cell_m: f64) -> Self {
        Self {
            nx,
            ny,
            nz: 1,
            origin_x,
            origin_y,
            cell_m,
        }
    }

    pub fn new_3d(nx: u32, ny: u32, nz: u32, origin_x: f64, origin_y: f64, cell_m: f64) -> Self {
        Self {
            nx,
            ny,
            nz,
            origin_x,
            origin_y,
            cell_m,
        }
    }

    /// Total cell count. Allocator and heap sizing use this.
    pub fn len(&self) -> usize {
        (self.nx as usize) * (self.ny as usize) * (self.nz as usize)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Convert `(i, j, k)` into the flat row-major index. No bounds
    /// check — callers are expected to clamp / verify; the hot
    /// stencil path runs inside cell-bound loops where bounds are
    /// already guaranteed.
    #[inline(always)]
    pub fn idx(&self, i: u32, j: u32, k: u32) -> usize {
        // Layout: i is the fastest-moving axis so neighbour-in-x
        // accesses are unit-stride (best for the bucket heap's
        // linear sweep of "next-bucket" candidates in §heap.rs).
        let ny = self.ny as usize;
        let nx = self.nx as usize;
        (k as usize) * ny * nx + (j as usize) * nx + (i as usize)
    }

    /// Reverse map from flat index back to `(i, j, k)`. Used by the
    /// path extractor when it pops a candidate from `came_from`.
    #[inline]
    pub fn unpack(&self, flat: usize) -> (u32, u32, u32) {
        let nx = self.nx as usize;
        let ny = self.ny as usize;
        let i = flat % nx;
        let j = (flat / nx) % ny;
        let k = flat / (nx * ny);
        (i as u32, j as u32, k as u32)
    }

    /// World-coordinate centre of cell `(i, j)`. The k axis (θ) is
    /// non-spatial.
    #[inline]
    pub fn cell_centre(&self, i: u32, j: u32) -> (f64, f64) {
        (
            self.origin_x + (i as f64 + 0.5) * self.cell_m,
            self.origin_y + (j as f64 + 0.5) * self.cell_m,
        )
    }

    /// World coords → integer cell `(i, j)`. Returns `None` when the
    /// point falls outside the grid extent. The k axis isn't a
    /// spatial coordinate so it isn't computed here.
    pub fn world_to_cell(&self, x: f64, y: f64) -> Option<(u32, u32)> {
        let fi = (x - self.origin_x) / self.cell_m;
        let fj = (y - self.origin_y) / self.cell_m;
        if fi < 0.0 || fj < 0.0 {
            return None;
        }
        let i = fi as u32;
        let j = fj as u32;
        if i >= self.nx || j >= self.ny {
            return None;
        }
        Some((i, j))
    }
}

/// Row-major 3D grid of `T`. The third dimension is 1 in the
/// isotropic / Finsler-2D phases and `n_theta` in the elastica
/// phase. Memory layout matches `GridShape::idx`.
#[derive(Debug, Clone)]
pub struct FmmGrid<T: Copy + Default> {
    pub shape: GridShape,
    data: Vec<T>,
}

impl<T: Copy + Default> FmmGrid<T> {
    /// Allocate a grid of `T::default()`.
    pub fn new(shape: GridShape) -> Self {
        Self {
            shape,
            data: vec![T::default(); shape.len()],
        }
    }

    /// Allocate filled with `fill`. Used by the solve loop to start
    /// arrival times at `+∞` (encoded as `f32::INFINITY`).
    pub fn filled(shape: GridShape, fill: T) -> Self {
        Self {
            shape,
            data: vec![fill; shape.len()],
        }
    }

    #[inline(always)]
    pub fn get(&self, i: u32, j: u32, k: u32) -> T {
        self.data[self.shape.idx(i, j, k)]
    }

    #[inline(always)]
    pub fn set(&mut self, i: u32, j: u32, k: u32, v: T) {
        let idx = self.shape.idx(i, j, k);
        self.data[idx] = v;
    }

    #[inline(always)]
    pub fn flat(&self) -> &[T] {
        &self.data
    }

    #[inline(always)]
    pub fn flat_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}

/// Convenience: `grid[(i, j, k)]` syntax for the rare read site
/// outside the hot loop where ergonomics matter more than the
/// compiler's ability to hoist bounds checks.
impl<T: Copy + Default> Index<(u32, u32, u32)> for FmmGrid<T> {
    type Output = T;
    fn index(&self, (i, j, k): (u32, u32, u32)) -> &T {
        &self.data[self.shape.idx(i, j, k)]
    }
}

impl<T: Copy + Default> IndexMut<(u32, u32, u32)> for FmmGrid<T> {
    fn index_mut(&mut self, (i, j, k): (u32, u32, u32)) -> &mut T {
        let idx = self.shape.idx(i, j, k);
        &mut self.data[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_idx_roundtrips() {
        let s = GridShape::new_3d(7, 5, 3, 0.0, 0.0, 10.0);
        for k in 0..s.nz {
            for j in 0..s.ny {
                for i in 0..s.nx {
                    let flat = s.idx(i, j, k);
                    assert_eq!(s.unpack(flat), (i, j, k));
                }
            }
        }
    }

    #[test]
    fn world_to_cell_inside_bbox() {
        let s = GridShape::new_2d(10, 10, 100.0, 200.0, 25.0);
        // (105, 215) lands in cell (0, 0)
        assert_eq!(s.world_to_cell(105.0, 215.0), Some((0, 0)));
        // (130, 240) lands in cell (1, 1)
        assert_eq!(s.world_to_cell(130.0, 240.0), Some((1, 1)));
        // Cell centres round-trip exactly.
        let c = s.cell_centre(3, 4);
        assert_eq!(s.world_to_cell(c.0, c.1), Some((3, 4)));
    }

    #[test]
    fn world_to_cell_outside_returns_none() {
        let s = GridShape::new_2d(10, 10, 0.0, 0.0, 10.0);
        assert!(s.world_to_cell(-1.0, 5.0).is_none());
        assert!(s.world_to_cell(5.0, -1.0).is_none());
        assert!(s.world_to_cell(101.0, 5.0).is_none());
        assert!(s.world_to_cell(5.0, 101.0).is_none());
    }

    #[test]
    fn grid_read_write() {
        let s = GridShape::new_2d(4, 4, 0.0, 0.0, 1.0);
        let mut g: FmmGrid<f32> = FmmGrid::filled(s, f32::INFINITY);
        assert!(g.get(2, 2, 0).is_infinite());
        g.set(2, 2, 0, 5.0);
        assert_eq!(g.get(2, 2, 0), 5.0);
        // Untouched cells stay at +∞.
        assert!(g.get(0, 0, 0).is_infinite());
    }
}
