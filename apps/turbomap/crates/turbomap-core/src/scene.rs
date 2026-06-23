//! Headless scene state: camera, viewport, the set of tiles the renderer
//! wants, and the set already ingested.
//!
//! This is the *contract* the host depends on; the wgpu renderer is a
//! separate concern that consumes a `Scene`.
//!
//! Two tile sets matter:
//! - [`Scene::visible_tiles`] — strictly inside the viewport. The renderer
//!   draws these.
//! - [`Scene::desired_tiles`] — visible plus a prefetch ring outside the
//!   viewport. The host fetches these so panning reveals ready tiles. By
//!   default, `desired ⊇ visible`.

use std::cell::RefCell;
use std::collections::HashSet;

use std::time::{Duration, Instant};

use crate::{
    camera::{Camera, CameraAnimation},
    geo::WorldPoint,
    tile::TileId,
};

/// Above this pitch the tile selection switches from the legacy single-zoom
/// rectangle to the mixed-LOD SSE quadtree.
///
/// The first SSE impl (corner-projection) over-refined every view-plane-
/// straddling tile to max zoom (device logs: `z=18..18`) and blanked on tilt.
/// Rewritten to DISTANCE-based screen-space error (`world_size·ppw·alt/dist`,
/// see lod.rs) which can't degenerate at the view plane — it gives a real
/// fine-near/coarse-far gradient and covers to the horizon with cheap coarse
/// tiles, which the legacy single-zoom path fundamentally can't do (its
/// footprint clamps to the near ground at high tilt → far field never loads).
const LOD_PITCH_DEG: f64 = 1.0;

/// Target on-screen tile EDGE length (px) for the SSE quadtree. ~one basemap
/// tile per ~320 px: crisp near the camera, coarsening toward the horizon.
/// Tunable on device (Phase 5).
const LOD_SSE_TARGET_PX: f64 = 320.0;

/// Where a tile sits in the core's streaming lifecycle, *relative to the
/// current camera*. The core owns this wanted↔resident classification; the
/// host owns fetch *transport* (in-flight HTTP, decode queue, backoff), which a
/// later slice reflects back as sub-states of [`TilePhase::Pending`].
///
/// This is the single source of truth the derived views read from
/// ([`Scene::pending_tiles`], the per-frame trace histogram) — so "wanted but
/// missing", "drawable now", and "evictable" are one classification instead of
/// three ad-hoc set differences scattered across the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePhase {
    /// Wanted this frame and GPU-resident — drawable now.
    Resident,
    /// Wanted this frame but not resident — the host must fetch it.
    Pending,
    /// Resident but not wanted this frame — an eviction candidate the GPU
    /// cache keeps only while its byte budget allows.
    Retained,
}

/// Why a wanted tile is wanted — its streaming PRIORITY. The host fetches
/// `pending_tiles` front-to-back up to a concurrency cap, so this ordering is
/// what makes the most-important data load first instead of an undifferentiated
/// distance race ("tiles load over each other / chaos" on first load).
///
/// Fetch order is the variant order below ([`TileTier::priority`]):
/// Overview → Visible → Prefetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TileTier {
    /// The cheap coarse backdrop (a handful of tiles). Fetched FIRST so a cold
    /// start shows a complete, if blurry, floor almost immediately — the screen
    /// is never empty grey, and every sharper tile fades in over real content
    /// (the anti-flicker floor). It's a few coarse tiles, so leading with it
    /// costs the viewport almost nothing under concurrent fetch.
    Overview,
    /// On-screen tiles at the target zoom — the viewport the user is looking at.
    /// Sharpens over the overview floor; ordered nearest-first within the tier.
    Visible,
    /// The off-screen prefetch ring. Pre-warmed LAST so a pan lands on ready
    /// tiles — but never at the expense of an on-screen tile still missing.
    Prefetch,
}

impl TileTier {
    /// Lower = fetched earlier. The primary `pending_tiles` sort key.
    fn priority(self) -> u8 {
        match self {
            TileTier::Overview => 0,
            TileTier::Visible => 1,
            TileTier::Prefetch => 2,
        }
    }
}

/// Memo key for [`Scene::visible_tiles`] — the bit-exact inputs the LOD
/// selection depends on (camera pose + viewport + zoom bounds). Identical key ⇒
/// identical visible set, so the cached result can be reused.
type VisibleKey = (u64, u64, u64, u64, u64, u32, u32, u8, u8);

#[derive(Debug)]
pub struct Scene {
    camera: Camera,
    viewport_px: (u32, u32),
    ingested: HashSet<TileId>,
    min_zoom: u8,
    max_zoom: u8,
    prefetch_margin_px: u32,
    /// In-flight camera transition. `tick()` advances it; user input
    /// cancels it.
    animation: Option<CameraAnimation>,
    /// Per-frame memo of [`Self::visible_tiles`]. The LOD select (a best-first
    /// quadtree over ~220 tiles) is a PURE function of the camera + viewport +
    /// zoom bounds, but it's queried ~8× per frame (raster/vector/hillshade
    /// prepares, the text + icon passes, the tile-count, the shadow + draped
    /// paths) — each recomputing the identical set. Caching it keyed on those
    /// inputs collapses that to one compute per distinct camera. `RefCell`
    /// because the callers hold `&Scene`. Always fresh: a moved camera changes
    /// the key, so the next query misses and recomputes (no explicit invalidation).
    visible_memo: RefCell<Option<(VisibleKey, Vec<TileId>)>>,
}

