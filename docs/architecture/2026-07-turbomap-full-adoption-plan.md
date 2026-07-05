# Turbomap Full-Adoption Plan — Phase 6: every host, every feature, one plane

**Status:** plan · **Date:** 2026-07-05
**Companion to:** `2026-07-turbomap-decima-inspired-engine-architecture.md`
(the design + Decision Record) and
`2026-07-turbomap-engine-implementation-plan.md` (Phases 0–5, all landed on
`claude/mapping-engine-architecture-nov1k1`).

## 0. Why this phase exists

Phases 0–5 evolved the **engine** completely: one streaming authority, one
content plane in the IR, one environment, a frame graph, enforced
observability. But an audit of the hosts (2026-07-05) shows the **adoption**
story is uneven — the IR can express almost everything the product does, and
most hosts don't use it:

1. The **web app** emits only raster + DEM + environment; routes/tracks,
   marker pins, and the user-location dot are DOM/SVG projected per animation
   frame (`useProjectedLayer.ts` drives all three) — map content that never
   enters the frame graph.
2. The **desktop app** never applies a Scene at all: it drives core `Map`
   imperatively through `TurbomapEngine::map_mut()`
   (markers, camera) and pushes cloud state into the Map **every frame**
   (`turbomap-app/src/app.rs::apply_clouds`) — the exact pattern C2/E2/P5.2
   deleted everywhere else. `turbomap-style-maplibre` feeds core
   `VectorStyle` directly, a private style side-channel.
3. The **sim harness** still authors content through three `#[doc(hidden)]`
   engine verbs (`set_sun_position`, `set_terrain_shadows`,
   `set_clouds_visible`; 8 call sites) that the shipped IR already expresses
   (`lighting: fixed`, `terrain-shadows`, `clouds.visible`).
4. **Android offline** is a second, parallel streaming stack: `TileStore.kt`
   (host-side read-through disk store) + `OfflineDownloadService` duplicate
   the engine's B5 disk cache and B6.2 provider chain, invisible to engine
   budgets, trace, and eviction.
5. **Hit-testing** exists on core (`Map::hit_test`) and uniffi, but not on
   the two production bindings (JNI surface, wasm); hosts compensate with
   DOM/Compose picking on their side-door pins.
