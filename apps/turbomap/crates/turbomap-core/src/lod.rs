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
use std::collections::BinaryHeap;

/// Hard cap on the emitted (best-first) set. Must stay under the device GPU tile
/// cache's working set (~240 tiles at the 80 MiB raster budget) — a desired set
/// LARGER than the cache thrashes (every tile evicted before it draws → grey).
/// 220 leaves headroom while giving best-first enough budget to refine the near
/// field to ~camera zoom AND keep the coarse far field out to the horizon.
use crate::capacity::LOD_TILE_CAP as MAX_TILES;

/// Distance-scaled SSE strength: how fast the refine target loosens with the
/// tile's distance-from-eye (in altitudes). 0 = uniform target (old behaviour);
/// higher = far field coarsens faster (fewer far tiles). At ~10 altitudes out
/// (near the horizon) the target is ~1 + 0.4·9 ≈ 4.6× the near target, so the
/// deep horizon pyramid collapses to a few coarse tiles. Tuned to trim the
/// far/coarse count without visible LOD popping (the far field is hazy anyway).
const FAR_COARSEN_K: f64 = 0.4;

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
    max_tiles: usize,
) -> Vec<LodTile> {
    let min_zoom = min_zoom.min(max_zoom);
    let target = sse_target_px.max(1.0);
    // Per-scene budget (≤ the global LOD_TILE_CAP). The DEM/terrain scene runs a
    // SMALLER budget than the imagery — best-first spends it on the highest
    // on-screen-error (nearest) tiles first, so a small budget yields fine near
    // relief + coarse far (a proto-clipmap): few tiles, concentrated where the
    // shape matters, far less of the slow DEM. Imagery keeps the full cap.
    let max_tiles = max_tiles.clamp(1, MAX_TILES);
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

    // BEST-FIRST refinement bounded by MAX_TILES: always subdivide the tile with
    // the LARGEST on-screen error next (a max-heap keyed on the screen-space span).
    // A depth-first walk — in ANY child order — spends the whole budget on whatever
    // subtree it descends first: near-first starves the far field, far-first starves
    // the near field. Best-first shares the budget by error, so the near field
    // refines fine AND the far field stays present (coarse) out to the horizon.
    // `span.to_bits()` is a monotonic key for positive finite spans (and ∞).
    let mut heap: BinaryHeap<(u64, u8, u32, u32)> = BinaryHeap::new();
    for root in footprint_roots(camera, viewport_px, min_zoom) {
        if let Some(span) = tile_span(root, camera, viewport_px, eye, ppw, alt) {
            heap.push((span.to_bits(), root.z, root.x, root.y));
        }
    }
    let mut out = Vec::new();
    while let Some((sbits, z, x, y)) = heap.pop() {
        if out.len() >= max_tiles {
            break;
        }
        let tile = TileId::new(z, x, y);
        let span = f64::from_bits(sbits);
        // Stop subdividing once the working set (emitted + still-queued) would blow
        // the budget — emit the rest at their current, coarser zoom (no dropouts).
        let budget_full = out.len() + heap.len() + 1 >= max_tiles;
        // Distance-scaled SSE: a tile far from the eye needs a LARGER on-screen
        // span to earn a subdivision, so the far field coarsens faster than the
        // near. The far horizon is tiny on screen + washed out by aerial haze, so
        // its deep pyramid is wasted detail (and, for the DEM, a flood of slow
        // tiles). Near tiles (dist≈altitude) keep the sharp `target`; the factor
        // only grows past ~1 altitude out. Trims the far/coarse count with no
        // perceptible loss (and no grey — the coarse tiles still cover it).
        let (nw, se) = tile.world_bounds();
        let ddx = eye[0].clamp(nw.x, se.x) - eye[0];
        let ddy = eye[1].clamp(nw.y, se.y) - eye[1];
        let ddz = -eye[2];
        let dist = (ddx * ddx + ddy * ddy + ddz * ddz).sqrt().max(1e-9);
        let eff_target = target * (1.0 + FAR_COARSEN_K * (dist / alt - 1.0).max(0.0));
        match tile.children() {
            Some(children) if z < max_zoom && span > eff_target && !budget_full => {
                for ch in children {
                    if let Some(cs) = tile_span(ch, camera, viewport_px, eye, ppw, alt) {
                        heap.push((cs.to_bits(), ch.z, ch.x, ch.y));
                    }
                }
            }
            _ => out.push(LodTile { id: tile }),
        }
    }
    out
}

