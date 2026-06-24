# 03 — Map realism: water, sun/time-of-day, clouds, hillshade

> Bring the web map's photoreal modes to parity: MET-driven realistic water, a movable sun with atmosphere + cast shadows on a time-of-day slider, a scrubable MET-radar cloud overlay, and a hillshade layer.

## Status
- **Android (gold standard):**
  - **Realistic water mode** — the wgpu water surface is driven by MET wave/wind forecast: wave direction + height drive wave ferocity, whitecaps appear when the sea turns extreme, shoreline foam. Implies 3D (water reflects terrain/sky).
  - **Sun mode** — a movable sun + analytic atmosphere + terrain cast shadows, with a **time-of-day slider** that defaults to **now** (real clock); moving it relights the scene and moves shadows.
  - **Cloud overlay** — a procedural cloud/precip overlay fed from **MET radar** rasters, with **play/scrub** controls (animate the radar timeline, crossfade between frames).
  - **Hillshade** — a shaded-relief layer derived from the DEM.
- **Web today:** None of these. `turbomap-web` exposes `set_realistic_water(on)`, `set_sun_time(unix_secs|null)`, and `set_terrain_shadows(strength)` — but the canvas never calls them, and there is no UI.
- **Renderer/back-end prerequisites:**
  - Already exposed in `turbomap-web`: `set_realistic_water`, `set_sun_time`, `set_terrain_shadows`.
  - **Requires exposing in `turbomap-web` (thin wasm-bindgen passthroughs over methods the engine + Android FFI already implement):**
    - `set_water_conditions(wave_from_deg?, wave_height_m?, wind_speed_ms?, wind_from_deg?)` — feed MET marine forecast into the water surface.
    - `enable_clouds(grid_w, grid_h)` / `disable_clouds()` / `set_clouds_visible(visible)` — lifecycle of the cloud overlay.
    - `ingest_radar_frame(slot, grid_w, grid_h, precip: Uint8Array, coverage: Uint8Array)` — upload a radar frame into slot 0 (current) or 1 (next).
    - `set_cloud_time(time, blend)` — animation clock + slot-0→slot-1 crossfade for play/scrub.
    - `set_cloud_geo_bounds(...)` — geo bounds the radar grid maps to (engine-implemented per task brief; confirm exact signature when adding the passthrough).
  - **All four realism modes are 3D / WebGPU-only.** Water and shadows need terrain; the engine treats water/shadows as no-ops without the DEM/3D scene. Doc 02 (3D + DEM) is a hard prerequisite.
  - **Data:** MET (ocean/marine forecast for water; radar/cloud rasters for the overlay). See doc 13 (conditions/weather) for the MET endpoints + proxying.

## User stories

### 1. Toggle realistic water
*As a coastal/fjord user in 3D, I want the sea to render realistically and reflect today's conditions, so that the map conveys how the water actually looks/behaves.*

**Acceptance criteria**
- A water toggle (in the Layers/realism controls) calls `set_realistic_water(true)`; if not already in 3D, entering water mode also enables 3D + terrain (it implies 3D).
- When marine forecast data is available for the viewport, the app calls `set_water_conditions(waveFromDeg, waveHeightM, windSpeedMs, windFromDeg)`; waves orient/scale to the forecast, whitecaps appear in extreme seas, shoreline foam shows.
- With no marine data, `set_water_conditions` is called with `null` fields ⇒ calm water (still rendered realistically, just calm).
- Toggling off calls `set_realistic_water(false)`; water returns to the flat basemap rendering.

**Web-specific notes**
- 3D/WebGPU-only; the toggle is disabled (with a tooltip) if the browser lacks WebGPU. Bearings are degrees the wave/wind comes *from* (matches FFI contract).

### 2. Scrub time-of-day (sun + shadows)
*As a route planner, I want to move the sun across the day, so that I can see where the terrain will be in light or shadow at a given time.*

**Acceptance criteria**
- A time-of-day slider defaults to **now** (current unix time); `set_sun_time(nowUnixSecs)` is called on enable.
- Dragging the slider calls `set_sun_time(unixSecs)` live; sun position, atmosphere, and cast shadows update each frame as it moves.
- A "now"/reset affordance snaps back to the live clock; passing `null` returns to the engine's default fixed sun.
- Cast shadows are enabled via `set_terrain_shadows(strength)` (strength 0..1; 0 = off) and gated to 3D terrain.

**Web-specific notes**
- 3D-only (shadows need terrain). Slider value is local UI; the unix timestamp is computed for the map's location/date. Default-to-now uses the browser clock.

### 3. Toggle clouds + scrub radar time
*As a user checking weather, I want a cloud/precip overlay I can play or scrub through recent radar frames, so that I can see where rain/cloud is and where it's heading.*

**Acceptance criteria**
- Enabling clouds calls `enable_clouds(grid_w, grid_h)`, then the host samples MET radar/cloud rasters for the current viewport, normalizes them to the grid, and uploads frames via `ingest_radar_frame(slot, ...)` (slot 0 = current frame, slot 1 = next).
- `set_cloud_geo_bounds(...)` is set to the viewport's geo bounds so the grid aligns to the map.
- A **play** button animates the timeline (advancing `set_cloud_time(time, blend)` — `time` drives drift/boil, `blend` crossfades slot 0→1); a **scrub** slider drives `blend` directly and can run forward or backward.
- Disabling clouds calls `disable_clouds()` (frees GPU) or `set_clouds_visible(false)` to hide without discarding frames.

**Web-specific notes**
- Requires the new wasm passthroughs (see prerequisites). The host owns radar fetch + normalization to the `grid_w × grid_h` precip/coverage byte planes. 3D/WebGPU-only. Re-sample + re-bound on significant pan/zoom so the grid stays aligned.

