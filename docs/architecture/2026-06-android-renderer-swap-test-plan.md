# Testing the wgpu Renderer for Replacing the Android Map Renderer

**Date:** 2026-06-12
**Companion to:** `2026-06-map-engine-architecture.md`,
`2026-06-map-engine-implementation-and-test-plan.md`, `2026-06-wgpu-foundation-audit.md`
**Scope:** *only* the Android swap — how we prove `TurbomapEngine` (wgpu/Rust via
uniffi) can replace MapLibre in `apps/android` **without regressing**, with a CI
gate behind every step. This is the concrete, Android-specific instantiation of
**Phase 5 (FFI + host integration)** and **Phase 6 (shadow + rollout)** of the
implementation/test plan, which currently have no test strategy.

---

## 0. Where we actually stand (verified 2026-06-12)

**Proven already — headless, in `turbomap_build.yml`. We do not re-litigate these:**

| Layer | Crate | Gate today |
| --- | --- | --- |
| T1 pure-Rust unit/property | `turbomap-core`, `turbomap-scene` | `cargo test --workspace` |
| T2 golden image | `turbomap-golden` | Lavapipe (`REQUIRE_GPU=1`), committed PNGs |
| T3 contract conformance + scene-vs-imperative parity | `turbomap-engine` | golden lane |
| T5 FFI round-trip (**Rust acting as host**) | `turbomap-ffi` (`tests/roundtrip.rs`) | golden lane |
| T7 headless sessions / frame budgets | `turbomap-sim` | golden lane |
| FFI surface stays generatable | uniffi-bindgen → Kotlin **+** Swift | rust lane, artifact upload |

So the engine, the `MapEngine` contract, the scene/diff, and the uniffi *surface*
are all green headless. **The Kotlin bindings are generated but never executed; no
Android code consumes them.**

**Untested frontier — the whole Android host side. This is the entire plan:**

