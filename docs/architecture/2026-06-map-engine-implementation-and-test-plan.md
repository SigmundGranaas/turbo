# Map Engine — Implementation & Testing Plan to Production

**Date:** 2026-06-09
**Companion to:** `2026-06-map-engine-architecture.md`
**Goal:** Get the renderer-agnostic map engine to production-ready shape for
replacing MapLibre — with the organizing constraint that **every single part of
the system is testable**, deterministically, in CI.

The driving principle: *no component ships without a named test strategy and a CI
gate.* This document pairs each piece of implementation work with exactly how it
is verified, and front-loads the harnesses so every later part is testable the
day it lands.

---

## 0. Current testing baseline (what we build on)

- `turbomap-core` already has **value-boundary unit tests in 15 files** (camera,
  scene visibility, MVT/PMTiles decode, hit-test, style matching).
- A **headless wgpu render harness already exists** — `turbomap-app/examples/
  snapshot.rs` renders an offscreen composite to PNG with no winit/network. This
  is the seed for the golden-image suite.
- Each host app has test infra: Android JUnit/Robolectric (`feature/*/test`),
  Flutter `integration_test/user_journeys_test.dart` + `run_e2e.sh`, iOS XCTest
  (`apps/ios/Tests`).
- **Gap:** there is no turbomap CI workflow yet (only `tileserver_build.yml`
  touches cargo). Phase 0 fixes this first.

---

## 1. The test architecture (how we test *every* part)

Seven layers, each deterministic and CI-gated. The whole point is that the
expensive/flaky parts (GPU, devices) are reduced to small, controlled surfaces.

| # | Layer | What it covers | Determinism mechanism |
| --- | --- | --- | --- |
| T1 | **Pure-Rust unit/property** | geo math, camera, scene diff, expression eval, tessellation, hit-test, decoders | headless, no GPU; `proptest` for numerics |
| T2 | **Golden-image** | every render pipeline's pixels | headless wgpu on a **fixed software adapter**, committed reference PNGs, perceptual tolerance |
| T3 | **Contract conformance** | behavioral spec every `MapEngine` impl must pass | one suite, run against turbomap **and** each adapter |
| T4 | **Differential / shadow** | "not worse than MapLibre" | same scene+camera → MapLibre adapter vs turbomap, diff pixels + projection + hit-test |
| T5 | **FFI binding** | uniffi marshalling, errors, lifecycle/leaks | generated Kotlin/Swift round-trip tests |
| T6 | **Host integration** | the native view, gestures, overlays, offline | Compose UI test / XCTest+snapshot / Flutter widget+integration |
| T7 | **E2E + performance** | real-device smoke, frame budgets, memory | device lane + criterion benches + GPU-timestamp gates |

### Four design-for-testability decisions (do these or the plan doesn't hold)

1. **The scene diff emits a pure `SceneDelta` value.** `diff(old, new) ->
   SceneDelta` is a total function with no GPU dependency, so T1 can assert the
   *minimal* change set for any edit. The renderer consumes the delta separately.
2. **The engine is constructable headless.** turbomap-core already is; keep it so
   T1–T4 never need a window. The surface is injected, never assumed.
3. **A deterministic render harness** (T2): force a single software backend
   (Lavapipe/`llvmpipe` on Linux CI, `WARP` on Windows, `SwiftShader` as
   fallback), pin time and tile data, compare to committed PNGs with a perceptual
   threshold (e.g. dssim/`< N` differing pixels). One update command regenerates
   references behind review.
4. **A record/replay trace format.** Capture real sessions as a value stream
   `(scene, camera, tile-ingest)` over time. Replay deterministically in T2/T4 as
   regression fixtures and shadow-parity cases. *Record a real user session once;
   use it as a test forever.* This is the cheapest source of realistic coverage.

---

## 2. Component-by-component: build + test matrix

Every part of the system, what it is, and how it's proven. "Today" = current
turbomap-core state.

