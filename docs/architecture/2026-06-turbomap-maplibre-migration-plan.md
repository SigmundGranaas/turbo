# turbomap ↔ MapLibre Migration — Implementation Plan

**Date:** 2026-06-09
**Subject:** Replacing the per-platform native map renderers with the in-tree
`apps/turbomap` wgpu renderer, exposed to the host apps via **uniffi**.
**Goal:** A single Rust/wgpu map core (`turbomap-core`) driving Android, iOS,
and Flutter, behind the map seams those apps already have.

---

## Premise correction (read this first)

The task was framed as "replace the native MapLibre instances," but only **one**
of the three apps actually runs MapLibre. The real incumbents are:

| Platform | Current renderer | Seam already in place |
| --- | --- | --- |
| **Android** | MapLibre `org.maplibre.gl:android-sdk:13.2.0` | `MapController` interface + `OfflineTileManager` (`apps/android/core/map`) |
| **iOS** | Apple **MapKit** (`MKMapView`) — *not* MapLibre | `TurboMapView: UIViewRepresentable` (`apps/ios/Sources/CoreMap`) |
| **Flutter** | **flutter_map** 8.2.2 (pure Dart) — *not* MapLibre | layer/overlay registries (`apps/flutter/lib/app`) |

This is therefore **three replacements against three incumbents**, not one. The
saving grace: every app already isolates its renderer behind a thin seam, so
**feature code barely changes**. The work concentrates in `core/map` (Android),
`CoreMap` (iOS), and the tile-provider/layer host (Flutter).

---

## Design rules (non-negotiable)

1. **`turbomap-core` stays headless.** No winit, no HTTP, no platform handles in
   core. It already depends only on `wgpu`, `glam`, `bytemuck`, `lyon`,
   `ab_glyph`, `log`, `thiserror`. The migration must not regress this.
2. **uniffi owns the control plane only.** Camera, sources, styling intent,
   hit-test, projection — plain functions over structs/strings/bytes. uniffi
   never sees a GPU handle.
3. **Surface + render loop is hand-written native glue.** Per platform: create
   the wgpu surface from the native drawable, drive a vsync'd render loop. This
   is the genuinely fiddly part and is kept small and explicit.
4. **Markers stay native overlays.** Pins/badges/icons remain
   Compose/SwiftUI/Flutter widgets reprojected via the projection API. Confirmed
   decision — keeps icon richness off the Rust critical path and out of scope.
5. **Offline tiles stay host-side.** iOS and Flutter already own disk caches;
   Android replaces MapLibre `OfflineManager` with the same kind of disk manager.
   turbomap's pull/push tile contract feeds whatever the host provides.
6. **Every phase is independently shippable** behind a feature flag, with the old
   renderer still selectable until parity is proven.

---

## The boundary that uniffi does *not* cross

This is the central technical fact that shapes the whole design.

**uniffi handles** (free, ~1:1 with the existing `MapController`):
`setCamera`, `flyTo`, `setBaseLayer`, `setOverlays`, `setRouteGeoJson`,
`hitTest`, `screenToLatLng`, `latLngToScreen`, `setBottomInset`, `resetNorth`,
`visibleBounds`, `ingestTile`, `pendingTiles`.

**uniffi does *not* handle** the GPU plane — you cannot pass an `ANativeWindow`
or `CAMetalLayer` through a uniffi type. Per platform you need a small,
hand-written shim:

- **Android:** `SurfaceView`/`TextureView` → `Surface` → `ANativeWindow`;
  create the wgpu surface inside Rust via `raw-window-handle`. The window handle
  crosses FFI as an opaque `u64`/pointer, **not** a uniffi value. Render loop
  driven by `Choreographer` (vsync) calling a Rust `render()`.
- **iOS:** back the view with `CAMetalLayer`; pass its pointer to Rust;
  wgpu-on-Metal renders into it. Loop driven by `CADisplayLink`.
