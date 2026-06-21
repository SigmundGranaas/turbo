# Tile LOD Retention + Source-Change Crossfade — Implementation Plan

**Status:** proposed · **Date:** 2026-06-21 · **Scope:** `turbomap-core` render path
(raster + terrain), `scene.rs` selection, scenario-harness validation.

## 1. Problem

On-device, basemap + terrain tile presentation feels "poppy" and inconsistent, and
actively *destroys* detail the engine already owns. Concretely (user-reported, all
reproduced in code):

1. **Inconsistent transitions.** A freshly-fetched tile fades in (nice); a tile served
   from cache snaps in instantly (harsh). The blend depends on *where the bytes came
   from*, not on whether a visible change is happening.
2. **Zoom-out downgrade.** Zooming out replaces a resident high-res tile with a *lower*-res
   coarser tile — discarding detail we already have, the moment the coarse tile is "ready."
3. **3D flattens on zoom.** Zooming when the new LOD's DEM tile isn't loaded drops the mesh
   to flat (z=0) instead of reusing the relief from a DEM tile we still hold.
4. **Far-field gray cutaway** when panning/tilting: regions beyond the target-LOD ring are
   simply not drawn.

These are four faces of **one** design flaw.

## 2. Current architecture (why the UX is like this)

The renderer is organised around **"draw the tile pyramid level at the current target
zoom."** Per frame, `RasterPipeline::prepare` (`render/raster.rs:546`):

```text
for tile in scene.visible_tiles():        // tiles at floor(zoom) ONLY (scene.rs tile_zoom())
    self_alpha = f(age_since_ingest)       // raster.rs:564 — fade keyed on bytes' ingest age
    if self_alpha < 1.0:                   // fallback exists ONLY during fade-in
        draw nearest cached ANCESTOR sub-sampled   (raster.rs:579)
        else (no ancestor) draw covered DESCENDANTS (raster.rs:600, only when no ancestor)
    if self_alpha > 0.0:
        draw self at self_alpha
```

Terrain mirrors this: `TerrainCache::bind_for` (`render/terrain.rs:360`) resolves the DEM
for a drawn quad as *the tile itself, else `nearest_ancestor` (walk UP only)*; if neither
is resident it returns `None` → placeholder → flat. (`elevation_at_world`, the *CPU*
sampler at `terrain.rs:381`, already picks deepest-resident-first — so marker/path drape is
fine; only the *GPU mesh binding* downgrades/flattens.)

**Root flaw:** the system models the screen as *"the grid of tiles at the target zoom,
with fallbacks as a temporary patch during fade-in."* There is **no persistent notion of
"for each screen region, the best-resolution tile I currently own, retained until something
genuinely better replaces it."**

Mapping flaw → mechanism:

| Symptom | Mechanism |
|---|---|
| Inconsistent fade | `self_alpha` from `age_secs` (`raster.rs:564`) — provenance, not transition |
| Zoom-out downgrade | `visible_tiles()` is one LOD; deeper resident tiles leave the set and stop drawing; ancestor preferred over descendant (`raster.rs:552-557`) |
| 3D flatten | `bind_for` walks **up** only; no descendant fallback (`terrain.rs:360-367`) |
| Far gray cutaway | only `visible_tiles()` (+margin) ever drawn; no retained coverage beyond the ring |

## 3. Target architecture

Three principles, one idea — **coverage that monotonically improves; transitions driven by
coverage change, not by a clock:**

1. **Retained best-available coverage.** Maintain, per layer, a *render set*: for every
   screen region, the finest resident tile covering it. Ideal-LOD tiles join as they load;
   an older tile is retired **only once its region is fully covered by same-or-finer
   tiles.** Zoom-out keeps drawing deep tiles (GPU mip-minifies them — mipmaps already
   built, `cache.rs:184`); never downgrade. (This is Mapbox GL's `SourceCache.retain`.)
2. **Source-change crossfade.** Track the source currently displayed for each ideal cell;
   when it changes — cache *or* network, identically — crossfade old→new over the window.
   Provenance becomes irrelevant. Replaces age-based fade.
3. **Terrain LOD retention.** The mesh's DEM binding uses best-available (descendant **or**
   ancestor); flatten only if *no* covering DEM is resident at any level.

