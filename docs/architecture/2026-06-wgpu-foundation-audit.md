# wgpu Foundation Audit — usage patterns, risks, and the hard choices

**Date:** 2026-06-09
**Companion to:** `2026-06-map-engine-architecture.md`, the implementation/test plan
**Why now:** before more is built on top, verify the renderer uses wgpu the way
the ecosystem supports it, surface the choices that get *more* expensive later,
and decide them. Findings are grounded in the code (file references) and in
measurements from the headless session simulator (`turbomap-sim`).

## Measured baseline (software rasteriser — worst-case "hardware")

Full basemap scene (raster land + water fills + cased roads + labels over
synthetic MVT), 640×400, zoom 11→13 journey, 3-frame tile latency, llvmpipe:

| Metric | Value |
| --- | --- |
| frame CPU p50 / p95 / max | **2.7 ms / 3.6 ms / 8.1 ms** |
| worst blank-screen coverage during zoom | **0.35 %** |
| tiles delivered over the journey | 370 |

On a CPU rasteriser the whole frame (driver + rasterisation) costs ~3 ms — the
engine-side CPU work is small. These are *relative* regression numbers (CI
budget caps in `turbomap-sim/tests/session.rs`), not mobile-GPU milliseconds.

---

## Decision 1 — Render-pass-per-layer must become one pass (the big one)

**Today:** `Map::render` (`map.rs:~940`) gives every layer its own
`begin_render_pass` — raster, each vector layer, hillshade, then text and
markers (`render/vector.rs:267`, `render/raster.rs:560/616`, …). Later passes
use `LoadOp::Load`.

**Why it matters:** desktop GPUs shrug at this. Tile-based mobile GPUs
(Adreno/Mali/Apple — i.e. *every deployment target*) pay a full
framebuffer load/store round-trip to memory per pass. The simulator's basemap
is already 7+ passes/frame; at 1080p that is the single largest avoidable
bandwidth cost on a phone, and the kind of thing that shows up as device
battery drain no headless test will ever see.

**Choice made:** consolidate to **one render pass per frame**: clear once,
then `set_pipeline` switches between raster/vector/hillshade/text/marker
pipelines inside the pass. This is the standard wgpu pattern (pipelines
already share one target format). It is a mechanical but cross-cutting
refactor of each pipeline's `render()` (take a `&mut RenderPass` instead of
the encoder) — **do it before the surface glue**, because it touches every
pipeline signature and is far cheaper to validate headless (the goldens prove
pixel-equivalence) than on devices.

**Consequences:** the per-layer `LoadOp::Clear` logic moves to the frame
level; depth attach unifies; GPU timestamp scopes move from per-pass to
per-pipeline ranges.

## Decision 2 — wgpu version policy: upgrade *before* the platform glue

**Today:** pinned to `wgpu = "22"` (workspace), with egui pinned in lockstep
for the desktop demo. wgpu has since moved several majors; known breaking
renames on our exact API surface: `ImageCopyTexture`/`ImageCopyBuffer` →
`TexelCopy*Info`, `Maintain` → `PollType`, render-pass lifetime removal,
`Instance::new(&desc)`, and — critically — **the surface-creation APIs**
(`SurfaceTargetUnsafe`, raw-window-handle integration) that the Android/iOS
glue will be written against.

**Choice made:** upgrade the workspace to the current wgpu **before writing
any surface glue**. Writing platform glue against 22 and migrating after
doubles the device-validation cost (the riskiest, least-automatable test
surface). The headless suites (goldens + sessions + conformance) are exactly
the safety net that makes the upgrade cheap *now* — that is what they're for.
The desktop demo's egui pins may lag; the demo is not on the critical path
(`turbomap-app` is already excluded from the deny-warnings gate).

## Decision 3 — Colour management contract (decided & fixed this session)

The simulator's pixel histogram caught vector/text/marker colours being fed
to the GPU as sRGB bytes and treated as linear — washing out everything
darker than white (authored `(70,70,80)` rendered `(143,143,152)`).

**Contract now in code** (`style.rs::Color::to_linear_f32/to_linear_bytes`):
colours are *authored* in sRGB, decoded **exactly once** at the GPU boundary
(vertex bake, text/marker instances, paint overrides, backgrounds), blended
in linear, re-encoded by the `*Srgb` target. Verified pixel-exact by
histogram: authored == rendered.

**Open sub-decision (Android):** GLES surfaces don't always offer an
`*Srgb` surface format. Policy: prefer an sRGB surface format; where
unavailable, render to an sRGB intermediate texture and blit. The offscreen
path (`render_png`, goldens) already uses `Rgba8UnormSrgb` everywhere.

## Decision 4 — Per-draw data: keep dynamic-offset uniforms

The per-tile uniform (256-byte stride, dynamic offsets, `vector.rs:16`) is a
fully supported, downlevel-safe pattern (works on WebGL2/GLES, unlike push
constants). Utilisation is 32 of 256 bytes — headroom for the rest of the
GPU-paint roadmap (width, opacity ramps) without layout changes. Instanced
attributes remain the escape hatch if per-draw data ever outgrows this.
**No change.**

## Decision 5 — Text: SDF atlas is the endgame; halos decide the timing

Today's text pipeline rasterises glyphs per size into a single atlas and does
per-frame layout + collision (`render/text.rs:55`). Fine at current label
counts (sessions confirm). But the two roadmap features — **halos** (the
single most visible "real map" text feature) and crisp text under continuous
zoom — both point at a signed-distance-field atlas (halo = distance
threshold, zoom = free). Quad-offset halos (draw 4–8 shifted copies) would
work tomorrow but quintuple label geometry and get thrown away by SDF.

**Choice made:** go directly to SDF when text is next touched; don't build
the throwaway halo. (Non-Latin shaping remains explicitly out of scope.)

## Confirmed-sound patterns (no action)

- **CPU tessellation (lyon) + GPU draw** — industry standard for vector maps;
  per-tile clip keeps it ~9× cheaper than naive (criterion bench).
- **Pull/push tile IO** with host-owned fetch — matches wgpu's threading
  model; no IO on the render thread anywhere.
- **LRU VRAM budgets** for textures and meshes; mip-mapped raster uploads.
- **Line AA via `fwidth` smoothstep** instead of MSAA — the cheap, standard
  map approach (fill edges stay un-AA'd; acceptable, revisit with SDF work).
- **Blocking readback loops** exist only in test/snapshot harnesses; the
  production path presents and never reads back.
- **Depth texture** recreated on resize only; `Rgba8UnormSrgb` target
  everywhere offscreen.

## Risk register (ranked)

| # | Risk | Mitigation |
| --- | --- | --- |
| 1 | Pass-per-layer bandwidth on TBDR GPUs | Decision 1; goldens prove pixel-equivalence of the refactor |
| 2 | wgpu 22 → current migration colliding with platform glue | Decision 2: upgrade first, behind the full headless suite |
| 3 | Surface formats on Android GLES (sRGB) | Decision 3 fallback (intermediate sRGB texture + blit) |
| 4 | Text layout cost at high label density | Sessions already measure it; SDF work includes layout caching |
| 5 | Absolute mobile perf unknowable headless | Accepted: sim numbers are relative budgets; first device session re-baselines them |
