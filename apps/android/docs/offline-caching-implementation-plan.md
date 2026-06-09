# Offline caching & tile management — implementation plan

_Companion to `offline-caching-analysis.md`. Android native app (`apps/android`)._

This plan turns every finding in the analysis into concrete, sequenced work. It
is organised into four phases (P0 → P3). Each task lists the **goal**, the
**files** touched, the **approach**, and the **tests**. Everything stays behind
the existing `OfflineTileManager` seam in `:core:map`, so feature code and tests
change minimally.

Legend: ✚ new file · ✎ edit · 🧪 test

---

## Phase 0 — Foundation: model, seam, and a testable core

These are prerequisites for almost everything else, so they land first.

### 0.1 Rich region model + status

- **Goal:** replace the thin `OfflineRegionInfo` with a model that carries
  status (incl. failure), extent, base layer, overlays, zoom range, tile count
  and creation time. Fixes findings **#1, #7**.
- **Files:**
  - ✎ `core/model/.../domain/Offline.kt`
- **Approach:**
  ```kotlin
  enum class OfflineStatus { Downloading, Complete, Paused, Failed }

  data class OfflineRegionInfo(
      val id: Long,
      val name: String,
      val status: OfflineStatus,
      val progress: Float,            // 0..1
      val sizeBytes: Long,
      val tileCount: Long,
      val base: BaseLayer,
      val overlays: Set<OverlayId>,
      val bounds: GeoBounds,
      val minZoom: Double,
      val maxZoom: Double,
      val createdAtEpochMs: Long,
      val errorReason: String? = null,
  ) {
      val complete get() = status == OfflineStatus.Complete   // keep call sites compiling
  }
  ```
  Keep a `complete` convenience getter so existing UI keeps working during the
  migration.
- 🧪 No new test (pure data); downstream tests updated in their own tasks.

### 0.2 Region metadata codec

- **Goal:** MapLibre only persists an opaque `byte[]` per region. Today it
  stores just the name. Encode the full metadata (name, base, overlays, bounds,
  zoom, createdAt) as JSON so it round-trips. Fixes findings **#6, #7, #9**.
- **Files:**
  - ✚ `core/map/.../core/map/OfflineRegionMetadata.kt`
- **Approach:** a pure object with `encode(meta): ByteArray` /
  `decode(bytes): Meta?` using `org.json` (already available) — no MapLibre
  imports, fully unit-testable. Be defensive: legacy regions whose metadata is a
  bare name string decode to a `Meta` with sensible defaults.
- 🧪 ✚ `OfflineRegionMetadataTest` — round-trip, legacy-name fallback, malformed
  bytes → null.

### 0.3 Tile-count / size estimator + area guard

- **Goal:** a pure estimator for "how many tiles / bytes will this download
  be?" plus the absurd-area guard. Fixes finding **#4** (and feeds #8).
- **Files:**
  - ✚ `core/map/.../core/map/TileMath.kt`
  - ✎ `core/map/.../core/map/RouteCorridor.kt` (reuse `spanDegrees`)
- **Approach:** standard slippy-map math. For each integer zoom `z` in
  `min..max`, tile range from `lon2tileX/lat2tileY` over the bounds; sum the
  tile counts; `bytes ≈ tiles × AVG_RASTER_TILE_BYTES` (≈ 20 KB, tunable).
  Multiply by the number of active sources (base + overlays). Expose
  `estimate(bounds, min, max, sources): OfflineEstimate(tiles, bytes)` and
  `isWithinLimits(estimate, span)`.
- 🧪 ✚ `TileMathTest` — known bbox/zoom → expected tile count; monotonic in zoom
  span; limit predicate.

### 0.4 Expand the `OfflineTileManager` seam

- **Goal:** add the operations the new UX needs, and update **all three**
  implementations. Fixes the plumbing for **#1, #2, #8, #9, #10**.
- **Files:**
  - ✎ `core/map/.../core/map/OfflineTileManager.kt` (interface + real impl)
  - ✎ `core/map/.../core/map/SyntheticOfflineTileManager.kt`
  - ✎ `feature/map/.../offline/...Test` `StubOfflineTileManager`