/// On-screen size estimate (px) of a tile for the SSE refine decision, or `None`
/// if the tile is demonstrably off-screen (all corners in front of the camera but
/// outside the viewport). Distance is to the NEAREST point of the tile's ground
/// rect to the eye — a coarse ancestor that contains the camera then measures
/// ~the eye height (so it refines down under the camera), and the far field grows
/// with ground distance (so it stays coarse). Robust at any pitch.
fn tile_span(
    tile: TileId,
    camera: &Camera,
    vp: (f64, f64),
    eye: [f64; 3],
    ppw: f64,
    alt: f64,
) -> Option<f64> {
    let (nw, se) = tile.world_bounds();
    let contains_centre = {
        let c = camera.center.to_world();
        c.x >= nw.x && c.x <= se.x && c.y >= nw.y && c.y <= se.y
    };
    let (front, behind) = project_corners(tile, camera, vp);
    if behind == 0 && !contains_centre && !screen_aabb_intersects_viewport(&front, vp) {
        return None;
    }
    let dx = eye[0].clamp(nw.x, se.x) - eye[0];
    let dy = eye[1].clamp(nw.y, se.y) - eye[1];
    let dz = -eye[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-9);
    Some((se.x - nw.x) * ppw * (alt / dist))
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
    let (mut minx, mut maxx, mut miny, mut maxy) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
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
        let tiles = select(&cam(0.0, 13.0), VP, 4, 14, SSE, MAX_TILES);
        let zooms: std::collections::HashSet<u8> = tiles.iter().map(|t| t.id.z).collect();
        assert_eq!(zooms.len(), 1, "pitch 0 must be a single LOD");
        assert_eq!(*zooms.iter().next().unwrap(), 13);
    }

    #[test]
    fn fine_near_coarse_far_at_high_pitch() {
        // A tilted view must mix LODs: fine tiles near the camera centre, coarser
        // ones toward the horizon.
        let tiles = select(&cam(78.0, 14.0), VP, 4, 14, SSE, MAX_TILES);
        let zooms: Vec<u8> = tiles.iter().map(|t| t.id.z).collect();
        let min = *zooms.iter().min().unwrap();
        let max = *zooms.iter().max().unwrap();
        assert!(
            max > min,
            "tilted view must span multiple LODs (got {min}..={max})"
        );
        assert!(max <= 14 && min >= 4, "LODs within source range");
    }

    #[test]
    fn bounded_count_at_pitch_80() {
        // The whole point: count stays bounded no matter how far the horizon
        // reaches (today this is thousands → capped → starved).
        let tiles = select(&cam(80.0, 14.0), VP, 4, 14, SSE, MAX_TILES);
        assert!(
            tiles.len() < 400,
            "count must stay bounded (got {})",
            tiles.len()
        );
        assert!(!tiles.is_empty(), "must still cover the near ground");
    }

    #[test]
    fn deterministic_for_a_fixed_camera() {
        let a = select(&cam(60.0, 13.0), VP, 4, 14, SSE, MAX_TILES);
        let b = select(&cam(60.0, 13.0), VP, 4, 14, SSE, MAX_TILES);
        assert_eq!(a, b, "selection must be deterministic frame-to-frame");
    }

    // --- Device-camera regression: the "tilt → only the ground directly below
    // renders / grey-out" bug. The earlier tests missed it because they used
    // `max_zoom == camera_zoom` (no over-zoom headroom). The real device runs
    // the camera at zoom 15 with `max_zoom = source_max + 3 = 18`, and a TALL
    // phone viewport. In that gap the old corner-projection SSE refined every
    // view-plane-straddling tile to max zoom (device log: `z=18..18`, no
    // gradient) and the footprint collapsed to the near ground. These reproduce
    // that exact configuration so the fix can't silently regress.
    const DEVICE_VP: (f64, f64) = (1080.0, 2400.0);

    #[test]
    fn device_tilt_spans_a_zoom_gradient_not_all_max() {
        let camera = cam(60.0, 15.0);
        let tiles = select(&camera, DEVICE_VP, 5, 18, SSE, MAX_TILES);
        let zooms: Vec<u8> = tiles.iter().map(|t| t.id.z).collect();
        let zmin = *zooms.iter().min().unwrap();
        let zmax = *zooms.iter().max().unwrap();
        assert!(
            zmax - zmin >= 3,
            "device tilt must span a real fine→coarse gradient; got z={zmin}..{zmax} \
             (one bug forced every tile to max zoom — z=18..18)"
        );
        // The near field must be reasonably fine — NOT the z=5..8 wash that read as
        // blank. (Best-first lands it ~2 levels below the camera zoom; refining the
        // immediate near to exactly the camera zoom is a known follow-up.)
        assert!(
            zmax >= 12,
            "near field must refine to a usable zoom; got max z={zmax} (was z=5..8 blur)"
        );
    }

    #[test]
    fn device_tilt_footprint_reaches_the_far_field() {
        let camera = cam(70.0, 15.0);
        let c = camera.center.to_world();
        let tiles = select(&camera, DEVICE_VP, 5, 18, SSE, MAX_TILES);
        let max_d = tiles
            .iter()
            .map(|t| {
                let (nw, se) = t.id.world_bounds();
                let dx = 0.5 * (nw.x + se.x) - c.x;
                let dy = 0.5 * (nw.y + se.y) - c.y;
                (dx * dx + dy * dy).sqrt()
            })
            .fold(0.0_f64, f64::max);
        // A z15 tile is ~1/2^15 ≈ 3e-5 world wide. A collapsed near-only footprint
        // stays within a handful of those; the horizon fan must reach far beyond.
        assert!(
            max_d > 1e-3,
            "tilted footprint must reach the far field; got max dist {max_d:.2e} \
             (the bug kept it within the near ground directly below the camera)"
        );
    }
}