## 4. Data structures

### 4.1 `RenderTile` (new, `render/raster.rs`)
What to draw for one ideal cell, after resolution:

```rust
struct RenderTile {
    ideal: TileId,            // the target-LOD cell this covers
    source: TileId,           // resident tile actually sampled (== ideal, ancestor, or descendant)
    coverage: Coverage,       // Exact | Ancestor{sub_uv} | Descendants(Vec<TileId>)
    alpha: f32,               // crossfade alpha for `source` over its backdrop
}
```

### 4.2 Retain/crossfade state (new field on `RasterLayer`, `map.rs:267`)
Render-thread-owned (no locking — lives inside `OnScreen`'s engine):

```rust
struct LodState {
    // Per ideal cell: the source currently shown + when it became current.
    shown: HashMap<TileId, ShownSource>,   // keyed by ideal TileId
}
struct ShownSource { source: TileId, since: Instant, prev: Option<TileId> }
```

`shown` is pruned each frame to the current ideal set ∪ in-flight crossfades (bounded by
viewport tile count, ~tens of entries — not the cache).

### 4.3 Cache residency queries (already exist, `render/cache.rs`)
`get`/`peek` (residency), `nearest_ancestor` (walk up), `covered_descendants(region, depth)`
(walk down). **Gap to fill:** a cheap "is `id` resident?" `contains(id)` (peek-based, no LRU
bump) for the selection walk. `TileId::sub_uv_in` / `ancestor` exist (`tile.rs:39,52`).

## 5. The selection algorithm (Stage 1 core)

Replace the `for tile in scene.visible_tiles()` loop with a resolver that, per ideal cell,
returns the best-available coverage **and never downgrades vs what was shown last frame**:

```text
resolve_coverage(ideal, cache, prev_shown) -> RenderTile:
    if cache.contains(ideal):
        return Exact(ideal)                              # ideal resident — best case
    # ideal not resident: prefer FINER resident tiles we already own (retain detail)…
    desc = cache.covered_descendants(ideal, MAX_DOWN)    # deeper tiles covering this cell
    if desc fully cover ideal:                           # complete fine coverage
        return Descendants(desc)
    # …else fall to the nearest resident ancestor (coarse, mip-minified)…
    if let Some(anc) = cache.nearest_ancestor(ideal):
        base = Ancestor(anc, ideal.sub_uv_in(anc))
        # …and overlay whatever partial descendants exist on top of the coarse base.
        return base + partial(desc)
    # nothing resident anywhere → request only (host pump already does this); draw clear.
    return Empty

# Retain rule: if prev_shown[ideal].source was FINER than the resolved source,
# and that finer source is still resident, keep drawing it until `desc`/exact reaches
# equal-or-better coverage. (Prevents the zoom-out downgrade flash.)
```

Drawing order per cell: coarsest → finest (ancestor backdrop, then descendants/exact on
top), so the finest data wins where present and there is never a clear-colour hole if *any*
covering tile is resident.

## 6. Source-change crossfade (Stage 2)

Drop the `age_secs` alpha. Each frame, after `resolve_coverage`:

```text
cur = resolved.source
st  = shown[ideal]
if st.source != cur:                       # the displayed source CHANGED this frame
    st.prev = st.source; st.source = cur; st.since = now
alpha = smoothstep(clamp((now - st.since)/FADE, 0, 1))
draw st.prev (if resident) at 1.0          # old source as backdrop
draw cur     at alpha                       # new source crossfading in
```

This fades **every** change uniformly — cache swap and network arrival look identical — and
a no-change frame is `alpha == 1.0` (steady, no cost). `FADE == 0` (goldens) → instant, as
today. Crossfading *between two real tiles* (not over grey) removes the "pop".

## 7. Terrain retention (Stage 3)

`render/terrain.rs`:

- `bind_for(tile)` → resolve to best-available: `tile` if resident, else **deepest resident
  descendant** covering it, else `nearest_ancestor`, else placeholder. Add
  `nearest_resident(tile)` doing the down-then-up walk; shader already remaps via
  `source_tile` + `sub_uv_in`, so descendant binding needs the inverse sub-UV (descendant is
  *smaller* than the drawn quad — draw per-descendant sub-quad, same shape as raster
  `Descendants`).
- Flatten (placeholder) only when **no** covering DEM is resident at any level — never just
  because the target LOD is missing. The CPU `elevation_at_world` is already best-available;
  this brings the GPU mesh to parity.

## 8. Staging

Each stage compiles, passes `cargo test -p turbomap-core` + clippy `-D warnings`, and is
validated on the scenario harness before the next.

- **Stage 0 — Harness assertions (gating, no behaviour change).** Extend
  `turbomap-app/examples/scenario.rs`: during the zoom-in/out + 3D sweeps, assert per frame:
  (a) **no-downgrade** — the mean sampled LOD for the viewport never drops while the deeper
  tile is resident; (b) **no-flat-while-DEM-resident** — mesh relief variance > ε whenever
  any covering DEM tile is resident; (c) **no clear-colour holes** in the covered viewport.
  These *fail* on today's code (lock in the bug), proving the fix when they pass.
- **Stage 1 — Retain set + best-available coverage** (`raster.rs::prepare`, `scene.rs`
  helper, `cache.contains`). Kills zoom-out downgrade + far gray cutaway. Fade still
  age-based for now.
- **Stage 2 — Source-change crossfade** (`LodState` on `RasterLayer`, replace `age_secs`
  alpha). Kills pop + cache/network inconsistency.
- **Stage 3 — Terrain LOD retention** (`terrain.rs` `bind_for`/`nearest_resident` + per-
  descendant sub-quad). Kills 3D flatten-on-zoom.
- **Stage 4 — Validate + tune.** Run full harness sweep; confirm Stage-0 assertions pass;
  re-check device PERF (render-thread cost must stay ≈ current ~2–8 ms; the resolver is a
  small per-viewport-cell quadtree walk, not per-pixel). Tune `FADE`, `MAX_DOWN`.

## 9. Edge cases / risks

- **Overdraw.** Retaining descendants + ancestor + self can stack 2–3 layers per cell.
  Bound: cap `Descendants` depth (`MAX_DOWN = 2`) and skip the ancestor backdrop when
  descendants fully cover. Worst case ≈ 3× the visible-tile instance count (still a few
  hundred quads — `MAX_TILES_PER_FRAME` already 256).
- **Retain-set growth → cache pressure.** Retained tiles must stay resident to be drawn; the
  LRU budget (80 MB) already evicts. Resolver must treat an evicted retained tile as gone
  (re-resolve), and `Scene::un_ingest` already unmarks on eviction
  ([[tile-cache-coherence]]). `LodState.shown` prunes evicted sources.
- **Crossfade against a moving camera.** Alpha is per ideal cell, independent of pan, so a
  cell mid-fade that scrolls off is just pruned — no visual artifact.
- **Goldens.** `FADE == 0` keeps deterministic instant swaps; selection change may shift
  which tile is sampled in a few goldens → re-bless intentionally (document in the commit).
- **Terrain seams.** Per-descendant sub-quads must share the halo-trim UV math
  (`resolve_dem_subuv`) so adjacent LODs don't crack at edges — reuse the existing path.

## 10. Out of scope (separate follow-ups)

- Cache budget thrash at 80 MB (`evictions` climbing) — raise/auto-size the budget; tracked
  separately.
- Prefetch ring shaping for the far field (`prefetch_margin_px`) — orthogonal to *drawing*
  best-available; the retain set already removes the hard cutaway.

## 11. Validation summary

`cargo run -p turbomap-app --example scenario -- --center 67.23,15.30 --pitch 80` plus the
Stage-0 assertions must show: no LOD-downgrade frame on zoom-out, no flat-relief frame while
a covering DEM is resident, no clear-colour hole in the covered viewport, and every source
change crossfaded (no instant cache snap). Device re-check: render-thread ms unchanged,
backlog still bounded, panning still smooth.