- **Approach:** new interface surface:
  ```kotlin
  fun download(spec: DownloadSpec)        // spec carries base, overlays, bounds, min/max zoom, detail, name
  fun retry(id: Long)
  fun pause(id: Long)
  fun resume(id: Long)
  fun cancel(id: Long)                     // alias/clarity over delete for in-flight
  fun delete(id: Long)
  fun estimate(spec: DownloadSpec): OfflineEstimate
  fun clearAmbientCache()
  ```
  `DownloadSpec` replaces the long positional `download(...)` parameter list.
  `SyntheticOfflineTileManager` grows status transitions (a coroutine that ramps
  progress, can be paused/failed) so the screen and its tests can drive every
  state without MapLibre.
- 🧪 ✎ `SyntheticOfflineTileManagerTest` — pause/resume/retry/cancel and a
  forced-failure path.

---

## Phase 1 — P0 correctness & robustness

### 1.1 Surface failures + retry

- **Goal:** stop swallowing errors. Fixes finding **#1**.
- **Files:**
  - ✎ `core/map/.../core/map/OfflineTileManager.kt`
  - ✎ `feature/map/.../offline/OfflineMapsScreen.kt`
  - ✎ `feature/map/.../offline/OfflineViewModel.kt`
  - ✎ string resources (`feature/map/.../res/values*/strings.xml`)
- **Approach:** wire every `onError` and `mapboxTileCountLimitExceeded` in
  `MapLibreOfflineTileManager` to set `OfflineStatus.Failed` with a reason
  string via `upsert`. Add a **Failed** card variant with an error message and a
  **Retry** button (`viewModel.retry(id)` → `manager.retry`).
- 🧪 ✎ `OfflineMapsScreenTest` — a failed region renders the error + retry;
  retry calls the manager.

### 1.2 Foreground download service (survives backgrounding)

- **Goal:** downloads continue when the app is backgrounded and can be
  paused/resumed/cancelled; recover after process death. Fixes finding **#2**.
- **Files:**
  - ✚ `feature/recording`-style service module or `core/map`:
    `OfflineDownloadService` (foreground service)
  - ✚ `core/map/.../core/map/OfflineDownloadStore.kt` (persisted desired-state)
  - ✎ `app/.../AndroidManifest.xml` (service + `FOREGROUND_SERVICE*` perms)
  - ✎ `OfflineTileManager` real impl (drive state via the service)
- **Approach:** MapLibre keeps downloading only while the process lives and the
  region is `STATE_ACTIVE`. A foreground service with a progress notification
  keeps the process alive and exposes pause/resume/cancel notification actions.
  Persist each region's *desired* state (active/paused) in a small DataStore so
  that on cold start we can re-activate incomplete regions (gated by the
  Wi-Fi-only policy from 1.3). The service stops itself when no region is
  active. Mirror the existing `RecordingService` pattern already in the repo for
  consistency.
- 🧪 service logic is hard to unit-test; cover the **desired-state store** and
  the **resume-decision** function (pure) instead. 🧪 ✚ `OfflineDownloadStoreTest`.

### 1.3 Connectivity / Wi-Fi-only policy

- **Goal:** never download on metered networks unless allowed; auto-pause when
  constraints break. Fixes finding **#3**.
- **Files:**
  - ✚ `core/map/.../core/map/DownloadPolicy.kt` (pure decision fn)
  - ✚ connectivity observer (ConnectivityManager `NetworkCallback`) in the
    service/manager
  - ✎ `core/model/.../domain/UserSettings.kt` (+`downloadOverWifiOnly`)
  - ✎ `core/data/.../SettingsRepository.kt` (persist the flag)
  - ✎ `feature/settings/...` (a toggle in settings)
- **Approach:** `DownloadPolicy.shouldRun(settings, network): Boolean` is pure
  and tested. The observer feeds `(isWifi, isMetered, isConnected)`; when the
  policy flips to false the service pauses active regions (`setDownloadState
  INACTIVE`, status → `Paused`), and resumes when it flips back.
- 🧪 ✚ `DownloadPolicyTest` — wifi-only × {wifi, metered, none}; allow-cellular
  case.

