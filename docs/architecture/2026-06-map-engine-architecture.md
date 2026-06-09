# Map Engine Architecture — a renderer-agnostic, expression-driven map system

**Date:** 2026-06-09
**Status:** Design / direction-setting
**Supersedes:** the earlier MapLibre-migration plan (which treated MapLibre as
the spec and turbomap as a drop-in clone — wrong frame).

**Thesis:** Design a map system that is *better* than MapLibre — faster, more
expressive, with total native control — where "we can swap the renderer" is a
**property that falls out of a good contract**, not the goal. Migrating off
MapLibre/MapKit/flutter_map then becomes a corollary, provable in shadow mode,
not a rip-and-replace.

---

## Goals

1. **Beat MapLibre on expressiveness** — first-class portable custom layers,
   data-driven styling that's compiled (not interpreted), and a unified
   style+data model.
2. **Beat MapLibre on speed** — single wgpu backend, GPU-resident data-driven
   paint, tessellate-once mesh caching, no per-frame CPU expression loop.
3. **Keep total native control** — the host owns the surface, tile IO, offline,
   gestures, overlays, and *authors the map in its own typed language*.
4. **Make renderer migration effortless** — feature code never names a renderer;
   it talks to one contract that today's renderers and turbomap both implement.

## Non-goals (for this document)

- The native surface/render-loop glue per platform (covered separately; it is
  real work but isolated behind the contract).
- Label shaping / complex typography (turbomap is single-font today; richness
  stays in native overlays where needed).
- Replacing the host-side offline/tile-IO stacks — those are kept and reused.

---

## Premise: there are three incumbents, behind three thin seams

Only Android runs MapLibre. The real picture:

| Platform | Incumbent | Existing seam |
| --- | --- | --- |
| Android | MapLibre `android-sdk` 13.2.0 | `MapController` + `OfflineTileManager` (`apps/android/core/map`) |
| iOS | Apple **MapKit** (`MKMapView`) | `TurboMapView: UIViewRepresentable` (`apps/ios/Sources/CoreMap`) |
| Flutter | **flutter_map** 8.2.2 | layer/overlay registries (`apps/flutter/lib/app`) |

Each app *already* hides its renderer behind a thin seam. The architecture
generalizes those three embryonic seams into **one real contract** and makes the
renderer an implementation detail.

---

## The architecture in one picture

```
   Host app (Compose / SwiftUI / Flutter)
   ├─ authors an immutable Scene in its own typed language
   ├─ owns surface, tile IO/auth/offline, gestures, native overlays
   └─ talks ONLY to ↓
   ┌─────────────────────────────────────────────────────────┐
   │  MapEngine  (the contract — renderer-agnostic)          │
   │  attach/resize/detach · camera/project · apply(scene)   │
   │  hitTest · pending_tiles/ingest_tile · capabilities     │
   └─────────────────────────────────────────────────────────┘
        ▲ implemented by adapters        ▲ implemented natively
   ┌──────────────┐ ┌────────────┐ ┌──────────────┐ ┌──────────────┐
   │ TurbomapEngine│ │MapLibre adp│ │ MapKit adp   │ │flutter_map ad│
   │ (uniffi→Rust) │ │ (Kotlin)   │ │ (Swift)      │ │ (Dart)       │
   └──────────────┘ └────────────┘ └──────────────┘ └──────────────┘
        │
   ┌─────────────────────────────────────────────────────────┐
   │  turbomap-core (wgpu)  — pure (scene, camera, tiles)→px  │
   │  scene diff · compiled expressions · GPU-resident paint  │
   │  mesh cache (style-epoch keyed) · custom-layer passes    │
   └─────────────────────────────────────────────────────────┘
```

The **`MapEngine` contract** is the product. Everything above it is
renderer-blind; everything below the `TurbomapEngine` row is where we out-build
MapLibre.

---

## Pillar 1 — The `MapEngine` contract

A narrow, stable seam. Sketch (Rust-flavored; mirrored per host language):

