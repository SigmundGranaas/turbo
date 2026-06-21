# Terrain LOD / Horizon / Shadows — Implementation Plan (TDD, risk-ordered)

Companion to `2026-06-terrain-lod-horizon-shadows.md` (the design). This is the
*build* plan: hardest/riskiest parts first, every change driven test-first, and
"real" wherever possible — pure logic as fast unit tests, everything visual on
the **scenario harness against the real Bodø/Sjunkhatten DEM** with framebuffer
assertions (not mocks, not synthetic).

## Guiding rules
- **TDD:** write the failing test first (it must fail for the right reason),
  then implement to green. No production line lands without a test that would
  catch its regression.
- **Real over mocked:** algorithmic cores (LOD selection, curvature, skirt
  geometry) → pure unit tests with real camera math. Anything that draws → the
  `scenario.rs` harness with the real DEM over HTTP, reading the framebuffer back
  and asserting pixel-level invariants. The harness already does readback +
  PNG dump + `catch_unwind` + NaN-guards; we extend its assertions.
- **Risk first:** the two coupled hard parts (SSE selection + crack-free mixed
  LOD) land together in Phase 1, because either alone is useless or broken.

## Risk register (hardest first)
- **H1 — SSE quadtree selection correctness.** Must be deterministic, bounded in
  count at any pitch, cover to the footprint edge, fine-near/coarse-far, and
  collapse to today's single-level rectangle at pitch 0 (goldens). Pure logic →
  fully unit-testable. *Mitigation: TDD the algorithm in isolation behind an
  `is_resident`-style predicate + camera, no GPU.*
- **H2 — LOD-seam T-junction cracks.** Mixed zoom = adjacent edges with
  different vertex counts → gaps that show sky. *Mitigation: per-tile **skirts**
  (a downward apron around each tile, depth ≥ max plausible neighbour height
  delta). Skirts hide cracks regardless of neighbour LOD — no neighbour-aware
  stitching needed. Test: a geometry unit test (apron depth ≥ delta) + a harness
  "no sky pixel between two ground tiles" probe.*
- **H3 — Far-plane / curvature precision.** Big distances in the RTC f32 frame;
  `vp.inverse()` near-singular at pitch 80. *Mitigation: unit-test the curvature
  drop + far-from-eye-height formulas; reuse the existing finite-sanitize gate;
  harness asserts the matrix stays finite + no clip line at pitch 80.*
- **H4 — Shadow cascade cost.** Widening the march can blow the frame budget.
  *Mitigation: far cascades are low-res over coarse DEM, updated less often;
  harness profiles march ms; bounded by design.*
- **H5 — Horizon dissolve seam.** A visible step where tiles meet haze.
  *Mitigation: harness gradient-monotonicity probe bottom→horizon.*

## Phase 0 — Test infrastructure (do first; no behaviour change)
Build the "real testing" toolkit so every later phase is gated.

**0a. Pure-logic scaffold** — `crates/turbomap-core/src/lod.rs` with the
selection signature stubbed (`todo!()`) + its unit-test module. Lets Phase 1
start red.

