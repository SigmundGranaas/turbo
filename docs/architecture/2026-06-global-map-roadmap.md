# Global-Map Roadmap (turbomap)

_Status: planning → in progress. Authored 2026-06-10._

This document plans the work to take the wgpu/Rust renderer in `apps/turbomap`
from "a well-architected 2D vector basemap that renders a synthetic world on
software Vulkan" to a credible global map. It is grounded in the current code,
not aspiration: each item names where it plugs into the existing architecture.

## Standing prerequisite: real-device validation

Nothing below is _validated_ until the engine runs on a real Android/iOS GPU.
Today everything is exercised headless on Lavapipe (software Vulkan) via golden
images + the `turbomap-sim` device-equivalent session harness. That proves
**correctness**, not performance or driver behaviour. Device validation is a
standing gate that should run in parallel with Phase 1.

## Where things actually stand (corrected)

A prior review under-counted the tile stack. The real state:

- **Tile fetch + cache exist.** `turbomap-tiles-http` (`reqwest::blocking`,
  URL templating, 15s timeout) and `turbomap-tiles-cache`
  (`DiskCachedSource<S>`, on-disk `<root>/<z>/<x>/<y>`, atomic `.tmp` writes)
  are real. `MapHost` prioritises by distance, caps in-flight per layer (16),
  and has an 80s `recently_failed` retry window.
- **Camera is full-perspective already.** `Camera::view_projection()`
  (`turbomap-core/src/camera.rs`) builds a `glam` `Mat4` with tilt (0–60°),
  bearing, and zoom; Web-Mercator world is normalised `[0,1]²` (`geo.rs`).
- **Styling compiles per frame.** `Layer::{Fill,Line,Symbol}` → core
  `VectorStyle` rules in `turbomap-engine/src/engine.rs`; colour/width are
  re-evaluated per frame for live zoom curves without re-tessellation.
- **Text** is `ab_glyph` + a single bundled Roboto, atlas keyed by `char`,
  SDF (8SSEDT). Latin-only.
- **Render** is a single pass: geometry (raster → hillshade → vector) → icons →
  text → markers, `sample_count: 1` everywhere (`turbomap-core/src/map.rs`,
  `render/`).

So the gaps are schema/conditional-fetch/prefetch (not "no HTTP"), and text is
the true disqualifier for a _global_ map.

## Workstreams

### A. Global text — the hardest blocker

`text.rs` iterates `text.chars()` and keys the atlas by `char`: no shaping,
bidi, or fallback. Plan (replace the layout front-end, keep the SDF atlas):

1. **Shaping** via `rustybuzz` (pure-Rust HarfBuzz). Shape runs into positioned
   `glyph_id`s; replace the `chars()` loop in `layout()` / `layout_along_path()`.
2. **Font stack + fallback** via `fontdb` (or a hand-rolled `FontStack`).
   Itemise a string into runs by script/coverage; shape each with the covering
   face. Default faces bundled (Latin + Arabic + Devanagari); CJK and emoji are
   large, so design fonts to be **host-providable** over the uniffi boundary
   rather than all bundled.
3. **Bidi** via `unicode-bidi` (reorder RTL before shaping) +
   `unicode-script`/`unicode-segmentation` for itemisation and grapheme-safe
   breaks.
4. **Atlas rekey**: `FontAtlas` key `char` → `(font_id, glyph_id)`. This is the
   load-bearing change; SDF generation, shelf packing, `LayoutGlyph`, the GPU
   pipeline (`render/text.rs`, `text_shader.wgsl`) are unchanged.

Effort: Large. Risk: Medium (shaping × along-line placement). Test: golden
frames for Arabic (RTL), Devanagari (reordering), CJK (host font), mixed bidi.

### B. Real data + tile-stack hardening

**B1 — Schema & style spec.** Target **OpenMapTiles** schema as canonical
(`water/landcover/landuse/transportation/transportation_name/building/
boundary/place/poi/...`). Add **z-ordering + bridge/tunnel (`brunnel`)
ordering** via an explicit sort key on `Rule`. Author a MapLibre-GL-style-JSON
loader into the Scene IR (the IR's `Paint<T>` is already close to data/zoom
expressions).