1. **No on-screen surface path.** `turbomap-ffi` exposes only `TurboMap::headless(...)`
   + offscreen `render_png()` (`crates/turbomap-ffi/src/lib.rs:143`). The
   `SurfaceView → ANativeWindow → wgpu surface` + `Choreographer` loop **does not
   exist**. The FFI doc says so explicitly ("you cannot pass `ANativeWindow`
   through uniffi … small pieces of hand-written per-platform glue").
2. **The Android seam is not renderer-agnostic.** `MapController`
   (`core/map/.../ui/map/TurboMap.kt:78`) is a thin wrapper *directly* over
   `MapLibreMap`; live data is pushed via `GeoJsonSource.setGeoJson`
   (`TurboMap.kt:253`), offline via `MapLibreOfflineTileManager`, styles via
   `LocalStyleServer`. There is no interface to put a second engine behind.
3. **No native toolchain in the Gradle build.** No JNA, no uniffi, no `jniLibs`,
   no `externalNativeBuild`/NDK, no cargo. uniffi-Kotlin needs **JNA** + the
   per-ABI `.so`.
4. **No device/emulator test lane at all.** `android_build.yml` runs only
   `testDebugUnitTest` (Robolectric/JVM) + `assembleDebug`; there is **no
   `androidTest`**. Surface lifecycle, real-GPU pixels, gestures, and perf are
   exactly what a headless lane can't cover — and we have no lane for them.

**Two pre-existing CI bugs to fix in passing:** `:core:map:testDebugUnitTest` is
omitted from `android_build.yml` (the map module's own unit tests don't run in
CI); and `cargo fmt --all --check` is informational-only in `turbomap_build.yml`.

---

## 1. Design-for-testability decisions (Android) — do these or the plan doesn't hold

These mirror the four headless decisions in the parent plan, recast for the host:

1. **Abstract the seam *before* writing any glue.** Introduce a Kotlin `MapEngine`
   interface in `:core:map` that mirrors the Rust contract, and make today's
   MapLibre code a `MapLibreEngine` adapter behind it — **zero behavior change**.
   Until two engines can sit behind one interface, neither A/B nor shadow testing
   is expressible. This is the keystone; everything else hangs off it.
2. **Keep the offscreen path as the device-side determinism anchor.**
   `render_png()` already renders the engine with no window. Reusing it *on the
   device* lets us run the **committed golden PNGs** against the real Adreno/Mali
   GPU — turning the device-GPU pixel question into the same perceptual-diff
   assertion CI already trusts, instead of a new bespoke check.
3. **Push the device surface down to the smallest possible area.** Everything
   provable on a JVM (binding marshalling, scene→delta, projection, hit-test,
   offscreen pixels via Lavapipe/host-GPU) is proven in the *unit* lane. The
   instrumented lane covers **only** what is genuinely device-only: surface
   attach/resize/loss, the vsync loop, real-GPU pixel confirmation, gestures, perf.
4. **Shadow parity is measured against the MapLibre adapter, never asserted bit-exact.**
   Same `Scene`+camera → MapLibre adapter vs `TurbomapEngine`; diff pixels
   (perceptual), `project()`/`unproject()` error (px), and hit-test agreement
   (set overlap), each with a reviewed threshold per surface. This is the
   "not worse than MapLibre" gate that authorizes the flag flip.

---

## 2. The Android test layers (extends the parent T1–T7)

| # | Layer | Covers | Determinism mechanism | Lane |
| --- | --- | --- | --- | --- |
| A1 | **Seam contract (Kotlin)** | `MapLibreEngine` honors the new `MapEngine` interface | Robolectric/JVM; existing core:map+feature:map tests stay green | unit (existing) |
| A2 | **Architecture boundary** | feature code never names a renderer; both adapters confined to `:core:map` | Konsist (`ArchitectureBoundaryTest`) | unit |
| B  | **FFI binding (Kotlin executes)** | JNA loads `.so`; construct/applyScene/camera/project/hitTest/render_png round-trip; **no leaks across attach/detach** | JVM test running the **host-GPU** offscreen path (CI: Lavapipe; dev Mac: Metal) | unit/new |
| C  | **Surface glue + render loop** | `SurfaceView`→`ANativeWindow`→wgpu; Choreographer; sRGB-format fallback; attach/resize/background/loss/rotation | instrumented (`androidTest`), emulator + 1 real device | **device (new lane)** |
| D1 | **Golden parity on device** | real-GPU pixels == committed Lavapipe goldens | `render_png()` on device vs `turbomap-golden/tests/golden/*.png`, perceptual diff | device |
| D2 | **Differential vs MapLibre** | "not worse than MapLibre" on shared scenes/traces | MapLibre adapter vs turbomap: pixel + projection + hit-test diff | device |
| E  | **Host integration** | live track/route/measure/marker as Scene layers; host-driven tile IO + offline disk; gestures; tap→hit popup | Robolectric (ViewModel/behavioral) + instrumented (gestures/offline) | unit + device |
| F  | **Shadow + perf + rollout** | side-by-side telemetry; p95 frame budget on reference devices; kill-switch | flagged shadow run; GPU-timestamp budgets (already wired) | device/nightly |

---

## 3. Staged plan — each stage its own gated step

Strangler order: seam first (de-risks everything), then bindings, then the one
fiddly device piece, then parity, then real wiring, then rollout.

### Stage A — Renderer-agnostic seam in `:core:map` *(zero behavior change; unblocks all testing)* — ✅ DONE (2026-06-12)
- **Shipped:** `core/map/.../core/map/MapEngine.kt` (the renderer-agnostic control-plane
  interface); `MapController` reparented as `MapLibreEngine : MapEngine` in
  `ui/map/TurboMap.kt`; `onMapReady`/`MapScreenState.controller` now typed `MapEngine`.
  New `ArchitectureBoundaryTest` gate *"the MapEngine seam stays renderer-agnostic"*
  (no `org.maplibre` import may reach the contract). `:core:map:testDebugUnitTest` added
  to `android_build.yml`. All existing map/feature suites green unchanged (= zero
  behavior change); detekt + `:app:assembleDebug` green.

- Define Kotlin `MapEngine` interface mirroring the Rust contract: `resize`,
  `detach`, `camera`/`setCamera`/`animateCamera`, `project`/`unproject`,
  `visibleBounds`, `applyScene`, `hitTest`, `pendingTiles`/`ingestTile`,
  `capabilities`. Fold today's `MapController` surface (`flyTo`, `frameTo`,
  `fromScreen`/`toScreen`, `setBottomInset`, …) into / on top of it.
- Reparent the current MapLibre code as `MapLibreEngine` (the *only* impl for now).
  The `TurboMap` composable talks to `MapEngine`, never `MapLibreMap` directly.
- **Gate:** every existing `:core:map` + `:feature:map` unit test green with no
  edits to their assertions; `ArchitectureBoundaryTest` extended — renderer
  imports confined to `:core:map`, and a new rule that the adapter type, not
  `MapLibreMap`, is what feature code sees. **Fix CI:** add
  `:core:map:testDebugUnitTest` to `android_build.yml`.

### Stage B — FFI binding tests (Kotlin actually executes the engine) — ✅ DONE (2026-06-12)
- **Shipped:** new Kotlin/JVM module `:core:turbomap`; JNA in the version catalog; the
  cdylib + Kotlin bindings are built/generated **at build time** (`buildRustFfi` /
  `generateFfiBindings` Gradle tasks → `target/debug/libturbomap_ffi.*`, generated kt under
  `build/generated`, never vendored). `TurbomapFfiRoundTripTest` drives the *generated
  Kotlin* bindings on the host GPU (Metal locally / Lavapipe in CI), mirroring `roundtrip.rs`:
  applyScene→delta, pump/pending/ingest, `renderPng` pixel checks, camera/project/unproject,
  circle hit-test, structured `FfiException` marshalling, and 25× attach/detach leak cycles.
  New CI **Lane D** = `.github/workflows/android_ffi.yml` (Rust + protoc + Lavapipe,
  `REQUIRE_GPU=1`), triggered on `apps/turbomap/**` or `:core:turbomap` changes. All 4 tests
  green; module detekt green.

