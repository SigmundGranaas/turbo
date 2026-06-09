# Offline caching & tile management — analysis and improvement plan

_Android native app (`apps/android`). Written 2026-06-09._

This report analyses how the native Android app caches map tiles and manages
offline regions today, identifies the gaps, and proposes a prioritised plan to
improve it.

## 1. How it works today

### 1.1 Tile rendering (online)

- The app renders with **MapLibre Android SDK 13.2.0**
  (`gradle/libs.versions.toml:23`). All MapLibre access is confined to
  `:core:map` by design.
- Map styles are **raster** and generated on-device by
  `MapStyles.styleJson(...)`
  (`core/map/.../ui/map/MapStyles.kt`). Three base layers are wired:
  - **Norgeskart** — Kartverket topo WMTS
    (`https://cache.kartverket.no/v1/wmts/.../{z}/{y}/{x}.png`)
  - **OSM** — `https://tile.openstreetmap.org/{z}/{x}/{y}.png`
  - **Satellite** — Esri `World_Imagery` ArcGIS tile cache
- Transparent data overlays are composited on top: **Trails** (Waymarked
  Trails) and **Avalanche** (NVE Bratthetskart). **Waves/Wind** are declared
  but unwired (`overlayTiles(...)` returns `null`), so they are dead toggles.
- `TurboMap` (`core/map/.../ui/map/TurboMap.kt`) builds the style from JSON and
  loads it into a `MapView`. Tiles are fetched **straight from their remote
  source at runtime**.
- **Runtime cache:** the only thing caching browsed tiles is MapLibre's
  built-in *ambient cache* (a SQLite/`mbtiles`-style DB, default ceiling ~50 MB,
  LRU-evicted). It is **never configured** anywhere — no
  `setMaximumAmbientCacheSize`, no prefetch/concurrency tuning, no "clear
  cache". Online panning therefore warms only a small, silently-capped cache
  that is shared with the explicit offline regions in the same DB.

### 1.2 Offline regions (explicit download)

- Seam: `OfflineTileManager`
  (`core/map/.../core/map/OfflineTileManager.kt`). The real implementation,
  `MapLibreOfflineTileManager`, wraps MapLibre's `OfflineManager` +
  `OfflineTilePyramidRegionDefinition`.
- `LocalStyleServer` (`core/map/.../core/map/LocalStyleServer.kt`) is a small
  loopback HTTP server that hands the on-device style JSON to MapLibre's
  downloader (which only accepts `http(s)` for the style document). Clean
  workaround. **Note: it serves the *base-only* style** — overlays are not
  passed to `styleUrl(base)`.
- Tile-count limit: `setOfflineMapboxTileCountLimit(100_000)`.
- Downloads are triggered in two places, both via `OfflineViewModel.download`:
  1. **"Download this area"** from the layers sheet
     (`feature/map/.../MapScreenModals.kt:96`) → current visible bounds, zoom
     `floor(zoom)..floor(zoom)+4`, clamped to `8..16`
     (`feature/map/.../offline/OfflineViewModel.kt:42`).
  2. **Download along a route** (`RouteViewModel.downloadAlongRoute`,
     `feature/map/.../RouteViewModel.kt:243`) → a 1.5 km-padded corridor
     (`RouteCorridor.bounds`), fixed zoom `8..15`.
- Region naming is reverse-geocoded (place name, else coordinates).
- In **DEBUG** builds the manager is swapped for an in-memory
  `SyntheticOfflineTileManager` so the screen is driveable on an emulator
  (`OfflineModule`, `OfflineTileManager.kt:132`).

### 1.3 Offline UI

- `OfflineMapsScreen` (`feature/map/.../offline/OfflineMapsScreen.kt`): a list
  of regions with total size, per-region progress (wavy indicator), and
  delete-with-confirm. There is **no** add/download entry point from this
  screen (downloads start elsewhere), no rename, no map preview of a region's
  extent, and no pause/resume/retry.
- Domain model `OfflineRegionInfo` (`core/model/.../domain/Offline.kt`) carries
  only `id, name, complete, progress, sizeBytes`.

## 2. Problems & gaps

Ordered roughly by severity.

### P0 — correctness / robustness

1. **Failures are swallowed silently.** Every `onError` in
   `MapLibreOfflineTileManager` is `= Unit` — create, status, delete, and the
   observer's `mapboxTileCountLimitExceeded` are all ignored
   (`OfflineTileManager.kt:95,104,110,111`). `OfflineRegionInfo` has **no
   error/failed state**, so a download that fails (no network, server 5xx,
   tile-limit exceeded) simply sticks at a percentage forever with no message
   and no retry. This is the single biggest functional gap.
2. **Downloads don't survive backgrounding.** Downloading runs only while the
   process is alive and the region is `STATE_ACTIVE`. There is no foreground
   service or `WorkManager` job, so backgrounding the app or an OS kill stalls
   a multi-MB download with no resume. There is also no way to **pause or
   cancel** an in-flight download — `delete` is the only escape.
3. **No connectivity / metered-network awareness.** Downloads fire on any
   network, including metered cellular. There is no "download over Wi-Fi only"
   option and no auto-pause when connectivity drops.