**B2 — Tile-stack hardening** (`turbomap-tiles-http`, `-cache`, `MapHost`):
conditional requests (ETag/Cache-Control), disk-cache LRU eviction, exponential
backoff + jitter (replace the fixed 80s window, honour `Retry-After`),
**PMTiles** (range requests) and/or **MBTiles** (sqlite) sources for offline,
viewport+next-zoom **prefetch**, and a host-implementable `TileSource` over
uniffi.

Effort: Large. Risk: Medium. Test: real OpenMapTiles/PMTiles extract → golden;
cache hit/miss/expiry + backoff unit tests; sim prefetch coverage.

### C. Cartographic polish

- **C1 SDF sprites + real icon set** — load a MapLibre sprite sheet (PNG+JSON)
  and/or generate SDF icons (reuse 8SSEDT) for crisp scaling + tinting + @2x;
  replace procedural `sprite.rs`. `IconPipeline` gains a tint uniform + SDF mode.
- **C2 cross-tile label dedup** — dedup by feature id (text + approx world pos)
  across visible tiles; repeat-distance for line labels. Lands in
  `render/text.rs::prepare` + a stable id from `tessellate.rs`. (Fixes the
  doubled "RINGVEGEN".)
- **C3 line dashes / gradients / pattern & image fills** — add cumulative
  arc-length to `VectorVertex`; `dash_pattern` on `Paint::Line`; ramp-texture
  line gradients (`line-progress`); pattern atlas + tile/world UVs for fills.

Effort: Medium/item, independent. Risk: Low–Medium.

### D. Motion & anti-aliasing

- **D1 label fade** — persistent label ids + per-label opacity state in
  `TextPipeline`; ~150ms in/out transitions (needs C2's stable ids + per-frame
  `dt`).
- **D2 MSAA** — 4× multisampled colour target + resolve in the single pass;
  flip `sample_count` 1→4. SDF text/icons composite over the resolve. FXAA/SMAA
  fallback if mobile bandwidth hurts (decide after device numbers).

### E. Camera & interaction

- **E1 gestures** — recognizer for inertia/fling (velocity + decay via
  `ease_to`), pinch-zoom (`zoom_around` exists), two-finger rotate (`bearing`),
  vertical-drag tilt (`pitch`). Put it in a shared controller fed by both the
  egui app and the uniffi device host. Camera already supports the DOF; nothing
  drives them.
- **E2 globe** — `ProjectionMode { Mercator, Globe }` branch in
  `Camera::view_projection()`; MapLibre-style mercator→globe vertex transform in
  the ground shaders with a zoom transition. Touches every ground/vertex shader.

## Sequence & rationale

| Phase | Items | Why |
|------|-------|-----|
| 0 (standing) | Real-device validation | Gates every quality claim. |
| 1 | **A global text** + **B data/tiles** | The two true disqualifiers; parallelizable. |
| 2 | **C2 dedup**, **C1 sprites**, **C3 dashes/gradients/fills** | Real data makes dedup urgent and exposes missing paint. |
| 3 | **D1 fade**, **E1 gestures**, **D2 MSAA** | Polish + feel; gestures ride with device work. |
| 4 | **E2 globe** | Largest surgery, least near-term value. |

Top risks: shaping × along-line placement (A); schema z-ordering correctness at
scale (B1); MSAA bandwidth on mobile (D2). All three de-risked by early device
validation.

## Execution notes

- Keep the established discipline: every increment verified headless (golden +
  sim), committed and pushed atomically (container resets lose uncommitted
  work), goldens regenerated only for intentional, reviewed visual changes.
- Prefer host-providable assets (fonts, sprites, tiles) over bundling large
  binaries, exposed across the uniffi boundary.

## Progress log

- _2026-06-10_: Roadmap authored.
- _2026-06-10_: **A1** — glyph-id atlas + font-stack fallback (Latin goldens
  byte-identical; CJK unit test).
- _2026-06-10_: **A2** — host fallback-font registration end-to-end
  (`TurbomapEngine::add_fallback_font`); `cjk-labels-fallback` golden (東京/大阪).
