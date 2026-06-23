//! Tile working-set capacity governor (Slice 4 of the tile-pipeline plan).
//!
//! ONE place that declares the tile-selection caps and the GPU-cache budget,
//! and *proves at compile time* that the entire desired set fits the cache with
//! headroom. This is what makes coarse↔fine thrash IMPOSSIBLE BY CONSTRUCTION
//! rather than merely unlikely: when `desired ≤ cache`, the LRU can never evict
//! a still-wanted tile, so the resident set can't flip-flop frame-to-frame
//! (the "visible flicker even when the camera is still" failure).
//!
//! These constants were previously scattered as bare literals — `MAX_TILES` in
//! both `lod.rs` (220) and `scene.rs` (160), `OVERVIEW_DEPTH` (3) duplicated in
//! `scene.rs`, and the cache budget reasoned about only in a prose comment on
//! `MapOptions::default`. Collecting them here, with the fit relationship
//! encoded as a `const` assertion, means a future cap bump or budget cut that
//! would reintroduce thrash *fails the build* instead of regressing silently.
//!
//! Values are unchanged from the device-tuned originals — this slice governs
//! the numbers, it doesn't retune them. (The host-side fetch/decode budgets in
//! the FFI — ingest ms, prefetch margin — stay device-tuned and are out of
//! scope here; they govern transport latency, not the resident working set.)

/// Max tiles the pitched mixed-LOD selector emits — the best-first SSE
/// refinement cap in [`crate::lod::select`].
pub(crate) const LOD_TILE_CAP: usize = 220;

/// Max tiles a single flat footprint rectangle emits (the visible viewport, the
/// visible+prefetch ring, or the coarse overview level — each is one capped
/// `Scene::tiles_for_margin_at` call).
pub(crate) const RECT_TILE_CAP: usize = 160;

/// Zoom levels below the visible set kept resident as the coarse backdrop floor
/// (the anti-flicker overview the best-available resolver draws while the fine
/// set streams in).
pub(crate) const OVERVIEW_DEPTH: u8 = 3;

/// Declared ceiling on the desired set for ANY camera: the larger of the two
/// visible-selection caps (pitched LOD vs flat rectangle) plus one overview
/// backdrop rectangle. The Slice-2 `bounded` invariant test checks the
/// *empirical* desired count never exceeds this; this is the *declared* bound
/// the cache is sized against.
pub(crate) const MAX_DESIRED_TILES: usize = {
    let visible = if LOD_TILE_CAP > RECT_TILE_CAP { LOD_TILE_CAP } else { RECT_TILE_CAP };
    visible + RECT_TILE_CAP // + the overview backdrop (itself a capped rectangle)
};

/// Conservative per-tile GPU footprint (decoded RGBA + a full mip chain). Real
/// 256² sRGB basemap tiles are ~350 KiB; 512 KiB is a deliberate over-estimate
/// so the capacity proof below holds even for larger or mip-heavy tiles.
pub(crate) const CONSERVATIVE_TILE_BYTES: usize = 512 * 1024;

/// Default per-layer GPU texture cache budget (a ceiling — memory is used only
/// as tiles resolve, not pre-allocated). Up to three live caches (raster +
/// vector + terrain); raster is the one that fills.
pub(crate) const CACHE_BUDGET_BYTES: usize = 512 * 1024 * 1024;

/// Tiles the cache is GUARANTEED to hold, even at the conservative tile size —
/// the pessimistic floor the fit proof uses (real capacity is ~3× this).
pub(crate) const CACHE_TILE_FLOOR: usize = CACHE_BUDGET_BYTES / CONSERVATIVE_TILE_BYTES;

// THE GOVERNOR, enforced at COMPILE TIME: the cache must hold at least TWICE the
// largest possible desired set — even at the pessimistic tile size — so the
// whole working set stays resident with real headroom for pan/revisit history
// (looking back doesn't re-fetch). If a future change breaks this, the crate
// won't compile, and thrash can't regress unnoticed.
const _: () = assert!(
    MAX_DESIRED_TILES * 2 <= CACHE_TILE_FLOOR,
    "tile working set may not fit the GPU cache with headroom — coarse↔fine \
     thrash risk; raise CACHE_BUDGET_BYTES or lower the selection caps",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desired_ceiling_fits_the_cache_with_headroom() {
        // The same relationship the compile-time assertion guards, surfaced as a
        // readable runtime check with the actual numbers in the failure message.
        assert!(
            MAX_DESIRED_TILES * 2 <= CACHE_TILE_FLOOR,
            "max desired {MAX_DESIRED_TILES} tiles needs ≥{} cache slots (2× headroom); \
             conservative floor is only {CACHE_TILE_FLOOR}",
            MAX_DESIRED_TILES * 2,
        );
    }
}