- **Flutter:** the riskiest surface story — Flutter does not hand you a GPU
  drawable cheaply. Options: platform view embedding the native surface, or the
  Texture/`FlutterGpu` path. Spike before committing.

Budget ~200–400 lines of native glue per platform for surface lifecycle
(create/resize/background/context-loss) and the loop. That is where the
debugging time goes — **not** the renderer.

---

## What `turbomap-core` already gives us (no new work)

- **FFI-clean core** that compiles to `aarch64-linux-android` and
  `aarch64-apple-ios` as-is.
- **Host boundary == the uniffi seam:** `Map::new(device, queue, format, size,
  camera, options)`; `render(encoder, view)`; `pending_tiles()`; `ingest_*()`.
  `turbomap-app` already proves this end-to-end against a real surface — the
  mobile port swaps winit for the platform drawable.
- **Interaction math done & tested:** continuous zoom, pitch (0–60°), bearing,
  `ease_to`, `screen_to_lng_lat` / `lng_lat_to_screen`, spatial-index `hit_test`.
- **Pipelines:** raster XYZ (+LRU VRAM cache), transparent raster overlays,
  Lyon line tessellation (width, round cap/join), instanced markers/circles,
  text atlas, DEM hillshade/terrain (exceeds MapLibre's default here).

**Not yet present:** any uniffi/JNI/wasm scaffolding — started from scratch here.

---

## Content/feature gap to close

| App feature | turbomap today | Action |
| --- | --- | --- |
| Raster base tiles (Kartverket/OSM/Esri XYZ) | ✅ raster + VRAM cache | reuse |
| Transparent raster overlays (trails, avalanche) | ✅ layer compositing | reuse |
| Track / route / measure polylines | ✅ Lyon lines, width, round cap/join | add per-line color/width styling parity |
| Measure points / user dot | ✅ markers + vector circles | minor |
| Waypoint pins (A/B/C, icons) | discs only | **keep native overlay** (decided) |
| Camera / projection / hit-test | ✅ tested | reuse |
| Terrain / hillshade (DEM) | ✅ bonus | reuse |
| **Style** | custom minimal `VectorStyle` (eq/in filters) | **no MapLibre GL JSON parser** — re-express, see below |
| Offline tiles | ❌ (by design) | host-side managers |

### The Android styling gap, concretely

Android today generates MapLibre v8 style JSON on-device (`LocalStyleServer` +
`MapStyles.styleJson()`) and renders dynamic geometry via `GeoJsonSource` +
`LineLayer`/`CircleLayer` with `PropertyFactory`. None of that maps onto a
GL-JSON engine in turbomap (there isn't one). Instead:

- **Base + overlay rasters** → turbomap raster layers fed by the existing XYZ
  templates in `MapStyles`/`MapTileStyles`. `LocalStyleServer` is **deleted** on
  the turbomap path — it only existed to satisfy MapLibre's HTTP-only style fetch.
- **Track/route/measure/user** GeoJSON sources → turbomap vector line meshes +
  markers, updated through a `setGeoData(id, geojson)` uniffi call that mirrors
  today's `getSourceAs<GeoJsonSource>(id).setGeoJson(...)`.

This is **re-expression, not a parser project** — the overlays are simple lines
and circles. But it is real, Android-specific work and lands in the Android phase.

---

## Phasing (Android-first pilot — decided)

Android is the pilot because it is the only true MapLibre incumbent; proving it
first de-risks the biggest target. Each phase is independently shippable behind a
renderer flag, with the existing renderer as fallback until parity.

### Phase 0 — `turbomap-ffi` crate (uniffi control plane)  *(headless, testable)*

**New crate:** `apps/turbomap/crates/turbomap-ffi`

- uniffi-wrap `turbomap-core`'s control surface: camera ops, `setBaseLayer`,
  `setOverlays`, `setGeoData`, `hitTest`, `screenToLatLng`/`latLngToScreen`,
  `setBottomInset`, `resetNorth`, `visibleBounds`, tile ingest/pending.
- **No surface, no GPU handle** in this crate's public API. A `MapHandle` opaque
  object holds the `Map` and is created by the *native* glue (Phase 1), not by
  uniffi directly.
- Generate Kotlin + Swift bindings; wire `cargo-ndk` (Android) and an
  `xcframework` build (iOS) into the existing build.
- **Verify:** headless unit tests on the FFI types; binding generation in CI.

### Phase 1 — Android surface glue + render loop  *(the real pilot)*

- `SurfaceView`/`TextureView` → `Surface` → `ANativeWindow`; pass the handle as
  `u64` into a hand-written JNI entry that builds the wgpu surface via
  `raw-window-handle` and constructs `Map`, returning the `MapHandle` uniffi owns.
- `Choreographer`-driven loop calls `render()`; handle `surfaceChanged` (resize),
  `surfaceDestroyed` (drop), and app background/foreground.
- Tile fetch pump reuses the existing Android tile/offline stack, pushed into
  core via `ingestTile`.
- **Verify:** a real `MapView` replacement showing Kartverket base + pan/zoom on
  device. This is the make-or-break milestone.

### Phase 2 — Android parity behind `MapController`

- Implement the full `MapController` interface against the FFI: `zoomIn/Out`,
  `flyTo`, `center`, `fromScreen`/`toScreen`, `visibleBounds`, `setBottomInset`,
  `zoom`, `bearing`, `resetNorth`, `frameTo`.
- Re-express track/route/measure/user overlays as turbomap line/marker data via
  `setGeoData`. Keep the Compose `MarkerPin` overlay exactly as-is, reprojected
  through `toScreen`.
- Replace MapLibre `OfflineManager` with a disk tile manager behind the existing
  `OfflineTileManager` interface (mirror the iOS `DiskOfflineTileManager` design).
- Delete `LocalStyleServer`; drop the MapLibre dependency from `core/map`.
- **Verify:** feature parity pass on the map screen + offline download flow;
  `ArchitectureBoundaryTest` updated.

### Phase 3 — iOS port

- `CAMetalLayer`-backed view, pointer into Rust, wgpu-Metal render loop on
  `CADisplayLink`. Cleanest surface path of the three.
- Implement against `TurboMapView`'s existing surface; reuse
  `DiskOfflineTileManager` and the `MKTileOverlay` URL templates as tile sources.
- Annotations stay `MKAnnotationView`/native overlay (decided).

### Phase 4 — Flutter spike, then port

- **Spike first:** prove GPU embedding (platform view vs. Texture/`FlutterGpu`)
  before committing scope. This is the largest unknown in the whole plan.
- On success, host the FFI behind the existing layer/overlay registries; keep the
  pluggable `TileProvider`/`TileStore` for offline.

---

## Risks & mitigations

| Risk | Mitigation |
| --- | --- |
| Surface lifecycle / context-loss bugs in native glue | Keep glue minimal & explicit; lean on `turbomap-app`'s proven Lost/Outdated recovery as reference; test background/rotate/resize early in Phase 1. |
| Flutter GPU embedding may be infeasible cheaply | Gated behind a spike (Phase 4); Flutter can stay on flutter_map indefinitely if the spike fails — it has the loosest app coupling. |
| Styling parity for Android GeoJSON layers | Scope is just lines + circles; `setGeoData` mirrors current `setGeoJson` call sites 1:1. |
| Text/label richness (single-font Roboto, no halos, Latin-only) | Out of scope for basemap parity; markers/labels stay native overlays where richness is needed. |
| Build-system complexity (cargo-ndk, xcframework, uniffi codegen) | Land in Phase 0 in isolation, in CI, before any app depends on it. |

---

## Recommendation

Greenlight. The core is genuinely built for this and beats MapLibre on terrain.
Sequence: **Phase 0 uniffi wrapper → Phase 1 Android surface pilot (make-or-break)
→ Phase 2 Android parity → Phase 3 iOS → Phase 4 Flutter spike.** Keep markers
native and offline tiles host-side; let uniffi own the control API and hand-write
only the surface/vsync glue.