### 4. Hillshade layer
*As a user who wants relief without full 3D, I want a hillshade overlay, so that I can read terrain shape on a flat or tilted map.*

**Acceptance criteria**
- A hillshade toggle adds a `hillshade` layer over the basemap, sourced from the DEM (`dem-xyz`), via `apply_scene`.
- Hillshade works in 2D and 3D (it's a draped relief shade, distinct from displaced terrain).
- Toggling off removes the `hillshade` layer; the basemap is unchanged otherwise.

**Web-specific notes**
- `scene.ts` already has a `hillshade` `Layer` variant. Needs the DEM source present; reuses the doc-02 DEM fetch lane. No new wasm passthrough (just `apply_scene`).

## Primary flows (web)

**Sun / time-of-day**
1. User opens realism controls → enables **Sun**. App ensures 3D + DEM, calls `set_terrain_shadows(strength)` and `set_sun_time(now)`. Slider seeds to now.
2. User drags slider → `set_sun_time(unixSecs)` per input; rAF keeps rendering while it moves; shadows + atmosphere relight.
3. Reset → `set_sun_time(now)` (or `null` for fixed sun).

**Water**
1. Enable **Water** → ensure 3D/DEM, `set_realistic_water(true)`.
2. App fetches MET marine forecast for the viewport (doc 13) → `set_water_conditions(...)`. On no data, all-`null` ⇒ calm.

**Clouds**
1. Enable **Clouds** → `enable_clouds(gw, gh)`, `set_cloud_geo_bounds(viewport)`.
2. Host fetches MET radar frames, normalizes to grid → `ingest_radar_frame(0, ...)` and `ingest_radar_frame(1, ...)`.
3. Play → animate `set_cloud_time(time, blend)`; Scrub slider → set `blend`. Pan/zoom → re-bound + re-ingest.
4. Disable → `disable_clouds()`.

**Hillshade**
1. Toggle → add `hillshade` layer to the scene (DEM source present), `apply_scene`. Toggle off → remove layer.

**Edge cases**
- **WebGPU unsupported:** all four are disabled with an explanatory tooltip (3D engine unavailable).
- **MET data error/empty:** water → calm (all-`null`); clouds → overlay enabled but empty / non-blocking toast; sun/hillshade unaffected (they need only DEM).
- **Not in 3D when enabling water/sun:** auto-enable 3D + DEM first (water/sun imply 3D).

## UI / UX on web
- **Where:** a realism section in the Layers sheet/panel (doc 01's rail), with toggles for Water, Sun, Clouds, Hillshade.
- **Time-of-day slider:** a horizontal slider in a bottom bar/sheet when Sun is active (mirrors Android's bottom slider); applies `set_viewport_inset` for its height on mobile.
- **Cloud play/scrub:** a transport row (play/pause + scrub) shown when Clouds is active.
- **Responsive:** touch = bottom sheet sliders; pointer = side panel + popovers. State for which modes are on lives in `uiStore` (new slices: `water`, `sunMode`+`sunTime`, `clouds`+`cloudTime`/`cloudPlaying`, `hillshade`).

## Data & APIs
- **MET marine (water):** wave-from bearing, wave height (m), wind speed (m/s), wind-from bearing for the viewport — via doc 13 / proxied MET. Maps to `set_water_conditions`.
- **MET radar/cloud (clouds):** recent radar frames sampled to a `grid_w × grid_h` precip + coverage byte grid for the viewport — via doc 13 / proxied MET.
- **DEM (sun shadows + hillshade + water reflection):** `GET ${API_BASE}/api/tiles/dem/rgb/{z}/{x}/{y}.png` (doc 02).
- **State:** Zustand realism slices (above). TanStack Query keys for MET marine/radar owned by doc 13; this doc consumes the parsed values.
- Tiles public; MET fetch/proxy auth per doc 13.

## Renderer integration
- **Already exported:** `set_realistic_water`, `set_sun_time`, `set_terrain_shadows`.
- **Must add to `turbomap-web` first (wasm-bindgen passthroughs over existing engine/FFI methods):** `set_water_conditions`, `enable_clouds`, `disable_clouds`, `set_clouds_visible`, `ingest_radar_frame`, `set_cloud_time`, `set_cloud_geo_bounds`. (FFI signatures live in `apps/turbomap/crates/turbomap-ffi/src/lib.rs`; mirror their `Option`/byte-plane shapes — `Option<f32>` → `number | null | undefined`, `Vec<u8>` → `Uint8Array`.)
- **Scene:** `hillshade` layer added/removed via `apply_scene` (DEM source present). All realism modes require the doc-02 3D + `dem-xyz` scene.

## Out of scope (this phase)
- Offline caching of MET marine/radar data.
- Authoring/customizing cloud appearance beyond the radar-driven defaults.
- Per-account persistence of realism toggles (local UI state for now; revisit with doc 19).

## Open questions
- Exact `set_cloud_geo_bounds` signature to expose (lat/lng min/max? center+span?) — confirm against the engine method before adding the passthrough.
- Radar grid resolution (`grid_w × grid_h`) and how many frames to buffer (just slots 0/1, or a longer scrub timeline requiring repeated re-ingest?).
- `set_terrain_shadows` default strength to mirror Android's sun mode.
- Should enabling Water/Sun *force* 3D (auto-toggle) or just gate the control until the user is in 3D? (Brief says they imply 3D — confirm auto-enter is desired.)
- Which MET products feed clouds (precip radar vs cloud-cover) and the marine endpoint specifics — pin in doc 13.
