//! Hilbert-curve tile addressing used by PMTiles.
//!
//! PMTiles maps `(z, x, y)` triples to a single 64-bit `tile_id` so that
//! tiles close in space are close in id-space, which makes the directory
//! layout cache-friendly. The mapping is the standard 2D Hilbert curve
//! preceded by an offset that accounts for all coarser-zoom tiles
//! (`(4^z - 1) / 3` tiles at zooms strictly less than `z`).

/// Encode a tile address as the PMTiles 64-bit `tile_id`.
pub fn tile_id(z: u8, x: u64, y: u64) -> u64 {
    if z == 0 {
        return 0;
    }
    // Count of tiles at zooms 0..z-1 = (4^z - 1) / 3. u128 keeps us from
    // overflowing at high z (z=22 ⇒ 4^22 ≈ 1.7e13).
    let base = ((1u128 << (2 * z as u128)) - 1) / 3;

    let n: u64 = 1u64 << z;
    let mut rx;
    let mut ry;
    let mut d: u64 = 0;
    let mut a = x;
    let mut b = y;
    let mut s = n / 2;
    while s > 0 {
        rx = if (a & s) > 0 { 1 } else { 0 };
        ry = if (b & s) > 0 { 1 } else { 0 };
        d += s * s * ((3 * rx) ^ ry);
        // Rotate quadrant appropriately. Note: the canonical Wikipedia
        // formulation passes the *full* grid size `n` here (not the
        // current `s`), and `a`/`b` are the full coordinates.
        if ry == 0 {
            if rx == 1 {
                a = n - 1 - a;
                b = n - 1 - b;
            }
            std::mem::swap(&mut a, &mut b);
        }
        s /= 2;
    }
    base as u64 + d
}

#[cfg(test)]
mod tests {
    //! Value boundary: developers using the PMTiles source trust the
    //! Hilbert encoder. PMTiles v3 fixes a couple of well-known mappings
    //! that any conformant implementation must match.
    use super::*;

    #[test]
    fn root_tile_is_id_zero() {
        // Tile (0, 0, 0) is always id 0.
        assert_eq!(tile_id(0, 0, 0), 0);
    }

    #[test]
    fn zoom_one_tiles_have_ids_one_through_four() {
        // The four zoom-1 tiles take ids 1, 2, 3, 4 in Hilbert order. The
        // exact assignment is part of the PMTiles spec — these are the
        // canonical values.
        let ids: Vec<u64> = (0..2)
            .flat_map(|y| (0..2).map(move |x| tile_id(1, x, y)))
            .collect();
        // Hilbert at z=1 visits (0,0), (0,1), (1,1), (1,0).
        // (x=0,y=0) → 1, (x=1,y=0) → 4, (x=0,y=1) → 2, (x=1,y=1) → 3
        // We iterate y outer, x inner: [
        //   (0,0)=1, (1,0)=4, (0,1)=2, (1,1)=3
        // ]
        assert_eq!(ids, vec![1, 4, 2, 3]);
    }

    #[test]
    fn ids_are_unique_at_zoom_three() {
        let mut seen = std::collections::HashSet::new();
        for y in 0..8 {
            for x in 0..8 {
                let id = tile_id(3, x, y);
                assert!(seen.insert(id), "duplicate id {id} for ({x}, {y})");
            }
        }
        assert_eq!(seen.len(), 64);
    }

    #[test]
    fn z3_offsets_above_z2_count() {
        // All zoom-3 ids must be > all zoom-2 ids. Smallest z3 id ≥ count
        // of tiles at z=0,1,2 = 1 + 4 + 16 = 21.
        let min_z3 = (0..8)
            .flat_map(|y| (0..8).map(move |x| tile_id(3, x, y)))
            .min()
            .unwrap();
        assert_eq!(min_z3, 21);
    }
}