### 1.4 Pre-flight estimate + guardrail in the UX

- **Goal:** show size before committing and block absurd areas. Fixes finding
  **#4** (uses 0.3).
- **Files:**
  - ✎ `feature/map/.../MapScreenModals.kt` (the "download this area" path)
  - ✚ a confirm dialog (or extend `MapLayersSheet`) showing tiles/MB + detail
    level
  - ✎ `feature/map/.../offline/OfflineViewModel.kt` (`estimate(spec)`)
  - ✎ `feature/map/.../RouteViewModel.kt` (`downloadAlongRoute` shows estimate)
- **Approach:** before `download`, call `manager.estimate(spec)`; render
  "~N tiles · ~120 MB". If `!isWithinLimits`, disable confirm with an
  explanation ("Area too large — zoom in or lower detail"). Replaces the current
  fire-and-forget `offlineViewModel.download(...)`.
- 🧪 ✎ `OfflineViewModelTest` — estimate surfaced; over-limit blocks download.

---

## Phase 2 — P1 features / field usefulness

### 2.1 Include overlays in offline downloads

- **Goal:** download avalanche/trail overlays alongside the base. Fixes finding
  **#5**.
- **Files:**
  - ✎ `core/map/.../core/map/LocalStyleServer.kt` (serve base **+ overlays**)
  - ✎ `core/map/.../core/map/OfflineTileManager.kt` (style URL carries overlays)
  - ✎ `core/map/.../ui/map/MapStyles.kt` (already supports overlays in
    `styleJson(base, overlays)` — reuse)
- **Approach:** encode overlays in the style URL (e.g.
  `/topo.json?ov=Avalanche,Trails`); `LocalStyleServer.serve` parses the query
  and calls `MapStyles.styleJson(base, overlays)`. The downloader then fetches
  overlay tiles into the region. Persist the overlay set in region metadata
  (0.2) and show it on the card.
- 🧪 ✎ `LocalStyleServerTest` (✚ if absent) — requested overlays appear in the
  served JSON; unknown overlay ignored.

### 2.2 Persist base layer + record it per region

- **Goal:** base-layer choice survives relaunch; each region knows its base;
  warn on mismatch offline. Fixes finding **#6** (+ surfaces #7 data).
- **Files:**
  - ✎ `core/model/.../domain/UserSettings.kt` (+`baseLayer`, `overlays`)
  - ✎ `core/data/.../SettingsRepository.kt`
  - ✎ `feature/map/.../MapViewModel.kt` (load initial base from settings;
    `setBaseLayer` persists)
  - ✎ `feature/map/.../offline/OfflineMapsScreen.kt` (show base/overlays chips)
- **Approach:** seed `MapUiState.baseLayer` from settings on init; persist on
  change. Region cards display the base (and overlay chips). Optional: a banner
  when the active base differs from all downloaded regions.
- 🧪 ✎ `MapViewModelTest` — initial base from settings; change persists.

### 2.3 Detail level (zoom span) control

- **Goal:** user picks Standard/Detailed; estimate updates live. Fixes finding
  **#8**.
- **Files:**
  - ✎ `core/model` (✚ `DetailLevel` enum → zoom span mapping) or keep in
    `DownloadSpec`
  - ✎ the download confirm UI (2.x / 1.4)
  - ✎ `OfflineViewModel` / `RouteViewModel` (map detail → min/max zoom)
- **Approach:** replace the hard-coded `floor(zoom)..+4` / `8..15` with a
  `DetailLevel` → `(minZoom, maxZoom)` table; recompute the estimate when the
  user changes it. Keep current values as the **Standard** default.
- 🧪 ✎ `OfflineViewModelTest` — detail level maps to expected zoom span.

### 2.4 Region management UX (rename, extent preview, update)

- **Goal:** richer Offline screen. Fixes findings **#7, #9**.
- **Files:**
  - ✎ `feature/map/.../offline/OfflineMapsScreen.kt`
  - ✎ `OfflineTileManager` (`rename`, `updateRegion`)
- **Approach:** card shows created date + tile count; a small static map
  thumbnail of `bounds`; **Rename** (edit metadata via 0.2); **Update** (re-run
  the same `DownloadSpec` to refresh tiles). Sort options (name/date/size).
