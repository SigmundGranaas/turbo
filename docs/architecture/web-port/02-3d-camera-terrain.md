# 02 — 3D orbit/tilt camera + DEM terrain

> Give the web map a true 3D mode — tilt and orbit around a pinned point over real displaced terrain — toggled against the existing flat 2D mode.

## Status
- **Android (gold standard):** Two map modes on the wgpu engine, switched by a **rail toggle**:
  - **2D mode** — flat, north-up. One finger pans, pinch zooms, a compass button resets bearing to north.
  - **3D mode** — terrain is displaced from a Terrain-RGB DEM, sky/atmosphere shown. **One finger orbits** (rotate bearing + tilt pitch) **about a pinned location**; **two fingers pan**; pinch zooms. Entering 3D tilts the camera up off nadir; the pinned point is the orbit pivot.
  Mode and camera pose persist across launches.
- **Web today:** Flat 2D only. `TurboMapCanvas` wires one-finger drag → `pan_by_pixels` and wheel → `zoom_around`. `uiStore.threeD` exists but is unused. No DEM source is added; the camera is never pitched.
- **Renderer/back-end prerequisites:**
  - **No new renderer methods needed.** `turbomap-web` already exposes `orbit_around`, `ease_to`, `pan_by_pixels`, `zoom_around`, `set_camera`, `camera_json`, `project`, `unproject`.
  - DEM via a `dem-xyz` source + `ingest_terrain_tile`. The DEM template already exists in `templates.ts` (`terrain.__terrain` → `${API_BASE}/api/tiles/dem/rgb/{z}/{x}/{y}.png`, encoding `mapbox-rgb`). `pending_tiles()` reports `kind:"terrain"`, `layer:"__terrain"`.
  - The `TileLoader` already understands the `terrain` kind path; the scene just needs the `dem-xyz` source wired and a terrain enable in the engine path (driven purely by including the DEM source in the applied scene).
  - **Endpoints used:** DEM `GET ${API_BASE}/api/tiles/dem/rgb/{z}/{x}/{y}.png`.

## User stories

### 1. Enter 3D mode
*As a hiker planning a route, I want to switch the map into 3D, so that I can see the lay of the terrain — ridges, valleys, steepness — instead of a flat top-down map.*

**Acceptance criteria**
- A 2D↔3D toggle on the map rail flips `uiStore.threeD`.
- Entering 3D: the scene is rebuilt to include the `dem-xyz` terrain source, the camera eases to a tilted pose (non-zero pitch, e.g. ~50°) about the current screen center via `ease_to`, and terrain begins displacing as DEM tiles arrive.
- Leaving 3D: the camera eases back to nadir (pitch 0, north-up), and the DEM source may be dropped from the scene (or kept but flattened).
- The toggle reflects state instantly; the camera transition is a smooth animation, not a jump.

**Web-specific notes**
- WebGPU is required for 3D (it is for the whole renderer). No extra gating beyond the app-level unsupported-browser notice.
- DEM tiles are an extra fetch lane; on slow connections terrain pops in progressively. Flat basemap stays visible meanwhile.

### 2. Orbit the camera (1-finger / drag)
*As a user inspecting a peak, I want to rotate and tilt the view around a fixed point by dragging, so that I can look at terrain from any angle.*

**Acceptance criteria**
- In 3D, a one-finger drag (or left-button pointer drag on desktop) calls `orbit_around(dBearing, dPitch, fx, fy)`, pivoting about the focus pixel (drag start, or screen center).
- Horizontal drag changes bearing; vertical drag changes pitch. Pitch is clamped to the engine's safe range (no flip past top, no going below the horizon into the "grey").
- The pivoted ground point stays under the cursor/finger during the orbit (focus-invariant), matching Android feel.
- In **2D**, the same one-finger drag instead pans (`pan_by_pixels`) — the gesture is mode-dependent.

**Web-specific notes**
- Desktop adds a modifier path: right-drag or `Ctrl`/`Alt`+drag can orbit even in 2D as a power-user shortcut (optional), but the primary contract is mode-driven.
- Trackpad two-finger scroll should remain wheel-zoom, not orbit, to avoid clashing with native scroll.

### 3. Pan in 3D (2-finger / dedicated)
*As a user, I want to move the map laterally while in 3D without losing my tilt, so that I can follow a ridgeline or valley.*

**Acceptance criteria**
- In 3D, a two-finger drag (touch) pans via `pan_by_pixels`, preserving pitch/bearing.
- On desktop, panning in 3D uses the right-button drag *or* a two-finger trackpad pan gesture; bearing/pitch are unchanged.
- Panning under pitch uses the engine's ground-plane unproject so the world point under the fingers tracks correctly (the `pan_by_pixels` math is shared with Android).

**Web-specific notes**
- Browsers don't give a clean "two-finger drag" event distinct from pinch; track active touch points: 2 touches with low scale-change = pan, with scale-change = zoom. Pointer-count gating lives in the canvas gesture layer.

### 4. Tilt control
*As a user, I want to control how steeply the camera looks across the terrain, so that I can go from near-top-down to near-horizon.*

**Acceptance criteria**
- Pitch is adjustable via the orbit drag's vertical component (and optionally a tilt button/slider on the rail).
- Pitch is clamped so the camera never dips below the terrain (no grey/under-terrain frame — the engine already clamps the eye above the center→eye terrain segment).
- High-pitch aerial haze is acceptable but must not white out the whole frame.