- _2026-06-10_: **A3** — complex shaping + bidi via `rustybuzz` +
  `unicode-bidi`. `FontAtlas::shape` returns visually-ordered shaped glyphs;
  `complex-scripts-shaping` golden proves Arabic (RTL+joining), Devanagari
  (reordering), and mixed Latin+Arabic bidi. All prior goldens unchanged.
  **Workstream A (global text) substantively complete** for shaped, fallback,
  bidi-correct rendering; remaining niceties: vertical CJK, emoji (colour),
  bundling default CJK/Arabic faces vs. host-provided.
- _2026-06-10_: **C2** — cross-tile label dedup. Line labels honour a
  same-name repeat distance (`LINE_LABEL_REPEAT_PX` = 250px), collapsing a
  road clipped across tiles to one along-line label (fixes the doubled
  "RINGVEGEN"); `road-name-along-line` golden regenerated to the single label.
- _2026-06-10_: **C3 (dashes)** — `Layer::Line.dash_array` (`[dash, gap]` px).
  `VectorVertex` carries world arc length (lyon `advancement`); the shader
  scales it by pixels-per-world and drops gap fragments, so dashes are
  pixel-constant across zoom. `dashed-line` golden (10 dashes); all solid-line
  goldens byte-identical (dist unused when not dashed). Gradients + pattern
  fills remain in C3.
- _Recovery note_: a container reset rewound the local clone to an old commit
  mid-session; all work was safe on origin and restored by fast-forward — the
  commit-and-push-each-increment discipline held.
- _2026-06-10_: **C1 (SDF icons)** — sprite atlas is now a single-channel
  SDF (8SSEDT) of monochrome shapes (dot/stop/marker/shield); the icon
  pipeline thresholds the field for crisp edges at any size and multiplies by
  a per-layer `icon-color` tint. `Symbol.icon_color` (Paint<Color>) threads
  scene→engine→IconSpec→IconRequest→instance. `icons-and-shields` golden
  regenerated (tinted SDF dot + blue shield with white ref); other goldens
  unchanged. Remaining C1: load a real MapLibre sprite sheet (RGBA) for
  multi-colour POIs, @2x, data-driven per-feature icon colour.
- _2026-06-10_: **D2 (MSAA)** — the single frame pass is now 4× multisampled
  (shared `MSAA_SAMPLES` + `multisample_state()`; depth + a colour target at
  4×, resolved to the surface). Smooths the geometry edges without shader AA
  (polygon fills); `msaa-diamond` golden proves antialiased diagonal edges.
  All prior goldens stayed within perceptual tolerance — none regenerated.
  One switch flips it for device-perf tuning.
- _Recovery note_: a SECOND container reset rewound local mid-session; again
  recovered by fast-forward from origin with zero loss.
- _2026-06-10_: **B decided + first slice shipped.** Decision (after the
  tile-data analysis doc): **OpenMapTiles schema**, PMTiles packaging.
  Landed: PMTiles `RangeReader` (file / in-memory / **HTTP-range** backends —
  one archive serves offline and serverless-online), a minimal v3 archive
  **writer**, and the first **real-data golden**: an OMT-schema fixture
  (water/landuse/transportation/boundary/place with class+rank) packed into
  a PMTiles archive and rendered through the engine (`omt-pmtiles-bergen`).
  Remaining in B: tile-stack hardening (ETag/backoff/LRU/prefetch), z-order +
  brunnel, host source over uniffi, GL style-JSON loader (later).
- _2026-06-10_: **Real-data milestone + foundational precision fix.** The
  user's visual critique of the synthetic golden led to fetching real OMT
  tiles (OpenFreeMap, central Bergen, slimmed to a 1.2 MiB committed
  PMTiles fixture) — which exposed that absolute-world-coordinate f32
  tessellation collapses z14 street geometry into ULPs. **Meshes are now
  tessellated tile-locally** ([0,1] across the tile, constant half-pixel
  tolerance) and placed on the GPU via per-tile origin/span in the tile
  uniform — full precision at every zoom; all low-zoom goldens unchanged.
  New `omt-real-bergen` golden renders real Bergen with a designed style
  (casings, class hierarchy, dashed paths, along-line street names).
- _Next_: B hardening, or C3 gradients/pattern fills, or E1 gestures.
