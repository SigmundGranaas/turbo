# Android offline tiles — off MapLibre, onto the wgpu cache

Status: build plan (2026-06-23). We are going all-in on the wgpu map and removing
MapLibre entirely. MapLibre is contained to TWO files in `:core:map`
(`ui/map/TurboMap.kt` — the map view; `OfflineTileManager.kt` — the offline
downloader, `MapLibreOfflineTileManager` via `OfflineManager`) plus one gradle
dep (`core/map/build.gradle.kts: libs.maplibre`). The downloader gates the
dependency removal, so it must be **rebuilt off MapLibre first**.

## How offline actually has to work now

The wgpu map serves tiles **read-through from a disk cache**
(`TurbomapMapView.launchTileFetch`: `tileCache.get(layer,z,x,y) ?: fetch+put`).
`TurbomapTileCache` (`:core:turbomap-android`, layout
`<dir>/<layer>/<z>/<x>_<y>.tile`, dir `<context.cacheDir>/turbomap-tiles-v2`) is
stateless-on-disk. So **offline = pre-populate that exact cache** for a region's
tiles; the map then renders them with zero network. No instance sharing needed —
just the same dir + layout.

## Design

A new `WgpuOfflineTileManager : OfflineTileManager` (replaces
`MapLibreOfflineTileManager`), in `:core:map`:

- **Tile store seam:** expose a public, non-MapLibre tile store from
  `:core:turbomap-android` — `TileStore` (`get/put/remove/exists`, the
  `<layer>/<z>/<x>_<y>.tile` layout) + a shared `TURBOMAP_TILE_DIR` constant for
  the `turbomap-tiles-v2` namespace. Add `:core:map` → `:core:turbomap-android`
  dep (acyclic; turbomap-android depends only on model+designsystem). The map
  controller and the offline manager both open a store on the same dir.
- **Download:** enumerate the region's tiles via `TileMath` (add a public
  `tilesFor(bounds, minZoom, maxZoom)` iterator) for the base + overlay + DEM
  layers, using the SAME URL templates the map uses (`MapStyles`). Fetch each
  with OkHttp (bounded concurrency, the existing fetch budget feel), `store.put`
  into the shared cache, stream progress → `OfflineRegionInfo`. Honor the
  network-allowed policy + pause/resume/retry via a per-region coroutine `Job`.
- **Metadata:** persist `OfflineRegionInfo` (id, name, bounds, zoom span, base,
  overlays, status, tileCount, bytes, createdAt) in a small store — Room table
  in `:core:data` (preferred — the app already uses Room) or a JSON file in
  `filesDir`. Survives relaunch; `regions` StateFlow reads it.
- **delete(id):** re-enumerate the region's tiles and `store.remove` each (minus
  tiles shared with another region — ref-count or "keep if any other region
  covers it"), drop the metadata row.
- **estimate:** unchanged (`TileMath.estimate`).
- **clearAmbientCache:** clear the non-region browse cache (the ambient tiles the
  map fetched while panning) without touching region tiles — needs a tag/marker
  distinguishing region tiles from ambient (e.g. a per-tile ref set, or a
  separate ambient dir).

## Stages (each builds + commits green)

1. **Tile-store seam** — public `TileStore` + `TURBOMAP_TILE_DIR` in
   `:core:turbomap-android`; point the map controller at it (no behaviour
   change); add `:core:map` → `:core:turbomap-android`.
2. **`TileMath.tilesFor` enumerator** + unit tests (count matches `tileCount`).
3. **`WgpuOfflineTileManager`** — download orchestration into the store +
   progress + pause/resume/retry/network-policy; metadata persistence.
4. **DI swap** — provide `WgpuOfflineTileManager` for `OfflineTileManager`
   (drop the `MapLibre`/`Synthetic` branch); offline feature unchanged behind
   the seam.
5. **Device-verify** offline: download a region, enable airplane mode, confirm
   the wgpu map renders it; delete frees the tiles.
6. **MapLibre removal** (separate plan) — wgpu-only map view + flag/toggle
   removal + delete `TurboMap.kt` + `MapLibreOfflineTileManager` + the gradle dep
   + fix `ArchitectureBoundaryTest`.

## Risks

- **Ambient vs region tiles:** `clearAmbientCache` must not wipe region tiles —
  needs a way to tell them apart (the map writes ambient tiles to the same dir).
  Simplest: region tiles ref-counted in the metadata store; ambient = unref'd.
- **Layer/URL parity:** the downloader must fetch the exact `(layer, z, x, y)`
  keys the map requests (same `MapStyles` templates + layer ids) or the cache
  misses at render. Shared `MapStyles` makes this exact.
- **DEM/vector:** a 3D region needs DEM + vector layers too, not just raster —
  enumerate all layers the map would request for that region.
