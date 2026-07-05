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

## 9. P6.6 — Android offline joins the provider chain (M, device-gated)

**Goal:** one streaming stack on the device. Offline packs are a provider
the scene declares, served through the engine's cache/lifecycle — not a
parallel host-side store.

**Changes**
- The scene Android builds declares `chain [offline, remote]` per tile
  source (`SourceDef::Chain` — shipped in B6.2, currently unused by any
  product host).
- `TileStore.kt`'s role collapses: either (a) the engine's `DiskCache`
  becomes the one on-device store and the offline downloader pre-populates
  *it* (preferred — budgets/eviction/trace come free), or (b) `TileStore`
  is wrapped as a host-resolved provider stub in the chain. Decide by
  measuring on device; (a) unless the JNI file-access story blocks it.
- `OfflineDownloadService` keeps its region/zoom enumeration but writes into
  the one store; its progress/size accounting reads the same numbers the
  engine trace publishes.
- `invariants.rs`-style architecture test on the Kotlin side: one disk store
  (no second cache class reachable from the map host).

**Gates / done:** JVM test — a "downloaded" region renders with the remote
provider stubbed to refuse (zero plan starts escape to remote at covered
zooms); on-device airplane-mode androidTest over a real downloaded region
(this also closes most of B6's deferred offline-cold-start gate with
existing content instead of a new bundle); the on-device agent validates
download → evict → re-download UX.

---

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