```rust
trait MapEngine {
    // lifecycle — the GPU/surface half is constructed by NATIVE glue,
    // not uniffi (you cannot pass ANativeWindow/CAMetalLayer through uniffi).
    fn resize(&mut self, size: Size, scale_factor: f32);
    fn detach(&mut self);

    // camera + projection (turbomap already has all of this, tested)
    fn camera(&self) -> CameraState;
    fn set_camera(&mut self, c: CameraState);
    fn animate_camera(&mut self, target: CameraState, anim: Easing, dur_ms: u32);
    fn project(&self, geo: LngLat) -> ScreenPoint;       // lng_lat_to_screen
    fn unproject(&self, p: ScreenPoint) -> LngLat;       // screen_to_lng_lat
    fn visible_bounds(&self) -> LngLatBounds;

    // the whole map state, declaratively (Pillar 2)
    fn apply(&mut self, scene: Scene);

    // interaction (turbomap already has spatial-index hit_test)
    fn hit_test(&self, p: ScreenPoint, tol_px: f32) -> Vec<Hit>;

    // tiles — host owns IO; pull/push (turbomap already works this way)
    fn pending_tiles(&self) -> Vec<TileRequest>;
    fn ingest_tile(&mut self, key: TileKey, payload: TilePayload);

    // honest about what this backend can do (Pillar 5)
    fn capabilities(&self) -> Capabilities;
}
```

**Why this makes migration effortless:**
- `MapLibreEngine`, `MapKitEngine`, `FlutterMapEngine` are **adapters** over
  today's renderers. Shipping them changes *zero* behavior but moves every app
  onto the contract. From then on turbomap is **additive**.
- `TurbomapEngine` implements the same contract via uniffi.
- Two engines can run in **shadow mode** — same scene + camera, diff their
  output — so parity is *measured*, not hoped for, before any flag flips.

**What's already true in `turbomap-core`:** camera/project/unproject,
`hit_test`, and `pending_tiles`/`ingest_*` exist today (`map.rs`, `camera.rs`,
`hit.rs`, `spatial_index.rs`). The contract mostly *names* what core already
does. The new surface area is `apply(scene)` and `capabilities`.

---

## Pillar 2 — The Scene: immutable, typed, diffed

MapLibre splits the world into **static style JSON** and **imperative runtime
mutation**. Your Android code lives in that crack: `GeoJsonSource` +
`LineLayer` + `setGeoJson` exists *only* to push live data (track/route/measure)
into a style that was meant to be static. We delete the split.

**One immutable value describes the entire map:**

```rust
struct Scene {
    sources: Map<SourceId, SourceDef>,   // raster XYZ, vector MVT, geojson, DEM
    layers:  Vec<Layer>,                 // ordered bottom→top
    terrain: Option<TerrainConfig>,
    light:   LightConfig,
}

enum Layer {
    Raster   { id, source, opacity: Paint<f32> },
    Fill     { id, source, source_layer, filter, paint: FillPaint },
    Line     { id, source, source_layer, filter, paint: LinePaint },
    Symbol   { id, source, source_layer, filter, layout, paint: SymbolPaint },
    Circle   { id, source, source_layer, filter, paint: CirclePaint },
    Hillshade{ id, source, paint: HillshadePaint },
    Custom   { id, renderer: Arc<dyn CustomLayer> },   // ← portable wgpu pass
}
```

**The host rebuilds the `Scene` and calls `apply(scene)`** — React/Compose for
the map, which is exactly the mental model of all three hosts (Compose, SwiftUI,
Flutter). The engine **diffs** against the previous scene:

- Scenes use **structural sharing** — unchanged layers/sources are identity-equal,
  so the diff is O(changed), not O(scene).
- Source data changes (e.g. live GPS trace) touch one source; only dependent
  layers re-upload.
- Paint changes that don't affect geometry are cheap uniform/attribute updates;
  paint changes that *do* (line width feeding tessellation) bump the **style
  epoch** (Pillar 4) and invalidate just those cached meshes.

**Track/route/measure stop being special.** They're `Line`/`Circle` layers over
a `geojson` source. The whole `installTurboLayers` / `setGeoJson` machinery on
Android collapses into scene authoring. `LocalStyleServer` — which exists only
to feed MapLibre's HTTP-only style loader — is **deleted** on the turbomap path.

**Custom layers are first-class and portable.** A `CustomLayer` is a wgpu render
pass that runs *identically on every platform*. In MapLibre, custom layers are a
per-platform GL/Metal escape hatch you maintain three times. Here, heatmaps,
flow fields, bespoke 3D, particle effects = write-once Rust + WGSL. **This is the
single biggest expressiveness gap we open over MapLibre.**