| Component | Today | Build work | Primary tests | Gate |
| --- | --- | --- | --- | --- |
| **Geo / Mercator math** | ✅ `geo.rs` | keep | T1 unit + `proptest` round-trips (project∘unproject ≈ id) | block |
| **Camera** (pan/zoom/pitch/bearing/ease) | ✅ tested `camera.rs` | extend for contract | T1 unit (already) + property (focus-anchored zoom invariant) | block |
| **Scene model + `SceneDelta` diff** | ❌ imperative `ingest_*` | new layer | T1: exhaustive diff cases (add/remove/reorder/repaint/source-update → expected minimal delta) | block |
| **Expression IR + compiler** | ❌ constant paint | new `Expr`/`Paint<T>` | T1 eval + property; **MapLibre expression conformance fixtures** | block |
| **GL Style Spec importer** | ❌ | new adapter → IR | T1 fixture tests (GL JSON → expected `Scene`); unsupported-op → `capabilities` report | block |
| **Tessellation** (line/fill) | ✅ `tessellate.rs` | data-driven attrs | T1 numeric/property (winding, counts, tolerance-vs-zoom) + T2 golden | block |
| **Raster pipeline** | ✅ | keep | T2 golden (tiles, opacity, overlay blend) | block |
| **Line pipeline** | ✅ | data-driven width/color | T2 golden (cap/join/width/zoom-curve) + T4 differential | block |
| **Fill pipeline** | ✅ | data-driven color | T2 golden + T4 | block |
| **Circle pipeline** | ✅ markers/circles | data-driven radius | T2 golden + T4 | block |
| **Symbol/text** | ⚠️ single-font atlas | keep scope-limited | T2 golden (placement/collision) + T1 collision math | warn→block |
| **Hillshade / terrain / DEM** | ✅ better than ML | keep | T2 golden (existing snapshot → golden) + T1 DEM decode | block |
| **Custom layer slot** | ❌ | new `CustomLayer` trait + pass | T1 (lifecycle calls) + T2 golden of a reference custom layer | block |
| **Mesh cache (style-epoch keyed)** | ✅ `vector_cache.rs` | re-key | T1 (epoch bump invalidates only affected) + bench | block |
| **Tile sources** (XYZ/MVT/PMTiles/DEM) | ✅ + cache | keep | T1 decode (already) + fault-injection (corrupt/empty/timeout) | block |
| **Hit-test + spatial index** | ✅ tested | extend for symbols | T1 unit (already) + T3 (apply→hit observable) | block |
| **`MapEngine` contract** | ❌ | new | **T3 conformance suite** | block |
| **Adapters** (MapLibre/MapKit/flutter_map) | ❌ | new per host | T3 conformance + T6 | block |
| **`turbomap-ffi` (uniffi)** | ❌ | new crate | T5 round-trip (Kotlin/Swift) + leak checks | block |
| **Surface/render-loop glue** | ❌ (winit only) | per platform | T6 instrumented (attach/resize/background/loss) | block |
| **Offline/tile IO host stacks** | host-side exists | reuse/extend | T6 + existing app tests | block |

---

## 3. Phasing — harness-first, then vertical slices behind gates