- 🧪 ✎ `OfflineMapsScreenTest` — rename/update actions invoke the manager;
  metadata renders.

---

## Phase 3 — P2 polish / strategic

### 3.1 Ambient-cache tuning + accurate disk usage + clear cache

- **Goal:** size the runtime cache, report true disk use, allow clearing. Fixes
  finding **#10**.
- **Files:**
  - ✎ `core/map/.../core/map/OfflineTileManager.kt`
    (`setMaximumAmbientCacheSize`, `clearAmbientCache`)
  - ✎ `feature/map/.../offline/OfflineMapsScreen.kt` ("Clear cache" + real disk
    total)
- **Approach:** configure the ambient ceiling on init (e.g. 256 MB, tunable);
  report disk usage from the offline DB file size (regions + ambient) rather
  than summing region sizes only; add a **Clear cache** action (regions
  untouched).
- 🧪 ✎ `OfflineMapsScreenTest` — clear-cache action wired.

### 3.2 Concurrency hardening

- **Goal:** remove the racy `refresh()` and unsynchronised map. Fixes finding
  **#11**.
- **Files:**
  - ✎ `core/map/.../core/map/OfflineTileManager.kt`
- **Approach:** wrap the MapLibre list/status callbacks in
  `suspendCancellableCoroutine`, gather with `async`/`awaitAll` on a confined
  dispatcher; confine `regionsById` to that single context (or guard with a
  mutex). Removes the manual countdown and the upsert/replace interleave.
- 🧪 covered indirectly; add a focused test on the status-aggregation helper if
  it can be extracted purely.

### 3.3 Licensing posture + tileserver/vector path (strategic)

- **Goal:** de-risk caching third-party tiles and set up the long-term fix.
  Fixes finding **#12**.
- **Files:** docs + a feature flag.
- **Approach (incremental):**
  1. Gate offline downloads of OSM/Esri behind a flag (or restrict offline to
     **Kartverket**, which permits caching) until licensing is cleared.
  2. Track the planned **`apps/tileserver`** + **vector tiles / MBTiles**
     migration as the durable fix: smaller offline footprint, on-device
     restyling (dark mode, overlays without separate downloads), and
     licence-clean sources. This is a larger workstream — capture it as its own
     design doc rather than a single task here.

---

## Dependency / sequencing summary

```
0.1 model ─┬─ 0.2 metadata ─┐
           ├─ 0.3 estimator ─┼─ 0.4 seam ─┬─ 1.1 failures+retry
           │                 │            ├─ 1.2 fg service ── (needs 1.3 policy)
           │                 │            ├─ 1.3 wifi policy
           │                 │            └─ 1.4 estimate UX
           └───────────────────────────────  2.1 overlays
                                              2.2 persist base
                                              2.3 detail level (needs 1.4)
                                              2.4 region UX (needs 0.2)
                                              3.1 / 3.2 / 3.3 (independent)
```

Recommended merge order: **0.x → 1.1 → 1.3 → 1.2 → 1.4 → 2.1 → 2.2 → 2.3 →
2.4 → 3.1 → 3.2 → 3.3.** The Phase 0 + 1.1/1.4 slice alone removes the two worst
failure modes ("stuck at X% with no error" and "huge download with no warning")
and is shippable on its own.

## Cross-cutting checklist

- **Backwards compat:** legacy regions (name-only metadata) must decode to a
  valid `OfflineRegionInfo` (0.2) — they predate the new fields.
- **DEBUG parity:** every new seam method must be implemented in
  `SyntheticOfflineTileManager` and the test `StubOfflineTileManager`, or the
  Offline screen breaks on emulator and tests stop compiling.
- **Architecture boundary:** no MapLibre imports leak out of `:core:map`
  (enforced by `ArchitectureBoundaryTest`) — keep pure logic (`TileMath`,
  `DownloadPolicy`, `OfflineRegionMetadata`) MapLibre-free.
- **Strings/i18n:** new user-facing strings in both `values/` and `values-nb/`.
- **Permissions:** foreground-service + connectivity permissions added to the
  manifest with the narrowest type.