impl Scene {
    pub fn new(camera: Camera, viewport_px: (u32, u32), min_zoom: u8, max_zoom: u8) -> Self {
        Self::with_margin(camera, viewport_px, min_zoom, max_zoom, 0)
    }

    pub fn with_margin(
        camera: Camera,
        viewport_px: (u32, u32),
        min_zoom: u8,
        max_zoom: u8,
        prefetch_margin_px: u32,
    ) -> Self {
        Self {
            camera,
            viewport_px,
            ingested: HashSet::new(),
            min_zoom,
            max_zoom,
            prefetch_margin_px,
            animation: None,
            visible_memo: RefCell::new(None),
        }
    }

    pub fn camera(&self) -> Camera {
        self.camera
    }

    /// Snap directly to `camera`. Cancels any in-flight animation — direct
    /// user input always wins.
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
        self.animation = None;
    }

    /// Start an animation toward `target`. Replaces any existing animation.
    pub fn ease_to(&mut self, target: Camera, duration: Duration) {
        self.animation = Some(CameraAnimation::new(self.camera, target, duration));
    }

    /// Sample the active animation at `now` and update the camera. Returns
    /// `true` if an animation is still in flight (host should keep
    /// requesting redraws).
    pub fn tick(&mut self, now: Instant) -> bool {
        if let Some(anim) = self.animation {
            self.camera = anim.sample(now);
            if anim.is_finished(now) {
                self.animation = None;
                return false;
            }
            return true;
        }
        false
    }

    pub fn is_animating(&self) -> bool {
        self.animation.is_some()
    }

    pub fn viewport_px(&self) -> (u32, u32) {
        self.viewport_px
    }

    pub fn set_viewport_px(&mut self, viewport_px: (u32, u32)) {
        self.viewport_px = viewport_px;
    }

    pub fn prefetch_margin_px(&self) -> u32 {
        self.prefetch_margin_px
    }

    pub fn set_prefetch_margin_px(&mut self, margin: u32) {
        self.prefetch_margin_px = margin;
    }

    /// The integer tile zoom used to address the pyramid at the current
    /// camera. Clamped to the source's supported range.
    pub fn tile_zoom(&self) -> u8 {
        let z = self.camera.zoom.floor() as i32;
        z.clamp(self.min_zoom as i32, self.max_zoom as i32) as u8
    }

    /// Tiles strictly inside the viewport. The renderer draws this set.
    ///
    /// When the camera is pitched (3D), this is a MIXED-ZOOM screen-space-error
    /// quadtree (fine near, coarse to the horizon) so a tilted view covers the
    /// whole frustum with a bounded tile count instead of a single zoom that
    /// either explodes or gets trimmed to a near sliver. At/near pitch 0 it's
    /// the legacy single-level rectangle — the 2D map + goldens are unchanged.
    pub fn visible_tiles(&self) -> Vec<TileId> {
        let key = self.visible_key();
        if let Some((k, tiles)) = self.visible_memo.borrow().as_ref() {
            if *k == key {
                return tiles.clone();
            }
        }
        let tiles = if self.camera.pitch_deg > LOD_PITCH_DEG {
            self.lod_tiles()
        } else {
            self.tiles_for_margin(0)
        };
        *self.visible_memo.borrow_mut() = Some((key, tiles.clone()));
        tiles
    }

    /// Bit-exact fingerprint of every input [`Self::visible_tiles`] depends on,
    /// for the per-frame memo. f64 fields go in as raw bits so the compare is
    /// exact and cheap (no epsilon needed — a settled camera reproduces the
    /// same bits, and any real move changes them).
    fn visible_key(&self) -> VisibleKey {
        let c = &self.camera;
        (
            c.center.lat.to_bits(),
            c.center.lng.to_bits(),
            c.zoom.to_bits(),
            c.pitch_deg.to_bits(),
            c.bearing_deg.to_bits(),
            self.viewport_px.0,
            self.viewport_px.1,
            self.min_zoom,
            self.max_zoom,
        )
    }

    /// Mixed-zoom SSE quadtree selection for the current pitched camera, mapped
    /// to plain tile ids. The coarse far leaves double as the overview backdrop.
    fn lod_tiles(&self) -> Vec<TileId> {
        let vp = (self.viewport_px.0 as f64, self.viewport_px.1 as f64);
        crate::lod::select(&self.camera, vp, self.min_zoom, self.max_zoom, LOD_SSE_TARGET_PX)
            .into_iter()
            .map(|t| t.id)
            .collect()
    }

    /// Tiles the host should keep loaded: the visible set plus a
    /// `prefetch_margin_px`-wide ring of off-screen tiles. The renderer does
    /// not draw the margin — it just keeps it warm so panning is smooth.
    ///
    /// The set (and its order here) is unchanged; [`Self::desired_tagged`] is
    /// the one builder, and this is its untagged projection.
    pub fn desired_tiles(&self) -> Vec<TileId> {
        self.desired_tagged().into_iter().map(|(t, _)| t).collect()
    }

    /// Every desired tile tagged with its streaming [`TileTier`] — the single
    /// builder of the wanted set. `pending_tiles` orders by `(tier, distance)`
    /// off this, so the most-important data (a coarse floor, then the viewport)
    /// loads before the off-screen ring.
    ///
    /// The SET is identical to the historical `desired_tiles`; this only adds
    /// the per-tile tier (and `desired_tiles` is now its projection), so the
    /// determinism / bounded / backdrop invariants are unaffected.
    pub fn desired_tagged(&self) -> Vec<(TileId, TileTier)> {
        // Zoom levels below the visible set kept as the backdrop floor (governed).
        use crate::capacity::OVERVIEW_DEPTH;
        let mut out: Vec<(TileId, TileTier)> = Vec::new();
        let mut seen: HashSet<TileId> = HashSet::new();

        if self.camera.pitch_deg > LOD_PITCH_DEG {
            // Pitched: the mixed-zoom LOD leaves ARE the drawn (visible) set and
            // already span fine→coarse to the horizon, so there's no separate
            // prefetch ring.
            for t in self.lod_tiles() {
                if seen.insert(t) {
                    out.push((t, TileTier::Visible));
                }
            }
        } else {
            // Flat: the visible single-zoom rectangle, plus the margin ring
            // around it. Classify each margin-rect tile by whether it's inside
            // the (margin-0) visible rect — preserving the exact historical set
            // (`tiles_for_margin(margin)`), now split into Visible vs Prefetch.
            let visible: HashSet<TileId> = self.tiles_for_margin(0).into_iter().collect();
            for t in self.tiles_for_margin(self.prefetch_margin_px) {
                let tier = if visible.contains(&t) {
                    TileTier::Visible
                } else {
                    TileTier::Prefetch
                };
                if seen.insert(t) {
                    out.push((t, tier));
                }
            }
        }

        // A coarse overview backdrop under the visible set — cheap (a handful of
        // tiles) and the floor the best-available resolver draws while the fine
        // set streams in, so newly-arrived tiles blend over real (if blurry)
        // content instead of fading up from the empty-tile grey ("everything
        // greys out when I zoom in at a tilt" / the zoom-out flash). Tagged
        // Overview but fetched FIRST (see `TileTier`).
        let z = self.tile_zoom();
        let overview_z = z.saturating_sub(OVERVIEW_DEPTH).max(self.min_zoom);
        if overview_z < z {
            for t in self.tiles_for_margin_at(0, overview_z) {
                if seen.insert(t) {
                    out.push((t, TileTier::Overview));
                }
            }
        }
        out
    }

    /// Classify every WANTED tile as [`Resident`](TilePhase::Resident) or
    /// [`Pending`](TilePhase::Pending), in `desired_tiles()` order. The hot
    /// path — `pending_tiles` and the trace's wanted-tile counts both read it,
    /// so the resident/pending split is defined in exactly one place. Does NOT
    /// scan for [`Retained`](TilePhase::Retained) tiles (see [`Self::tile_phases`]).
    fn wanted_phases(&self) -> Vec<(TileId, TilePhase)> {
        self.desired_tiles()
            .into_iter()
            .map(|t| {
                let phase = if self.ingested.contains(&t) {
                    TilePhase::Resident
                } else {
                    TilePhase::Pending
                };
                (t, phase)
            })
            .collect()
    }

    /// Every tile the core is tracking — the union of *wanted* and *resident* —
    /// tagged with its [`TilePhase`]. The complete lifecycle picture for the
    /// per-frame trace histogram: wanted tiles as Resident/Pending, plus
    /// resident-but-no-longer-wanted tiles as [`Retained`](TilePhase::Retained)
    /// (the cache's eviction candidates).
    pub fn tile_phases(&self) -> Vec<(TileId, TilePhase)> {
        let mut out = self.wanted_phases();
        let wanted: HashSet<TileId> = out.iter().map(|(t, _)| *t).collect();
        for t in &self.ingested {
            if !wanted.contains(t) {
                out.push((*t, TilePhase::Retained));
            }
        }
        out
    }

    /// Desired tiles that have not yet been ingested ([`TilePhase::Pending`]),
    /// ordered by `(tier, distance)`: most-important [`TileTier`] first
    /// (Overview floor → Visible viewport → Prefetch ring), then nearest-first
    /// within a tier. The host pumps this front-to-back up to a concurrency
    /// cap, so the viewport always streams before the off-screen ring and the
    /// camera-centre tile leads its tier — instead of an undifferentiated
    /// distance race where a near ring tile could beat a far viewport tile.
    pub fn pending_tiles(&self) -> Vec<TileId> {
        let centre = self.camera.center.to_world();
        let mut pend: Vec<(TileId, TileTier)> = self
            .desired_tagged()
            .into_iter()
            .filter(|(t, _)| !self.ingested.contains(t))
            .collect();
        pend.sort_by(|(a, ta), (b, tb)| {
            ta.priority().cmp(&tb.priority()).then_with(|| {
                tile_centre_sq_distance(*a, centre)
                    .partial_cmp(&tile_centre_sq_distance(*b, centre))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        pend.into_iter().map(|(t, _)| t).collect()
    }

    /// Mark a tile as available to the renderer.
    pub fn ingest(&mut self, id: TileId) {
        self.ingested.insert(id);
    }

    /// Forget that `id` was ingested — called when the GPU cache evicts
    /// the tile under memory pressure. Without this the `ingested` set
    /// and the actual cache residency drift apart: `pending_tiles` keeps
    /// excluding an evicted tile forever, so it's never re-requested and
    /// shows as a permanent grey hole until the layer is rebuilt.
    pub fn un_ingest(&mut self, id: &TileId) {
        self.ingested.remove(id);
    }

    /// How many tiles the renderer believes are resident. Lets a stress
    /// test assert the bookkeeping tracks the (budget-bounded) cache.
    pub fn ingested_len(&self) -> usize {
        self.ingested.len()
    }

    /// World-space bounds of the current viewport. Used by the renderer to
    /// build its instance buffer.
    pub fn viewport_world_bounds(&self) -> (WorldPoint, WorldPoint) {
        let ppw = self.camera.pixels_per_world_unit();
        let centre = self.camera.center.to_world();
        let half_w = self.viewport_px.0 as f64 * 0.5 / ppw;
        let half_h = self.viewport_px.1 as f64 * 0.5 / ppw;
        (
            WorldPoint::new(centre.x - half_w, centre.y - half_h),
            WorldPoint::new(centre.x + half_w, centre.y + half_h),
        )
    }

    fn tiles_for_margin(&self, margin_px: u32) -> Vec<TileId> {
        self.tiles_for_margin_at(margin_px, self.tile_zoom())
    }

    /// As [`Self::tiles_for_margin`] but at an explicit tile zoom `z` (used to
    /// also request a coarse overview level as a guaranteed backdrop).
    fn tiles_for_margin_at(&self, margin_px: u32, z: u8) -> Vec<TileId> {
        // We unproject the four (margin-extended) viewport corners onto
        // the ground plane to get the actual visible-on-ground area.
        // For pitch=0 / bearing=0 this collapses to the legacy
        // axis-aligned rectangle (verified by the existing tile-
        // coverage tests). For pitched views it produces a trapezoid
        // that can extend far toward the horizon — we take the AABB
        // of the four ground points and request tiles inside it.
        //
        // Tile *count* is capped so a near-horizon view at low pitch
        // doesn't request thousands of tiles. The cap keeps us within
        // backpressure budget; tiles outside the cap stay unloaded
        // (the frustum trapezoid gets larger faster than the cap
        // grows, but the cap still requests the closest tiles first
        // via `pending_tiles`' distance sort).
        // Cap the per-level working set. Steep tilts (now up to 80°) project a
        // frustum trapezoid reaching far toward the horizon; without a cap
        // that's thousands of tiles. The governed cap keeps the working set
        // (visible + overview + prefetch, ×3 layer caches) inside the GPU budget
        // so it can't thrash/OOM — the `capacity` module proves desired ≤ cache
        // at compile time. Still covers the near ground the user is looking at
        // (far tiles fade into haze/sky); closest-first via `pending_tiles`.
        use crate::capacity::RECT_TILE_CAP as MAX_TILES;
        let n = 1u32 << z;
        let vw = self.viewport_px.0 as f64;
        let vh = self.viewport_px.1 as f64;
        let m = margin_px as f64;
        let corners_px = [
            (-m, -m),
            (vw + m, -m),
            (vw + m, vh + m),
            (-m, vh + m),
        ];
        let mut min_world_x = f64::INFINITY;
        let mut max_world_x = f64::NEG_INFINITY;
        let mut min_world_y = f64::INFINITY;
        let mut max_world_y = f64::NEG_INFINITY;
        for (px, py) in corners_px {
            let w = self.camera.pixel_to_world((px, py), (vw, vh));
            // Clamp to valid Mercator world bounds — at high pitch
            // corners can fall off the world plane (behind the
            // horizon).
            let wx = w.x.clamp(0.0, 1.0);
            let wy = w.y.clamp(0.0, 1.0);
            min_world_x = min_world_x.min(wx);
            max_world_x = max_world_x.max(wx);
            min_world_y = min_world_y.min(wy);
            max_world_y = max_world_y.max(wy);
        }

        let min_x = (min_world_x * n as f64).floor() as i64;
        let max_x = (max_world_x * n as f64).ceil() as i64 - 1;
        let min_y = (min_world_y * n as f64).floor() as i64;
        let max_y = (max_world_y * n as f64).ceil() as i64 - 1;

        // The frustum footprint is the AABB of the unprojected viewport
        // corners. We do NOT clip it to a fixed tile-ring around the camera:
        // a steep-pitch view legitimately reaches many tiles toward the
        // horizon (a fixed ±8 ring collapsed a tilted high-zoom view to a
        // sliver of near tiles — everything else greyed to sky). Instead the
        // `MAX_TILES` count cap below bounds the working set for OOM safety,
        // trimming toward the camera so the near field (what the user is
        // looking at) is always kept and the far horizon — which fades into
        // haze/sky anyway — is what gets dropped.
        let min_x = min_x.max(0).min((n - 1) as i64) as u32;
        let max_x = max_x.max(0).min((n - 1) as i64) as u32;
        let min_y = min_y.max(0).min((n - 1) as i64) as u32;
        let max_y = max_y.max(0).min((n - 1) as i64) as u32;

        let count = ((max_x - min_x + 1) as usize) * ((max_y - min_y + 1) as usize);
        if count > MAX_TILES {
            // Trim toward the camera so the tiles we keep are the ones the
            // user is most likely looking at. The camera centre (look-at
            // point) can sit just outside the footprint AABB at steep pitch,
            // so clamp it into the rect first — otherwise the growing window
            // can be empty/inverted (a u32 underflow) and starve the result.
            // Bigger arrays are useless: backpressure deferred them anyway,
            // and we'd just spend cycles enumerating them every frame.
            let centre = self.camera.center.to_world();
            let cx = ((centre.x * n as f64) as i64).clamp(min_x as i64, max_x as i64);
            let cy = ((centre.y * n as f64) as i64).clamp(min_y as i64, max_y as i64);
            let mut radius = 1i64;
            loop {
                let nx0 = (cx - radius).max(min_x as i64) as u32;
                let nx1 = (cx + radius).min(max_x as i64) as u32;
                let ny0 = (cy - radius).max(min_y as i64) as u32;
                let ny1 = (cy + radius).min(max_y as i64) as u32;
                let c = ((nx1 - nx0 + 1) as usize) * ((ny1 - ny0 + 1) as usize);
                if c >= MAX_TILES || (nx0 == min_x && nx1 == max_x && ny0 == min_y && ny1 == max_y)
                {
                    let mut out = Vec::with_capacity(c);
                    for y in ny0..=ny1 {
                        for x in nx0..=nx1 {
                            out.push(TileId::new(z, x, y));
                        }
                    }
                    return out;
                }
                radius += 1;
            }
        }

        let mut out = Vec::with_capacity(count);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                out.push(TileId::new(z, x, y));
            }
        }
        out
    }
}

fn tile_centre_sq_distance(t: TileId, world_centre: WorldPoint) -> f64 {
    let n = (1u64 << t.z) as f64;
    let cx = (t.x as f64 + 0.5) / n;
    let cy = (t.y as f64 + 0.5) / n;
    let dx = cx - world_centre.x;
    let dy = cy - world_centre.y;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    //! Value boundary: the host-visible pull-push contract. The renderer
    //! needs to declare what tiles it wants, the host fetches them, and once
    //! ingested they must drop off the pending list. The tile-selection math
    //! also needs to be deterministic and bounded so hosts can size buffers.

    use super::*;
    use crate::geo::LatLng;

    #[test]
    fn at_zoom_zero_with_tiny_viewport_only_one_tile_is_pending() {
        let scene = Scene::new(Camera::new(LatLng::new(0.0, 0.0), 0.0), (100, 100), 0, 22);
        let pending = scene.pending_tiles();
        assert_eq!(pending, vec![TileId::new(0, 0, 0)]);
    }

    #[test]
    fn ingesting_a_pending_tile_removes_it_from_the_pending_list() {
        let mut scene = Scene::new(Camera::new(LatLng::new(0.0, 0.0), 0.0), (100, 100), 0, 22);
        assert_eq!(scene.pending_tiles(), vec![TileId::new(0, 0, 0)]);
        scene.ingest(TileId::new(0, 0, 0));
        assert!(
            scene.pending_tiles().is_empty(),
            "must be empty after ingest"
        );
    }

    #[test]
    fn un_ingest_makes_an_evicted_tile_pending_again() {
        // The coherence contract behind the "grey tile that never reloads"
        // bug: when the GPU cache evicts a tile under memory pressure, the
        // host calls `un_ingest`, and the tile MUST reappear in `pending`
        // so it gets re-fetched. Without this it greyed out permanently
        // until the layer was rebuilt.
        let mut scene = Scene::new(Camera::new(LatLng::new(0.0, 0.0), 0.0), (100, 100), 0, 22);
        let tile = TileId::new(0, 0, 0);
        scene.ingest(tile);
        assert!(scene.pending_tiles().is_empty(), "ingested → not pending");

        scene.un_ingest(&tile); // simulate a cache eviction
        assert_eq!(
            scene.pending_tiles(),
            vec![tile],
            "an evicted (un-ingested) but still-desired tile must re-pend"
        );
    }

    #[test]
    fn changing_the_camera_re_exposes_uningested_tiles() {
        // After a pan to a new region, freshly visible tiles should appear in
        // the pending list even if previously visible tiles were ingested.
        let mut scene = Scene::new(Camera::new(LatLng::new(0.0, 0.0), 1.0), (200, 200), 0, 22);
        // Ingest the whole desired set (visible + the coarse overview backdrop).
        for t in scene.desired_tiles() {
            scene.ingest(t);
        }
        assert!(scene.pending_tiles().is_empty());

        // Jump to a different region. New visible set must include something
        // not yet ingested.
        scene.set_camera(Camera::new(LatLng::new(60.0, -60.0), 4.0));
        assert!(
            !scene.pending_tiles().is_empty(),
            "new region must produce pending tiles"
        );
    }

    #[test]
    fn visible_tile_count_matches_viewport_in_tile_units() {
        // For a 1024×768 viewport with 256-px tiles, the renderer needs at
        // least ⌈1024/256⌉ × ⌈768/256⌉ = 4×3 = 12 tiles, and at most one
        // extra row + column when the camera straddles tile boundaries.
        let scene = Scene::new(Camera::new(LatLng::new(0.0, 0.0), 4.0), (1024, 768), 0, 22);
        let count = scene.visible_tiles().len();
        assert!(
            (12..=20).contains(&count),
            "expected 12..=20 visible tiles, got {count}",
        );
    }

    #[test]
    fn tile_zoom_is_clamped_to_source_range() {
        // A camera deep below the source's min zoom should still address
        // valid tiles — the host can't fetch z=−3, and the source declared
        // its supported range, so the scene must clamp.
        let scene = Scene::new(Camera::new(LatLng::new(0.0, 0.0), 0.0), (256, 256), 4, 14);
        assert_eq!(scene.tile_zoom(), 4);
        let scene_high = Scene::new(Camera::new(LatLng::new(0.0, 0.0), 20.0), (256, 256), 4, 14);
        assert_eq!(scene_high.tile_zoom(), 14);
    }

    #[test]
    fn steep_pitch_at_high_zoom_keeps_a_deep_footprint_not_a_ring_sliver() {
        // Regression: "when I pan/tilt down at a hiking zoom, everything but
        // the closest tile is removed — culling is too aggressive." The old
        // tile selector clipped the footprint to a fixed ±8-tile ring around
        // the camera CENTRE. At a steep pitch the centre (look-at point) sits
        // just *behind* the on-ground footprint, so the ring kept only a few
        // forward tiles — the rest of the visibly-needed terrain greyed to
        // sky. A tilted high-zoom view legitimately reaches many tiles toward
        // the horizon; the working set must follow the frustum (bounded only
        // by the count cap), not collapse to a sliver.
        //
        // Phone-tall viewport, z16, max tilt, looking north.
        let scene = Scene::new(
            Camera::new(LatLng::new(67.27, 15.05), 16.0)
                .with_pitch(80.0)
                .with_bearing(0.0),
            (1080, 2400),
            0,
            18,
        );
        let visible = scene.visible_tiles();
        // Steep pitch now selects a MIXED-LOD quadtree, not a single-zoom ring.
        // It must cover the view (not collapse to a near sliver)…
        assert!(
            visible.len() > 20,
            "steep-pitch footprint collapsed to {} tiles — culling too aggressive",
            visible.len()
        );
        // …stay bounded (SSE + the backstop, not an unbounded horizon strip)…
        assert!(
            visible.len() < 400,
            "footprint unbounded ({} tiles) — count cap not holding",
            visible.len()
        );
        // (The fine-near/coarse-far MIXING is camera-dependent — a high-zoom view
        // covers a small ground span that's legitimately near-uniform — so the LOD
        // pyramid is asserted in `lod::tests::fine_near_coarse_far_at_high_pitch`
        // on a wide view. Here we only guard that steep pitch covers a deep,
        // bounded footprint instead of collapsing to a near sliver.)
    }

    #[test]
    fn tile_ids_are_within_valid_range_for_their_zoom() {
        // At zoom z there are 2^z tiles per axis. The pending set must never
        // address a coordinate ≥ 2^z, even near the antimeridian.
        let scene = Scene::new(
            Camera::new(LatLng::new(0.0, 179.0), 3.0),
            (1024, 768),
            0,
            22,
        );
        let max = 1u32 << 3;
        for t in scene.visible_tiles() {
            assert!(t.x < max && t.y < max, "out-of-range tile: {:?}", t);
        }
    }

    // ---- prefetch margin contract -----------------------------------------

    #[test]
    fn desired_tiles_includes_a_coarse_overview_backdrop() {
        // Even with no prefetch margin, the desired set adds a coarse overview
        // level under the visible tiles (a guaranteed fade backdrop, so tiles
        // never fade up from the empty-tile grey on load / zoom-out). Every
        // visible tile is still desired; the extras are strictly coarser.
        let scene = Scene::with_margin(
            Camera::new(LatLng::new(60.39, 5.32), 11.0),
            (1024, 768),
            0,
            22,
            0,
        );
        let visible: HashSet<_> = scene.visible_tiles().into_iter().collect();
        let desired: HashSet<_> = scene.desired_tiles().into_iter().collect();
        assert!(visible.is_subset(&desired), "visible must stay desired");
        let visible_z = scene.tile_zoom();
        assert!(
            desired.iter().any(|t| t.z < visible_z),
            "a coarser overview backdrop level must be included",
        );
    }

    #[test]
    fn desired_tiles_is_a_strict_superset_of_visible_when_margin_is_set() {
        // With a one-tile margin in the interior of the world (so clamping
        // doesn't bite), every visible tile is still desired and at least
        // one extra ring tile shows up.
        let scene = Scene::with_margin(
            Camera::new(LatLng::new(60.39, 5.32), 11.0),
            (1024, 768),
            0,
            22,
            256,
        );
        let visible: HashSet<_> = scene.visible_tiles().into_iter().collect();
        let desired: HashSet<_> = scene.desired_tiles().into_iter().collect();
        assert!(visible.is_subset(&desired), "visible must be a subset");
        assert!(
            desired.len() > visible.len(),
            "margin must add at least one tile (visible {}, desired {})",
            visible.len(),
            desired.len(),
        );
    }

    #[test]
    fn pending_is_ordered_by_tier_then_distance() {
        // The streaming priority (Slice 3): the cheap coarse Overview floor
        // first, then the Visible viewport nearest-first, then the off-screen
        // Prefetch ring — so the area the user is looking at resolves before the
        // margin, and the centre tile leads. Replaces the old global
        // distance-only sort.
        let scene = Scene::with_margin(
            Camera::new(LatLng::new(60.39, 5.32), 11.0),
            (1024, 768),
            0,
            22,
            256,
        );
        let tier: std::collections::HashMap<TileId, TileTier> =
            scene.desired_tagged().into_iter().collect();
        let pending = scene.pending_tiles();
        let centre = scene.camera().center.to_world();

        // Tier priority is non-decreasing along pending…
        let prios: Vec<u8> = pending.iter().map(|t| tier[t].priority()).collect();
        for w in prios.windows(2) {
            assert!(w[0] <= w[1], "tier priority not non-decreasing: {prios:?}");
        }
        // …and within a tier, nearest-first.
        for w in pending.windows(2) {
            if tier[&w[0]] == tier[&w[1]] {
                let (d0, d1) = (
                    tile_centre_sq_distance(w[0], centre),
                    tile_centre_sq_distance(w[1], centre),
                );
                assert!(d0 <= d1, "within-tier not nearest-first near {w:?}");
            }
        }
        // The very first fetch covers the camera centre (the Overview floor tile
        // under it), so the centre area shows immediately.
        let head = pending[0];
        let (nw, se) = head.world_bounds();
        assert!(
            centre.x >= nw.x && centre.x <= se.x && centre.y >= nw.y && centre.y <= se.y,
            "head tile {head:?} did not contain camera centre {centre:?}",
        );
        // The fixture must actually exercise both ends of the tier range.
        assert!(
            prios.contains(&TileTier::Overview.priority())
                && prios.contains(&TileTier::Prefetch.priority()),
            "fixture should produce both an overview floor and a prefetch ring",
        );
    }

    // ---- Slice-2 invariants: the tile-selection RULES, enforced ------------
    //
    // These turn properties the pipeline used to satisfy only *emergently*
    // (and that broke, then got spot-patched) into executable, headless
    // invariants: determinism, a bounded working set, an always-present coarse
    // backdrop (the anti-flicker precondition), and monotonic convergence at a
    // fixed camera (no thrash). They run on the pure selection math — no GPU.

    /// A spread of cameras across location / zoom / pitch / bearing, including
    /// the steep-tilt + high-zoom cases that historically broke selection.
    fn camera_sweep() -> Vec<Camera> {
        let mut out = Vec::new();
        for (lat, lng) in [(0.0, 0.0), (60.39, 5.32), (67.27, 15.05), (-33.9, 151.2)] {
            for zoom in [2.0, 7.0, 11.0, 14.0, 16.0] {
                for pitch in [0.0, 20.0, 55.0, 80.0] {
                    for bearing in [0.0, 45.0, 200.0] {
                        out.push(
                            Camera::new(LatLng::new(lat, lng), zoom)
                                .with_pitch(pitch)
                                .with_bearing(bearing),
                        );
                    }
                }
            }
        }
        out
    }

    /// A phone-tall scene with a prefetch margin — the on-device shape.
    fn sweep_scene(cam: Camera) -> Scene {
        Scene::with_margin(cam, (1080, 2400), 0, 18, 256)
    }

    #[test]
    fn selection_is_deterministic_for_a_fixed_camera() {
        // Same camera + viewport ⇒ byte-identical visible / desired / pending
        // (order included). Two INDEPENDENT scenes, not two calls, so hidden
        // per-instance state would also trip it. The reconcile loop depends on
        // this: a desired set that reshuffled frame-to-frame would thrash
        // fetches even with a still camera.
        for cam in camera_sweep() {
            let a = sweep_scene(cam);
            let b = sweep_scene(cam);
            assert_eq!(a.visible_tiles(), b.visible_tiles(), "visible nondeterministic @ {cam:?}");
            assert_eq!(a.desired_tiles(), b.desired_tiles(), "desired nondeterministic @ {cam:?}");
            assert_eq!(a.pending_tiles(), b.pending_tiles(), "pending nondeterministic @ {cam:?}");
        }
    }

    #[test]
    fn the_desired_working_set_is_bounded() {
        // The working set must fit the GPU cache BY CONSTRUCTION — an unbounded
        // desired set is the coarse↔fine thrash / OOM the count caps prevent.
        // The bound is the governed ceiling (`capacity::MAX_DESIRED_TILES` = the
        // larger visible cap + the overview backdrop), which the `capacity`
        // module separately proves fits the cache with headroom at compile time.
        // Here we check the EMPIRICAL desired count never exceeds that ceiling.
        let bound = crate::capacity::MAX_DESIRED_TILES;
        for cam in camera_sweep() {
            let n = sweep_scene(cam).desired_tiles().len();
            assert!(n <= bound, "desired set {n} > governed ceiling {bound} @ {cam:?}");
        }
    }

    #[test]
    fn desired_always_carries_a_coarser_backdrop_under_the_finest_tiles() {
        // The anti-flicker PRECONDITION: whenever the finest wanted tile is
        // below the source's min zoom, the desired set also carries a strictly
        // coarser tile — so the best-available resolver always has an ancestor
        // floor to draw while the fine tiles stream in, instead of the empty
        // clear colour ("everything greys out when I zoom in at a tilt").
        for cam in camera_sweep() {
            let desired = sweep_scene(cam).desired_tiles();
            let zmax = desired.iter().map(|t| t.z).max().unwrap();
            if zmax > 0 {
                // min_zoom is 0 in the sweep scene.
                assert!(
                    desired.iter().any(|t| t.z < zmax),
                    "no coarser backdrop under z{zmax} @ {cam:?}",
                );
            }
        }
    }

    #[test]
    fn ingesting_makes_monotonic_progress_at_a_fixed_camera() {
        // At a FIXED camera the loader must converge: ingesting a pending tile
        // removes it from pending and introduces no new ones, so pending
        // shrinks to empty. A violation is the fixed-camera thrash where
        // fetches never settle. Uses the steep-tilt high-zoom case (the largest
        // working set). Phases stay consistent: an ingested wanted tile reads
        // back as Resident, never Pending.
        let mut scene = sweep_scene(
            Camera::new(LatLng::new(67.27, 15.05), 16.0).with_pitch(80.0),
        );
        let mut prev = scene.pending_tiles();
        assert!(!prev.is_empty(), "expected pending tiles to drive the test");
        let mut guard = 0;
        while let Some(&next) = prev.first() {
            let before: HashSet<TileId> = prev.iter().copied().collect();
            scene.ingest(next);
            // The ingested tile is now Resident, not Pending.
            assert!(
                scene.tile_phases().iter().any(|(t, p)| *t == next && *p == TilePhase::Resident),
                "ingested tile {next:?} not classified Resident",
            );
            let now = scene.pending_tiles();
            let now_set: HashSet<TileId> = now.iter().copied().collect();
            assert!(now_set.is_subset(&before), "ingest introduced new pending tiles @ {next:?}");
            assert!(now.len() < prev.len(), "ingest did not shrink pending @ {next:?}");
            prev = now;
            guard += 1;
            assert!(guard < 10_000, "pending did not converge");
        }
    }

    #[test]
    fn pending_never_orders_a_lower_tier_before_a_higher_one() {
        // Slice-3 priority invariant across the camera sweep: pending is
        // strictly tier-ordered (Overview → Visible → Prefetch). In particular
        // no off-screen Prefetch tile is ever fetched before an on-screen
        // Visible tile that's still pending — "most important first" as an
        // enforced rule, not an emergent side effect of the distance sort.
        for cam in camera_sweep() {
            let scene = sweep_scene(cam);
            let tier: std::collections::HashMap<TileId, TileTier> =
                scene.desired_tagged().into_iter().collect();
            let prios: Vec<u8> = scene
                .pending_tiles()
                .iter()
                .map(|t| tier[t].priority())
                .collect();
            for w in prios.windows(2) {
                assert!(w[0] <= w[1], "tier priority regressed in pending @ {cam:?}: {prios:?}");
            }
        }
    }
}