- New module `:core:map:turbomap` (or `:core:turbomap`): add **JNA**, drop in the
  uniffi-generated `turbomap_ffi.kt` (already produced by CI) + the host `.so`
  (build the cdylib for the *host* triple for JVM tests; per-ABI Android `.so`
  comes in Stage C).
- JVM/Robolectric tests (Lane D-equivalent on host GPU): `TurboMap.headless(w,h,cam)`
  → `applyScene(json)` (reuse a `turbomap-scene` fixture) → assert `DeltaSummary`;
  `setCamera`/`camera` round-trip; `project`∘`unproject` ≈ id; `hitTest` returns
  the seeded feature; `renderPng()` bytes decode to a `w×h` image. **Leak/lifecycle:**
  N× construct→drop cycles under a memory watch; error marshalling (bad scene JSON
  → `FfiError.InvalidScene`, no panic across the boundary).
- **Gate (new CI Lane D — FFI):** these tests green on a runner with a GPU adapter
  (Lavapipe). Triggered on `turbomap-ffi`/binding or `:core:turbomap` changes.

### Stage C — Surface glue + render loop *(the one genuinely device-only piece)* — ✅ DONE (2026-06-12)
- **On-screen path shipped + verified on the emulator's real GPU:** `turbomap-ffi/src/surface.rs`
  (android-only) is hand-written JNI — `NativeWindow::from_surface` → `wgpu::Surface` via
  `SurfaceTargetUnsafe::RawHandle` → configure (prefers an sRGB surface format, Decision 3) →
  `TurbomapEngine` → render/present/resize/destroy, every entry point `catch_unwind`-guarded.
  Kotlin `NativeSurfaceMap` (`external` JNI, `System.loadLibrary`). Instrumented
  `TurbomapSurfaceOnDeviceTest` creates a surface from an `ImageReader` (a real producer
  surface), applies a circle scene, **presents a frame, and reads back the central yellow
  disc** — 3/3 device tests green (this + the two offscreen ones). Deps `jni 0.21` / `ndk 0.9`
  / `raw-window-handle 0.6` are android-target-gated (host builds never pull the NDK crates);
  `#![allow(unsafe_code)]` scopes the workspace lint to this FFI module.
- **Remaining polish (not blocking the swap):** a Compose `SurfaceView`/`Choreographer` host
  wired into the real app (the test drives frames directly); the Decision-3 *intermediate
  sRGB texture + blit* fallback for devices that expose **no** sRGB surface format (only the
  format-preference path is in); and a fuller real-`View` background/foreground/surface-loss
  suite (the code's lost/outdated→reconfigure path is exercised, but not a true Activity
  lifecycle). These ride along when the engine is wired behind the `MapEngine` seam in Stage E.

#### Earlier-staged scope (foundation, also 2026-06-12)
- **Shipped (on-device foundation, verified on the `turbo_e` emulator, arm64, API 36):**
  new Android-library module `:core:turbomap-android` (kept separate from the host-JVM
  `:core:turbomap` to avoid the JNA jar-vs-`@aar` collision); cargo-ndk cross-compiles
  `turbomap-ffi` to per-ABI `.so` (arm64-v8a + x86_64) into jniLibs; JNA `@aar`; bindings
  generated at build time into AGP's default source roots (gitignored). Instrumented
  `TurbomapOnDeviceTest` runs the engine on the device's **real Vulkan GPU** (renderPng
  pixel checks + camera projection) — 2/2 green, proving cargo-ndk `.so` → APK jniLibs →
  JNA-on-ART → uniffi → wgpu. Fixed a real bug en route: the FFI offscreen instance
  requested Vulkan debug-utils/validation in debug builds, which **SIGSEGV'd the emulator's
  `vulkan.ranchu` driver** in `vkSetDebugUtilsObjectNameEXT`; `offscreen.rs` now strips
  `DEBUG|VALIDATION` on every target (host FFI + golden lanes re-verified green). New CI
  **Lane E** = `.github/workflows/android_device.yml` (cargo-ndk + KVM emulator-runner).
  Anti-staleness: the cargo `Exec` tasks use `upToDateWhen { false }` so a Rust edit can't
  ship a stale `.so`.
- **Still pending (the on-screen path):** a `TurboMap::from_surface(ANativeWindow)` FFI/JNI
  constructor (today only `headless()` exists), the `SurfaceView`/`TextureView` +
  `Choreographer` host, the sRGB surface-format fallback (audit Decision 3), and an
  instrumented attach/resize/background/surface-loss suite. The foundation above is what
  that glue stands on.

#### Original Stage C scope (for reference)
- Add the on-screen path: a `TurboMap::from_surface(handle, …)` FFI constructor (or
  a thin JNI shim) that builds a wgpu `Surface` from the `ANativeWindow`
  (`raw-window-handle`), + a Kotlin `SurfaceView`/`TextureView` host driving a
  `Choreographer` frame callback that calls `tick()`/render/present. Implement the
  **Decision-3 sRGB fallback** (prefer `*Srgb` surface format; else render to an
  sRGB intermediate + blit).
- Wire the Android NDK build: per-ABI cargo build → `jniLibs/{arm64-v8a,…}`
  (cargo-ndk or `externalNativeBuild`), ABI-split APK like MapLibre today.