### 5. Return to 2D / north-up
*As a user who got disoriented, I want a one-tap reset, so that I get back to a flat, north-up map.*

**Acceptance criteria**
- A compass/north button (visible when bearing ≠ 0 or pitch ≠ 0) eases the camera to bearing 0, pitch 0 via `ease_to`.
- Tapping the 3D→2D toggle does the same and flattens terrain.
- The reset keeps the current center/zoom.

### 6. Terrain appears (DEM)
*As a user in 3D, I want the ground to actually rise and fall, so that the 3D view is meaningful, not a tilted flat plane.*

**Acceptance criteria**
- With the `dem-xyz` source in the scene, `pending_tiles()` reports `terrain` tiles; the host fetches DEM PNGs and `ingest_terrain_tile`s them; visible terrain displaces.
- Terrain LOD follows the camera (nearer terrain higher-res) without blanking distant terrain on tilt.
- If DEM tiles fail to load, the map shows flat terrain for that area rather than erroring; the basemap drapes regardless.

### 7. Camera persistence
*As a returning user, I want the map to reopen where and how I left it (location, zoom, and mode), so that I continue where I stopped.*

**Acceptance criteria**
- Camera pose (`camera_json`: lat/lng/zoom/pitch/bearing) and `threeD` are persisted (debounced) and restored on next load.
- Restore happens before/at the first frame so the user doesn't see a 2D→3D reflow.
- Restored pose passes the same finite/clamp sanitization the engine applies (no NaN/out-of-range pose).

**Web-specific notes**
- Persist to `localStorage` (per-device), debounced (~500ms after camera settles). Account-synced camera is out of scope (Android persists locally too).

## Primary flows (web)

**Enter 3D and look around**
1. User taps **3D** on the rail → `setThreeD(true)`.
2. `TurboMapCanvas` rebuilds the scene to include the `dem-xyz` source and calls `apply_scene`, then `ease_to(currentLat, currentLng, currentZoom, currentBearing, ~600ms)` with a target pitch (set via `set_camera` after, or a tilted `ease_to` variant).
3. rAF loop drains `pending_tiles()`; terrain + basemap tiles fetched and ingested. Terrain rises as DEM arrives.
4. User one-finger drags → `orbit_around`; two-finger drags → `pan_by_pixels`; wheel/pinch → `zoom_around`.
5. Camera + mode persisted on settle.

**Return to 2D**
1. User taps the compass (or 2D toggle).
2. `ease_to(..., bearing 0, ...)` with target pitch 0; on arrival the DEM source can be removed and `threeD=false` set.

**Edge — WebGPU unsupported**
- Handled at app boot (whole renderer unavailable) → unsupported-browser notice; 3D toggle is moot.

**Edge — DEM endpoint error**
- Flat fallback for affected tiles; non-blocking. No modal.

**Edge — orbit into terrain**
- Engine pitch/eye clamp prevents under-terrain grey; gesture layer additionally clamps pitch to a max (e.g. ~80°) to avoid the aerial-haze whiteout.

## UI / UX on web
- **Where:** 2D/3D toggle + compass on the map's right rail (mirrors Android). Optional tilt affordance on the rail.
- **Gesture map (canvas layer):**
  - 2D: 1-finger/left-drag = pan; wheel/pinch = zoom.
  - 3D: 1-finger/left-drag = orbit; 2-finger/right-drag = pan; wheel/pinch = zoom.
- **Responsive:** touch uses finger-count gating; pointer uses button + modifier gating. The mode (`uiStore.threeD`) is the switch for which gesture maps to which engine call.
- **Compass** appears only when bearing/pitch ≠ 0; tapping resets via `ease_to`.

## Data & APIs
- **Tiles (public):** DEM `GET ${API_BASE}/api/tiles/dem/rgb/{z}/{x}/{y}.png` (Terrain-RGB / `mapbox-rgb` encoding). Basemap drape per doc 01.
- **State:** Zustand `uiStore.threeD` / `setThreeD` (exists). Camera pose + mode persistence to `localStorage`.
- No auth required (tiles are public).

## Renderer integration
- **Scene sources:** add `dem-xyz` (`tiles: [demTemplate]`, `encoding: 'mapbox-rgb'`, with `halo`) when 3D is on. `scene.ts`'s `SourceDef` already has the `dem-xyz` variant; wire it into `buildBaseScene`/scene composition for 3D.
- **turbomap-web methods called:** `apply_scene`, `set_camera` / `ease_to` (enter/exit + reset), `orbit_around` (orbit), `pan_by_pixels` (pan), `zoom_around` (zoom), `camera_json` (persist), `project`/`unproject` (focus math). **No new wasm passthrough required** — all already exported.
- **Tile servicing:** `pending_tiles()` → fetch `terrain` kind → `ingest_terrain_tile`.

## Out of scope (this phase)
- Offline DEM pre-download.
- Realistic water, sun/time-of-day lighting, cast shadows, clouds — see doc 03.
- Account-synced camera/mode (local-only persistence here).

## Open questions
- Default entry pitch for 3D (Android value to mirror?) and the pitch clamp ceiling (engine clamps under-terrain; what's the haze-safe max — ~80°?).
- Does `ease_to` accept a target pitch, or must we follow with `set_camera` to set pitch? (`ease_to` signature is lat/lng/zoom/bearing/duration — confirm how pitch is animated on web.)
- DEM zoom range + `halo` value to match Android's terrain config.
- Should exiting 3D drop the `dem-xyz` source (free GPU) or keep it flattened for instant re-entry?