Each phase has explicit **entry gates** (what must be green to start) and **exit
gates** (what proves it's done). Phases ship independently behind a renderer flag.

### Phase 0 — Test infrastructure first *(unblocks everything)*
- Add **turbomap CI workflow**: `cargo test --workspace`, `clippy -D warnings`,
  `fmt --check`, on every PR touching `apps/turbomap`.
- Stand up the **T2 golden-image harness**: promote `examples/snapshot.rs` into a
  `turbomap-golden` test crate — fixed software adapter, committed PNGs under
  `tests/golden/`, perceptual diff, `UPDATE_GOLDEN=1` regeneration, artifact
  upload of diffs on failure.
- Define the **record/replay trace format** + a `replay` test runner.
- Wire **criterion benches** skeleton (T7) with thresholds (warn-only at first).
- **Exit gate:** CI green on existing tests; one golden test (the current
  hillshade snapshot) passing; a recorded trace replays deterministically.

### Phase 1 — `MapEngine` contract + adapters + conformance suite
- Define the contract (Rust trait + per-host interface) and the `Scene`/`Expr`
  IR types.
- Write the **T3 conformance suite** *before* implementations (spec-first).
- Implement `MapLibreEngine`/`MapKitEngine`/`FlutterMapEngine` **adapters over
  today's renderers**; move each app onto the contract — **zero behavior change**.
- **Exit gate:** all three adapters pass the conformance suite; apps run on the
  contract with no visual diff (T4 baseline captured here for later parity).

### Phase 2 — Scene model + diff
- Implement `Scene`, `SceneDelta`, `diff()`, and the engine's `apply()` over the
  existing pipelines (replacing imperative `ingest_*` internally).
- **Exit gate:** T1 diff matrix exhaustive & green; T2 golden for
  add/remove/reorder/repaint; track/route/measure re-expressed as scene layers
  with golden parity to the old path.

### Phase 3 — Expression system + GL import
- `Expr`/`Paint<T>` (`Const`→`Zoom`→`Data`, in that order), compiler, GPU
  attribute lowering for `Line`/`Circle` first.
- GL Style Spec importer → IR.
- **Exit gate:** MapLibre expression conformance fixtures green (or explicitly
  reported unsupported via `capabilities`); GL-import fixture set renders to
  golden parity for the styles the apps actually use.

### Phase 4 — Renderer parity per pipeline (turbomap engine)
- `TurbomapEngine` behind the contract; each pipeline gets golden + differential
  coverage vs the MapLibre adapter on shared scenes/traces.
- Custom-layer reference implementation (e.g. a heatmap) with golden.
- **Exit gate:** T4 differential within tolerance for every pipeline on the
  recorded traces; perf benches within budget (T7).

### Phase 5 — FFI + Android host integration *(Android = pilot, per prior decision)*
- `turbomap-ffi` (uniffi) bindings; Android surface glue (`SurfaceView` →
  `ANativeWindow` → wgpu) + `Choreographer` loop; `OfflineManager` replaced by a
  disk manager behind the existing `OfflineTileManager` interface;
  `LocalStyleServer` deleted.
- **Exit gate:** T5 binding round-trips green; T6 Compose/instrumented tests
  (attach/resize/background/surface-loss, pan/zoom, route draw, offline download,
  marker hit) green on device lane.

### Phase 6 — Shadow mode + staged Android rollout
- Run MapLibre adapter and `TurbomapEngine` side-by-side on real sessions; collect
  T4 differential metrics (pixel diff, projection error, hit-test agreement) as
  telemetry behind a flag.
- Staged rollout with a kill-switch back to the MapLibre adapter.
- **Exit gate:** shadow metrics under threshold across a defined session corpus;
  crash-free + frame-budget SLOs met; rollout dashboards live.

### Phase 7 — iOS, then Flutter
- iOS: `CAMetalLayer` + `CADisplayLink`; reuse `DiskOfflineTileManager`. Same T3/
  T5/T6 gates, XCTest snapshot tests for the SwiftUI view.
- Flutter: **spike GPU embedding first** (platform view vs texture); only then
  port. flutter_map adapter remains the fallback indefinitely.
- **Exit gate (per platform):** conformance + differential + host integration +
  shadow parity, same bars as Android.

---

## 4. CI architecture (the lanes)

- **Lane A — Rust fast (every PR):** `fmt`, `clippy -D warnings`, `cargo test`
  (T1), expression + GL-import fixtures, diff-matrix. Minutes; blocks merge.
- **Lane B — Golden + differential (every PR touching renderer/scene/style):** T2
  on the fixed software adapter; T4 on recorded traces; uploads diff images on
  failure. Software rendering keeps this deterministic and runner-agnostic.
- **Lane C — Benches (nightly + label-triggered):** criterion regressions
  (tessellation, diff, expression eval) + frame-time budgets via GPU timestamps;
  posts deltas, blocks on threshold breach when labeled `perf`.
- **Lane D — FFI (on `turbomap-ffi`/binding changes):** generate Kotlin/Swift
  bindings, T5 round-trip + leak checks.
- **Lane E — Host integration (per app, on app or engine change):** Android
  instrumented (emulator/device farm), iOS XCTest+snapshot, Flutter
  `integration_test` (extend `user_journeys_test.dart`).
- **Lane F — E2E smoke + shadow metrics (nightly + pre-release):** real-device
  load/pan/zoom/route/offline; shadow differential corpus report.

Determinism notes: pin the wgpu adapter and driver in Lanes B/C; seed all data;
freeze time in the harness; treat golden updates as reviewed artifacts, never
silent.

---

## 5. Production-readiness checklist (beyond green tests)

**Correctness & parity**
- [ ] Conformance suite passes for turbomap + all shipped adapters (T3)
- [ ] Differential within tolerance on the session corpus (T4)
- [ ] Feature parity checklist signed off: raster base, transparent overlays,
      line/fill/circle/symbol, camera+projection+hit-test, offline, gestures,
      native overlays

**Robustness (fault injection has tests, not hopes)**
- [ ] Surface loss/recreate, app background/foreground, rotation/resize (T6)
- [ ] VRAM budget enforced; eviction under pressure verified (T1+T7)
- [ ] Malformed/empty/timeout tiles degrade gracefully (T1 fault-injection)
- [ ] No panics across public FFI boundary; errors marshalled (T5)
- [ ] Memory-leak checks across attach/detach cycles (T5/T6)

**Performance (budgeted, gated)**
- [ ] p95 frame time within budget on reference devices (T7, GPU timestamps)
- [ ] Tessellation/diff/expression benches within regression thresholds (Lane C)
- [ ] Steady-state memory under cap on a long pan/zoom session

**Observability & rollout**
- [ ] Structured logging + tile/cache/frame metrics exported
- [ ] Shadow-mode telemetry dashboards (pixel diff, projection error, hit agree)
- [ ] Renderer **feature flag** + **kill-switch** to the legacy adapter
- [ ] Staged rollout plan with rollback criteria

**Process**
- [ ] Every component row in §2 has its gate green
- [ ] Golden references reviewed (no silent updates)
- [ ] Architecture-boundary tests updated (Android `ArchitectureBoundaryTest`)

---

## 6. Risks specific to testing & mitigations

| Risk | Mitigation |
| --- | --- |
| GPU golden tests flaky across drivers | Pin a single **software** adapter in CI; reserve real-GPU runs for nightly E2E; perceptual (not exact) diff. |
| "Test every part" balloons scope | The §2 matrix *is* the scope cap — one row, one gate; nothing outside it blocks. |
| Differential vs MapLibre never bit-exact | T4 uses perceptual + projection/hit tolerances, not equality; thresholds reviewed per pipeline. |
| Expression conformance is huge | Import MapLibre's published expression fixtures; stage `Const→Zoom→Data`; report gaps via `capabilities` rather than faking. |
| Device-lane cost/slowness | Keep T6 minimal-but-real; push breadth into headless T1–T4; nightly for device E2E. |
| Flutter GPU embedding may block | Gated spike; flutter_map adapter stays as indefinite fallback. |

---

## 7. Recommended first move

Execute **Phase 0** now: land the turbomap CI workflow, promote
`examples/snapshot.rs` into the golden-image harness, and define the record/replay
trace format. Until those exist, "test every part" is aspirational; once they do,
every subsequent component arrives already gated.
