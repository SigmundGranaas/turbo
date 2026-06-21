# Terrain LOD + Horizon + Cascaded Shadows — demo → prototype

**Status:** proposed · **Date:** 2026-06-21 · **Scope:** `turbomap-core` camera +
tile selection (`scene.rs`) + terrain mesh/DEM + sky/aerial-perspective + shadows
(`render/{terrain,raster,sky,shadow}.rs`, `map.rs`), validated on the scenario
harness.

## 1. Problem (device-reported, reproduced in code)

Tilting toward the horizon ("panning down close to the ground") is broken:

1. **Clips everything in view.** At steep pitch the frustum footprint AABB
   degenerates (corners fall behind the horizon → clamped to world bounds), and
   the `MAX_TILES=160` trim shrinks to a radius window around the camera centre
   — which at steep pitch sits *outside* the on-screen area, so the kept tiles
   aren't what's visible. Result: mostly sky, a sliver of ground.
2. **Hard horizon cutaway.** Even when tiles do load, they stop at a clean line:
   the frustum reaches far toward the horizon but we only request ONE zoom level
   (`tile_zoom() = floor(zoom)`) across all of it → thousands of fine tiles →
   capped → the far field is simply dropped, and `far = altitude·100` clips the
   geometry beyond a fixed distance.
3. **No LOD.** Near and far are requested at the same resolution — the opposite
   of what a game terrain renderer does (fine near, coarse far).
4. **Shadows are a fixed near patch.** `SHADOW_DIM=96` over a single small
   `world_size`; distant peaks neither cast nor receive shadow, and the area
   doesn't follow the tilted view.

Root cause, stated once: **the terrain is drawn as a flat, single-LOD tile ring
with a near far-plane and no atmospheric horizon** — fine for a top-down 2D map,
wrong for a tilted 3D view. There is nothing to "replace" the far field with, so
it just stops.

## 2. Current architecture (the relevant seams)

- **Tile selection** — `scene.rs::tiles_for_margin_at(margin, z)` unprojects the
  4 viewport corners to the ground plane, takes their AABB at a SINGLE zoom `z`,
  clamps to world bounds, and trims to `MAX_TILES` via a camera-centred radius.
- **Camera** — `camera.rs`: perspective `FOV_Y`, `near = altitude·0.01`,
  `far = altitude·100`, flat ground plane (z=0). No curvature, no horizon-derived
  far plane.
- **Terrain** — one DEM zoom, displaced per-frame in the vertex shader; the
  Stage-1 best-available resolver already draws **mixed-zoom** basemap tiles
  (ancestor/descendant), so the renderer can already consume a mixed-LOD set.
- **Atmosphere** — Hosek–Wilkie sky dome + analytic aerial perspective (`haze_*`
  in `Globals`) already exist and are sun-lit; today the haze density folds
  altitude + a pitch ramp but isn't tied to a horizon dissolve.
- **Shadows** — `shadow.rs` CPU horizon-march over a fixed `SHADOW_DIM²` grid at
  one `world_size`, sun-direction from `sun.rs`.

## 3. Target architecture

A real-terrain pipeline: **screen-space-error quadtree LOD**, a **horizon-true
far plane with Earth curvature**, an **atmospheric dissolve** so the far edge
never just stops, and **cascaded shadows** that follow the view — all driven by
the one sun/atmosphere model.

### 3.1 Screen-space-error (SSE) quadtree LOD — the core
Replace single-zoom selection with a **quadtree refinement** over the visible
frustum: start from a coarse root tile covering the footprint; subdivide a tile
only while its **projected on-screen size exceeds a target** (e.g. one tile ≈
256–384 px). Near the camera → deep subdivision (fine z14); toward the horizon →
shallow (coarse z8–z10). This:
- **bounds tile count** (a few hundred) regardless of pitch — kills the
  MAX_TILES-trim starvation;
- **covers to the horizon** with cheap coarse tiles instead of dropping it;
- yields a **mixed-zoom set the Stage-1 resolver already renders** (no new draw
  path), and the per-kind lanes already fetch it sanely.
Applies identically to basemap raster AND the DEM heightmap (same quadtree;
coarse far DEM is cheap and already server-cached).

### 3.2 Horizon-true far plane + Earth curvature
- **Far plane** from eye height: the ground horizon distance is
  `d_h ≈ √(2·R·h)` (R = Earth radius in world units at this latitude, h = eye
  height). Set `far` to cover `d_h` (+ margin) instead of a flat `altitude·100`,
  so distant terrain isn't clipped before the horizon.