- **Gate (new CI Lane E — device, emulator + ≥1 real device):** instrumented tests
  for **attach → first frame**, **resize/rotation**, **background→foreground**,
  **surface loss → recreate** (no crash, frames resume), detach (no leak). This is
  the lane the project does not yet have; standing it up is part of this stage.

### Stage D — Parity on the device
- **D1 golden-on-device:** an instrumented test renders the `turbomap-golden`
  scenes through `renderPng()` on the device GPU and perceptual-diffs against the
  committed reference PNGs (same tolerance as CI). Proves Adreno/Mali == Lavapipe.
- **D2 differential vs MapLibre:** drive the same `Scene`+camera through the
  `MapLibreEngine` adapter and `TurbomapEngine`; diff rendered pixels (perceptual),
  `project`/`unproject` (px error), and `hitTest` (feature-set agreement) on the
  app's *actual* scenes (basemap + overlays from `MapStyles`) and on recorded
  traces (parent plan's record/replay).
- **Gate:** D1 within golden tolerance; D2 within per-surface reviewed thresholds
  (capture the baseline here for rollout).

### Stage E — Real host integration behind the seam — 🟡 KEYSTONE DONE (2026-06-12)
- **Shipped + verified on the emulator:** the `MapEngine` contract moved to renderer-agnostic
  `:core:model` (`domain.MapEngine`) so both engines implement it without depending on each
  other; `MapLibreEngine` (`:core:map`) and the **new `TurbomapMapEngine`**
  (`:core:turbomap-android`, over the wgpu engine via JNI) are now peers behind one seam.
  Exposed the control plane on the on-screen handle (`nativeSetCamera`/`nativeCamera`/
  `nativeProject`/`nativeUnproject`); the adapter composes the derived contract (zoom step,
  north reset, `visibleBounds` from corner unprojection, frame-to-fit). On-device contract
  test (`TurbomapMapEngineContractTest`) proves it honours the contract — camera moves,
  project∘unproject identity, visible-bounds-contain-centre (6/6 device tests green).
- **Scene authoring shipped + host-verified:** `TurbomapScene` (`:core:turbomap`) maps the app's
  live state (basemap + overlay rasters + track/route/measure/user) → Scene-IR JSON —
  "track/route/measure stop being special; they're Line/Circle layers over a geojson source".
  `TurbomapSceneTest` drives the real engine: the app's full state applies as 6 layers with
  **none unsupported**, the geojson drains in-process, only the basemap raster stays pending,
  and the frame renders (3 host tests green). Renderer-agnostic — takes raster URL specs (the
  `BaseLayer`/`OverlayId`→URL mapping stays in `MapStyles`).
- **Cutover host + flag shipped (experimental, default OFF) — 2026-06-12:** `TurbomapMapView`
  (`:core:turbomap-android`) is the on-screen Compose host — `SurfaceView` + `Choreographer`
  render loop + **host-driven raster tile fetch** (`pendingTilesJson`→HTTP→`ingestRaster`, all
  native calls marshalled to main) + `detectTransformGestures` pan/zoom → camera, handing a
  `TurbomapMapEngine` up via `onMapReady`. `MapStyles.turbomapRasterSpecs` reuses the MapLibre
  tile URLs. A DataStore-backed **Settings toggle** ("Experimental wgpu map") drives
  `MapUiState.experimentalWgpuMap`; `MapScreen` swaps `TurboMap` ↔ `TurbomapMapView` on it.
  Builds green (full host gate + detekt + `assembleDebug`); the `.so` ships in the APK
  (arm64-v8a + x86_64); app smoke-launches healthy with the flag off.
- **Overlay parity via a shared seam (2026-06-12):** the projected overlays (markers, editable
  waypoints, photo pins) are extracted into one renderer-agnostic `MapOverlay`
  (`:core:designsystem`) that projects through `MapEngine.toScreen`/`fromScreen` — **both**
  `TurboMap` (MapLibre) and `TurbomapMapView` (wgpu) draw their pins with it, so the on-map UI
  is identical. `MapScreen`'s marker-selection + map-tap logic is hoisted into shared lambdas
  used by both branches. The turbomap path now also has tap, long-press-to-add-marker, and the
  bearing readout (Compose `detectTapGestures` → `MapEngine.fromScreen`; per-frame camera tick).
  The MapLibre path is behaviour-preserving (it projects via `MapLibreEngine`, same projection);
  verified by the unchanged `:core:map`/`:feature:map` suites + a healthy smoke-launch.
  On-screen visual fidelity (tile alignment, colours, gesture feel) is the user's device test.
- **Offline + inset (2026-06-12):** `TurbomapTileCache` — a read-through disk cache
  (`cacheDir/turbomap-tiles`, `layer/z/x/y`→atomic file) consulted before the network and
  written on fetch, so visited areas render offline (5 unit tests). `setBottomInset` is now
  honoured adapter-side: `flyTo`/`frameTo` lift the centred target into the visible band above
  the live sheet (on-device test asserts the target moves up). A projection-wide inset (scale
  bar / continuous unproject reflecting the band) still needs engine viewport-padding support.
- **Stage D — projection parity gate (2026-06-12):** `TurbomapProjectionParityTest` asserts the
  engine's `project` is the **Web Mercator** projection MapLibre is defined by — correct centring,
  axis directions (E→right, N→up), longitude symmetry, and **conformality** (isotropic
  px/mercator-unit). Host-side + deterministic; this is what makes overlays land where MapLibre
  puts them, so it gates the eventual flag default-flip. (A live pixel-diff vs MapLibre
  `MapSnapshotter` is a follow-up — flaky on a software-GL emulator, so not a main-gating test.)
- **Genuinely remaining:** download-region offline parity (the disk cache is the substrate),
  the live MapLibre pixel-diff + golden-on-device (Stage D visual), Stage F (shadow telemetry +
  perf budgets + flag default-flip), and engine-side viewport padding for a full `setBottomInset`.
- **Remaining (full parity + rollout):** a turbomap Compose host with marker/waypoint/photo
  (`SurfaceView`/`Choreographer`) that authors the app's live track/route/measure/markers as
  **Scene layers** (replacing `setGeoJson`/`installTurboLayers`/`LocalStyleServer` on the
  turbomap path); host-driven offline behind the existing `OfflineTileManager`; tap→`hitTest`;
  the renderer flag selecting `MapLibreEngine` vs `TurbomapMapEngine`. This is the
  shadow-mode-gated flip — it changes shipping rendering, so it lands with Stage F parity.

#### Original Stage E scope (for reference)
- Author the app's live data as **Scene layers** instead of `setGeoJson`:
  track/route/measure/user-location/markers become `Line`/`Circle` layers over
  `geojson` sources in an `applyScene` the ViewModel rebuilds (React-for-the-map).
  `installTurboLayers` machinery and `LocalStyleServer` are **deleted on the
  turbomap path** (MapLibre adapter keeps them).
- Offline: a disk `OfflineTileManager` impl feeding the **host-driven**
  `pendingTiles()`/`ingest*` contract (host owns fetch/auth/cache), behind the
  *existing* `OfflineTileManager` interface. `SyntheticOfflineTileManager` stays
  for debug.
- Gestures → `setCamera`/`animateCamera`; tap → `hitTest` → feature popup (a new
  capability — MapLibre path has no rendered-feature query today).
- **Gate:** existing Robolectric ViewModel/Compose tests (`MapViewModelTest`,
  `RouteViewModelTest`, `OfflineViewModelTest`, `LiveSheetTest`, …) green against
  the turbomap adapter selected by the flag; new instrumented tests for
  pan/zoom/route-draw/offline-download/marker-hit on device.

### Stage F — Shadow mode, perf budgets, staged rollout
- Run both engines on real sessions behind a flag; emit D2 differential metrics as
  telemetry. Frame-budget SLOs on reference devices via the **already-wired GPU
  timestamps** (`render/gpu_timestamps.rs`); steady-state memory cap on a long
  pan/zoom; VRAM eviction under pressure.
- **Renderer feature flag + kill-switch back to `MapLibreEngine`**; staged rollout
  with rollback criteria.
- **Gate:** shadow metrics under threshold across a defined session corpus;
  crash-free + frame-budget SLOs met on reference devices.

---

## 4. CI lanes to add (Android)

- **Existing — Android unit (every PR):** detekt + `testDebugUnitTest` +
  `assembleDebug`. **Add `:core:map:testDebugUnitTest`** (Stage A); A1/A2/E
  Robolectric tests ride here.
- **New Lane D — FFI binding (on `turbomap-ffi`/`:core:turbomap` change):** JVM
  tests executing the Kotlin bindings against the host-GPU offscreen path
  (Lavapipe on CI). Stage B.
- **New Lane E — device (emulator on PR for app/engine change; ≥1 real device
  nightly):** instrumented surface-lifecycle, golden-on-device (D1), differential
  (D2), gestures/offline (E). Stage C–E. *This lane does not exist today; standing
  it up is itself a deliverable.*
- **New Lane F — shadow + perf (nightly + pre-release):** flagged shadow corpus
  report + frame-budget/memory SLOs. Stage F.

Determinism: pin the wgpu adapter in Lanes D; reuse committed goldens for
device parity; perceptual (never bit-exact) diffs; freeze tile data + time in
fixtures; golden/threshold updates are reviewed artifacts, never silent.

---

## 5. Production-readiness checklist (Android swap)

**Parity**
- [ ] Kotlin `MapEngine` conformance: `MapLibreEngine` + `TurbomapEngine` pass one
      suite (A1)
- [ ] Golden-on-device within tolerance (D1); differential within per-surface
      thresholds on app scenes + traces (D2)
- [ ] Feature parity signed off: raster base + transparent overlays
      (Norgeskart/OSM/Sat, trails, avalanche), track/route/measure/markers,
      camera+projection+hit-test, offline download, gestures, native Compose
      overlays still reproject via `project()`

**Robustness**
- [ ] Surface loss/recreate, background/foreground, rotation/resize (C)
- [ ] No panic across the FFI boundary; errors marshalled (`FfiError`) (B)
- [ ] No leak across attach/detach + construct/drop cycles (B, C)
- [ ] Malformed/empty/timeout tiles degrade gracefully via host IO (E)

**Performance**
- [ ] p95 frame time within budget on reference devices, GPU timestamps (F)
- [ ] Steady-state memory under cap on long pan/zoom; VRAM eviction verified (F)

**Rollout**
- [ ] Renderer flag + kill-switch to `MapLibreEngine` (F)
- [ ] Shadow telemetry dashboards (pixel diff, projection error, hit agreement) (F)
- [ ] `ArchitectureBoundaryTest` updated; both adapters confined to `:core:map` (A2)
- [ ] CI: `:core:map` unit tests run; Lanes D/E/F live

---

## 6. Risks specific to the Android swap

| Risk | Mitigation |
| --- | --- |
| No device lane exists yet | Standing up Lane E is an explicit Stage-C deliverable; keep it minimal-but-real, push breadth into B/D headless. |
| Surface-format sRGB on Android GLES/Vulkan | Decision 3 fallback (sRGB intermediate + blit); asserted by D1 golden-on-device. |
| Real-GPU pixels diverge from Lavapipe | D1 makes this an explicit, perceptual gate rather than a hope. |
| Seam refactor (A) silently changes behavior | A ships with **zero** assertion edits to existing tests; any diff is a real regression. |
| FFI leaks invisible to functional tests | Dedicated leak/lifecycle cycles in B and C, not just round-trips. |
| Differential never bit-exact vs MapLibre | D2 is perceptual + projection/hit tolerances, reviewed per surface — same philosophy as parent T4. |
| `.so` size / ABI bloat | ABI-split APK (as MapLibre today); track size delta vs the removed MapLibre AAR. |

---

## 7. Recommended first move

**Stage A** — the renderer-agnostic Kotlin `MapEngine` seam with the current
MapLibre code reparented as `MapLibreEngine`, zero behavior change, plus fixing
`:core:map` into CI. Until two engines fit behind one interface, A/B and shadow
testing can't even be written. Stage B (Kotlin bindings actually executing the
engine, against the offscreen path we already trust) is the cheapest proof that
the Rust work reaches the device, and it needs no surface glue — so it can land in
parallel with A.

