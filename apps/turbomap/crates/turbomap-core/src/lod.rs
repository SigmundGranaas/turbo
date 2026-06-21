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

/// Hard cap on the emitted set. This is NOT just a runaway backstop: the device
/// GPU tile cache holds a bounded working set (~160 tiles at the default 80 MiB
/// budget), and a desired set LARGER than the cache thrashes — every tile is
/// evicted before it can be drawn, so the terrain never resides and the screen
/// greys out (worst when zoomed in at a tilt, where the near field refines fine
/// AND the footprint still reaches the horizon). So the cap matches the proven
/// working-set budget; `select` refines NEAR roots first, so when the cap bites
/// it drops the FAR field — which is hazed/curved away to the horizon anyway —
/// rather than punching holes in the near terrain that fills the screen.
const MAX_TILES: usize = 160;

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
    // Camera eye in ABSOLUTE world (look-at centre + the RTC eye offset) plus the
    // focal scale, for DISTANCE-based screen-space error. Estimating a tile's
    // on-screen size from `world_size · ppw · (altitude / eye_distance)` is robust
    // at any pitch — unlike projecting the tile's corners, which degenerates when
    // a tile straddles the camera's view plane (corners on both sides) and forced
    // every such tile to max zoom, exploding the count and blanking the map on
    // tilt. Foreshortening (altitude/dist) gives the fine-near/coarse-far gradient.
    let vp_u = (viewport_px.0 as u32, viewport_px.1 as u32);
    let c = camera.center.to_world();
    let eo = camera.eye_offset_world(vp_u);
    let eye = [c.x + eo[0] as f64, c.y + eo[1] as f64, eo[2] as f64];
    let ppw = camera.pixels_per_world_unit();
    let alt = (camera.altitude_world(vp_u) as f64).max(1e-9);

    let mut roots = footprint_roots(camera, viewport_px, min_zoom);
    // Refine NEAR (to the eye) roots first, so when the MAX_TILES cap bites the
    // dropped roots are the FARTHEST ones — the hazed/curved-away far field —
    // not the near ground filling the screen.
    roots.sort_by(|a, b| {
        eye_dist2(*a, eye).partial_cmp(&eye_dist2(*b, eye)).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut out = Vec::new();
    for root in roots {
        refine(root, camera, viewport_px, max_zoom, target, eye, ppw, alt, &mut out);
        if out.len() >= MAX_TILES {
            break;
        }
    }
    out
}

/// Squared distance from the camera `eye` (absolute world, with z) to a tile's
/// ground centre (z = 0). Drives near-first refinement order.
fn eye_dist2(tile: TileId, eye: [f64; 3]) -> f64 {
    let (nw, se) = tile.world_bounds();
    let dx = 0.5 * (nw.x + se.x) - eye[0];
    let dy = 0.5 * (nw.y + se.y) - eye[1];
    let dz = -eye[2];
    dx * dx + dy * dy + dz * dz
}

/// The `min_zoom` tiles overlapping the camera's ground footprint, clamped to
/// the world. Built as a FAN from the camera out to the true ground horizon —
/// NOT the AABB of the four unprojected viewport corners, because at high pitch
/// the top corners' rays point at/above the horizon and `pixel_to_world`
/// collapses them to the camera centre, shrinking the footprint to the near
/// ground (the "only the tiles directly below me render" bug). The fan uses the
/// eye→centre forward direction and the frustum half-width, extended to the
/// horizon distance, so the far field is always covered (coarsely, via the
/// distance-based refine). Refinement deepens it where the screen demands.
fn footprint_roots(camera: &Camera, vp: (f64, f64), z: u8) -> Vec<TileId> {
    let n = 1u32 << z;
    let (vw, vh) = vp;
    let vp_u = (vw as u32, vh as u32);
    let center = camera.center.to_world();

    let mut min_x = center.x;
    let mut max_x = center.x;
    let mut min_y = center.y;
    let mut max_y = center.y;
    let mut add = |x: f64, y: f64| {
        let x = x.clamp(0.0, 1.0);
        let y = y.clamp(0.0, 1.0);
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    };

    // Near edge: the bottom screen corners ALWAYS unproject to ground in front of
    // the camera (robust), giving the near footprint width.
    for px in [0.0, vw] {
        let w = camera.pixel_to_world((px, vh), vp);
        add(w.x, w.y);
    }

    // Far field: extend along the ground-forward direction (eye → look-at centre)
    // out to the horizon, with the frustum's lateral half-width at that range.
    let eo = camera.eye_offset_world(vp_u);
    let (fwx, fwy) = (center.x - eo[0] as f64, center.y - eo[1] as f64);
    let flen = (fwx * fwx + fwy * fwy).sqrt();
    if flen > 1e-9 {
        let fwd = (fwx / flen, fwy / flen);
        let right = (fwd.1, -fwd.0);
        let alt = camera.altitude_world(vp_u) as f64;
        let coslat = camera.center.lat.to_radians().cos().abs().max(1e-3);
        let horizon = crate::camera::ground_horizon_world(
            alt as f32,
            camera.pitch_deg.to_radians() as f32,
            coslat as f32,
        ) as f64;
        let far_d = horizon.max(alt * 4.0);
        // tan(hfov/2) = aspect · tan(FOV_Y/2); FOV_Y ≈ 36.87° → tan ≈ 0.334.
        let half_w = far_d * (vw / vh) * 0.334;
        let far = (center.x + fwd.0 * far_d, center.y + fwd.1 * far_d);
        add(far.0 + right.0 * half_w, far.1 + right.1 * half_w);
        add(far.0 - right.0 * half_w, far.1 - right.1 * half_w);
        add(far.0, far.1);
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
#[allow(clippy::too_many_arguments)]
fn refine(
    tile: TileId,
    camera: &Camera,
    vp: (f64, f64),
    max_zoom: u8,
    target: f64,
    eye: [f64; 3],
    ppw: f64,
    alt: f64,
    out: &mut Vec<LodTile>,
) {
    if out.len() >= MAX_TILES {
        return;
    }
    // A tile whose ground rect contains the camera's look-at point is under/around
    // the camera — keep it (it's the near ground filling the lower frame).
    let contains_centre = {
        let c = camera.center.to_world();
        let (nw, se) = tile.world_bounds();
        c.x >= nw.x && c.x <= se.x && c.y >= nw.y && c.y <= se.y
    };
    // Cull only tiles that are demonstrably off-screen (all corners in front of
    // the camera but outside the viewport). We do NOT cull on "corners behind the
    // view plane" any more — that's the case the old corner-projection SSE got
    // wrong, dropping/over-refining tiles that straddle the plane at high pitch.
    let (front, behind) = project_corners(tile, camera, vp);
    if behind == 0 && !contains_centre && !screen_aabb_intersects_viewport(&front, vp) {
        return;
    }
    // DISTANCE-based screen-space size: on-screen px ≈ world_size · ppw · (alt/dist)
    // to the eye. Near → large (refine), far → small (coarse). Never degenerates at
    // the view plane, so it gives a true fine-near/coarse-far gradient at any tilt.
    let (nw, se) = tile.world_bounds();
    let dx = 0.5 * (nw.x + se.x) - eye[0];
    let dy = 0.5 * (nw.y + se.y) - eye[1];
    let dz = -eye[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-9);
    let world_size = se.x - nw.x;
    let span = world_size * ppw * (alt / dist);
    if tile.z >= max_zoom || span <= target {
        out.push(LodTile { id: tile });
        return;
    }
    match tile.children() {
        Some(children) => {
            for c in children {
                refine(c, camera, vp, max_zoom, target, eye, ppw, alt, out);
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
