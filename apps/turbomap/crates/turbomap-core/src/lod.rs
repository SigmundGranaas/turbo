//! Screen-space-error (SSE) quadtree tile selection for tilted 3D terrain.
//!
//! Today the engine requests a SINGLE zoom across the whole frustum
//! (`scene::tiles_for_margin_at`), so a tilted view toward the horizon explodes
//! into thousands of fine tiles → capped → far field dropped (the hard cutaway)
//! and, at steep pitch, the trim starves the on-screen set (clips everything).
//!
//! A real terrain renderer instead refines a quadtree by *projected screen-space
//! size*: subdivide a tile only while it would cover more than `sse_target_px`
//! on screen. Near the camera → deep subdivision (fine tiles); toward the
//! horizon → shallow (coarse tiles). This bounds the tile COUNT at any pitch and
//! covers all the way to the footprint edge, emitting a MIXED-ZOOM set the
//! Stage-1 best-available resolver already draws.
//!
//! Phase 0 lands the seam (type + signature + the contract as `#[ignore]`d
//! tests); Phase 1 implements the refinement and un-ignores them (TDD red→green).

use crate::camera::Camera;
use crate::geo::WorldPoint;
use crate::tile::TileId;

/// Hard backstop on the emitted set — SSE termination already bounds the count,
/// this just guarantees it can never run away (e.g. a degenerate camera).
const MAX_TILES: usize = 384;

/// One tile chosen by the LOD walk. A thin wrapper now (just the id the resolver
/// needs); Phase 1+ may carry its computed screen-space error / neighbour LODs
/// for skirt sizing without changing call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LodTile {
    pub id: TileId,
}

/// Select the mixed-zoom tile set covering the camera's ground footprint, each
/// tile refined until its projected on-screen span is ≤ `sse_target_px`, clamped
/// to `[min_zoom, max_zoom]`. Deterministic; bounded in count; collapses to the
/// legacy single-level rectangle at pitch 0 (so the 2D path is unchanged).
///
/// Phase 0: unimplemented — the seam exists so Phase 1's tests compile + start
/// red. Not called on the render path yet.
pub fn select(
    camera: &Camera,
    viewport_px: (f64, f64),
    min_zoom: u8,
    max_zoom: u8,
    sse_target_px: f64,
) -> Vec<LodTile> {
    let min_zoom = min_zoom.min(max_zoom);
    let target = sse_target_px.max(1.0);
    let mut out = Vec::new();
    for root in footprint_roots(camera, viewport_px, min_zoom) {
        refine(root, camera, viewport_px, max_zoom, target, &mut out);
        if out.len() >= MAX_TILES {
            break;
        }
    }
    out
}

/// The `min_zoom` tiles overlapping the camera's ground footprint — the AABB of
/// the four unprojected viewport corners, clamped to the world. Bounded (a
/// handful at a coarse zoom); refinement deepens it where the screen demands.
fn footprint_roots(camera: &Camera, vp: (f64, f64), z: u8) -> Vec<TileId> {
    let n = 1u32 << z;
    let (vw, vh) = vp;
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (px, py) in [(0.0, 0.0), (vw, 0.0), (vw, vh), (0.0, vh)] {
        let w = camera.pixel_to_world((px, py), vp);
        let wx = w.x.clamp(0.0, 1.0);
        let wy = w.y.clamp(0.0, 1.0);
        min_x = min_x.min(wx);
        max_x = max_x.max(wx);
        min_y = min_y.min(wy);
        max_y = max_y.max(wy);
    }
    let x0 = (min_x * n as f64).floor().clamp(0.0, (n - 1) as f64) as u32;
    let x1 = (max_x * n as f64).ceil().clamp(1.0, n as f64) as u32 - 1;
    let y0 = (min_y * n as f64).floor().clamp(0.0, (n - 1) as f64) as u32;
    let y1 = (max_y * n as f64).ceil().clamp(1.0, n as f64) as u32 - 1;
    let mut roots = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            roots.push(TileId::new(z, x, y));
        }
    }
    roots
}