---

## 8. Post-cutover usability fixes (2026-06, device QA)

On-device QA of the wgpu renderer surfaced four issues. Root causes + status:

| Symptom (device report) | Root cause | Status |
| --- | --- | --- |
| Map renders in a small grey-bordered island | `BACKGROUND_CLEAR` (sRGB 170,170,165) is the empty-tile colour; the island was just the few tiles that loaded before fetching stalled. **Not** a projection bug. | **Fixed** — proven by `TurbomapRasterFillOnDeviceTest`: with every pending tile ingested the basemap fills >90% of a 360×780 surface, <2% grey. |
| Tiles load a little, then stop | `scheduleTileFetch` marked every pending key into a permanent `fetched` set *before* fetching, then `onTransform` (many events/pinch) called `fetchJob.cancel()` — orphaning in-flight tiles in `fetched` forever, so they were skipped on every later pass. | **Fixed** — dedup on a transient `inFlight` set only (no permanent suppression → evicted tiles can reload); no destructive cancel. |
| Loading far slower than MapLibre | Tiles fetched strictly sequentially (one HTTP at a time, 10 s timeout each). | **Fixed** — bounded-concurrency parallel fetch (`Semaphore(6)`), nearest-first. |
| At max zoom tiles desync / drift, camera snaps | GPU pipelines tessellated in **absolute f32 world coords**; near world 0.5 at z≈20 f32 carries ~4–5 px of error (more above), so f64 unproject and f32 render disagreed. (The documented "centre-relative world coords on the GPU" TODO.) | **Fixed** — relative-to-centre (floating-origin): `Camera::view_projection_matrix_rtc(origin)`; raster/vector/hillshade feed `(world − origin)` as f32. Text/icon/marker already used f64 `world_to_screen`; no shader changes. |