**0b. Harness framebuffer probes** (`scenario.rs`), each a pure fn over the
readback `RgbaImage` returning a number a test asserts:
- `viewport_coverage(img)` → fraction of the *lower* frame that is ground (not
  sky/haze clear). Detects the sky-sliver bug (#1).
- `horizon_gradient_monotonic(img)` → scanning bottom→top, luma must not *step*
  (Δ over a threshold between adjacent rows) in the ground band. Detects the hard
  cutaway line (#2/H5).
- `sky_holes_between_ground(img)` → count sky-coloured pixels enclosed by ground
  on ≥3 sides. Detects LOD cracks (H2).
- `lod_pyramid(scene)` → histogram of selected tile zooms + total count (from the
  selection API, not pixels). Asserts fine-near/coarse-far + bounded count (H1).
- Reuse existing `render_and_readback` + the shadow luma diff.

**0c. A dedicated harness mode** `--probe lod` that runs the pitch sweep
(0→80°) and emits the probe numbers as machine-readable lines, so a
`turbomap-sim` gpu-test can assert them headlessly (the real gate).

Gate for Phase 0: probes compile + run on the *current* build and **fail the
right way** (coverage low + cutaway present at pitch 80) — locking the bugs.

## Phase 1 — SSE quadtree LOD + skirts (the keystone, coupled)
Land selection and crack-free meshing together.

**Tests first (red):**
- `lod::tests` (pure): `refines_near_to_target_sse`, `coarsens_toward_horizon`,
  `bounded_count_at_pitch_80` (< 400), `collapses_to_single_level_at_pitch_0`
  (== today's rectangle, exact), `covers_to_footprint_edge`, deterministic order.
- `skirt::tests` (pure geometry): `apron_depth_covers_max_edge_delta`,
  `skirt_ring_is_watertight_against_neighbor_level`.
- Harness gate: `lod_pyramid` bounded + layered; `sky_holes_between_ground == 0`
  across the pitch sweep on the real DEM.

**Implement:**
- `lod.rs::select(camera, viewport, source_zoom_range) -> Vec<LodTile>` —
  quadtree refine from a frustum-covering root; subdivide while projected tile
  span > `SSE_TARGET_PX`; stop at source max-z or when off-frustum. `LodTile { id,
  // covers a screen-space-bounded cell }`. Wire `scene.rs::desired_tiles` /
  `pending_tiles` to it (raster + DEM share the walk). The Stage-1 resolver
  already draws mixed-zoom, and the per-kind lanes already fetch it.
- **Skirts** in the terrain mesh (`raster.rs` grid build): add an apron ring of
  vertices around the 17×17 grid pulled downward by a per-tile skirt depth
  (uniform/instance field), so seams between LOD levels are filled. Vertex shader
  drops apron verts by `skirt_depth · meters_to_world`.

Result: #1 (clips-everything) and the count half of #2 fixed; no cracks.

## Phase 2 — Horizon far-plane + Earth curvature
**Tests first:**
- `camera::tests`: `far_plane_reaches_ground_horizon_for_eye_height`,
  `curvature_drop_matches_d_squared_over_2r`, `vp_finite_at_pitch_80_after_curvature`.
- Harness: `horizon_gradient_monotonic` shows no geometry-clip step; far ground
  present out to the horizon at pitch 80.

**Implement:** `camera.rs` far = `max(altitude·k, horizon_dist(eye_h)·margin)`;
add a `curvature_radius_world` to terrain `Globals`; terrain WGSL drops world-z
by `s²/(2R)` (s = ground distance from camera centre, RTC-safe). Reuse the
existing `Camera::sanitized` finite gate.

## Phase 3 — Atmospheric dissolve to the horizon
**Tests first:** harness `horizon_gradient_monotonic` must pass *with the dissolve
on* (terrain→haze→sky, no step); a unit test that haze α → 1 at the far edge.
**Implement:** extend aerial-perspective `haze_density` to ramp by **ground
distance** to fully opaque at the far LOD/horizon edge (sky-matched colour). Lit
by the existing sun model.

## Phase 4 — Cascaded LOD shadows, sun-integrated
**Tests first:**
- `shadow::tests` (pure march on a synthetic ridge): `far_cascade_low_res_still_casts`,
  `cascade_split_covers_view`, `march_cost_bounded`.
- Harness: shadow luma diff extends into the far cascade; near cascade unchanged;
  sun-time sweep drives all cascades; march ms within budget (profiled).
**Implement:** `shadow.rs` near + far cascade(s); far marches coarse far-DEM from
the LOD set; origins/extents follow the LOD frustum; one `sun_dir`; far cascades
coalesced/throttled on the worker thread.

## Phase 5 — Device validation + tune
On the Pixel: tune `SSE_TARGET_PX`, curvature radius, haze ramp, cascade splits;
re-run `tile_profiler` + PERF (render ms, tile count, march ms). Commit/deploy
per stage as gates pass.

## Acceptance matrix (issue → the test that proves it)
| Reported issue | Gate |
|---|---|
| Pan-down clips everything | `viewport_coverage` high at pitch 80 (harness) |
| Hard horizon cutaway line | `horizon_gradient_monotonic` (harness, Ph2+3) |
| Tiles stop loading, no LOD | `lod_pyramid` bounded + layered (unit + harness) |
| Cracks from mixed LOD | `sky_holes_between_ground == 0` + skirt geom unit test |
| Shadows too small an area | shadow luma extends to far cascade (harness, Ph4) |
| 2D path must not regress | `collapses_to_single_level_at_pitch_0` exact + goldens |

## Per-phase definition of done
Compiles; new tests green; `cargo test -p turbomap-core` + `-p turbomap-sim
--features gpu-tests` green; clippy `-D warnings`; harness pitch-80 frame dump
visually inspected + its probes green; device smoke pass. Commit + push the phase.