6. The **overlay compositing track** (C3's documented honesty debt): tubes,
   circles, markers, icons, text draw in fixed overlay passes; the IR's
   layer order is only honored within the positional stack.
7. **Enforcement is manual**: the invariant grep-gates were run by hand;
   CI triggers only on main/PRs and runs plain `cargo test` — zero GPU
   suites, zero sim gates, no wasm check. The full verification ladder
   exists only as in-container session runs recorded in the progress log.

Phase 6 closes all seven **using only existing features and content** — no
new datasets, no new subsystems, no gated milestones. The goal is the owner's
standing directive taken to completion: a full evolution, not a modernized
core with legacy hosts around it. When P6 is done, invariant 7 ("one config
channel, no side doors") holds with **zero exceptions**, and every claim is
enforced by CI, not by discipline.

## 1. Ground rules (unchanged law, restated)

1. **Parity-first.** Any slice touching the render path proves
   pixel/behaviour equivalence (goldens compare-mode, conformance, sim gates)
   before changing behaviour.
2. **Commit and push every increment.** Container resets have rewound this
   clone three times now; origin is the only durable state.
3. **Gates are numbers, property tests, or greps** — never vibes.
4. **Kotlin lands compile-unverified in this container** (no Android SDK) and
   is flagged per commit for the on-device agent; the P6.6 slice is
   explicitly device-gated.
5. **Nothing new.** A slice that needs content or data the repo doesn't
   already have is out of scope for this phase (see §9).

## 2. Slice map

| # | Slice | Size | Depends on |
| --- | --- | --- | --- |
| P6.0 | Enforcement: CI ladder + invariant grep-gates | M | — |
| P6.1 | Sim/golden harness onto the IR; last hidden verbs deleted | S | — |
| P6.2 | Desktop app becomes a Scene host; `map_mut` demoted | M | P6.1 |
| P6.3 | Web content plane: routes/pins/location as scene layers | L | — |
| P6.4 | Hit-testing on the production bindings; hosts adopt | S/M | — (P6.3 taps consume it) |
| P6.5 | Compositing honesty: overlay track honors IR order | M/L | P6.3 helps exercise |
| P6.6 | Android offline onto the provider chain | M | device session |

Critical path: **P6.0 first or alongside everything** (it is what makes the
rest stick), then P6.1 → P6.2 → P6.3, with P6.4 parallel to P6.3 and P6.5
after it. P6.6 runs whenever a device/SDK session is available.

---

## 3. P6.0 — Enforcement: the ladder becomes CI (M)

**Goal:** the verification that Phases 0–5 ran by hand runs on every push,
and the architecture's grep-enforceable invariants are tests.

**Changes**
- `.github/workflows/turbomap_build.yml` (or a sibling `turbomap_gpu.yml`):
  - trigger on this branch (and PRs to main) — today none of the 55 landed
    commits have ever run CI, because every workflow is main-only;
  - a GPU lane: install `mesa-vulkan-drivers` (Lavapipe) + `protobuf-compiler`,
    run `REQUIRE_GPU=1 cargo test` with
    `--features turbomap-engine/gpu-tests,turbomap-golden/gpu-tests,turbomap-ffi/gpu-tests`
    (the suites are feature-gated; plain `cargo test` runs zero of them —
    this exact silent-skip burned two sessions);
  - a wasm lane: `cargo check -p turbomap-web --target wasm32-unknown-unknown`
    (compile-clean is necessary but NOT sufficient on wasm — the D1
    `std::time::Instant` panic compiled fine — so the web lane's Playwright
    boot smoke stays the runtime gate);
  - the sim behavioural gates (release, ~10–13 min build + ~600–760 s run):
    nightly schedule + `workflow_dispatch`, not per-push.
- New `apps/turbomap/crates/turbomap-engine/tests/invariants.rs` (plain
  `cargo test`, no GPU): walks the workspace source and asserts the
  grep-gates —
  - `pending_tiles` → 0 hits outside `turbomap-world` internals (P5.1's
    manual gate, made permanent);
  - `image::load_from_memory` / `turbomap_mvt::decode` only under
    `codec.rs`/the worker pool (invariant 10);
  - `DemEncoding` only in codecs (D3's gate, made permanent);
  - slots for the gates P6.1/P6.2 add (`doc(hidden)` content verbs = 0,
    `map_mut` = 0 outside the engine crate).

**Gates / done:** the new lanes are green on this branch; deleting one
guarded line (e.g. reintroducing a `pending_tiles` call) fails `cargo test`
locally, not just CI.

---

## 4. P6.1 — The sim harness authors scenes, not setters (S)

**Goal:** invariant 7 becomes total. The last imperative content verbs die.

**Changes**
- `turbomap-sim/src/session.rs` (3 call sites) +
  `turbomap-sim/tests/session.rs` (5 call sites): replace
  `set_sun_position` → `environment.lighting: {mode: fixed, azimuth_deg,
  altitude_deg}`, `set_terrain_shadows` → `environment.terrain-shadows`,
  `set_clouds_visible` → `clouds.visible` — all already in the shipped IR;
  the scenarios apply scenes like any host.
- `turbomap-engine/src/engine.rs`: delete the three `#[doc(hidden)]` verbs
  (`set_sun_position`, `set_terrain_shadows`, `set_clouds_visible`).
- `invariants.rs`: add the gate — zero `doc(hidden)` content setters on
  `TurbomapEngine`.

**Gates / done:** 8/8 sim gates green on the scene-declared path
(`REQUIRE_GPU=1`, release); every golden byte-stable; the grep-gate holds.

---

## 5. P6.2 — The desktop app becomes an ordinary Scene host (M)

**Goal:** the last imperative host adopts the IR; the `map_mut()` structural
side-door closes. After this slice, core `Map`'s imperative API has exactly
two callers: the engine's own reconcile plane, and the golden/test harnesses
inside the workspace.

**Changes**
- `turbomap-app/src/app.rs`:
  - build one Scene document (sources, layers, environment, clouds) and
    `apply()` it through `TurbomapEngine`; mutations (style toggle, clouds
    on/off, time scrub) rebuild + re-apply, exactly like the Android
    controller after P5.2;
  - delete `apply_clouds()`'s per-frame push — clouds become scene-declared
    (`CloudsDef` + `field-2d` source) with frame data through the transport
    verb (`ingest_field`), the clock through `set_cloud_time`;
  - markers via IR `circle`/`symbol` layers over a `geo-json` source instead
    of `map_mut().add_marker/remove_marker`; camera through the engine's own
    verbs (`pan_by_pixels`, `zoom_around`, `set_camera` equivalents already
    exist on the engine).
- `turbomap-style-maplibre`: emit Scene IR layers (`fill`/`line`/
  `fill-extrusion` with filters + paints) instead of a core `VectorStyle` —
  the engine already compiles those IR layers *into* `VectorStyle`
  (`engine.rs::vector_style_from_scene`), so the translation gains nothing
  by bypassing it. The app is this crate's only consumer.
- `turbomap-engine`: demote `map_mut()` to `#[doc(hidden)]` for the in-crate
  gpu tests that need it (`clouds_sim.rs`), delete the public contract.
- `invariants.rs`: `map_mut` → 0 hits outside `turbomap-engine`.

**Gates / done:** the desktop demo runs visually equivalent (manual boot +
the scenario harness's CSV unchanged within noise); goldens byte-stable;
grep-gate holds; `turbomap-app` no longer depends on `turbomap-core`
directly for content (Cargo.toml is the proof).

---

## 6. P6.3 — The web content plane (L)

**Goal:** everything the web map *shows* is declared in its Scene; DOM is
for chrome (interactive popups, callouts, controls), never for map content.

**Changes**
- `apps/web/src/map-engine/scene.ts`: widen the TS mirror — `geo-json`
  sources; `tube`, `line`, `circle`, `symbol` layer types (kebab-case tags,
  snake_case fields — the serde shape is pinned by
  `turbomap-scene/tests/scene_serde.rs`).
- Content migrates to scene fragments composed into `buildBaseScene`:
  - route/track line (`map-core/RouteOverlay.tsx`) → `tube` (3D-draped, the
    layer P5.2 built for exactly this) or `line` where flat is wanted;
  - marker pins (`map-core/MapPointMarkers.tsx`,
    `features/markers/MarkerPins.tsx`) → `circle`/`symbol` layers over a
    `geo-json` source (the pin *content*; the tap-opened React popup stays
    DOM);
  - user-location dot (`map-core/UserLocation.tsx`) → `circle` layer
    (accuracy ring + dot), updated by scene re-apply on position change.
- `useProjectedLayer.ts` survives only for genuine UI anchored to a geo
  point (popup positioning); the content-drawing rAF loops are deleted.
- Feature stores publish *data* (points, lines) through `map-core`; the host
  (`src/map`) folds them into the one scene document — same shape as the
  Android controller, and the existing eslint-boundaries tiers already
  enforce the direction.

**Gates / done:** typecheck + lint + vitest green, with a new vitest pinning
the emitted scene JSON for a route+pins+location fixture; production build +
wasm rebuild + the headless-Chromium boot smoke; grep: the three deleted
overlay components stay deleted; `useProjectedLayer` call sites ≤ the UI
survivors, enumerated in the test.

**Honest caveat:** container Chromium exposes no WebGPU adapter, so pixel
verification of the new layers rides the engine's Lavapipe gpu suites (same
code path) + one session in a real WebGPU browser (standing item for the
on-device/desktop agent).

---

## 7. P6.4 — Hit-testing crosses the production bindings (S/M)

**Goal:** "what did I tap" is the engine's answer on every host — required
the moment P6.3 turns DOM pins (with free DOM events) into scene layers.

**State today:** `Map::hit_test` exists (`map.rs`), the engine implements
the `MapEngine` trait's `hit_test`, uniffi exposes it (the Kotlin JVM
contract test already calls `map.hitTest`). Missing: the **JNI surface**
(Android's production path) and **wasm**.

**Changes**
- `turbomap-ffi/src/surface.rs`: a snapshot-consistent JNI hit-test
  (same pattern as the terrain-aware unproject — wait-free read against the
  published snapshot, never blocking the render thread).
- `turbomap-web/src/lib.rs`: wasm `hit_test(x, y, tolerance_px)` returning
  the hits as JSON/JsValue.
- Hosts adopt: web pin taps + route taps resolve through it (P6.3's layers);
  Android measure-point/tube taps route through `TurbomapMapEngine`.

**Gates / done:** the ffi gpu roundtrip test taps a scene-declared circle
and asserts the hit id; a web vitest against the wasm binding shape; the
conformance suite gains `check_hit_test_semantics` so `ModelEngine` pins
what a hit means per layer kind.

---

## 8. P6.5 — Compositing honesty: one ordered stack (M/L)

**Goal:** retire C3's documented exception. The IR's layer order is the
composited order for *every* layer kind — tubes, circles, symbols, markers
included — not just the positional basemap stack.

**Changes**
- `turbomap-core`: the overlay passes (route/tube, circles, icons, text,
  markers) become per-slot draws scheduled by IR order within the frame
  graph's existing phases, instead of fixed whole-track passes; `Layer::Tube`
  stops being a special diffed set in `turbomap-engine::reconcile` and joins
  the ordinary positional diff (the P5.2 workaround this slice deletes).
- **Parity first:** the default scene order (basemap → hillshade → content
  overlays) must reproduce today's pass order exactly — every existing
  golden byte-stable — before any reordered-scene behaviour lands.
- Conformance: `check_cross_track_ordering` upgrades from "documents the
  overlay exception" to asserting interleave (e.g. `fill` above `circle`
  below `tube` renders in declared order); one new golden pins an
  interleaved scene.

**Gates / done:** all pre-existing goldens byte-stable; the new interleave
golden + conformance clause green; `capabilities()` stops carving out the
overlay track; sim frame budgets hold (the reorder must not add passes).

**Risk note:** this is the render-path surgery of the phase — same playbook
as D1 (port 1:1, prove equivalence, then extend).

---

## 9. P6.6 — Android offline: one store, gated (M, device-gated)

**Premise revision (2026-07-05, survey):** the plan's original framing
("a second, parallel streaming stack that must join the engine's provider
chain") was partially WRONG. The architecture explicitly assigns tile IO —
"auth, caching, offline" — to the host (`host_resolver.rs` module docs), and
the Android host already implements exactly the right shape: ONE shared
`TileStore` (the map's fetch loop reads through it before OkHttp — literally
`chain [offline, remote]` semantics evaluated at the transport — and
`WgpuOfflineTileManager` pre-populates the same store), one layer/template
vocabulary (`MapStyles` feeds both the scene builder and `defaultLanes`), no
OkHttp disk cache anywhere. The declarative `SourceDef::Chain` (B6.2) is the
IN-PROCESS platforms' story (desktop/wasm); forcing it across JNI would
invert the architecture's IO division for no benefit.

**What the slice therefore delivers** (Kotlin, compile-unverified here):
- `OneTileStoreArchitectureTest` (JVM source-scan, same ratchet rule as
  `invariants.rs`): no OkHttp disk `.cache(` may appear beside the one
  `TileStore` in `turbomap-android` or `core/map`; the read-through line and
  the shared store dir are tripwired.
- `TurbomapOfflineOnDeviceTest` — the AIRPLANE-MODE gate: phase 1 drains a
  surface session's streaming plan and "downloads" every requested tile into
  a `TileStore` (the downloader's exact write path); phase 2 boots a FRESH
  surface over the same scene + camera against an unresolvable host
  (`offline.invalid`) and serves ONLY `store.get` — a store miss fails
  (pinning invariant 5, deterministic selection, across sessions), and the
  frame must reach ≥85 % basemap coverage. This also closes most of B6's
  deferred offline-cold-start gate with existing content.

**Gates / done:** both tests green on device (on-device agent); the
download → evict → re-download UX validated there too.

## 10. Explicitly out of scope for Phase 6

- **Vector basemap on web/Android** — the engine supports it (goldens prove
  it); shipping it is a data/product decision, not adoption debt.
- **Fonts in the scene** — `add_fallback_font` is host asset injection,
  defensible as-is.
- **The gated milestones** (M-TIN, M-MODELS, M-3DTILES) and the
  architecture's payoff round (water/snow/vegetation) — Phase 6 is the
  precondition that makes their eventual verdicts meaningful.

## 11. Risks

| Risk | Mitigation |
| --- | --- |
| P6.5 destabilizes rendering | Parity-first: default order reproduces today's passes, goldens byte-stable before interleave lands. |
| Web pins lose DOM affordances (CSS, a11y, events) | Only the *content* mark moves into the scene; interactive chrome stays DOM, anchored via the surviving projection hook; hit-testing (P6.4) replaces DOM events for map content. |
| Kotlin changes unverifiable in-container | Same P5 discipline: compile-clean best effort, flagged per commit, P6.6 gated on a device session; the `android_ffi` JVM lane (Lavapipe) covers the chain logic once CI triggers include this branch (P6.0). |
| Sim-gate CI cost | Nightly + dispatch, not per-push; the per-push lanes (workspace, GPU suites, wasm, invariants) stay under ~15 min. |
| Scene re-apply per location tick too chatty on web | The reconcile diff already no-ops unchanged layers; if the location layer churns, batch position updates to animation-frame cadence before re-apply (measure first — A1 trace has the numbers). |

## 12. Progress log

- _2026-07-05_: Plan authored from the full-adoption audit (this doc §0).

- _2026-07-05_: **P6.0 landed + verified — the ladder is CI.** The build
  workflow triggers on this branch and gains the GPU lane (Lavapipe,
  `REQUIRE_GPU=1`, the `gpu-tests` features plain `cargo test` silently
  skips) and a wasm32 check lane; `turbomap_sim.yml` runs the 8 release
  behavioural gates nightly + on dispatch. `tests/invariants.rs` makes the
  manual grep-gates executable with a ratchet rule (allowlists only
  shrink): no `.pending_tiles(` calls; image/MVT wire decodes confined to
  the codec + transport side with core/scene/world format-blind;
  `DemEncoding` out of the render path; `decode_elevation` single-homed.
  **First CI run in the campaign's history: run 136, all lanes green on
  GitHub runners (5.5 min)** — Phases 0–5 landed 55+ commits with zero CI
  because every lane was main-only.
- _2026-07-05_: **P6.1 landed + verified — the sim authors scenes.** `Sim`
  owns its Scene document like every real host: `set_sun` /
  `set_terrain_shadows` / `set_clouds_visible` edit the environment block
  (fixed lighting, `terrain_shadows`, `CloudsDef::visible`) and re-apply;
  the three `#[doc(hidden)]` engine verbs are deleted and
  `content_has_one_authoring_surface` forbids the entire setter family on
  the engine/ffi/wasm bindings forever. Invariant 7 now holds with ZERO
  exceptions. Verified: full ladder; **8/8 sim gates on the scene-declared
  environment path** (release, 539.9 s); goldens byte-stable.
- _2026-07-05_: **P6.4 landed + verified — hit-testing on every binding.**
  Geo-json feature properties ride into engine markers and out through
  hits (`parse_points_with_props`; a tapped pin answers with its domain
  id); wasm `hit_test` + JNI `nativeHitTest` (wait-free: exact under
  `try_lock`, `[]` under contention) share one serializer
  (`hits_to_json`); the Kotlin contract gains `hitTest`/`MapHit`
  (compile-unverified — device agent); the reference `ModelEngine`
  answers circle hits for real (the scene crate now parses geo-json at
  runtime — serde_json promoted); and `check_hit_test_semantics` pins
  the contract for every backend: layer id + properties are the pinned
  surface, `feature_id` stays engine-internal. Full ladder green; 8/8
  sim gates (release, 697.5 s).
- _2026-07-05_: **P6.3 landed + verified — the web content plane.**
  Routes/tracks (halo+stroke `line` layers, dash for unsolved previews),
  marker pins (kind-coloured circle pairs; every pin a feature carrying
  its id; selected pin emphasized), and the user-location dot
  (halo+ring+dot circles) are scene-declared through a `map-core`
  content store (`mapContent.ts`); the surface subscribes and re-applies
  the one document. DOM survives only as interactive chrome — waypoint
  drag handles, popups, the click ring; `RouteOverlay` kept its handles
  and lost its SVG polyline, `MarkerPins`/`UserLocationLayer` render
  nothing. Pin taps resolve through the engine's hit test. Gates: tsc,
  eslint (boundaries hold), 21 vitest incl. the new `sceneContent` shape
  pins, production build, wasm-pack rebuild, headless-Chromium boot
  smoke. **Execution note:** apt's binaryen-108 `wasm-opt` corrupts the
  module ("Table.grow failed" at instantiation) — build `--no-opt`
  locally; CI downloads a proper binaryen.
- _2026-07-05_: **P6.2a landed + verified — styles compile to the IR.**
  `turbomap-style-maplibre` gains `parse_style_layers` (MapLibre JSON →
  scene layers; covers match/interpolate/fill-extrusion beyond the old
  VectorStyle parser) + `without_water_fill_layers`; the app's hand
  styles are IR layer lists (`styles.rs`) with fidelity gates proving
  the engine compiles them to rules equivalent to the originals; the IR
  gains `Filter::ZoomRange` (serde-pinned, purely additive) and the
  engine one shared compile seam (`compile_vector_layer_style`) used by
  reconcile and the fidelity tests alike. Documented deviations live in
  the ir.rs docs + tests (streets catch-all as explicit Not-In; legacy
  width stops interpolate linearly; desktop vector-feature interactivity
  narrows until the app adopts engine hit-testing in P6.2b). Remaining
  for P6.2b: the app host rebuild on `TurbomapEngine` (in progress).