The RTC fix is verified by `camera::rtc_keeps_high_zoom_geometry_locked_to_the_f64_projection`
(at z20 the RTC projection tracks the exact f64 `world_to_screen` to <1 px while the absolute
path is >4 px off), and is golden-clean: `golden_raster_parchment` is unchanged, `hillshade-bergen`
shifts 6 px / 196 608 (the pre-existing Metal-vs-Lavapipe flake, far under the 2 % CI budget),
engine `golden_parity` + conformance pass, and the 8-test on-device suite still passes.

Engine-side viewport padding (`setBottomInset` → `nativeSetViewportInset`,
projection + GPU view-matrix shift) also landed and is covered by
`camera::viewport_inset_*` (unit) and `TurbomapMapEngineContractTest` (device).

---

## 9. Production-hardening plan (2026-06)

Directives: **no MapLibre fallback** — the wgpu engine must *fail loudly and
report the error*; and **rendering moves to a dedicated render thread**. Staged,
each its own gated commit, following the strangler/golden-gate discipline.

### Stage 1 — Fail-fast + error reporting (replaces "fallback")
- **Rust** (`surface.rs`): capture the failure reason instead of returning a bare
  `0`/`null`. Add `nativeLastError() -> String?` backed by a `Mutex<Option<String>>`
  set at every `build()` `?`-fail and in each `with_map` `catch_unwind` `Err` arm
  (panic payload), plus device-lost / surface-create failures from `render()`.