/// Recursively subdivide while the tile's on-screen size exceeds `target`,
/// stopping at `max_zoom`. Frustum-culled tiles are dropped; tiles straddling
/// the camera (some corners behind the view plane) are always refined (they're
/// the near ground filling the lower frame).
fn refine(tile: TileId, camera: &Camera, vp: (f64, f64), max_zoom: u8, target: f64, out: &mut Vec<LodTile>) {
    if out.len() >= MAX_TILES {
        return;
    }
    // A tile whose ground rect contains the camera's look-at point is under/around
    // the camera — its own corners may all project behind the view (esp. a huge
    // coarse root), but it must never be culled; it's refined down to the branch
    // that's actually on screen.
    let contains_centre = {
        let c = camera.center.to_world();
        let (nw, se) = tile.world_bounds();
        c.x >= nw.x && c.x <= se.x && c.y >= nw.y && c.y <= se.y
    };
    let (front, behind) = project_corners(tile, camera, vp);
    // Cull: entirely behind the camera (nothing on screen) AND not under us.
    if front.is_empty() && behind > 0 && !contains_centre {
        return;
    }
    // Cull: fully in front but off-screen (outside the viewport + a margin).
    if behind == 0 && !contains_centre && !screen_aabb_intersects_viewport(&front, vp) {
        return;
    }
    // Screen-space size for the refine decision:
    //  • all corners in front → max EDGE length (so pitch 0 collapses to a single
    //    level == camera zoom: a z-tile is exactly 256 px/edge there);
    //  • some corners behind (a tile CROSSING the horizon) → measure from the
    //    visible near corners only (max pairwise px). For a far horizon-crosser
    //    that's its small near edge → it stays COARSE (the key fine-near/coarse-
    //    far behaviour). Forcing ∞ here was the bug that drove the whole horizon
    //    to max zoom (a single LOD);
    //  • <2 visible corners → the tile is under/around the camera → refine (∞).
    let span = if behind == 0 {
        max_edge_px(&front)
    } else if front.len() >= 2 {
        max_pairwise_px(&front)
    } else {
        f64::INFINITY
    };
    if tile.z >= max_zoom || span <= target {
        out.push(LodTile { id: tile });
        return;
    }
    match tile.children() {
        Some(children) => {
            for c in children {
                refine(c, camera, vp, max_zoom, target, out);
            }
        }
        None => out.push(LodTile { id: tile }),
    }
}

/// The four ground corners (nw, ne, se, sw) of a tile.
fn tile_corners(tile: TileId) -> [WorldPoint; 4] {
    let (nw, se) = tile.world_bounds();
    [
        nw,
        WorldPoint::new(se.x, nw.y),
        se,
        WorldPoint::new(nw.x, se.y),
    ]
}

/// Project a tile's corners to screen; returns the in-front screen points (in
/// corner order, skipping any behind the view plane) and a count of behind ones.
fn project_corners(tile: TileId, camera: &Camera, vp: (f64, f64)) -> (Vec<(f64, f64)>, usize) {
    let mut front = Vec::with_capacity(4);
    let mut behind = 0;
    for c in tile_corners(tile) {
        match camera.world_to_screen(c, vp) {
            Some(p) => front.push(p),
            None => behind += 1,
        }
    }
    (front, behind)
}