- **Curvature droop** in the terrain vertex shader: drop world-z by
  `Δz ≈ s²/(2R)` (s = ground distance from the camera). Distant ground curves
  *below* the eye line and meets the sky at the true horizon — no flat wall, and
  it caps how much far terrain is even visible (natural occlusion).

### 3.3 Atmospheric dissolve (never "just stop")
Ramp the existing aerial-perspective haze to **fully opaque at the far LOD/horizon
edge**, keyed on ground distance (not just altitude). The last coarse tiles fade
into the sky-matched haze colour, so the boundary is atmosphere, not a line. This
is the "fog / curvature / something" the user asked for — and it's lit by the
same sun (golden-hour horizon glow falls out for free).

### 3.4 Cascaded LOD shadows, view-following, sun-integrated
- Split the shadow march into **cascades**: a **near cascade** (current
  `SHADOW_DIM`, tight extent, crisp) + one or more **far cascades** (lower
  resolution, large extent) covering the tilted view toward the horizon.
- Far cascades march the **coarse far-DEM** from the LOD set (cheap), so distant
  peaks shadow the valleys without exploding the march cost.
- The cascade origins/extents **follow the LOD frustum**; all use the single
  `sun_dir` from `sun.rs`, so shadows, terrain shading, sky, and the horizon
  dissolve are mutually consistent at any time of day. (Builds on the existing
  worker-thread march; far cascades update less often.)

## 4. Staging (each compiles, harness-validated, committed)

- **Stage A — SSE quadtree LOD selection** (`scene.rs`, new `lod.rs`). Replace
  `tiles_for_margin_at` single-zoom + MAX_TILES with quadtree refinement →
  mixed-zoom raster + DEM set. *Fixes #1 clips-everything + #2 count/cutaway.*
  Gate: harness pan-down sweep — viewport covered to the footprint edge at every
  pitch, tile count bounded (< ~400), near cells fine / far cells coarse.
- **Stage B — Horizon far plane + curvature** (`camera.rs` far; terrain WGSL
  curvature droop). *Far terrain curves to the true horizon instead of clipping.*
  Gate: no geometry-clip line in the harness pitch-80 frames; far plane ≥ d_h.
- **Stage C — Atmospheric dissolve** (`raster.rs`/terrain WGSL haze ramp by
  ground distance to opaque at range). *No hard edge — terrain → haze → sky.*
  Gate: bottom-of-frame→horizon luma gradient is monotonic (no step) in harness.
- **Stage D — Cascaded LOD shadows** (`shadow.rs` cascades + `map.rs` wiring).
  *Distant relief shadows; area follows the view; far cascade low-res.* Gate:
  shadow coverage extends to the far cascade; near cascade unchanged; sun-time
  slider still drives all cascades; march cost bounded.
- **Stage E — Device validation + tune** the SSE target, curvature radius, haze
  ramp, cascade splits on the Pixel; re-check PERF (render ms, tile count).

## 5. Critical files

| Concern | Path |
|---|---|
| LOD selection (new) | `crates/turbomap-core/src/lod.rs` + `scene.rs` |
| Mixed-zoom draw (exists) | `render/raster.rs::resolve_cell` (Stage-1) |
| Far plane / curvature | `camera.rs` (`view_projection_from_origin`), terrain WGSL |
| DEM LOD | `render/terrain.rs` (`bind_for`, height grid) |
| Atmosphere dissolve | `render/raster.rs` Globals haze + shader |
| Cascaded shadows | `render/shadow.rs`, `map.rs::update_terrain_shadows` |
| Sun | `sun.rs` (single source for all of the above) |
| Validation | `turbomap-app/examples/scenario.rs` (pitch sweep + frame dumps) |

## 6. Verification

`cargo run -p turbomap-app --example scenario -- --center 67.23,15.30 --pitch 80`
must, after each stage: cover the viewport to its footprint edge (no sky-sliver),
show no hard horizon line, keep tile count bounded, and keep shadows extending
into the far cascade — with `cargo test -p turbomap-core` + `-p turbomap-sim
--features gpu-tests` green and clippy clean. Device pass on the Pixel closes it.

## 7. Out of scope (separate)
Vector-tile LOD (same idea, later); shared GPU device / surface recreate (#282);
the 2D (pitch=0) path stays byte-identical (the quadtree collapses to the legacy
single-level rectangle at pitch 0, so goldens hold).