- **Kotlin**: `TurbomapSurfaceController` surfaces `onEngineError(reason)`;
  `TurbomapMapView` exposes the callback. `MapScreen` shows a **visible error
  surface** ("Map engine failed: <reason>") — never a silent grey — and `Log.e`s
  it. Unrecoverable GPU/device-lost = reported + surfaced; in `debug` builds it
  rethrows so it crashes visibly. No fallback path.
- **Verify**: on-device test forcing init failure asserts the reason propagates;
  Robolectric test for the error-state UI.

### Stage 2 — Render thread + thread-safe engine (backbone)
- **Kotlin**: replace the main-thread Choreographer loop with a dedicated render
  `HandlerThread` (own `Looper` + `Choreographer`). All mutating/render native
  calls (`create/applyScene/render/resize/ingest/destroy/setCamera/setViewportInset`)
  post to it.
- **Rust**: drop the "single-threaded, unlocked" contract. Put camera + projection
  inputs behind a fast lock so UI-thread `project/unproject/camera/visibleBounds`
  run concurrently with the render thread; the frame snapshots the camera briefly,
  then does GPU work unlocked. Mutations take the lock + set dirty.
- **Verify**: existing contract + fill on-device tests pass from the render thread;
  new concurrency stress test (UI-thread projection hammered during render) — no
  crash/UB; UI thread never blocks on a frame.

### Stage 3 — Render-on-demand + decoupled overlay tick
- Render only when **dirty** (camera/scene/ingest/resize/inset change, or an active
  fling/zoom animation); park when idle. Bump `cameraTick` (overlay relayout) only
  on frames where the camera actually moved, signalled from the render thread.
- **Verify**: instrumented assert of zero renders while idle; overlays still track
  during pan + animations.

### Stage 4 — Shared GPU device; surface-only recreate
- Cache `Instance`+`Adapter`+`Device`+`Queue` (process-wide, shared with the
  offscreen/golden path); `surfaceCreated` builds only the `Surface` + configures;
  `surfaceDestroyed` drops only the surface. Recreate device only on true loss.
- **Verify**: rotation/resume re-enumerates no adapters (timing/log); surface
  attach/detach/reattach on-device test.

### Stage 5 — Networking unification
- Move tile IO out of the Compose host into a `:core:data`/`:core:map` repository
  on the shared OkHttp (`core/data/di/NetworkModule`) + a cache aligned with
  `OfflineTileManager` (Cache-Control/ETag, offline regions, shared with MapLibre).
  The host talks to a `TileSource` seam; the controller stays renderer-glue only.
- **Verify**: offline-region tiles serve with no network; cache shared with
  MapLibre; repository unit tests.

### Stage 6 — Memory budget + telemetry
- Bound the engine texture LRU + disk cache to a configured budget; add an FFI
  `nativeStats()` (tile counts, texture bytes); report (log + debug overlay).
- **Verify**: LRU eviction test; stats sane on device.

### Stage 7 — Gates
- Device golden lane (D1/E) in CI on the emulator; differential projection/hit
  test vs `MapLibreEngine` (D2 — kept as a correctness gate even without runtime
  fallback); sRGB-on-GLES confirmation; floating-origin invariant guard.

### Stage 0 — Tile pipeline rearchitecture (declarative reconciler) — TOP PRIORITY

Root cause of the "slow / inconsistent / stops after panning" report: the host
tile pump is **edge-triggered and fire-and-forget**. `scheduleTileFetch` runs
only on `attachOrResize` / `applyScene` / `onTransform`; the render loop never
pumps. Consequences: (a) when gestures stop, missing tiles (failed or starved)
are **never retried** → permanent gaps; (b) no cancellation → a fast pan floods
the queue with stale-position tiles that starve the current view behind the
`Semaphore(6)`; (c) `HttpURLConnection` has no pool/HTTP2, and a few hung
requests hold all permits and stall everything. No retry, no priority, no
cancellation — the three things a tile pipeline must have.