/// Max screen-space EDGE length (not diagonal) over consecutive corners. Edge
/// length is the right screen-space-error metric: at pitch 0 a tile of zoom `z`
/// is exactly 256 px per edge when the camera zoom is `z`, so the SSE threshold
/// collapses to a single level (== camera zoom) — the 2D path is unchanged.
fn max_edge_px(corners: &[(f64, f64)]) -> f64 {
    if corners.len() < 2 {
        return f64::INFINITY;
    }
    let mut max = 0.0_f64;
    for i in 0..corners.len() {
        let a = corners[i];
        let b = corners[(i + 1) % corners.len()];
        max = max.max(((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt());
    }
    max
}

/// Max pairwise screen distance among the given points — the on-screen "size"
/// when only a subset of a tile's corners are in front of the camera (a tile
/// crossing the horizon). Returns ∞ for fewer than 2 points.
fn max_pairwise_px(pts: &[(f64, f64)]) -> f64 {
    if pts.len() < 2 {
        return f64::INFINITY;
    }
    let mut max = 0.0_f64;
    for i in 0..pts.len() {
        for j in i + 1..pts.len() {
            let (a, b) = (pts[i], pts[j]);
            max = max.max(((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt());
        }
    }
    max
}

/// Does the screen AABB of the (in-front) corners intersect the viewport plus a
/// generous margin? The margin keeps tiles that straddle the screen edge from
/// being culled (and doubles as a cheap prefetch ring).
fn screen_aabb_intersects_viewport(corners: &[(f64, f64)], vp: (f64, f64)) -> bool {
    if corners.is_empty() {
        return false;
    }
    let (vw, vh) = vp;
    let m = vw.max(vh) * 0.25;
    let (mut minx, mut maxx, mut miny, mut maxy) =
        (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in corners {
        minx = minx.min(x);
        maxx = maxx.max(x);
        miny = miny.min(y);
        maxy = maxy.max(y);
    }
    maxx >= -m && minx <= vw + m && maxy >= -m && miny <= vh + m
}

#[cfg(test)]
mod tests {
    //! The Phase-1 contract, written test-first. These are `#[ignore]`d until
    //! Phase 1 implements `select` (un-ignoring them is the red→green step). The
    //! camera math is real (no mocks); only the algorithm is pending.

    use super::*;
    use crate::camera::Camera;
    use crate::geo::LatLng;

    fn cam(pitch_deg: f64, zoom: f64) -> Camera {
        Camera::new(LatLng::new(67.23, 15.30), zoom).with_pitch(pitch_deg)
    }

    const VP: (f64, f64) = (1080.0, 2280.0);
    const SSE: f64 = 320.0;

    #[test]
    fn collapses_to_single_level_at_pitch_0() {
        // At pitch 0 every tile is the same distance class, so the quadtree must
        // resolve to one zoom level — byte-for-byte the legacy rectangle, so the
        // 2D map + goldens are untouched.
        let tiles = select(&cam(0.0, 13.0), VP, 4, 14, SSE);
        let zooms: std::collections::HashSet<u8> = tiles.iter().map(|t| t.id.z).collect();
        assert_eq!(zooms.len(), 1, "pitch 0 must be a single LOD");
        assert_eq!(*zooms.iter().next().unwrap(), 13);
    }

    #[test]
    fn fine_near_coarse_far_at_high_pitch() {
        // A tilted view must mix LODs: fine tiles near the camera centre, coarser
        // ones toward the horizon.
        let tiles = select(&cam(78.0, 14.0), VP, 4, 14, SSE);
        let zooms: Vec<u8> = tiles.iter().map(|t| t.id.z).collect();
        let min = *zooms.iter().min().unwrap();
        let max = *zooms.iter().max().unwrap();
        assert!(max > min, "tilted view must span multiple LODs (got {min}..={max})");
        assert!(max <= 14 && min >= 4, "LODs within source range");
    }

    #[test]
    fn bounded_count_at_pitch_80() {
        // The whole point: count stays bounded no matter how far the horizon
        // reaches (today this is thousands → capped → starved).
        let tiles = select(&cam(80.0, 14.0), VP, 4, 14, SSE);
        assert!(tiles.len() < 400, "count must stay bounded (got {})", tiles.len());
        assert!(!tiles.is_empty(), "must still cover the near ground");
    }

    #[test]
    fn deterministic_for_a_fixed_camera() {
        let a = select(&cam(60.0, 13.0), VP, 4, 14, SSE);
        let b = select(&cam(60.0, 13.0), VP, 4, 14, SSE);
        assert_eq!(a, b, "selection must be deterministic frame-to-frame");
    }
}