4. **No size guardrail before committing.** The user is never shown an estimate
   ("~N tiles / ~120 MB") before a download starts. `RouteCorridor.spanDegrees`
   exists as an "absurd area" guard but is **unused**. A province-sized box at
   z16 can silently exceed the 100k tile limit — which then fails silently per
   (1).

### P1 — features / UX

5. **Safety-critical overlays are never available offline.** The download
   definition uses `styleUrl(base)` and `LocalStyleServer` serves a base-only
   style, so **avalanche (NVE Bratthet) and trail overlays are not downloaded**.
   For a Norwegian backcountry app this is the layer most needed offline.
6. **Base layer is not persisted and not tracked per region.** `baseLayer`
   lives only in in-memory `MapUiState` (`MapViewModel.kt:31,101`) and resets to
   Norgeskart every launch, despite a DataStore `SettingsRepository` already
   existing. A region is downloaded for **one** base layer; switch to Satellite
   offline and you see nothing, with no indication of which base a region
   covers.
7. **Thin region metadata.** No bounds, base layer, zoom range, created date, or
   tile count on `OfflineRegionInfo`. Consequences: can't show a region on a
   map, can't sort by date, can't dedupe/merge overlapping regions, can't
   "update" a stale one, and reverse-geocoded names collide.
8. **Coarse, fixed zoom policy with no user control.** `8..16` (area) and
   `8..15` (route) are hard-coded. No detail-vs-size trade-off control; z16 may
   be too shallow for detailed Kartverket topo, while a long route corridor at
   z8..15 can be huge.
9. **No update / staleness handling.** Tiles are fetched once and never
   refreshed; Kartverket topo updates over time with no TTL or "update region".

### P2 — polish / longer-term

10. **Unmanaged ambient cache & misleading disk reporting.** The runtime cache
    is left at MapLibre defaults; for a mapping app it should be sized up so
    recently-browsed areas survive going offline. The Offline screen's "total
    size" counts only explicit regions, **understating** true disk use, and
    there is no "clear cache" control.
11. **Thread-safety / refresh design.** `regionsById` is a plain `mutableMapOf`
    mutated from MapLibre callback threads. `refresh()` re-lists everything with
    a manual countdown and replaces the sorted list, which can interleave with
    the observer's `upsert` during active downloads and momentarily flicker/drop
    in-progress regions.
12. **Tile-licensing risk for offline caching.** Bulk-downloading from
    `tile.openstreetmap.org` violates the OSM tile usage policy, and Esri's
    public `World_Imagery` endpoint has terms that restrict bulk/offline
    caching. Caching these into offline regions is a compliance risk worth a
    review.

## 3. Recommendations

### P0 — make downloads trustworthy

- **Add a failure state and surface it.** Extend `OfflineRegionInfo` with a
  status (`Downloading | Complete | Failed(reason) | Paused`) and wire every
  `onError` + `mapboxTileCountLimitExceeded` to it. Show an error row with a
  **Retry** action.
- **Run downloads in a foreground service or `WorkManager`** with a network
  constraint, a persistent notification, and **pause/resume/cancel**, so they
  survive backgrounding and process death.
- **Add a "Download over Wi-Fi only" setting** (persisted) and auto-pause on
  metered/no connectivity.
- **Pre-flight size estimate + guardrail.** Estimate tile count/bytes from
  bounds × zoom span, show it in the confirm dialog, and reject areas above a
  threshold using the existing `RouteCorridor.spanDegrees`.

### P1 — make offline actually usable in the field

- **Include active overlays (esp. avalanche) in the download**, or at minimum
  record which layers a region covers and warn when they're missing offline.
- **Persist base layer + overlay selection** via the existing DataStore, and
  **store the base layer in region metadata**; warn when the current base
  differs from any downloaded region.
- **Enrich `OfflineRegionInfo`** (bounds, base, zoom range, date, tile count),
  render a region's extent on a mini-map, and allow **rename**.
- **Let the user pick a detail level** (e.g. Standard / Detailed) that maps to a
  zoom span, with the live size estimate from the guardrail above.

### P2 — strategic / housekeeping

- **Tune the ambient cache** (`setMaximumAmbientCacheSize`), expose **Clear
  cache**, and report *true* disk usage (ambient + regions).
- **Region update/refresh** for stale tiles and **merge/dedupe** of overlapping
  regions.
- **Harden concurrency**: guard `regionsById`, and replace the `refresh()`
  countdown with structured coroutines.
- **Move toward the planned custom tile server** (`apps/tileserver`, reserved in
  the monorepo) and **vector tiles + MBTiles** packaging. Vector tiles are far
  smaller offline, restylable on-device (dark mode, overlays without separate
  downloads), and route around the third-party tile-licensing risk — the
  highest-leverage long-term fix for both size and legal exposure.

## 4. Suggested sequencing

1. P0 error-state + retry + foreground/WorkManager downloads (correctness).
2. P0 Wi-Fi-only + pre-flight size estimate (cost control).
3. P1 overlay inclusion + base-layer persistence/metadata (field usefulness).
4. P1 richer region model + UI (rename, extent preview, detail level).
5. P2 ambient-cache tuning, updates/dedupe, and the tileserver/vector-tiles
   migration.

The first two items remove the "stuck at 47% with no explanation" and
"downloaded 400 MB on cellular" failure modes and are small, well-bounded
changes behind the existing `OfflineTileManager` seam.