Replace the glue with a **continuously-reconciled tile manager** (the source of
truth is the engine's desired set; the loop drives the host toward it):

- **Reconcile tick** (driven by the render frame, or a steady ~250 ms timer while
  the desired set is non-empty; stops when complete): each tick diff
  `desired` (engine `pending_tiles`, already excludes present) against
  `in-flight` and:
  - enqueue `desired − in-flight` into a **priority queue** (key: distance to
    camera centre, then zoom) — re-prioritised every tick as the camera moves;
  - **cancel** `in-flight − desired` so workers free up for the current view.
- **Bounded worker pool** on the shared **OkHttp** client (connection pool,
  HTTP/2, sane timeouts), not `HttpURLConnection`.
- **Retry is implicit**: a failed tile simply isn't `present`, so it reappears in
  `desired` next tick and is retried (with short backoff to avoid hammering).
- **Cache** via a `TileSource` seam aligned with `OfflineTileManager`
  (Cache-Control/ETag, offline regions, shared with MapLibre).
- The Compose host shrinks to "publish camera/scene → engine; pump on the tick";
  no fetch logic in the view.

This **absorbs old Stage 5** (networking unification) and is the prerequisite for
a good experience, so it runs first. It composes with Stage 2: the render thread
provides the natural reconcile cadence (reconcile after each frame), and the
manager lives on the IO side off the UI thread.

- **Verify**: pan-and-release leaves **zero** missing desired tiles (instrumented
  reconcile-to-empty assert); a stale-tile cancellation test (fast pan cancels
  off-screen fetches); a forced-failure tile is retried and eventually present;
  offline-region tiles serve with no network.

### Hardening status (2026-06-12)
- ✅ **Stage 0** — declarative tile reconciler (commit on main).
- ✅ **Stage 1** — fail-fast + `nativeLastError` + visible error surface.
- ✅ **Stage 2** — dedicated render thread + `Mutex<OnScreen>` (concurrency stress test).
- ✅ **Stage 3** — render-on-demand + camera-only overlay tick.
- ⬜ **Stage 4** (shared GPU device), **Stage 0b** (OfflineTileManager cache), **Stage 6** (memory/telemetry), **Stage 7** (device golden + differential gates) — remaining.

### Hardening status (updated 2026-06-13)
- ✅ Stage 0, 1, 2, 3 (see above).
- ✅ **Tile fade-in** — per-tile 0.3 s smoothstep blend; `is_animating` keeps render-on-demand alive during the fade.
- ✅ Two real stall fixes found by running the app on the emulator: undecodable-tile busy-loop (evict + backoff), and the **HTTP/2 StreamResetException dead-slot leak** (always resume the fetch) — the actual cause of grey-after-panning.
- ✅ **Stage 6** — `nativeStats` cache telemetry (budget already enforced by the 128 MB LRU).
- ⬜ **Stage 4** (shared GPU device — minor: avoids re-init on rotation; touches the just-stabilised surface lifecycle), **Stage 0b** (OfflineTileManager cache sharing — cross-module DI), **Stage 7** (device-golden + differential-vs-MapLibre CI lane) — remaining; all infra, low user-facing value.

---

## 10. Physics & motion plan (iPhone-like feel)

The engine already has the full camera-physics system (`ActiveAnim::{Ease,
PanFling,ZoomFling}`, `Map::fling`, `ease_to`, `zoom_around_animated`,
`tick(now)`); the on-screen Android path bypasses it (per-event `setCamera`, no
momentum, dead stop on release). Physics stays engine-side (renderer-agnostic,
shared with iOS); the host feeds gestures in and the render loop advances time.

- ✅ **M0 — universal tile fade** (incl. cached tiles): done (617d49fb) — fade
  over a backdrop or the clear colour; no more pop.
- **M1 — FFI + tick drive**: `nativeFling(vx,vy)`, `nativeEaseTo(lat,lng,zoom,
  bearing,ms)`, `nativeZoomAroundAnimated(factor,fx,fy,ms)`, `nativeCancelAnimation()`;
  render frame calls `engine.tick_now()`; an animated frame counts as a camera
  move → bump `cameraTick` (overlays follow) + `requestReconcile()` (tiles load
  along the trajectory). `is_animating` already keeps render-on-demand alive then
  parks. Gate: on-device fling moves the camera over frames then settles; cancel stops it.
- **M2 — gesture physics**: replace `detectTransformGestures` with `awaitEachGesture`
  + `VelocityTracker`. Down → `nativeCancelAnimation()` (catch the motion exactly);
  move → pan/zoom live + track velocity; up → `nativeFling(v)`; pinch-release →
  optional zoom-fling. Gate: emulator (swipe glides + decelerates; touch mid-glide
  stops dead) + a pure velocity-mapping unit test.
- **M3 — eased programmatic moves**: `TurbomapMapEngine.flyTo/zoomIn/zoomOut/
  resetNorth/frameTo` + centre-on-me route through `ease_to`/`zoom_around_animated`
  (accel/decel) instead of instant `setCamera`. Gate: emulator — rail + locate ease.
- **M4 — tune to iPhone feel**: calibrate fling τ, ease durations, min-fling
  velocity; confirm no grey trail behind a fling + clean park afterward.

### Motion status (2026-06-13)
- ✅ M0 universal fade · ✅ M1 FFI+tick · ✅ M2 gesture velocity/fling/catch · ✅ M3 eased programmatic moves.
- ◑ **M4 tuning**: defaults set + mechanically verified (fling glides, loads tiles along the path with no grey trail, settles to idle; 0 errors). The three feel knobs to dial on a real device: engine fling **τ = 0.32 s** (`FlingAnimation::new`; iPhone ≈ 0.45 s = floatier), `MIN_FLING_VELOCITY = 120` px/s, ease durations 450/250 ms. Final taste calibration is a user feel-loop.