**Today vs required:** `turbomap-core` has typed pipelines (raster, vector
fill/line, hillshade, markers, text) but no `Scene`/diff layer and an imperative
`ingest_*`/`set_terrain_source` API (`map.rs`). Required: a scene-diff layer that
sits on top of the existing pipelines and drives them, plus the typed `Layer`
enum (a superset of today's `VectorStyle::Rule`).

---

## Pillar 3 — Styling & expressions: typed, compiled, GPU-resident

**Decision: a typed native style API is first-class; a MapLibre GL Style Spec
importer is the interop adapter.** You get compile-time safety and speed for new
work, and keep Maputnik / existing styles working.

**Paint values are expressions, not constants:**

```rust
enum Paint<T> {
    Const(T),
    Zoom(Vec<(f32 /*zoom*/, T)>),           // interpolated zoom curve
    Data(Expr<T>),                           // data-driven on feature props
}
```

Three things make this beat MapLibre's interpreted expression engine:

1. **Compile at style-load, not per-frame.** A typed builder (or the GL importer)
   produces an `Expr<T>` AST that is lowered *once* to either a closure tree
   (CPU, for layout decisions) or a GPU schedule (for paint). MapLibre
   re-interprets expression trees on the CPU; we don't.
2. **Data-driven paint lives on the GPU.** Feature attributes referenced by paint
   expressions are packed into instance/vertex buffers at tessellation time;
   color/width/opacity are evaluated in WGSL with zoom as a uniform. The
   per-feature CPU paint loop — a real MapLibre hot spot — disappears.
3. **One AST, two front-ends.** The typed native builder and the GL Style Spec
   importer both target the same `Expr`/`Layer` IR, so interop and the native API
   never diverge.

**Typed native authoring (host-side, compile-checked):**

```kotlin
// Kotlin, generated binding over the shared IR
val scene = scene {
  rasterSource("base", xyz(kartverketTopoTemplate))
  geojsonSource("route", routeGeoJson)
  rasterLayer("base-l", source = "base")
  lineLayer("route-l", source = "route") {
    color = const(Color.RoutBlue)
    width = zoom(14 to 2.dp, 18 to 6.dp)        // zoom curve, GPU-evaluated
  }
}
engine.apply(scene)
```

**GL import (ecosystem path):** `Style.fromGL(json)` parses a MapLibre GL style
into the same `Scene`/`Expr` IR — existing styles and Maputnik keep working,
they just run on the faster backend.

**Today vs required:** `turbomap-core`'s `style.rs` has `VectorStyle` = ordered
`Rule`s with `Filter::Eq/In` and **constant** paint only. Required: the `Expr`
IR + compiler, `Paint<T>` zoom/data variants, the GPU attribute-packing path in
`tessellate.rs`, and the GL importer. The existing `Filter` is a strict subset of
the target — a clean starting point, not a rewrite.

---

## Pillar 4 — Speed architecture

- **Single wgpu backend, every platform.** No GL-vs-Metal divergence; one set of
  shaders, one perf profile. (`turbomap-core` is already pure wgpu 22.)
- **Tessellate-once, cache by style epoch.** Vector meshes + labels are cached
  (already: `VectorMeshCache`, `render/vector_cache.rs`). Key them by
  `(tile, style_epoch)` so a paint tweak that doesn't change geometry never
  re-tessellates.
- **GPU-resident data-driven paint** (Pillar 3) removes the per-feature CPU loop.
- **Instanced everything** — markers/circles already instance; extend to symbol
  and data-driven line/fill attributes.
- **Async pull/push tiles** — host fetches off-thread, pushes via `ingest_tile`;
  core never blocks on IO (already the contract).
- **Depth-tested terrain** with shared DEM cache (already: `render/terrain.rs`,
  `dem.rs`) — already richer than MapLibre's default hillshade.
- **Built-in profiling** — optional GPU timestamps already wired
  (`render/gpu_timestamps.rs`) for per-pass budgets.

---

## Pillar 5 — Native control & configuration (kept total)

The engine is a **pure function of `(scene, camera, tiles) → pixels`**. The host
keeps everything else:

- **Surface** — host creates the drawable and the render loop (Choreographer /
  CADisplayLink / Flutter texture); native glue constructs the engine over it.
- **Tile IO / auth / caching / offline** — stays host-side. iOS
  (`DiskOfflineTileManager`) and Flutter (`TileStore` + sqflite) already own this
  and feed the pull/push contract directly. Android replaces MapLibre's
  `OfflineManager` with the same kind of disk manager behind the existing
  `OfflineTileManager` interface.
- **Gestures → camera intent** — host translates touches; engine exposes
  `set_camera`/`animate_camera` + projection.
- **Native overlays** — pins/badges/sheets stay native (decided), reprojected via
  `project()`. Engine is not in the business of native UI.
- **Configuration** is explicit and typed, injected at construction:
  `Capabilities` (does this backend do terrain? custom layers? max texture?),
  VRAM budgets, prefetch margins, worker counts (`MapOptions` already models
  budgets). Adapters report reduced capabilities honestly so hosts degrade
  gracefully.

You lose none of MapLibre's imperative control — you *gain* compile-time safety,
because the map is authored in Kotlin/Swift/Dart, not stringly-typed JSON.

---

## How the contract spans languages

- **turbomap** exposes the contract via **uniffi** — generated Kotlin/Swift
  bindings over the Rust `MapEngine`. The control plane (camera/scene/hit-test/
  tiles) goes through uniffi cleanly. The **surface attach is native glue**
  (opaque window handle → `raw-window-handle` → wgpu surface), constructed
  outside uniffi and handed the engine.
- **Legacy adapters** implement the *same* host-language interface natively
  (`MapLibreEngine` in Kotlin, etc.). They don't go through uniffi at all.
- **The `Scene`/`Expr` IR is the shared schema.** Define it once and code-gen the
  per-language builders (or hand-mirror initially). Both the uniffi path and the
  native adapters consume the same scene values, which is what makes shadow-mode
  parity testing possible.

---

## Migration, as a corollary

1. **Define `MapEngine` + the `Scene` IR.** Wrap today's MapLibre/MapKit/
   flutter_map as adapters. Apps move onto the contract with **zero behavior
   change** — this ships first and de-risks everything after.
2. **Build `TurbomapEngine`** behind the same contract + the Android surface glue
   (the one genuinely fiddly piece). Prove it in **shadow mode** against the
   MapLibre adapter on real scenes.
3. **Flip the flag per platform** once shadow parity holds. Android first (only
   true MapLibre incumbent), then iOS, then a Flutter GPU-embedding spike.

The scary surface/render-loop work from the old plan still exists, but it's now
isolated behind the contract and *measured* before it ships.

---

## Where `turbomap-core` stands against this architecture

| Capability | Today | Required for this architecture |
| --- | --- | --- |
| Pure wgpu, headless, FFI-clean | ✅ | keep |
| Camera / project / hit-test / tiles pull-push | ✅ (`map.rs`, `camera.rs`, `hit.rs`) | name them in the contract |
| Typed pipelines (raster/fill/line/symbol/circle/hillshade) | ⚠️ most present; markers=discs | superset into `Layer` enum |
| **Scene value + diff** | ❌ imperative `ingest_*` | **new layer on top of pipelines** |
| **Expression IR + compiler** | ❌ constant paint, eq/in filters | **new** (`Expr`, `Paint<T>`) |
| **GPU-resident data-driven paint** | ❌ | extend `tessellate.rs` + shaders |
| **GL Style Spec importer** | ❌ | new adapter → shared IR |
| **Portable custom layers** | ❌ | new `CustomLayer` trait + pass slot |
| uniffi `MapEngine` bindings | ❌ none | new `turbomap-ffi` crate |
| Mesh cache | ✅ (`vector_cache.rs`) | re-key by style epoch |
| Terrain / DEM | ✅ (beats MapLibre default) | keep |

The foundation is strong and already exceeds MapLibre on terrain; the net-new
work is the **scene/diff layer, the expression system, custom layers, and the
contract crate** — i.e. exactly the things that make it *more expressive*, not
just equivalent.

---

## Risks & mitigations

| Risk | Mitigation |
| --- | --- |
| Scope creep — "rebuild all of MapLibre" | The contract + adapters ship first and stand alone; turbomap features land incrementally behind a flag with shadow parity. Never a big-bang. |
| Expression system is a deep rabbit hole | Start with `Const`/`Zoom` (covers most real styles), add `Data` expressions GPU-side after; GL importer can stub unsupported ops and report via `capabilities`. |
| GPU data-driven paint complexity | Land it for `Line`/`Circle` first (your actual overlays), generalize later. |
| Surface/render-loop glue (Android first) | Isolated behind the contract; provable in shadow mode before flipping the flag. |
| Flutter GPU embedding unknown | Gated behind a spike; Flutter can stay on the `flutter_map` adapter indefinitely. |

---

## Open questions for the next iteration

1. **Scene IR home** — define once + code-gen per-language builders, or hand-write
   thin builders per host over a uniffi-described IR? (Affects tooling investment.)
2. **Symbol/label richness** — how far do we push GPU label placement vs leaving
   rich labels to native overlays? (turbomap is single-font today.)
3. **Capability degradation policy** — when an adapter can't honor a scene
   (e.g. MapKit + custom layer), do we no-op, fall back, or refuse at `apply`?

## Recommended first artifact

Define the **`MapEngine` contract + `Scene`/`Expr` IR** and wrap the *current*
renderers as adapters — the move that puts every app on the contract with zero
behavior change and unlocks shadow-mode parity. Everything expressive (custom
layers, GPU paint, GL import) then lands incrementally behind it.
